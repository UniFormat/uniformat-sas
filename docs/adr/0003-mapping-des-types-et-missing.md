# ADR 0003 — Mapping des types Arrow et conservation des missing spéciaux

- Statut : proposée (2026-10-30)
- Spécification liée : `30` §1 (table de classification), §6 (encodages), §6.2 (octets invalides) ; `10` §5.2 à §5.4 ; `20` §6.3 à §6.4

## Contexte

La spécification établit (`30` §1, `inféré`) que le **format ne change jamais le type de stockage** : une colonne numérique est un double IEEE (ou ses `n` octets de poids fort, 3 à 8), une colonne caractère est une chaîne d'octets. Le format donne le sens : date (jours depuis 1960-01-01, soit `Date32 = valeur − 3 653`), datetime (secondes depuis 1960-01-01T00:00:00, soit `valeur − 315 619 200` secondes Unix), heure (secondes depuis minuit, éventuellement négative ou ≥ 24 h : repli `Duration`), ou nombre ordinaire. Un nom absent de la table est un **format utilisateur** (catalogue `.sas7bcat`, non décodé) : la colonne reste Float64 ou Utf8. Un format `w.d` fixe une échelle d'affichage, pas un stockage décimal : Decimal est une option. Les valeurs manquantes (`.`, `._`, `.A`–`.Z`) sont des NaN portant un code (`10` §5.3, famille Unix little endian vérifiée) ; tout autre NaN est lu comme `.`. Les chaînes sont dans l'encodage de la table (octet 70), complétées par des blancs ; des octets invalides sont conservés tels quels par SAS (`30` §6.2) ; l'octet 70 n'est pas toujours fiable (`10` U22, F10) ; la plage de dates valides de SAS est 1582-01-01 à l'an 20 000 (`30` §0/§1, F05).

## Décision 1 — Mapping par défaut

| Classification (`30` §1) | Type Arrow par défaut | Conversion | Options |
|---|---|---|---|
| numérique ordinaire (`BEST`, `COMMA`, `DOLLAR`, `E`, `PERCENT`, `Z`, sans format, …) | `Float64` | valeur du double ; octets de poids faible complétés par des zéros pour les largeurs 3 à 7 | `Decimal128(38, d)` si `ReadOptions::decimal_for_fixed` et si le format porte `.d` (arrondi décimal de l'affichage ; jamais par défaut) |
| entier affiché (`F`, `Z`, `COMMAw.0`…) | `Float64` | idem | `Int64` seulement via `ReadOptions::integers_when_exact` (vérification par colonne impossible en streaming : option désactivée par défaut) |
| date (`DATE`, `YYMMDD`, `DDMMYY`, `MMDDYY`, `WEEKDATE`, `MONYY`, …) | `Date32` | `floor(valeur) − 3 653` jours ; partie fractionnaire ignorée (**choix du driver**, la spécification ne tranche pas) | `DateMode::AsFloat` (garde Float64 brut) |
| datetime (`DATETIME`, `DATEAMPM`, …) | `Timestamp(Microsecond, sans fuseau)` | `valeur − 315 619 200` secondes Unix, arrondi à la microseconde (SAS stocke des secondes éventuellement fractionnaires) | `TimestampUnit::Second` (la table de `30` §1 indique la seconde) |
| heure (`TIME`, `HHMM`, `TOD`…) | `Time64(Microsecond)` | secondes depuis minuit | valeurs < 0 ou ≥ 86 400 s : règle `OutOfRange` ; `TimeMode::Duration` donne `Duration(Microsecond)` (repli de `30` §1) |
| caractère | `Utf8` | décodage de l'encodage de la table (ci-dessous) | `LargeUtf8` si un lot dépasse 2 Gio ; `Binary` par colonne ou globalement (`bytes_as_binary`) |
| format ou informat inconnu / utilisateur | type de stockage de la colonne | aucune interprétation | le nom de format reste dans les métadonnées |
| format NLS / localisé (`30` §2.1) | type de stockage | aucune interprétation (aucune spécification) | idem |

### Options étudiées pour le mapping

| Sujet | Options | Choix |
|---|---|---|
| numérique avec format `w.d` | `Float64` (retenue) ; `Decimal128(38, d)` | `Float64` : le stockage SAS est un double, `d` n'est que de l'affichage (`30` §1) ; `Decimal` en option |
| datetime | `Timestamp(Second)` (table de `30`) ; `Timestamp(Microsecond)` (retenue) | microseconde : conserve les secondes fractionnaires ; seconde en option |
| heure hors 0…86 400 s | `Time64` + règle `OutOfRange` (retenue) ; `Duration` partout | `Time64` par défaut (type attendu) ; `Duration` en option car `Time64` ne représente pas les négatives |
| chaîne à octets invalides | `Replace` (retenue) ; `Error` ; `Binary` | `Replace` compté ; `Binary` pour ne rien perdre |

**`OutOfRange`** (date hors 1582-01-01…20 000, heure hors 0…86 400 s, datetime non représentable en microsecondes `i64`) : `Null` (défaut, une valeur nulle plus un compteur d'avertissement `out_of_range` dans `ReadReport`), `Error`, ou `Raw` (valeur laissée telle quelle impossible en streaming typé : réservé à `DateMode::AsFloat`). Aucune valeur n'est modifiée silencieusement : le compteur est toujours rempli.

**Blancs finaux** (`TrailingBlanks`) : `Trim` (défaut : retire les `0x20` finaux — comportement de SAS à la lecture, `10` §5.4 — et un champ entièrement nul est lu vide), `Keep` (octets conservés, la longueur de colonne est la largeur SAS). Les blancs de tête sont toujours conservés. Un champ partiellement rempli terminé par `00` n'est pas observé (`10` §5.4) : les octets nuls ne sont pas retirés (avertissement `U-trailing-nul` dans `ReadReport`).

**Encodage et octets invalides** : on décode avec `encoding_rs` d'après l'octet 70 (table `30` §6.1, 127 codes ; 29 → `windows-1252`, 28 → `windows-1252` avec avertissement de divergence C1, F10 pour les désaccords) ; `ReadOptions::encoding_override` impose un autre encodage (nécessaire pour `datareader/test16` et `test19`, `10` U22). Code 0 ou 204 : détection (UTF-8 valide sinon `windows-1252`) avec avertissement (F12). Octets invalides pour l'encodage (`InvalidUtf8`) : `Replace` (défaut : U+FFFD et compteur `invalid_bytes`), `Error`, ou `Binary` (la colonne entière devient `Binary`, aucune perte ; SAS lui-même transmet ces octets, `30` §6.2). Troncature à l'octet d'un caractère multi-octets (`61 C3`) : elle est lue comme octets invalides.

## Décision 2 — Conservation du code de missing spécial

### Options étudiées

| Option | Principe | Pour | Contre |
|---|---|---|---|
| A. Colonne annexe | une colonne `UInt8` par colonne numérique : `0` = valeur présente, `1` = `.`, `2` = `._`, `3` à `28` = `.A` à `.Z` | par ligne, sans perte ; lue par n'importe quel consommateur Arrow ; DataFusion peut la projeter, la filtrer, l'agréger ; coût nul si non demandée (colonne absente du schéma) | le schéma change (nom supplémentaire à choisir) ; deux colonnes à garder alignées |
| B. Métadonnée de champ Arrow | clé `sas.missing_codes` sur le champ | schéma inchangé | la métadonnée est **par colonne**, pas par ligne : ne peut dire quelle ligne est `.B` ; inutilisable pour restituer les valeurs |
| C. Type extension (`sas.numeric`, stockage `Struct{value: Float64, missing: UInt8}`) | un type par valeur | une seule colonne logique | peu de consommateurs savent le lire (pandas, polars, DuckDB le dégradent en struct) ; DataFusion exige des UDF ; casse l'interopérabilité simple |
| D. Encoder dans le NaN (payload) | `Float64` NaN dont le payload porte le code (comme SAS, R/`haven`) | une colonne, aucun schéma de plus | Arrow ne garantit pas la conservation du payload par les noyaux de calcul ni par les conversions ; non vérifiable dans SQL ; `null` Arrow perd l'information (le validity bitmap masque la valeur) |

### Décision

**Option A, activable (`MissingMode`)** :

- `MissingMode::Null` (défaut) : `Float64` avec valeur nulle pour tout manquant, **pas de colonne annexe** (schéma minimal, comportement attendu par la plupart des usages).
- `MissingMode::Sidecar` : pour chaque colonne numérique `x` du schéma, ajout d'une colonne `UInt8` non nullable `x__missing` (suffixe configurable) placée juste après `x`. `x` vaut null dès que `x__missing ≠ 0`. Le champ `x` reçoit la métadonnée `sas.missing_sidecar = "x__missing"`.
- Métadonnée complémentaire (option B, informative) : `sas.has_special_missing` ∈ {`true`, `false`, `unknown`} sur le champ, remplie quand le plan de pages permet de le savoir sans lire les données (jamais en streaming : `unknown`).
- Les codes `1` à `28` suivent `10` §5.3 : `1` = `.` ; `2` = `._` ; `3 + (lettre − 'A')` = `.A`…`.Z`. Tout NaN dont le code n'est pas reconnu devient `1` (règle de lecture de la spécification). Le code complet (octet de rang 5 en little endian, `0xFD` = `.A` … `0xE4` = `.Z`, `0xFF` = `._`, `0xFE` = `.`) reste interne au cœur (`MissingKind`).
- Écriture : `ArrowWriter` accepte la colonne annexe (même suffixe) et écrit le code dans l'octet voulu ; sans colonne annexe, un null numérique s'écrit `.`.

### Restitution Python

- `read_sas(..., missing="sidecar")` renvoie les colonnes annexes dans la table Arrow ; `read_sas_pandas` : colonne `x` (`float64` + `NaN`) et, si demandé, DataFrame annexe dans `df.attrs["sas_missing"]` (nom de colonne → `pandas.Series` de `str` : `"."`, `"_"`, `"A"`…, ou `None`), ou colonnes `x__missing` conservées avec `keep_missing_columns=True` ; `read_sas_polars` : colonne `x` (`Float64` nulle) et colonne `x__missing` (`UInt8`, ou `Categorical` de `"."`, `"A"`… avec `missing_as="categorical"`). pandas et polars n'ont pas de type « NA étiqueté » : la restitution ne peut pas être plus fidèle que la colonne annexe.

### Restitution R (`haven`)

- Une colonne numérique avec manquants spéciaux est rendue comme **NA étiquetés de `haven`** (`haven::tagged_na("b")`, test par `haven::na_tag()` ; les lettres sont minuscules, voir « Mise en œuvre (J5) »), en construisant le `double` R avec le motif de NaN que `haven` documente ; la valeur nulle sans code est `NA_real_`. Le détail du motif est une dépendance externe à `haven` à vérifier au jalon 5 (hors de la spécification du format).
- Attributs : `label` (label de variable), `format.sas` (nom de format avec largeur et décimales, par exemple `DATE9.`), `informat.sas` (extension du driver), `width` si pertinent. Dates : classe `Date` ; datetimes : `POSIXct` (UTC) ; heures : `hms::hms`. Attributs de table : `label` (label de la table), `sas7bdat.created`, `sas7bdat.encoding`, `sas7bdat.compression`.

## Décision 3 — Métadonnées dans Arrow

Champ : `sas.label`, `sas.format` (`NAME`), `sas.format_width`, `sas.format_decimals`, `sas.informat`, `sas.length` (largeur SAS), `sas.type` (`num`/`char`), `sas.logical_type` (`number`/`date`/`datetime`/`time`/`text`), `sas.index` (colonne indexée, `10` U17). Schéma : `sas.table_name`, `sas.table_label`, `sas.encoding`, `sas.encoding_code`, `sas.created`, `sas.modified` (ISO 8601 UTC), `sas.release`, `sas.host`, `sas.compression` (`none`/`char`/`binary`), `sas.row_count`, `sas.deleted_count`. Les métadonnées sont dans `arrow_schema(&TableMeta, &ReadOptions)` : le schéma est complet sans lire de page de données.

## Conséquences

- (+) Le mapping par défaut est sûr : pas de perte de valeur ; chaque écart (hors plage, octets invalides, NaN de code inconnu) est compté dans `ReadReport`.
- (+) Le missing spécial est conservé exactement, à la demande, avec un coût de schéma uniquement quand il est demandé ; DataFusion peut filtrer `x__missing = 3` (`.A`).
- (−) La colonne annexe double le nombre de colonnes numériques : choix volontaire, désactivé par défaut.
- (−) `Date32` perd la partie fractionnaire d'une date SAS ; `Timestamp(Microsecond)` diffère du `Timestamp(Second)` de la table de `30` : choix du driver, documenté, réversible par option.
- (−) La restitution pandas/polars ne peut pas représenter un NA étiqueté ; seule la colonne annexe porte l'information.
- Inconnues liées : U21 (largeurs 1 et 2, manquants big endian), U22, F05, F06, F10 à F12 (ADR 0006).

## Mise en œuvre (J4, 2026-11-04)

- Réalisés : mapping par défaut (`Float64`, `Date32`, `Timestamp(Microsecond)`, `Time64(Microsecond)`, `Utf8`), `DateMode::AsFloat`, `TimestampUnit::Second`, `TimeMode::Duration`, `OutOfRange::{Null, Error}` avec compteur `ReadStats::out_of_range`, `TrailingBlanks`, `InvalidBytes::{Replace, Error, Binary}` avec compteur `replaced_text`, `empty_as_null`, `encoding_override`, `MissingMode::{Null, Sidecar}` (suffixe configurable), métadonnées de champ et de schéma de la décision 3 (`sas.created` en ISO 8601 UTC d'après le décalage de l'écrivain).
- **Non réalisés** : `decimal_for_fixed`, `integers_when_exact`, `LargeUtf8` ; `sas.has_special_missing` vaut toujours `unknown`.
- Ajout non prévu : une colonne caractère devient `Binary` quand l'encodage de la table n'a pas d'équivalent standard (EBCDIC, pages IBM…) et qu'aucun `encoding_override` n'est donné ; le schéma le dit sans lire de donnée.
- La colonne annexe existe pour toute colonne à stockage numérique, dates, datetimes et heures comprises.

## Mise en œuvre (J5, 2026-11-04)

- **Python** (`uniformat-sas-py`, module `uniformat_io.sas`) : `read_sas` (flux `pyarrow.RecordBatchReader`), `read_sas_table`, `read_sas_pandas` (`df.attrs["sas"]` = métadonnées, `df.attrs["sas_missing"]` = codes en `str`, `keep_missing_columns`), `read_sas_polars` (`missing_as="uint8" | "categorical"`), `schema`, `metadata`. Les options de `ReadOptions` sont toutes exposées (`trailing_blanks`, `missing`, `dates`, `timestamp_unit`, `time`, `out_of_range`, `invalid_bytes`, `empty_as_null`, `encoding`, `strict`, `batch_rows`, `plan`).
- **R** (`uniformat-sas-r`, paquet `uniformatsas`) : `read_sas7bdat`, `sas7bdat_meta`. Attributs de colonne `label`, `format.sas` (nom + largeur + `.décimales`, comme `haven`), `informat.sas` (extension) ; `width` n'est pas posé (non pertinent). Classes : `Date`, `POSIXct` (UTC), `hms::hms` d'après le **format seul**.
- **Précision sur les NA étiquetés (point ouvert levé)** : `haven::read_sas` rend `.A` à `.Z` avec les étiquettes **minuscules** `a` à `z` ; `._` reste `"_"` ; `.` est `NA_real_`. Le paquet R fait de même, de sorte que `haven::na_tag()` donne le même résultat sur les deux lectures. Le motif est celui de R : `NA_real_` = NaN de mot bas 1954 (`0x7FF00000000007A2`), étiquette dans l'octet de rang 4 (little endian) ; vérifié avec `haven::na_tag()` et `haven::tagged_na()`.
- **Écarts avec `haven`, constatés et assumés** : lignes supprimées masquées ici, visibles dans `haven` ; largeur du format conservée ici pour les fichiers de `PROC IMPORT` ; octets invalides remplacés par U+FFFD ici. Un NUL dans un champ texte coupe la chaîne R (avertissement).
- **Ouvert** : le type logique retombe sur l'**informat** quand il n'y a pas de format (`classify_format`) ; Arrow et Python rendent alors `Date32` / `Timestamp` / `Time64` là où SAS affiche la valeur brute et où `haven` rend un `numeric` ; R suit le format seul. À trancher (option `dates_from_informat` ou alignement sur le format seul).

