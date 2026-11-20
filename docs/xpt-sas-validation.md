# Validation XPORT v5 sur un corpus produit par SAS

Validation effectuée le **19 novembre 2026** sur le corpus externe fourni pour
l'issue #1. L'archive reste hors du dépôt ; seul ce compte rendu, le validateur et
un vecteur de régression synthétique sont versionnés.

## Provenance et intégrité

- Archive : `sas-xpt-v5-reference.zip`, 199 948 octets.
- SHA-256 : `40771566d435e1ed3a40f50322d89e48f0a19e7daa1228d35148ae939dbbfd8b`.
- 208 fichiers du manifeste vérifiés, aucune empreinte divergente.
- Producteur documenté par les programmes et journaux livrés : SAS 9.4 TS1M7
  (`9.04.01M7P08062020`), moteur XPORT, Linux x86-64 / RHEL 8.10.
- Sessions UTF-8, LATIN1 et WLATIN1. Les programmes de l'archive ont été examinés
  comme preuves de génération ; aucun script livré n'a été exécuté localement.

Il s'agit d'une comparaison avec des fichiers et références produits par SAS,
non d'une nouvelle exécution locale de SAS complet. Le runtime minimal local
ne possède toujours pas le moteur XPORT.

## Résultats

| Contrôle | Résultat |
|---|---|
| Bibliothèques / membres | 14 fichiers, 15 membres, ordre conservé |
| Schémas | 46 colonnes : noms, types, largeurs stockées, labels, formats et informats avec largeur/décimales |
| Observations | 116 lignes identiques à la relecture SAS |
| Numériques présents | 194 valeurs comparées bit à bit aux références `HEX16.` |
| Manquants | 68 cellules, couverture des 28 codes SAS et du code « présent » |
| Caractères | 88 cellules comparées au texte CSV et aux octets hexadécimaux, blancs compris |
| Dates/heures natives | 36 cellules vérifiées en jours et microsecondes ; option `time=duration` pour les heures négatives ou ≥ 24 h |
| Réécriture | 15 allers-retours XPT → Arrow → XPT identiques, données et métadonnées Arrow |
| Cas vides et multi-membres | Validés, notamment ZETA puis ALPHA dans la même bibliothèque |

Les vérifications locales associées passent : **114 tests Rust** (30 optionnels
ignorés), **20 tests Python** (13 optionnels ignorés, corpus XPT SAS activé),
Clippy sur le workspace et toutes les cibles, formatage, frontières de dépendances
et tables générées.

Les références `source/` sont les tables WORK avant transport ; `reread/` est
la relecture du fichier par SAS. Les deux sont comparées, et leurs différences
sont conservées dans le rapport. Elles ne doivent pas être attribuées au lecteur :

- SAS ne restitue pas la dernière ligne entièrement blanche des deux tables
  caractère de longueur 12 : 5 → 4 et 20 → 19 lignes, même sans bourrage dans
  le second cas. Notre politique de fin de fichier donne les mêmes comptes.
- Le cas volontairement hors limites v5 est déjà tronqué par l'étape DATA SAS :
  caractères 201 → 200 octets, labels 41 → 40 octets, nom mixte mis en majuscules.
  Notre écrivain continue de refuser ces pertes avant le commit.
- SAS sature les valeurs numériques hors plage IBM aux bornes, et normalise le
  zéro négatif. Notre écrivain continue de refuser les valeurs hors plage.
- `PROC CONTENTS` à la relecture SAS rapporte certaines longueurs numériques
  courtes avec un octet supplémentaire et des positions de tampon différentes.
  Le lecteur expose les **largeurs réellement stockées dans les NAMESTR**, vérifiées
  contre `checks/xpt_namestr.csv`, et les formats/labels de référence.

## Correction issue de la validation

Quatre valeurs du fichier de bornes numériques contiennent le maximum IBM sur
56 bits de mantisse. L'arrondi IEEE au plus proche produisait `2^252`, hors de la
plage réinscriptible dans XPORT. SAS restitue le f64 immédiatement inférieur
(`4FAFFFFFFFFFFFFF`, signe conservé).

Le décodeur applique maintenant cette borne supérieure. Les autres arrondis,
y compris les égalités au pair à l'intérieur de la plage, sont inchangés.
La réécriture des quatre valeurs réussit désormais. Un test Rust obligatoire
couvre les deux signes, une largeur courte et un arrondi intérieur.

## Particularité de la session SAS LATIN1

Le fichier produit dans cette session contient `0x80` pour `€`, et son CSV SAS
transcode cet octet en euro. Ce comportement observé n'est pas ISO-8859-1.
La comparaison au texte de référence utilise donc **explicitement Windows-1252**
pour ce fichier comme pour WLATIN1. Un contrôle séparé vérifie que l'option
`latin1` produit U+0080 et que `windows-1252` produit U+20AC pour le même octet.
UTF-8 strict reste le défaut ; aucune détection automatique ni substitution des
encodages validés par l'utilisateur n'a été introduite.

## Rejouer

Après extraction de l'archive dans un répertoire externe et installation du
binding Python de cette branche :

```sh
python tests/data/xpt/validate_sas_reference.py /chemin/sas-xpt-v5-reference
UNIFORMAT_XPT_SAS_CORPUS=/chemin/sas-xpt-v5-reference \
  python -m pytest bindings/python/tests/test_xpt_sas_reference.py -q
```

Le validateur vérifie le manifeste, les schémas, les cellules, les conversions
natives et les allers-retours, puis produit un rapport JSON. Le test est ignoré
si la variable de corpus est absente ; un chemin fourni mais invalide échoue.
Les données SAS ne sont pas nécessaires au test synthétique de la borne IBM.

Cette preuve couvre une version SAS sur une plateforme, et la lecture de ses
XPT v5. Les fichiers réécrits sont validés par le driver et les tests indépendants
R/haven existants ; ils n'ont pas été soumis à une nouvelle exécution SAS.
XPORT v8 reste un jalon distinct, non couvert par ce corpus.
