# ADR 0002 — Modèle d'I/O : plages d'octets, `Read + Seek`, `mmap`, `object_store`, async

- Statut : proposée (2026-10-30)
- Spécification liée : `10` §1, §3, §6 ; `15` §2.1 ; `20` §4.1 (écriture, compteurs réécrits après coup)

## Contexte

Faits du format utiles à l'I/O :

1. La page `k` est à `HL + k × P` (`10` §1, §6) : une plage de pages est une plage d'octets connue dès l'en-tête.
2. Le schéma ne demande que l'en-tête et les pages de métadonnées (page 0 pour la plupart des tables ; 5 pages pour 5 000 colonnes). La fin des métadonnées se découvre en lisant les pages dans l'ordre (`10` §6 étape 2).
3. Le nombre de lignes par page n'est connu qu'en lisant l'en-tête de page (40 octets, `block_count`). Pour une table non compressée écrite par SAS, les pages de données intermédiaires sont pleines (168 et 337 lignes pour `P` = 4 096 et 8 192 dans le corpus, formule `r × RL + ⌈r/8⌉ + 40 ≤ P`, `20` §4.1) mais ce n'est pas un invariant du format.
4. Les lignes compressées sont décompressées une par une, sans état partagé (`15` §2.1) : une plage de pages se décode seule.
5. L'écriture connaît `row_count`, `page_count` et la capacité de la page 0 seulement à la fin ; `RowSize` et l'en-tête sont réécrits après coup (`20` §4.1 point 7, §6.2).
6. Consommateurs : DataFusion (async, objets distants), Python/R (synchrone, sans runtime), Flight (async).

## Options étudiées

| Option | Pour | Contre |
|---|---|---|
| A. `Read + Seek` dans le cœur | simple, familier | lecture séquentielle ; pas d'accès par plages sur objet distant ; mélange cœur et I/O |
| B. Tampons d'octets dans le cœur + trait `ByteSource` (plage `(offset, len)`) dans `uniformat-sas-io` (retenue) | une seule abstraction pour tranche mémoire, fichier (`pread`), `mmap`, S3/HTTP ; parallélisme par plages naturel ; cœur testable sans disque | l'appelant gère le prefetch ; trait à stabiliser |
| C. Cœur async (`AsyncRead`/`Stream`) | intégration directe avec DataFusion | impose un runtime à Python et R ; fuzzing plus difficile ; coloration async dans tout le parseur |
| D. `mmap` seulement | zéro copie, très rapide en local | inutilisable sur objet distant ; `unsafe` (fichier modifié pendant la lecture) ; fichiers > espace d'adressage 32 bits |

## Décision

**Option B.**

- Le cœur ne lit rien : il reçoit `&[u8]` (en-tête, page, en-tête de page).
- `uniformat-sas-io` définit `trait ByteSource { fn len(&self) -> u64; fn read_range(&self, offset: u64, len: usize) -> io::Result<Bytes>; }`, **synchrone**, avec : `&[u8]`/`Bytes`, `File` (lecture positionnée, sans `Seek` partagé donc thread-safe), `MmapSource` (feature `mmap`, seul endroit où `unsafe` est admis, ADR 0007), et un adaptateur `Read + Seek` (verrou interne) pour les flux existants.
- `uniformat-sas-arrow` fournit en feature `object-store` un adaptateur **async** : les plages de pages sont récupérées par `object_store::get_range(s)` puis passées au cœur ; le décodage CPU est exécuté sur le pool bloquant du runtime. **L'async n'entre ni dans le cœur ni dans `uniformat-sas-io`**. Python et R utilisent la voie synchrone ; une URL `s3://` ou `https://` dans Python passe par l'adaptateur async piloté par un runtime privé de `uniformat-sas-py`.
- **Plan de pages** (`PagePlan`) : `open` lit l'en-tête et les pages de métadonnées ; `page_plan()` obtient `PageInfo` de chaque page en lisant 40 octets par page. Chemin rapide : après la dernière page de métadonnées/mixte, les pages de données sont supposées pleines (`rmax` lignes) et **vérifiées à la lecture** (si `block_count` diffère, retour au balayage complet des en-têtes). Sur objet distant, le balayage complet est une requête de 40 octets par page, regroupées par lots de 256 pages ; le plan est mis en cache. Un fichier compressé n'a pas de formule : balayage complet systématique (le nombre de pointeurs de chaque page est dans son en-tête).
- **Lecture par plages** : `read_pages(Range<u64>)` ; unité de parallélisme et de partage = groupe de pages contigu (par défaut ~8 Mio, au moins une page) ; ordre des lignes = ordre des pages.
- **Écriture** : `SasWriter<W: Write + Seek>` ; les pages complètes sont écrites au fil de l'eau, l'en-tête et la page 0 sont réécrits à `finish`. Pour un sink sans `Seek` (réseau), `write_to_vec`/fichier temporaire puis envoi : documenté, pas de tampon caché.
- **Bornes** : toute taille lue dans le fichier est comparée à `Limits` et à `ByteSource::len()` avant allocation (`page_length`, `page_count`, `column_count`, longueurs de texte).

## Conséquences

- (+) Une même logique pour fichier local, `mmap`, S3 et Flight ; DataFusion et Flight partitionnent par `Range<u64>` de pages sans nouveau format.
- (+) Le cœur reste testable et fuzzable sur `&[u8]`.
- (−) Le chemin rapide du plan de pages suppose des pages pleines : si un écrivain tiers (ou SAS) laisse des pages partiellement remplies, il faut le balayage complet. Test dédié : fichiers du corpus avec lignes supprimées et `taille_buf*`.
- (−) L'écriture avec réécriture exige `Seek` ; c'est un choix assumé (le format place les totaux au début, `20` §4.1).
- (−) Deux surfaces de lecture (sync et async) à maintenir en cohérence ; elles partagent `PageDecoder` et `PagePlan`.
- Questions ouvertes : taille optimale d'un groupe de pages sur S3 (à mesurer au jalon 4) ; support de `Range` HTTP sans `Content-Length`.

## Mise en œuvre (J4, 2026-11-04)

- `read_pages` rend les octets d'une plage de pages en **une seule lecture** (`Cow<[u8]>`), et non un itérateur de pages : l'unité de requête sur objet distant est le groupe de pages. `page_plan(PlanMode)` : `Fast` ne lit rien (la première page de données, déjà vue à l'ouverture, donne `rmax`), `Full` lit l'en-tête de chaque page restante.
- Le chemin rapide est **vérifié à chaque page lue** (`PagePlan::check_page`) ; en cas de contradiction, `IoError::PlanMismatch` demande le plan complet. Le repli automatique de l'ADR n'est pas réalisé (il n'est pas sûr pour des partitions lues indépendamment) ; aucun des 180 fichiers non compressés du corpus ne contredit le plan rapide.
- `SasFile::open` lit l'en-tête de chaque page (40 octets) avant d'en lire le corps : seules les pages de métadonnées sont lues en entier. Une table compressée a un plan séquentiel (une partition).
- Adaptateur `object_store` (feature `object-store` de `uniformat-sas-arrow`) : `head`, lecture d'un préfixe de 1 Mio (doublé si les métadonnées le demandent), puis une requête par groupe de pages ; décodage sur le pool bloquant de tokio. Requêtes mesurées sur un serveur HTTP de test : `2 + groupes`.
- Question ouverte « taille optimale d'un groupe sur S3 » : **non mesurée** (`group_bytes` vaut 8 Mio par défaut, sans mesure sur S3).
