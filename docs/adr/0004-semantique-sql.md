# ADR 0004 — Sémantique SQL du driver (DataFusion)

- Statut : proposée (2026-10-30)
- Spécification liée : `10` §3.3, §3.5, §5.3, §5.4 ; `15` §2.1 ; `30` §1 ; `20` §4.1 (formule de pages)

> **Ces règles sont des choix de conception du driver**, pas des résultats de reverse engineering : la spécification ne dit pas comment SAS compare ou trie dans une requête. Quand le comportement de SAS est connu, il sert de motivation, jamais de preuve.

## Contexte

SAS (`PROC SQL`, `PROC SORT`) traite le blanc final comme du remplissage (`'abc'` et `'abc  '` sont égaux), trie les manquants avant toute valeur (`.` < `._` < `.A` < … < `.Z` < nombres) et ne voit pas les lignes supprimées. DataFusion/Arrow ont d'autres conventions : comparaison de chaînes exacte, `NULL` à trois valeurs, tri `NULLS LAST` en ascendant par défaut. Le format ne contient aucune statistique de colonne (pas de min/max, pas de nombre de nuls). Il contient en revanche `row_count` et `deleted_count` (`RowSize`) et la position exacte de chaque page.

## Décision

### 1. Comparaison de chaînes avec espaces finaux

- Le comportement par défaut du driver est `TrailingBlanks::Trim` (ADR 0003) : la colonne Arrow ne contient **jamais** de blanc final. `'abc' = col` est donc vrai pour une valeur stockée `'abc  '`, comme dans SAS, avec la sémantique exacte de DataFusion (aucune règle spéciale).
- Avec `TrailingBlanks::Keep`, les valeurs conservent leurs blancs (`'abc  '` ≠ `'abc'`, comparaison exacte de DataFusion). Le `TableProvider` n'ajoute pas de règle de comparaison blanche : le SQL se comporte comme sur n'importe quelle table Arrow. La cohérence entre les deux modes est la suivante : `Trim` ≡ `Keep` + `rtrim(col)` à l'exception des blancs de tête, toujours conservés.
- Les chaînes **vides** : SAS stocke un blanc (`10` §5.4) ; le driver rend `''` (jamais `NULL`) car SAS ne distingue pas vide et manquant caractère. Un option `empty_as_null` (défaut faux) donne `NULL` pour les valeurs vides.

### 2. Ordre des manquants

- Un manquant numérique est `NULL` (logique à trois valeurs) ; `x = x` est inconnu comme sur toute table Arrow. Le tri suit DataFusion : `ASC NULLS LAST` par défaut (différence assumée avec SAS, qui place les manquants en premier).
- Pour retrouver l'ordre SAS, deux moyens documentés : `ORDER BY x NULLS FIRST` (tous les manquants avant les nombres) ; et avec `MissingMode::Sidecar` (ADR 0003) `ORDER BY CASE WHEN x__missing = 0 THEN 1 ELSE 0 END, x__missing, x` (`.` avant `._` avant `.A`…). Le driver n'implémente **pas** de collation SAS dans le moteur.
- Les agrégats (`SUM`, `AVG`, `COUNT(x)`) ignorent les `NULL` comme partout dans DataFusion ; `COUNT(*)` compte les lignes visibles.

### 3. Lignes supprimées invisibles

Toute ligne dont le bit de suppression est à 1 (page de type `& 0x0080`, `10` §3.5) est absente de tous les résultats, de `COUNT(*)`, des statistiques et des agrégats. `row_count` de `RowSize` compte les lignes **physiques** ; le nombre de lignes visibles est `row_count − deleted_count` (accord avec « Observations 90, Deleted Observations 10 » de `PROC CONTENTS` sur le fichier de test). Les cas de bitmap non observés (pages de données, 32 bits, plusieurs pages ; U19) lisent le bitmap selon la règle générale de `10` §3.5 avec l'avertissement `U19` (ADR 0006) ; en mode `Strict`, une page à bitmap non observé est une erreur.

### 4. Projection, `LIMIT`, parallélisme, statistiques exposées à DataFusion

- **Projection poussée** : `scan(projection)` ne décode que les colonnes demandées. Les colonnes occupent des plages d'octets connues dans la ligne (`offset`, `width`, `10` §5.1) : le décodage d'une ligne ne touche pas les autres. Les pages sont lues entièrement (le format est orienté ligne) ; la décompression traite la ligne entière (RLE/RDC sont par ligne, `15` §2.1) mais le décodage des valeurs n'est fait que pour la projection.
- **`LIMIT` poussé** : `scan(limit)` arrête la lecture après `limit` lignes visibles : pages lues dans l'ordre, arrêt au lot qui atteint la limite. En présence de plusieurs partitions, chaque partition reçoit la limite et DataFusion applique le `LIMIT` global.
- **Filtres** : `supports_filters_pushdown` renvoie `Unsupported` pour tous (le format n'a ni index exploitable ni statistiques de pages ; les index SAS ne sont pas décrits, `10` §3.4). DataFusion filtre après le scan.
- **Lecture parallèle par plages de pages** : `target_partitions` partitions = plages disjointes et contiguës de pages `[p_i, p_{i+1})` calculées avec `PagePlan` (ADR 0002) pour équilibrer en nombre de lignes physiques. Chaque partition décode ses pages indépendamment (aucun état partagé entre lignes, aucun état partagé entre pages). L'ordre des lignes d'un résultat sans `ORDER BY` n'est pas garanti ; chaque partition est ordonnée par page. La page 0 contient les métadonnées et peut contenir des lignes (page mixte) : la première partition la contient toujours.
- **Statistiques** (`ExecutionPlan::statistics`) : `num_rows` = `row_count − deleted_count`, `Precision::Exact` si `deleted_count = 0` ou si le fichier ne présente que le cas de bitmap observé (une page mixte LE64) ; `Precision::Inexact` dans les autres cas de lignes supprimées. `total_byte_size` = `page_count × P` (exact). Colonnes : `min`, `max`, `null_count`, `distinct_count` = `Absent` (le format n'a pas de statistiques). Un `COUNT(*)` sans filtre est répondu depuis les statistiques sans lire de ligne quand `Exact`.
- **Schéma** : `TableProvider::schema` est calculé depuis les métadonnées seules (ADR 0003).

### 5. Types et fonctions

Aucun type SQL propre à SAS n'est créé. Les formats (`DATE9.`) sont des métadonnées, pas des fonctions d'affichage (affichage hors périmètre v1, `docs/architecture.md` §5.4). Les colonnes de format utilisateur gardent leur type de stockage.

## Options étudiées

| Sujet | Options | Choix |
|---|---|---|
| blancs finaux | (a) comparaison SAS (rtrim implicite) implantée par une règle du moteur ; (b) retrait à la lecture (retenue) ; (c) conservation brute | (b) par défaut, (c) en option ; (a) écartée : demande un opérateur de comparaison spécial dans DataFusion |
| ordre des manquants | (a) collation SAS ; (b) `NULL` standard (retenue) + colonne annexe | (b) : pas de nouvelle collation ; l'ordre SAS s'obtient par `ORDER BY` documenté |
| lignes supprimées | (a) exposer avec colonne `_deleted` ; (b) invisibles (retenue) | (b) ; un mode `include_deleted` pour diagnostic est réservé à la CLI |
| statistiques | (a) tout `Absent` ; (b) `num_rows` seulement (retenue) | (b) |

## Conséquences

- (+) Comportement prévisible et standard dans DataFusion ; la différence avec SAS (tri des manquants) est documentée, avec la requête équivalente.
- (+) `COUNT(*)` en O(1) quand le fichier est « simple ».
- (−) Pas de filtrage au niveau des pages : une requête sélective lit tout le fichier (aucune statistique de page n'existe dans le format).
- (−) Le comportement de `LIMIT` sans `ORDER BY` n'est pas déterministe en parallèle.
- Questions ouvertes : exposer un index de pages par position de ligne pour accélérer `OFFSET` (jalon 4) ; surcharge d'une règle d'optimisation pour `rtrim` sous `Keep`.

## Mise en œuvre (J4, 2026-11-04)

- Projection et `LIMIT` poussés jusqu'au décodage ; filtres non poussés. Partitions = groupes de pages du plan, équilibrés en lignes physiques ; une table compressée n'a qu'une partition.
- `num_rows` : `Exact` si `deleted_count = 0`, sinon `Inexact` (la clause « cas de bitmap observé » n'est pas appliquée, faute de preuve par fichier avec le plan rapide). `total_byte_size` = `page_count × P` (exact). Statistiques de colonnes : absentes.
- `COUNT(*)` sans filtre sur une table sans suppression est répondu par DataFusion depuis les statistiques : aucune page de données n'est demandée (vérifié sur un serveur HTTP de test).
