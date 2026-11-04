# Spécification des formats, informats, catalogues et encodages `.sas7bdat`

> Changement OpenSpec : `specify-sas-formats-encodings` (prompt 04). Dernière mise à jour : 2026-10-30.
> Objet : dire au futur driver Rust comment interpréter le nom de format d'une colonne (type logique, type Arrow), comment afficher une valeur comme SAS pour les formats courants, comment lire un catalogue `.sas7bcat`, et comment traduire le code d'encodage de l'en-tête en encodage standard. Le document se lit sans `rea_output/evidence/`. Les offsets du fichier (noms de format, largeur, décimales, code d'encodage) sont dans `10_format_sas7bdat.md`.

## 0. Statut, méthode et limites

**Aucune décompilation n'a été faite** : la confirmation interne NULL de l'analyse d'interopérabilité n'est pas consignée dans `rea_output/STATE.md`. Conséquences :

- **aucune entrée n'est `confirmé`** (la définition exige une preuve binaire en plus d'une preuve fichier ou runtime) ; tout est `inféré` (preuve runtime ou fichier, souvent concordante avec une source publique) ou `inconnu` ;
- l'**inventaire exhaustif des formats natifs** (tables de noms de `src/sasexe/tkefmt.so`, `tkefmf*.so`, `tkefmi*.so`, tâches 2.1 et 2.2) et la **table des encodages tirée de `tknlsenc.so`/`tknlsicv.so`** (tâche 6.1) **n'ont pas été extraits** ; ils sont remplacés par (a) une liste de candidats tirée de la documentation publique de SAS 9.4 testée dans l'oracle, (b) les tables publiques de ReadStat, pandas et Parso (hypothèses) ;
- l'oracle `sas_small` est un **sous-ensemble** de SAS : `PROC FORMAT` y est absent, les formats ISO 8601 (`E8601DA`…), les formats NLS et plusieurs formats de la documentation y sont absents (section 2), `$HEX` ne se charge pas, `HEX`/`BINARY`/`OCTAL` y affichent comme un format numérique ordinaire, et seuls les encodages UTF-8, latin1 et wlatin1 sont gérés. Un format absent de l'oracle n'est donc pas absent de SAS : il est classé d'après la documentation (section 1) et son affichage n'est pas vérifié.

**Méthode** : (1) noms de formats de la documentation publique ; (2) interrogation de l'oracle par `PUT(valeur, format)` sur 5 875 couples (valeur, format) ; (3) vérificateur d'affichage Python comparé à ces sorties ; (4) catalogue généré par le SAS complet (`PROC FORMAT`, `corpus/files/catalogue/`) et catalogues publics ; (5) table des encodages à partir de trois tables publiques et des fichiers du corpus.

**Résultats de vérification** (rejouables : `sh rea_output/tools/run_checks.sh`) :

| Critère de fin | Résultat | Preuve |
|---|---|---|
| Classification complète (tout nom inventorié a une catégorie et un type Arrow) | **atteint** pour les 241 noms de la liste (172 formats, 69 informats) | `format_classification.py --check` |
| Affichage identique à `PROC PRINT`/`PUT` pour les formats de la liste | **non atteint** : 4 992 cas sur 5 875 identiques (85 %) ; 146 formats testés, 15 non implémentés ; écarts regroupés en 6 familles (section 4.6) | `evidence/verification_affichage.txt` |
| Catalogue relu à l'identique de `CNTLOUT=` | **non atteint** : seuls l'en-tête, le répertoire et les libellés sont lus ; plages et options non décodées | `evidence/catalogue_dump_partiel.txt` |
| Encodages couverts | **atteint pour les codes observés** (0, 20, 29, 40, 62, 138) ; table publique de 127 codes **confirmée et étendue par le binaire** (`tknls_einfo` : 265 emplacements, 225 codes nommés, section 6.3) ; texte décodé identique à l'oracle pour 50 fichiers non UTF-8 | `evidence/table_encodages.md`, `evidence/verification_encodage.txt` |

---

## 1. Table de classification format → type Arrow

Époque SAS : **1960-01-01**. Une date SAS est un nombre de jours depuis 1960-01-01 (3 653 jours avant 1970-01-01 : `Date32` = valeur − 3 653) ; un datetime est un nombre de secondes depuis 1960-01-01T00:00:00 (`Timestamp(Second)` = valeur − 315 619 200) ; une heure est un nombre de secondes depuis minuit. Une valeur manquante SAS (`.`, `._`, `.A`–`.Z`) devient `null` en Arrow (le code du manquant spécial peut être conservé à part, voir `10_format_sas7bdat.md` 5.3).

**Règles générales** (toutes `inféré`) :
- Le **format ne change jamais le type de stockage** : une colonne numérique reste un double IEEE ; le format sert à lui donner un sens (date, datetime, heure) ou une présentation. Les colonnes caractère restent des chaînes.
- Un nom de format absent de la liste (ci-dessous) est un **format utilisateur** : il se résout dans un catalogue `.sas7bcat` (section 5) ; sans catalogue, traiter la colonne selon son type de stockage (Float64 ou Utf8).
- Un format portant une largeur et des décimales (`COMMA10.2`) fixe une **échelle d'affichage** `d`, pas un stockage décimal : par défaut `Float64` ; `Decimal128(38, d)` est une option du driver, à n'utiliser que si l'utilisateur la demande (arrondi décimal de l'affichage, voir section 4).
- Les formats **ambigus** sont signalés dans la dernière colonne avec leur repli.
- Heures : SAS accepte des heures négatives ou supérieures à 24 h (observé : `-1` → `-0:00:01`, `86400` → `24:00:00`, `90000` → `25:00:00`). Un `Time64` Arrow ne les représente pas : repli sur `Duration(Second)`.

| nom | format / informat | catégorie logique | type Arrow cible | époque et unité | présent dans l'oracle | ambiguïté et repli |
|---|---|---|---|---|---|---|
| `BEST` | format | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | oui | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `BESTX` | format | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | oui | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `BINARY` | format | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | oui | le format change la représentation affichée, jamais le type stocké |
| `COMMA` | format | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | oui | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `COMMAX` | format | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | oui | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `DOLLAR` | format | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | oui | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `DOLLARX` | format | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | oui | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `E` | format | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | oui | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `EURO` | format | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | non | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `EUROX` | format | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | non | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `FRACT` | format | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | oui | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `HEX` | format | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | oui | le format change la représentation affichée, jamais le type stocké |
| `IB` | format | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | oui | le format change la représentation affichée, jamais le type stocké |
| `IBR` | format | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | oui | le format change la représentation affichée, jamais le type stocké |
| `IEEE` | format | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | oui | le format change la représentation affichée, jamais le type stocké |
| `MGF` | format | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | non | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `NEGPAREN` | format | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | oui | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `NUMX` | format | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | non | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `OCTAL` | format | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | oui | le format change la représentation affichée, jamais le type stocké |
| `PD` | format | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | oui | le format change la représentation affichée, jamais le type stocké |
| `PDJULG` | format | date | Date32 | jours depuis 1960-01-01 | non | — |
| `PDJULI` | format | date | Date32 | jours depuis 1960-01-01 | non | — |
| `PERCENT` | format | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | oui | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `PERCENTN` | format | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | non | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `PIB` | format | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | oui | le format change la représentation affichée, jamais le type stocké |
| `PIBR` | format | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | oui | le format change la représentation affichée, jamais le type stocké |
| `PK` | format | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | oui | le format change la représentation affichée, jamais le type stocké |
| `RB` | format | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | oui | le format change la représentation affichée, jamais le type stocké |
| `ROMAN` | format | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | oui | le format change la représentation affichée, jamais le type stocké |
| `S370FF` | format | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | non | le format change la représentation affichée, jamais le type stocké |
| `S370FIB` | format | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | non | le format change la représentation affichée, jamais le type stocké |
| `S370FPD` | format | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | non | le format change la représentation affichée, jamais le type stocké |
| `SSN` | format | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | oui | le format change la représentation affichée, jamais le type stocké |
| `WORDF` | format | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | oui | le format change la représentation affichée, jamais le type stocké |
| `WORDS` | format | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | oui | le format change la représentation affichée, jamais le type stocké |
| `Z` | format | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | oui | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `ZD` | format | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | oui | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `BESTD` | format | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | oui | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `COMMAXD` | format | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | non | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `NLNUM` | format | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | non | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `NLMNY` | format | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | non | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `NLPCT` | format | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | non | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `D` | format | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | oui | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `DOLLARD` | format | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | non | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `DOLLARDX` | format | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | non | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `PVALUE` | format | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | non | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `PERCENTD` | format | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | non | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `NEGPARENS` | format | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | non | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `DATE` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `DATEAMPM` | format | datetime | Timestamp(Second) | secondes depuis 1960-01-01T00:00:00 | oui | — |
| `DATETIME` | format | datetime | Timestamp(Second) | secondes depuis 1960-01-01T00:00:00 | oui | — |
| `DAY` | format | date (composante affichée) | Date32 | jours depuis 1960-01-01 | oui | la valeur stockée est une date complète ; le format n'affiche qu'une composante (jour, mois, trimestre…) |
| `DDMMYY` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `DDMMYYB` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `DDMMYYC` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `DDMMYYD` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `DDMMYYN` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `DDMMYYP` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `DDMMYYS` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `DOWNAME` | format | date (composante affichée) | Date32 | jours depuis 1960-01-01 | oui | la valeur stockée est une date complète ; le format n'affiche qu'une composante (jour, mois, trimestre…) |
| `DTDATE` | format | datetime (formaté en date) | Timestamp(Second) | secondes depuis 1960-01-01T00:00:00 | non | la valeur stockée est un datetime ; seul l'affichage est une date |
| `DTMONYY` | format | datetime (formaté en date) | Timestamp(Second) | secondes depuis 1960-01-01T00:00:00 | non | la valeur stockée est un datetime ; seul l'affichage est une date |
| `DTWKDATX` | format | datetime (formaté en date) | Timestamp(Second) | secondes depuis 1960-01-01T00:00:00 | non | la valeur stockée est un datetime ; seul l'affichage est une date |
| `DTYEAR` | format | datetime (formaté en date) | Timestamp(Second) | secondes depuis 1960-01-01T00:00:00 | non | la valeur stockée est un datetime ; seul l'affichage est une date |
| `DTYYQC` | format | datetime (formaté en date) | Timestamp(Second) | secondes depuis 1960-01-01T00:00:00 | non | la valeur stockée est un datetime ; seul l'affichage est une date |
| `EURDFDD` | format | date | Date32 | jours depuis 1960-01-01 | non | — |
| `EURDFDE` | format | date | Date32 | jours depuis 1960-01-01 | non | — |
| `EURDFDN` | format | date | Date32 | jours depuis 1960-01-01 | non | — |
| `EURDFDWN` | format | date (composante affichée) | Date32 | jours depuis 1960-01-01 | non | la valeur stockée est une date complète ; le format n'affiche qu'une composante (jour, mois, trimestre…) |
| `EURDFMN` | format | date (composante affichée) | Date32 | jours depuis 1960-01-01 | non | la valeur stockée est une date complète ; le format n'affiche qu'une composante (jour, mois, trimestre…) |
| `EURDFMY` | format | date | Date32 | jours depuis 1960-01-01 | non | — |
| `EURDFWDX` | format | date | Date32 | jours depuis 1960-01-01 | non | — |
| `EURDFWKX` | format | date (composante affichée) | Date32 | jours depuis 1960-01-01 | non | la valeur stockée est une date complète ; le format n'affiche qu'une composante (jour, mois, trimestre…) |
| `HHMM` | format | heure | Time64(Microsecond) si 0 ≤ v < 86400, sinon Duration(Second) | secondes depuis minuit (peut être négative ou ≥ 86400) | oui | valeurs négatives et ≥ 24 h admises par SAS (observé : `-1`, `86400`, `90000`) |
| `HOUR` | format | heure | Time64(Microsecond) si 0 ≤ v < 86400, sinon Duration(Second) | secondes depuis minuit (peut être négative ou ≥ 86400) | oui | valeurs négatives et ≥ 24 h admises par SAS (observé : `-1`, `86400`, `90000`) |
| `JULDAY` | format | date (composante affichée) | Date32 | jours depuis 1960-01-01 | oui | la valeur stockée est une date complète ; le format n'affiche qu'une composante (jour, mois, trimestre…) |
| `JULIAN` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `MINGUO` | format | date | Date32 | jours depuis 1960-01-01 | non | — |
| `MMDDYY` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `MMDDYYB` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `MMDDYYC` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `MMDDYYD` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `MMDDYYN` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `MMDDYYP` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `MMDDYYS` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `MMSS` | format | heure | Time64(Microsecond) si 0 ≤ v < 86400, sinon Duration(Second) | secondes depuis minuit (peut être négative ou ≥ 86400) | oui | valeurs négatives et ≥ 24 h admises par SAS (observé : `-1`, `86400`, `90000`) |
| `MMYY` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `MMYYC` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `MMYYD` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `MMYYN` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `MMYYP` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `MMYYS` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `MONNAME` | format | date (composante affichée) | Date32 | jours depuis 1960-01-01 | oui | la valeur stockée est une date complète ; le format n'affiche qu'une composante (jour, mois, trimestre…) |
| `MONTH` | format | date (composante affichée) | Date32 | jours depuis 1960-01-01 | oui | la valeur stockée est une date complète ; le format n'affiche qu'une composante (jour, mois, trimestre…) |
| `MONYY` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `NENGO` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `NLDATE` | format | date | Date32 | jours depuis 1960-01-01 | non | — |
| `NLDATEMN` | format | date (composante affichée) | Date32 | jours depuis 1960-01-01 | oui | la valeur stockée est une date complète ; le format n'affiche qu'une composante (jour, mois, trimestre…) |
| `NLDATEYQ` | format | date | Date32 | jours depuis 1960-01-01 | non | — |
| `NLDATM` | format | datetime | Timestamp(Second) | secondes depuis 1960-01-01T00:00:00 | non | — |
| `NLTIME` | format | heure | Time64(Microsecond) si 0 ≤ v < 86400, sinon Duration(Second) | secondes depuis minuit (peut être négative ou ≥ 86400) | non | valeurs négatives et ≥ 24 h admises par SAS (observé : `-1`, `86400`, `90000`) |
| `PDFDATE` | format | date | Date32 | jours depuis 1960-01-01 | non | — |
| `QTR` | format | date (composante affichée) | Date32 | jours depuis 1960-01-01 | oui | la valeur stockée est une date complète ; le format n'affiche qu'une composante (jour, mois, trimestre…) |
| `QTRR` | format | date (composante affichée) | Date32 | jours depuis 1960-01-01 | oui | la valeur stockée est une date complète ; le format n'affiche qu'une composante (jour, mois, trimestre…) |
| `TIME` | format | heure | Time64(Microsecond) si 0 ≤ v < 86400, sinon Duration(Second) | secondes depuis minuit (peut être négative ou ≥ 86400) | oui | valeurs négatives et ≥ 24 h admises par SAS (observé : `-1`, `86400`, `90000`) |
| `TIMEAMPM` | format | heure | Time64(Microsecond) si 0 ≤ v < 86400, sinon Duration(Second) | secondes depuis minuit (peut être négative ou ≥ 86400) | oui | valeurs négatives et ≥ 24 h admises par SAS (observé : `-1`, `86400`, `90000`) |
| `TOD` | format | datetime (formaté en heure) | Timestamp(Second) | secondes depuis 1960-01-01T00:00:00 | oui | ambigu : le format affiche l'heure d'un datetime ; repli : Timestamp(Second) |
| `WEEKDATE` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `WEEKDATX` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `WEEKDAY` | format | date (composante affichée) | Date32 | jours depuis 1960-01-01 | oui | la valeur stockée est une date complète ; le format n'affiche qu'une composante (jour, mois, trimestre…) |
| `WEEKU` | format | date (composante affichée) | Date32 | jours depuis 1960-01-01 | non | la valeur stockée est une date complète ; le format n'affiche qu'une composante (jour, mois, trimestre…) |
| `WEEKV` | format | date (composante affichée) | Date32 | jours depuis 1960-01-01 | non | la valeur stockée est une date complète ; le format n'affiche qu'une composante (jour, mois, trimestre…) |
| `WEEKW` | format | date (composante affichée) | Date32 | jours depuis 1960-01-01 | non | la valeur stockée est une date complète ; le format n'affiche qu'une composante (jour, mois, trimestre…) |
| `WORDDATE` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `WORDDATX` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `YEAR` | format | date (composante affichée) | Date32 | jours depuis 1960-01-01 | oui | la valeur stockée est une date complète ; le format n'affiche qu'une composante (jour, mois, trimestre…) |
| `YYMM` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `YYMMC` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `YYMMD` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `YYMMDD` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `YYMMDDB` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `YYMMDDC` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `YYMMDDD` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `YYMMDDN` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `YYMMDDP` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `YYMMDDS` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `YYMMN` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `YYMMP` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `YYMMS` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `YYMON` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `YYQ` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `YYQC` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `YYQD` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `YYQN` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `YYQP` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `YYQR` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `YYQRC` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `YYQRD` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `YYQRN` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `YYQRP` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `YYQRS` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `YYQS` | format | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `YYQZ` | format | date | Date32 | jours depuis 1960-01-01 | non | — |
| `E8601DA` | format | date | Date32 | jours depuis 1960-01-01 | non | — |
| `E8601DN` | format | date | Date32 | jours depuis 1960-01-01 | non | — |
| `E8601DT` | format | datetime | Timestamp(Second) | secondes depuis 1960-01-01T00:00:00 | non | — |
| `E8601DZ` | format | datetime | Timestamp(Second) | secondes depuis 1960-01-01T00:00:00 | non | — |
| `E8601LZ` | format | datetime | Timestamp(Second) | secondes depuis 1960-01-01T00:00:00 | non | — |
| `E8601TM` | format | heure | Time64(Microsecond) si 0 ≤ v < 86400, sinon Duration(Second) | secondes depuis minuit (peut être négative ou ≥ 86400) | non | valeurs négatives et ≥ 24 h admises par SAS (observé : `-1`, `86400`, `90000`) |
| `E8601TZ` | format | heure | Time64(Microsecond) si 0 ≤ v < 86400, sinon Duration(Second) | secondes depuis minuit (peut être négative ou ≥ 86400) | non | valeurs négatives et ≥ 24 h admises par SAS (observé : `-1`, `86400`, `90000`) |
| `B8601DA` | format | date | Date32 | jours depuis 1960-01-01 | non | — |
| `B8601DN` | format | date | Date32 | jours depuis 1960-01-01 | non | — |
| `B8601DT` | format | datetime | Timestamp(Second) | secondes depuis 1960-01-01T00:00:00 | non | — |
| `B8601DZ` | format | datetime | Timestamp(Second) | secondes depuis 1960-01-01T00:00:00 | non | — |
| `B8601LZ` | format | datetime | Timestamp(Second) | secondes depuis 1960-01-01T00:00:00 | non | — |
| `B8601TM` | format | heure | Time64(Microsecond) si 0 ≤ v < 86400, sinon Duration(Second) | secondes depuis minuit (peut être négative ou ≥ 86400) | non | valeurs négatives et ≥ 24 h admises par SAS (observé : `-1`, `86400`, `90000`) |
| `B8601TZ` | format | heure | Time64(Microsecond) si 0 ≤ v < 86400, sinon Duration(Second) | secondes depuis minuit (peut être négative ou ≥ 86400) | non | valeurs négatives et ≥ 24 h admises par SAS (observé : `-1`, `86400`, `90000`) |
| `$` | format | caractère | Utf8 | — | oui | décoder avec l'encodage du fichier (voir section encodages) |
| `$CHAR` | format | caractère | Utf8 | — | oui | décoder avec l'encodage du fichier (voir section encodages) |
| `$HEX` | format | caractère | Utf8 | — | oui | décoder avec l'encodage du fichier (voir section encodages) |
| `$UPCASE` | format | caractère | Utf8 | — | non | décoder avec l'encodage du fichier (voir section encodages) |
| `$VARYING` | format | caractère | Utf8 | — | oui | décoder avec l'encodage du fichier (voir section encodages) |
| `$BINARY` | format | caractère | Utf8 | — | oui | décoder avec l'encodage du fichier (voir section encodages) |
| `$OCTAL` | format | caractère | Utf8 | — | oui | décoder avec l'encodage du fichier (voir section encodages) |
| `$QUOTE` | format | caractère | Utf8 | — | oui | décoder avec l'encodage du fichier (voir section encodages) |
| `$REVERJ` | format | caractère | Utf8 | — | non | décoder avec l'encodage du fichier (voir section encodages) |
| `$REVERS` | format | caractère | Utf8 | — | non | décoder avec l'encodage du fichier (voir section encodages) |
| `$ASCII` | format | caractère | Utf8 | — | non | décoder avec l'encodage du fichier (voir section encodages) |
| `$EBCDIC1047` | format | caractère | Utf8 | — | oui | décoder avec l'encodage du fichier (voir section encodages) |
| `$MSGCASE` | format | caractère | Utf8 | — | non | décoder avec l'encodage du fichier (voir section encodages) |
| `$N` | format | caractère | Utf8 | — | non | décoder avec l'encodage du fichier (voir section encodages) |
| `$F` | format | caractère | Utf8 | — | oui | décoder avec l'encodage du fichier (voir section encodages) |
| `BEST` | informat | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | oui | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `BINARY` | informat | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | oui | le format change la représentation affichée, jamais le type stocké |
| `COMMA` | informat | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | oui | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `COMMAX` | informat | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | oui | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `DOLLAR` | informat | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | oui | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `DOLLARX` | informat | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | oui | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `E` | informat | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | oui | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `EUROX` | informat | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | non | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `HEX` | informat | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | oui | le format change la représentation affichée, jamais le type stocké |
| `IB` | informat | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | oui | le format change la représentation affichée, jamais le type stocké |
| `IBR` | informat | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | oui | le format change la représentation affichée, jamais le type stocké |
| `IEEE` | informat | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | oui | le format change la représentation affichée, jamais le type stocké |
| `MDYAMPM` | informat | datetime | Timestamp(Second) | secondes depuis 1960-01-01T00:00:00 | oui | — |
| `NEGPAREN` | informat | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | oui | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `NUMX` | informat | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | non | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `OCTAL` | informat | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | oui | le format change la représentation affichée, jamais le type stocké |
| `PD` | informat | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | oui | le format change la représentation affichée, jamais le type stocké |
| `PDJULG` | informat | date | Date32 | jours depuis 1960-01-01 | non | — |
| `PERCENT` | informat | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | oui | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `PIB` | informat | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | oui | le format change la représentation affichée, jamais le type stocké |
| `PK` | informat | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | oui | le format change la représentation affichée, jamais le type stocké |
| `RB` | informat | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | oui | le format change la représentation affichée, jamais le type stocké |
| `ROMAN` | informat | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | non | le format change la représentation affichée, jamais le type stocké |
| `S370FF` | informat | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | non | le format change la représentation affichée, jamais le type stocké |
| `S370FIB` | informat | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | non | le format change la représentation affichée, jamais le type stocké |
| `S370FPD` | informat | numérique (affiché en binaire, hexadécimal ou texte) | Float64 | valeur stockée inchangée (double IEEE) | non | le format change la représentation affichée, jamais le type stocké |
| `STIMER` | informat | heure | Time64(Microsecond) si 0 ≤ v < 86400, sinon Duration(Second) | secondes depuis minuit (peut être négative ou ≥ 86400) | non | valeurs négatives et ≥ 24 h admises par SAS (observé : `-1`, `86400`, `90000`) |
| `TIME` | informat | heure | Time64(Microsecond) si 0 ≤ v < 86400, sinon Duration(Second) | secondes depuis minuit (peut être négative ou ≥ 86400) | oui | valeurs négatives et ≥ 24 h admises par SAS (observé : `-1`, `86400`, `90000`) |
| `WEIGHT` | informat | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | non | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `Z` | informat | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | non | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `ZD` | informat | numérique (échelle fixe `d` si le format porte `.d`) | Float64 (Decimal128(38, d) en option) | valeur stockée inchangée (double IEEE) | oui | le format n'altère pas la valeur : l'échelle `d` est de l'affichage, pas du stockage |
| `ANYDTDTE` | informat | date | Date32 | jours depuis 1960-01-01 | non | — |
| `ANYDTDTM` | informat | datetime | Timestamp(Second) | secondes depuis 1960-01-01T00:00:00 | non | — |
| `ANYDTTME` | informat | heure | Time64(Microsecond) si 0 ≤ v < 86400, sinon Duration(Second) | secondes depuis minuit (peut être négative ou ≥ 86400) | non | valeurs négatives et ≥ 24 h admises par SAS (observé : `-1`, `86400`, `90000`) |
| `DATE` | informat | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `DATETIME` | informat | datetime | Timestamp(Second) | secondes depuis 1960-01-01T00:00:00 | oui | — |
| `DDMMYY` | informat | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `EURDFDE` | informat | date | Date32 | jours depuis 1960-01-01 | non | — |
| `EURDFDT` | informat | datetime | Timestamp(Second) | secondes depuis 1960-01-01T00:00:00 | non | — |
| `JULIAN` | informat | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `MMDDYY` | informat | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `MONYY` | informat | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `NENGO` | informat | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `NLDATE` | informat | date | Date32 | jours depuis 1960-01-01 | non | — |
| `NLDATM` | informat | datetime | Timestamp(Second) | secondes depuis 1960-01-01T00:00:00 | non | — |
| `NLTIME` | informat | heure | Time64(Microsecond) si 0 ≤ v < 86400, sinon Duration(Second) | secondes depuis minuit (peut être négative ou ≥ 86400) | non | valeurs négatives et ≥ 24 h admises par SAS (observé : `-1`, `86400`, `90000`) |
| `YYMMDD` | informat | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `YYMMDDB` | informat | date | Date32 | jours depuis 1960-01-01 | non | — |
| `YYMMDDC` | informat | date | Date32 | jours depuis 1960-01-01 | non | — |
| `YYMMDDD` | informat | date | Date32 | jours depuis 1960-01-01 | non | — |
| `YYMMDDN` | informat | date | Date32 | jours depuis 1960-01-01 | non | — |
| `YYMMDDP` | informat | date | Date32 | jours depuis 1960-01-01 | non | — |
| `YYMMDDS` | informat | date | Date32 | jours depuis 1960-01-01 | non | — |
| `YYQ` | informat | date | Date32 | jours depuis 1960-01-01 | oui | — |
| `E8601DA` | informat | date | Date32 | jours depuis 1960-01-01 | non | — |
| `E8601DT` | informat | datetime | Timestamp(Second) | secondes depuis 1960-01-01T00:00:00 | non | — |
| `E8601TM` | informat | heure | Time64(Microsecond) si 0 ≤ v < 86400, sinon Duration(Second) | secondes depuis minuit (peut être négative ou ≥ 86400) | non | valeurs négatives et ≥ 24 h admises par SAS (observé : `-1`, `86400`, `90000`) |
| `B8601DA` | informat | date | Date32 | jours depuis 1960-01-01 | non | — |
| `B8601DT` | informat | datetime | Timestamp(Second) | secondes depuis 1960-01-01T00:00:00 | non | — |
| `B8601TM` | informat | heure | Time64(Microsecond) si 0 ≤ v < 86400, sinon Duration(Second) | secondes depuis minuit (peut être négative ou ≥ 86400) | non | valeurs négatives et ≥ 24 h admises par SAS (observé : `-1`, `86400`, `90000`) |
| `$` | informat | caractère | Utf8 | — | oui | décoder avec l'encodage du fichier (voir section encodages) |
| `$CHAR` | informat | caractère | Utf8 | — | oui | décoder avec l'encodage du fichier (voir section encodages) |
| `$UPCASE` | informat | caractère | Utf8 | — | non | décoder avec l'encodage du fichier (voir section encodages) |
| `$VARYING` | informat | caractère | Utf8 | — | oui | décoder avec l'encodage du fichier (voir section encodages) |
| `$BINARY` | informat | caractère | Utf8 | — | oui | décoder avec l'encodage du fichier (voir section encodages) |
| `$OCTAL` | informat | caractère | Utf8 | — | oui | décoder avec l'encodage du fichier (voir section encodages) |
| `$HEX` | informat | caractère | Utf8 | — | oui | décoder avec l'encodage du fichier (voir section encodages) |
| `$ASCII` | informat | caractère | Utf8 | — | non | décoder avec l'encodage du fichier (voir section encodages) |
| `$EBCDIC1047` | informat | caractère | Utf8 | — | oui | décoder avec l'encodage du fichier (voir section encodages) |

- Preuve : classification d'après la documentation publique (S7 de `00_etat_de_l_art.md`) et l'affichage observé dans l'oracle (section 4) pour les formats présents.
- Confiance : inféré (catégorie et époque confirmées par les sorties de l'oracle pour les 110 formats présents ; documentation seule pour les 62 absents).

---


### 1.1 Noms natifs absents de la documentation publique (extraits des binaires, 2026-10-31)

La décompilation des modules `tkefmf*.so` et `tkefmi*.so` (6.4, `evidence/decompile/formats_natifs.tsv`) a révélé **388 couples nom/sens** (277 noms) qui ne figurent pas dans la table ci-dessus. Ils sont classés d'après la **famille** posée par l'analyse (colonne `famille` du TSV) et le type de sortie ou d'entrée de la fonction ; aucun n'a été essayé dans l'oracle. Confiance : `inféré` pour tous. La largeur par défaut, minimale et maximale de chacun est dans le TSV. Les règles générales de la section 1 s'appliquent.

| nom | sens | famille (TSV) | catégorie logique | type Arrow cible | époque et unité | module |
|---|---|---|---|---|---|---|
| `$BASE64X` | format | binaire_hex_base64 | texte (hexadécimal, base64) ou octets | Utf8 ou Binary | — | tkefmfbi.so |
| `$BASE64X` | informat | informat_caractere_general | caractère | Utf8 | — | tkefmich.so |
| `$BYVAL` | format | octets_machine_ibm370_ascii_ebcdic | numérique ou texte lu depuis des octets machine IBM370/ASCII/EBCDIC | Float64 ou Utf8 selon le type de la colonne | valeur stockée inchangée | tkefmfuw.so |
| `$BYVAL` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `$CB` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `$CHARZB` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `$CSTR` | format | octets_machine_ibm370_ascii_ebcdic | numérique ou texte lu depuis des octets machine IBM370/ASCII/EBCDIC | Float64 ou Utf8 selon le type de la colonne | valeur stockée inchangée | tkefmfuw.so |
| `$CSTR` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `$EBCDIC` | format | octets_machine_ibm370_ascii_ebcdic | numérique ou texte lu depuis des octets machine IBM370/ASCII/EBCDIC | Float64 ou Utf8 selon le type de la colonne | valeur stockée inchangée | tkefmfuw.so |
| `$EBCDIC` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `$F` | informat | informat_caractere_general | caractère | Utf8 | — | tkefmich.so |
| `$FREE` | format | caractere | caractère | Utf8 | — | tkefmfms.so |
| `$IS_INTRINSIC` | format | caractere | caractère | Utf8 | — | tkefmfms.so |
| `$LOGVS` | format | logique_varchar | caractère | Utf8 | — | tkefmflv.so |
| `$LOGVSR` | format | logique_varchar | caractère | Utf8 | — | tkefmflv.so |
| `$PHEX` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `$QUOTE` | informat | informat_caractere_general | caractère | Utf8 | — | tkefmich.so |
| `$REVERJ` | informat | informat_caractere_general | caractère | Utf8 | — | tkefmich.so |
| `$REVERS` | informat | informat_caractere_general | caractère | Utf8 | — | tkefmich.so |
| `$UUID` | format | binaire_hex_base64 | texte (hexadécimal, base64) ou octets | Utf8 ou Binary | — | tkefmfbi.so |
| `$UUID` | informat | informat_caractere_general | caractère | Utf8 | — | tkefmich.so |
| `$VSLOG` | format | logique_varchar | caractère | Utf8 | — | tkefmflv.so |
| `$VSLOGR` | format | logique_varchar | caractère | Utf8 | — | tkefmflv.so |
| `$XPORTCH` | format | octets_machine_ibm370_ascii_ebcdic | numérique ou texte lu depuis des octets machine IBM370/ASCII/EBCDIC | Float64 ou Utf8 selon le type de la colonne | valeur stockée inchangée | tkefmfuw.so |
| `BESTDOTX` | format | numerique | numérique | Float64 | valeur stockée inchangée | tkefmfms.so |
| `BITS` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `BZ` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `CB` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `DTWEEKV` | format | nls_autre | numérique (locale NLS, famille à confirmer) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `EURFRATS` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURFRBEF` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURFRCHF` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURFRCZK` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURFRDEM` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURFRDKK` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURFRESP` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURFRFIM` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURFRFRF` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURFRGBP` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURFRGRD` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURFRHUF` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURFRIEP` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURFRITL` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURFRLUF` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURFRNLG` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURFRNOK` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURFRPLZ` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURFRPTE` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURFRROL` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURFRRUR` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURFRSEK` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURFRSIT` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURFRTRL` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURFRYUD` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURO` | informat | informat_caractere_general | caractère | Utf8 | — | tkefmich.so |
| `EURTOATS` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURTOBEF` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURTOCHF` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURTOCZK` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURTODEM` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURTODKK` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURTOESP` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURTOFIM` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURTOFRF` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURTOGBP` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURTOGRD` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURTOHUF` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURTOIEP` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURTOITL` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURTOLUF` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURTONLG` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURTONOK` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURTOPLZ` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURTOPTE` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURTOROL` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURTORUR` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURTOSEK` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURTOSIT` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURTOTRL` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `EURTOYUD` | format | monnaie_euro_conversion | numérique (conversion monétaire euro) | Float64 | valeur stockée inchangée | tkefmfms.so |
| `F` | format | numerique | numérique | Float64 | valeur stockée inchangée | tkefmfms.so |
| `F` | informat | informat_caractere_general | caractère | Utf8 | — | tkefmich.so |
| `FLOAT` | format | octets_machine_ibm370_ascii_ebcdic | numérique ou texte lu depuis des octets machine IBM370/ASCII/EBCDIC | Float64 ou Utf8 selon le type de la colonne | valeur stockée inchangée | tkefmfuw.so |
| `FLOAT` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `IEEER` | format | octets_machine_ibm370_ascii_ebcdic | numérique ou texte lu depuis des octets machine IBM370/ASCII/EBCDIC | Float64 ou Utf8 selon le type de la colonne | valeur stockée inchangée | tkefmfuw.so |
| `IEEER` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `IS8601DA` | format | date_iso8601 | date (ISO 8601) | Date32 | jours depuis 1960-01-01 | tkefmfdt.so |
| `IS8601DN` | format | heure_iso8601 | heure (ISO 8601) | Time64(Microsecond) | secondes depuis minuit | tkefmftm.so |
| `IS8601DT` | format | heure_iso8601 | heure (ISO 8601) | Time64(Microsecond) | secondes depuis minuit | tkefmftm.so |
| `IS8601DZ` | format | heure_iso8601 | heure (ISO 8601) | Time64(Microsecond) | secondes depuis minuit | tkefmftm.so |
| `IS8601LZ` | format | heure_iso8601 | heure (ISO 8601) | Time64(Microsecond) | secondes depuis minuit | tkefmftm.so |
| `IS8601TM` | format | heure_iso8601 | heure (ISO 8601) | Time64(Microsecond) | secondes depuis minuit | tkefmftm.so |
| `IS8601TZ` | format | heure_iso8601 | heure (ISO 8601) | Time64(Microsecond) | secondes depuis minuit | tkefmftm.so |
| `LOGPROB` | format | octets_machine_ibm370_ascii_ebcdic | numérique ou texte lu depuis des octets machine IBM370/ASCII/EBCDIC | Float64 ou Utf8 selon le type de la colonne | valeur stockée inchangée | tkefmfuw.so |
| `MDYAMPM` | format | heure_datetime | datetime | Timestamp(Microsecond) | secondes depuis 1960-01-01T00:00:00 | tkefmftm.so |
| `MRB` | format | octets_machine_ibm370_ascii_ebcdic | numérique ou texte lu depuis des octets machine IBM370/ASCII/EBCDIC | Float64 ou Utf8 selon le type de la colonne | valeur stockée inchangée | tkefmfuw.so |
| `MRB` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `MSEC` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `NLBEST` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLD` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLDATEL` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLDATEM` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLDATEMD` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLDATEMDL` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLDATEMDM` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLDATEMDS` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLDATES` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLDATEW` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLDATEW` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLDATEWN` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLDATEYM` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLDATEYML` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLDATEYMM` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLDATEYMS` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLDATEYQL` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLDATEYQM` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLDATEYQS` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLDATEYR` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLDATEYW` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLDATMAP` | format | nls_datetime | datetime (locale NLS) | Timestamp(Microsecond) | secondes depuis 1960-01-01T00:00:00 | tkefmfnl.so |
| `NLDATMAP` | informat | nls_datetime | datetime (locale NLS) | Timestamp(Microsecond) | secondes depuis 1960-01-01T00:00:00 | tkefmfnl.so |
| `NLDATMDT` | format | nls_datetime | datetime (locale NLS) | Timestamp(Microsecond) | secondes depuis 1960-01-01T00:00:00 | tkefmfnl.so |
| `NLDATML` | format | nls_datetime | datetime (locale NLS) | Timestamp(Microsecond) | secondes depuis 1960-01-01T00:00:00 | tkefmfnl.so |
| `NLDATMM` | format | nls_datetime | datetime (locale NLS) | Timestamp(Microsecond) | secondes depuis 1960-01-01T00:00:00 | tkefmfnl.so |
| `NLDATMMD` | format | nls_datetime | datetime (locale NLS) | Timestamp(Microsecond) | secondes depuis 1960-01-01T00:00:00 | tkefmfnl.so |
| `NLDATMMDL` | format | nls_datetime | datetime (locale NLS) | Timestamp(Microsecond) | secondes depuis 1960-01-01T00:00:00 | tkefmfnl.so |
| `NLDATMMDM` | format | nls_datetime | datetime (locale NLS) | Timestamp(Microsecond) | secondes depuis 1960-01-01T00:00:00 | tkefmfnl.so |
| `NLDATMMDS` | format | nls_datetime | datetime (locale NLS) | Timestamp(Microsecond) | secondes depuis 1960-01-01T00:00:00 | tkefmfnl.so |
| `NLDATMMN` | format | nls_datetime | datetime (locale NLS) | Timestamp(Microsecond) | secondes depuis 1960-01-01T00:00:00 | tkefmfnl.so |
| `NLDATMS` | format | nls_datetime | datetime (locale NLS) | Timestamp(Microsecond) | secondes depuis 1960-01-01T00:00:00 | tkefmfnl.so |
| `NLDATMTM` | format | nls_datetime | datetime (locale NLS) | Timestamp(Microsecond) | secondes depuis 1960-01-01T00:00:00 | tkefmfnl.so |
| `NLDATMTZ` | format | nls_datetime | datetime (locale NLS) | Timestamp(Microsecond) | secondes depuis 1960-01-01T00:00:00 | tkefmfnl.so |
| `NLDATMW` | format | nls_datetime | datetime (locale NLS) | Timestamp(Microsecond) | secondes depuis 1960-01-01T00:00:00 | tkefmfnl.so |
| `NLDATMW` | informat | nls_datetime | datetime (locale NLS) | Timestamp(Microsecond) | secondes depuis 1960-01-01T00:00:00 | tkefmfnl.so |
| `NLDATMWN` | format | nls_datetime | datetime (locale NLS) | Timestamp(Microsecond) | secondes depuis 1960-01-01T00:00:00 | tkefmfnl.so |
| `NLDATMWZ` | format | nls_datetime | datetime (locale NLS) | Timestamp(Microsecond) | secondes depuis 1960-01-01T00:00:00 | tkefmfnl.so |
| `NLDATMYM` | format | nls_datetime | datetime (locale NLS) | Timestamp(Microsecond) | secondes depuis 1960-01-01T00:00:00 | tkefmfnl.so |
| `NLDATMYML` | format | nls_datetime | datetime (locale NLS) | Timestamp(Microsecond) | secondes depuis 1960-01-01T00:00:00 | tkefmfnl.so |
| `NLDATMYMM` | format | nls_datetime | datetime (locale NLS) | Timestamp(Microsecond) | secondes depuis 1960-01-01T00:00:00 | tkefmfnl.so |
| `NLDATMYMS` | format | nls_datetime | datetime (locale NLS) | Timestamp(Microsecond) | secondes depuis 1960-01-01T00:00:00 | tkefmfnl.so |
| `NLDATMYQ` | format | nls_datetime | datetime (locale NLS) | Timestamp(Microsecond) | secondes depuis 1960-01-01T00:00:00 | tkefmfnl.so |
| `NLDATMYQL` | format | nls_datetime | datetime (locale NLS) | Timestamp(Microsecond) | secondes depuis 1960-01-01T00:00:00 | tkefmfnl.so |
| `NLDATMYQM` | format | nls_datetime | datetime (locale NLS) | Timestamp(Microsecond) | secondes depuis 1960-01-01T00:00:00 | tkefmfnl.so |
| `NLDATMYQS` | format | nls_datetime | datetime (locale NLS) | Timestamp(Microsecond) | secondes depuis 1960-01-01T00:00:00 | tkefmfnl.so |
| `NLDATMYR` | format | nls_datetime | datetime (locale NLS) | Timestamp(Microsecond) | secondes depuis 1960-01-01T00:00:00 | tkefmfnl.so |
| `NLDATMYW` | format | nls_datetime | datetime (locale NLS) | Timestamp(Microsecond) | secondes depuis 1960-01-01T00:00:00 | tkefmfnl.so |
| `NLDATMZ` | format | nls_datetime | datetime (locale NLS) | Timestamp(Microsecond) | secondes depuis 1960-01-01T00:00:00 | tkefmfnl.so |
| `NLMNIAED` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIAED` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIAUD` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIAUD` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIBGN` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIBGN` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIBRL` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIBRL` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNICAD` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNICAD` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNICHF` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNICHF` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNICNY` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNICNY` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNICZK` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNICZK` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIDKK` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIDKK` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIEEK` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIEEK` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIEGP` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIEGP` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIEUR` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIEUR` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIGBP` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIGBP` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIHKD` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIHKD` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIHRK` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIHRK` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIHUF` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIHUF` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIIDR` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIIDR` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIILS` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIILS` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIINR` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIINR` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIJPY` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIJPY` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIKRW` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIKRW` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNILTL` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNILTL` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNILVL` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNILVL` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIMOP` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIMOP` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIMXN` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIMXN` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIMYR` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIMYR` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNINOK` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNINOK` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNINZD` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNINZD` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIPLN` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIPLN` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIROL` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIROL` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIRUB` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIRUB` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIRUR` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIRUR` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNISEK` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNISEK` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNISGD` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNISGD` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNISKK` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNISKK` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNITHB` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNITHB` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNITRY` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNITRY` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNITWD` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNITWD` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIUSD` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIUSD` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIZAR` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNIZAR` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNLAED` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLAED` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLAUD` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLAUD` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLBGN` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLBGN` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLBRL` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLBRL` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLCAD` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLCAD` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLCHF` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLCHF` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLCNY` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLCNY` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLCZK` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLCZK` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLDKK` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLDKK` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLEEK` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLEEK` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLEGP` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLEGP` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLEUR` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLEUR` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLGBP` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLGBP` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLHKD` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLHKD` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLHRK` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLHRK` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLHUF` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLHUF` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLIDR` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLIDR` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLILS` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLILS` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLINR` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLINR` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLJPY` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLJPY` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLKRW` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLKRW` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLLTL` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLLTL` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLLVL` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLLVL` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLMOP` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLMOP` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLMXN` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLMXN` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLMYR` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLMYR` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLNOK` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLNOK` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLNZD` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLNZD` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLPLN` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLPLN` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLROL` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLROL` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLRUB` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLRUB` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLRUR` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLRUR` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLSEK` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLSEK` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLSGD` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLSGD` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLSKK` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLSKK` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLTHB` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLTHB` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLTRY` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLTRY` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLTWD` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLTWD` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLUSD` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLUSD` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLZAR` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNLZAR` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLMNY` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNYI` | format | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLMNYI` | informat | nls_monnaie | numérique (devise NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLNUM` | informat | nls_numerique | numérique (locale NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLNUMI` | format | nls_numerique | numérique (locale NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLNUMI` | informat | nls_numerique | numérique (locale NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLPCT` | informat | nls_numerique | numérique (locale NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLPCTI` | format | nls_numerique | numérique (locale NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLPCTI` | informat | nls_numerique | numérique (locale NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLPCTN` | format | nls_numerique | numérique (locale NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLPCTP` | format | nls_numerique | numérique (locale NLS) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `NLSTRMON` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLSTRMON` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLSTRQTR` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLSTRWK` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLTIMAP` | format | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLTIMAP` | informat | nls_date | date (locale NLS) | Date32 | jours depuis 1960-01-01 | tkefmfnl.so |
| `NLTIMEL` | format | nls_heure | heure (locale NLS) | Time64(Microsecond) | secondes depuis minuit | tkefmfnl.so |
| `NLTIMEM` | format | nls_heure | heure (locale NLS) | Time64(Microsecond) | secondes depuis minuit | tkefmfnl.so |
| `NLTIMES` | format | nls_heure | heure (locale NLS) | Time64(Microsecond) | secondes depuis minuit | tkefmfnl.so |
| `ODDSR` | format | numerique | numérique | Float64 | valeur stockée inchangée | tkefmfms.so |
| `PCPIB` | format | octets_machine_ibm370_ascii_ebcdic | numérique ou texte lu depuis des octets machine IBM370/ASCII/EBCDIC | Float64 ou Utf8 selon le type de la colonne | valeur stockée inchangée | tkefmfuw.so |
| `PCPIB` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `PDJULI` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `PDTIME` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `PIBR` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `PUNCH` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `RMFDUR` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `RMFSTAMP` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `ROW` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `RSTDOCNY` | format | divers_numerique | numérique | Float64 | valeur stockée inchangée | tkefmfms.so |
| `RSTDOCYY` | format | divers_numerique | numérique | Float64 | valeur stockée inchangée | tkefmfms.so |
| `RSTDONYN` | format | divers_numerique | numérique | Float64 | valeur stockée inchangée | tkefmfms.so |
| `RSTDOPNY` | format | divers_numerique | numérique | Float64 | valeur stockée inchangée | tkefmfms.so |
| `RSTDOPYN` | format | divers_numerique | numérique | Float64 | valeur stockée inchangée | tkefmfms.so |
| `RSTDOPYY` | format | divers_numerique | numérique | Float64 | valeur stockée inchangée | tkefmfms.so |
| `S370FHEX` | format | octets_machine_ibm370_ascii_ebcdic | numérique ou texte lu depuis des octets machine IBM370/ASCII/EBCDIC | Float64 ou Utf8 selon le type de la colonne | valeur stockée inchangée | tkefmfuw.so |
| `S370FHEX` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `S370FIBR` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `S370FIBU` | format | octets_machine_ibm370_ascii_ebcdic | numérique ou texte lu depuis des octets machine IBM370/ASCII/EBCDIC | Float64 ou Utf8 selon le type de la colonne | valeur stockée inchangée | tkefmfuw.so |
| `S370FPDU` | format | octets_machine_ibm370_ascii_ebcdic | numérique ou texte lu depuis des octets machine IBM370/ASCII/EBCDIC | Float64 ou Utf8 selon le type de la colonne | valeur stockée inchangée | tkefmfuw.so |
| `S370FPDU` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `S370FPIB` | format | octets_machine_ibm370_ascii_ebcdic | numérique ou texte lu depuis des octets machine IBM370/ASCII/EBCDIC | Float64 ou Utf8 selon le type de la colonne | valeur stockée inchangée | tkefmfuw.so |
| `S370FPIB` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `S370FPIBU` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `S370FRB` | format | octets_machine_ibm370_ascii_ebcdic | numérique ou texte lu depuis des octets machine IBM370/ASCII/EBCDIC | Float64 ou Utf8 selon le type de la colonne | valeur stockée inchangée | tkefmfuw.so |
| `S370FRB` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `S370FZD` | format | octets_machine_ibm370_ascii_ebcdic | numérique ou texte lu depuis des octets machine IBM370/ASCII/EBCDIC | Float64 ou Utf8 selon le type de la colonne | valeur stockée inchangée | tkefmfuw.so |
| `S370FZD` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `S370FZDB` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `S370FZDL` | format | octets_machine_ibm370_ascii_ebcdic | numérique ou texte lu depuis des octets machine IBM370/ASCII/EBCDIC | Float64 ou Utf8 selon le type de la colonne | valeur stockée inchangée | tkefmfuw.so |
| `S370FZDL` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `S370FZDS` | format | octets_machine_ibm370_ascii_ebcdic | numérique ou texte lu depuis des octets machine IBM370/ASCII/EBCDIC | Float64 ou Utf8 selon le type de la colonne | valeur stockée inchangée | tkefmfuw.so |
| `S370FZDS` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `S370FZDT` | format | octets_machine_ibm370_ascii_ebcdic | numérique ou texte lu depuis des octets machine IBM370/ASCII/EBCDIC | Float64 ou Utf8 selon le type de la colonne | valeur stockée inchangée | tkefmfuw.so |
| `S370FZDT` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `S370FZDU` | format | octets_machine_ibm370_ascii_ebcdic | numérique ou texte lu depuis des octets machine IBM370/ASCII/EBCDIC | Float64 ou Utf8 selon le type de la colonne | valeur stockée inchangée | tkefmfuw.so |
| `S370FZDU` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `SHRSTAMP` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `SIZEK` | format | divers_numerique | numérique | Float64 | valeur stockée inchangée | tkefmfms.so |
| `SIZEKB` | format | divers_numerique | numérique | Float64 | valeur stockée inchangée | tkefmfms.so |
| `SIZEKMG` | format | divers_numerique | numérique | Float64 | valeur stockée inchangée | tkefmfms.so |
| `SIZEKMG` | informat | informat_caractere_general | caractère | Utf8 | — | tkefmich.so |
| `SMFSTAMP` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `TODSTAMP` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `TU` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `VAXRB` | format | binaire_hex_base64 | texte (hexadécimal, base64) ou octets | Utf8 ou Binary | — | tkefmfbi.so |
| `VAXRB` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |
| `WEEKU` | informat | nls_autre | numérique (locale NLS, famille à confirmer) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `WEEKV` | informat | nls_autre | numérique (locale NLS, famille à confirmer) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `WEEKW` | informat | nls_autre | numérique (locale NLS, famille à confirmer) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `XPORTFLT` | format | octets_machine_ibm370_ascii_ebcdic | numérique ou texte lu depuis des octets machine IBM370/ASCII/EBCDIC | Float64 ou Utf8 selon le type de la colonne | valeur stockée inchangée | tkefmfuw.so |
| `XPORTINT` | format | octets_machine_ibm370_ascii_ebcdic | numérique ou texte lu depuis des octets machine IBM370/ASCII/EBCDIC | Float64 ou Utf8 selon le type de la colonne | valeur stockée inchangée | tkefmfuw.so |
| `XYYMMDD` | format | date | date | Date32 | jours depuis 1960-01-01 | tkefmfdt.so |
| `XYYMMDD` | informat | informat_caractere_general | caractère | Utf8 | — | tkefmich.so |
| `YEN` | format | numerique | numérique | Float64 | valeur stockée inchangée | tkefmfms.so |
| `YEN` | informat | informat_caractere_general | caractère | Utf8 | — | tkefmich.so |
| `YYWEEKU` | format | nls_autre | numérique (locale NLS, famille à confirmer) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `YYWEEKV` | format | nls_autre | numérique (locale NLS, famille à confirmer) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `YYWEEKW` | format | nls_autre | numérique (locale NLS, famille à confirmer) | Float64 | valeur stockée inchangée | tkefmfnl.so |
| `ZDB` | informat | informat_binaire | numérique (lu depuis des octets machine) | Float64 | valeur stockée inchangée | tkefmibi.so |

## 2. Inventaire des formats et informats natifs

**Limite** : l'inventaire n'est pas extrait des modules `tkefmt.so`/`tkefmf*.so`/`tkefmi*.so` (décompilation non faite). Il liste 241 noms de la documentation publique de SAS 9.4 et indique, par test dans l'oracle (`PUT(1, NOM.)` et `INPUT("1", NOM.)`, `evidence/fmt_existence.log`), lesquels `sas_small` connaît. La colonne « module » de la spécification d'origine reste **`inconnu`** pour tous.

- **formats présents dans `sas_small`** (110) : `BEST` `BESTX` `BINARY` `COMMA` `COMMAX` `DOLLAR` `DOLLARX` `E` `FRACT` `HEX` `IB` `IBR` `IEEE` `NEGPAREN` `OCTAL` `PD` `PERCENT` `PIB` `PIBR` `PK` `RB` `ROMAN` `SSN` `WORDF` `WORDS` `Z` `ZD` `BESTD` `D` `DATE` `DATEAMPM` `DATETIME` `DAY` `DDMMYY` `DDMMYYB` `DDMMYYC` `DDMMYYD` `DDMMYYN` `DDMMYYP` `DDMMYYS` `DOWNAME` `HHMM` `HOUR` `JULDAY` `JULIAN` `MMDDYY` `MMDDYYB` `MMDDYYC` `MMDDYYD` `MMDDYYN` `MMDDYYP` `MMDDYYS` `MMSS` `MMYY` `MMYYC` `MMYYD` `MMYYN` `MMYYP` `MMYYS` `MONNAME` `MONTH` `MONYY` `NENGO` `NLDATEMN` `QTR` `QTRR` `TIME` `TIMEAMPM` `TOD` `WEEKDATE` `WEEKDATX` `WEEKDAY` `WORDDATE` `WORDDATX` `YEAR` `YYMM` `YYMMC` `YYMMD` `YYMMDD` `YYMMDDB` `YYMMDDC` `YYMMDDD` `YYMMDDN` `YYMMDDP` `YYMMDDS` `YYMMN` `YYMMP` `YYMMS` `YYMON` `YYQ` `YYQC` `YYQD` `YYQN` `YYQP` `YYQR` `YYQRC` `YYQRD` `YYQRN` `YYQRP` `YYQRS` `YYQS` `$` `$CHAR` `$HEX` `$VARYING` `$BINARY` `$OCTAL` `$QUOTE` `$EBCDIC1047` `$F`
- **formats documentés mais absents de `sas_small`** (62) : `EURO` `EUROX` `MGF` `NUMX` `PDJULG` `PDJULI` `PERCENTN` `S370FF` `S370FIB` `S370FPD` `COMMAXD` `NLNUM` `NLMNY` `NLPCT` `DOLLARD` `DOLLARDX` `PVALUE` `PERCENTD` `NEGPARENS` `DTDATE` `DTMONYY` `DTWKDATX` `DTYEAR` `DTYYQC` `EURDFDD` `EURDFDE` `EURDFDN` `EURDFDWN` `EURDFMN` `EURDFMY` `EURDFWDX` `EURDFWKX` `MINGUO` `NLDATE` `NLDATEYQ` `NLDATM` `NLTIME` `PDFDATE` `WEEKU` `WEEKV` `WEEKW` `YYQZ` `E8601DA` `E8601DN` `E8601DT` `E8601DZ` `E8601LZ` `E8601TM` `E8601TZ` `B8601DA` `B8601DN` `B8601DT` `B8601DZ` `B8601LZ` `B8601TM` `B8601TZ` `$UPCASE` `$REVERJ` `$REVERS` `$ASCII` `$MSGCASE` `$N`
- **informats présents dans `sas_small`** (37) : `BEST` `BINARY` `COMMA` `COMMAX` `DOLLAR` `DOLLARX` `E` `HEX` `IB` `IBR` `IEEE` `MDYAMPM` `NEGPAREN` `OCTAL` `PD` `PERCENT` `PIB` `PK` `RB` `TIME` `ZD` `DATE` `DATETIME` `DDMMYY` `JULIAN` `MMDDYY` `MONYY` `NENGO` `YYMMDD` `YYQ` `$` `$CHAR` `$VARYING` `$BINARY` `$OCTAL` `$HEX` `$EBCDIC1047`
- **informats documentés mais absents de `sas_small`** (32) : `EUROX` `NUMX` `PDJULG` `ROMAN` `S370FF` `S370FIB` `S370FPD` `STIMER` `WEIGHT` `Z` `ANYDTDTE` `ANYDTDTM` `ANYDTTME` `EURDFDE` `EURDFDT` `NLDATE` `NLDATM` `NLTIME` `YYMMDDB` `YYMMDDC` `YYMMDDD` `YYMMDDN` `YYMMDDP` `YYMMDDS` `E8601DA` `E8601DT` `E8601TM` `B8601DA` `B8601DT` `B8601TM` `$UPCASE` `$ASCII`

- Preuve : `evidence/fmt_existence.log` (sortie de `corpus/gen/probe/fmt/existence.sas`).
- Confiance : inféré (présence), inconnu (module d'origine, exhaustivité).

### 2.1 Formats NLS et localisés (hors priorité, sans spécification)

Listés seulement, sans algorithme : `NLDATE`, `NLDATEMN`, `NLDATEYQ`, `NLDATM`, `NLTIME`, `NLNUM`, `NLMNY`, `NLPCT`, `NLSTRMON`… (famille `NL*`), `EURDFDD`, `EURDFDE`, `EURDFDN`, `EURDFDT`, `EURDFDWN`, `EURDFMN`, `EURDFMY`, `EURDFWDX`, `EURDFWKX` (formats de date européens localisés), `NENGO`, `MINGUO` (calendriers japonais et taïwanais). Dans l'oracle, `NLDATEMN` et `NENGO` existent (`NLDATEMN` affiche le nom du mois en toutes lettres, `NENGO10.` des dates japonaises de la forme `R.02/01/01`) ; les autres sont absents. Le driver doit les traiter comme **inconnus** : conserver la valeur et le nom du format.

---

## 3. Informats

Les informats (champ « informat » d'une colonne, `10_format_sas7bdat.md` 4.8) décrivent comment SAS a lu une valeur à la création. Ils n'altèrent pas la valeur stockée : le driver peut les ignorer pour la lecture et les recopier à l'écriture. La table de la section 1 donne leur catégorie logique (par exemple `ANYDTDTE` → date). Aucun algorithme d'informat n'est spécifié ici.

---

## 4. Algorithmes d'affichage des formats courants

L'oracle `sas_small` est la seule référence de comportement. Le vérificateur `rea_output/tools/sasfmt.py` implémente ce qui suit et est comparé à `evidence/fmt_display_*.txt` par `rea_output/tools/check_display.py`. Les affirmations de cette section sont `inféré` (preuve runtime, pas de preuve binaire).

### 4.1 Conventions communes
- **Valeur manquante** : le caractère du manquant (`.` pour `.`, `_` pour `._`, la lettre pour `.A`–`.Z`) est aligné à droite dans la largeur `w` ; pour les formats `w.d`, `COMMA`, `DOLLAR`, `Z` le caractère est placé à l'endroit du point décimal, soit dans les `w − d` premiers caractères, les `d` derniers restant blancs (`10.3` : `       .` + 3 blancs) ; `PERCENT` retire 2 caractères de plus.
- **Largeur insuffisante** : si la valeur ne tient pas, SAS réduit d'abord les décimales (`Fw.d`), supprime ensuite les séparateurs de milliers puis le symbole (`COMMAw.d`, `DOLLARw.d`) en conservant `d`, puis bascule vers `BESTw` ; à défaut, `w` astérisques.
- **Arrondi** : à `d` décimales, la moitié s'arrondit **en s'éloignant de zéro** sur la représentation décimale la plus courte du double (`0.005` → `0.01`, `2.5` → `3`, `-0.5` → `-1`).
- **Signe de zéro** : une valeur négative qui s'arrondit à zéro garde son signe (`-0.001` en `8.2` → `   -0.00`).

### 4.2 `w.d`, `BEST`, `E`, `Z`
- **`w.d`** : valeur arrondie à `d` décimales, alignée à droite ; si elle ne tient pas, `d` diminue jusqu'à 0 (`99999.999` en `8.2` → `100000.0`) ; au-delà, repli sur `BESTw`.
- **`BESTw`** : écriture décimale avec autant de chiffres que la largeur le permet (au plus 14 décimales), zéros de fin supprimés (`0.30000000000000004` → `0.3`) ; si la partie entière dépasse la largeur, notation `1.23E6` (mantisse décimale) ou `123E9` (mantisse entière) : la règle de choix entre les deux n'est pas reproduite (section 4.6). `0.00001` s'écrit `0.00001` (pas `1E-5`).
- **`Ew.d`** : `d.ddddE+xx` avec `w − 7` décimales (`e10.` : `1.235E+03`) ; arrondi comme ci-dessus.
- **`Zw.d`** : comme `w.d`, complété à gauche par des zéros, le signe `-` en tête (`z8.2` de `-1` → `-0001.00`).

### 4.3 `COMMA`, `DOLLAR`, `PERCENT`, `NEGPAREN`
- **`COMMAw.d`** : séparateur de milliers `,` ; si la valeur ne tient pas, les séparateurs sont supprimés (`1234567.89` en `comma10.2`).
- **`DOLLARw.d`** : préfixe `$` (le `-` d'un négatif se place après le `$` : `$-1`) ; ordre des replis : sans séparateurs, puis sans `$`, puis `BESTw` (`dollar12.2` de `1E10` → ` 10000000000`).
- **`PERCENTw.d`** : valeur × 100 suivie de `%` ; un positif est suivi d'un blanc réservé, un négatif est entre parenthèses collées aux bords du champ (`(  100%)` pour `-1` en `percent8.`) ; astérisques à la place des chiffres quand la largeur manque (`**%`).
- **`NEGPARENw.d`** : négatif entre parenthèses (`(1)`), positif suivi d'un blanc réservé.

### 4.4 Dates, datetimes, heures
- **Calendrier** : grégorien proleptique **sauf que SAS ne reconnaît pas de 29 février les années multiples de 4 000** (4 000, 8 000, 12 000, 16 000, 20 000) : à partir de ces dates, un jour SAS correspond au lendemain du jour grégorien (vérifié : `2 000 000` → 26 octobre 7435 au lieu du 25 ; `5 000 000` → 17 juillet 15649). Plage valide : **1582-01-01** (jour −138 061) à l'an 20 000 ; avant 1582-01-01 tout est en astérisques ; au-delà de l'an 9 999, la partie année d'un format à 4 chiffres s'affiche `****` (`****-01-01` pour le jour 2 936 548), au-delà de l'an 20 000 tout est en astérisques.
- **`DATEw.`** : `ddMONyy` (w = 7), `ddMONyyyy` (9), `dd-MON-yyyy` (11), `ddMON` (5) ; les largeurs 6, 8 et 10 ne sont pas vérifiées ; `DATE3.` est refusé (`Width specified for format DATE is invalid`). **`YYMMDDw.`/`DDMMYYw.`/`MMDDYYw.`** : années à 4 chiffres si `w ≥ 10`, séparateur `-` (YYMMDD) ou `/` ; suffixes `B` (espace), `C` (`:`), `D` (`-`), `N` (aucun, `w ≥ 8` pour l'année à 4 chiffres), `P` (`.`), `S` (`/`). **`MONYYw.`**, **`YYMONw.`**, **`YYMMw.`** (`2020M01`), **`MMYYw.`**, **`YYQw.`** (`2020Q1`), **`YYQRw.`** (`2020QI`), **`JULIANw.`** (`2020001`), **`JULDAYw.`**, **`YEARw.`**, **`DAY`**, **`MONTH`**, **`QTR`**, **`QTRR`**, **`WEEKDAY`** (dimanche = 1), **`DOWNAME`**/**`MONNAME`** (noms en anglais complets, alignés à droite), **`WORDDATE`** (`January 1, 2020`), **`WEEKDATE`** (`Wed, Jan 1, 2020` ou `Wednesday, January 1, 2020` selon la largeur).
- **`DATETIMEw.d`** : `ddMONyyyy:hh:mm:ss` (`w ≥ 18`), `ddMONyy:hh:mm:ss` (16 et 17), `ddMONyy:hh:mm` (13 à 15), seulement la date (9 à 12) ; les secondes sont **arrondies** à l'entier (`1.5` → `:02`, `0.999` → `:01`), ou à `d` décimales. **`DATEAMPM`**, **`TOD`** (heure d'un datetime, `hh:mm:ss`).
- **`TIMEw.d`** : `h:mm:ss` (`w ≥ 8`), `h:mm` (5 à 7), `h` (< 5) ; arrondi à la seconde (`59.5` → `0:01:00`) ; négatif précédé de `-` ; si `w` manque, l'heure seule en nombre entier. **`HHMMw.d`** (arrondi à la minute), **`HOURw.d`**, **`MMSSw.d`**, **`TIMEAMPMw.`** (`12:00:00 AM`).

- **Mise en œuvre (jalon J3)** : `sas7bdat::temporal::sas_days_to_ymd` applique ce calendrier ; vérifié sur 29 dates `yymmdd10.` de l'oracle (dont `2 000 000` → 7435-10-26, `1 000 000` → 4697-11-28, `5 000 000` → 15649-07-17) et 9 valeurs hors plage rapportées `OutOfRange`. La borne haute (20 000-12-31) est déduite du texte ci-dessus : l'oracle n'a pas de valeur entre `5 000 000` (affichée) et `7 305 000` (astérisques).

### 4.5 `$CHARw.`, `$w.`, `$QUOTEw.`
- `$CHARw.` : les `w` premiers octets (caractères) de la valeur, **espaces de tête et de fin conservés** ; `$w.` : idem sans les blancs de fin ; `$QUOTEw.` : la valeur entre guillemets tronquée pour tenir dans `w` (`"abcdef"` pour `w = 8`). Les formats `$HEX`, `$BINARY`, `$OCTAL`, `$F`, `$VARYING` ne s'affichent pas dans l'oracle comme leur nom l'indique (le chargement de `$HEX` échoue) : comportement non vérifié.

### 4.6 Ce qui n'est pas reproduit (consigné dans `60_inconnues.md`, entrées `F02` à `F06`)
Sur 5 875 cas, 883 divergent, en 6 familles :
1. **Repli en notation E** de `w.d`, `BEST`, `PERCENT`, `COMMA`, `DOLLAR`, `Z` quand le nombre est trop grand pour la largeur : forme sans point décimal (`123E9`, `12E3`) non reproduite.
2. **Très petites valeurs** en `BEST` (`1E-5` s'écrit `0.00001`) et `BEST` à 14 chiffres (`1/3` à 32 de large).
3. **Formats non implémentés** (15) : `NENGO`, `NLDATEMN`, `D`, `BESTD`, `COMMAX`, `DOLLARX`, `HEX`, `BINARY`, `OCTAL` (non conformes dans l'oracle), `WORDS`, `WORDF`, `FRACT`, `WORDDATX`, `WEEKDATX`.
4. **Dates hors plage** : détails des astérisques pour `WORDDATE`, `WEEKDATE`, `YYQR`, `YYMON` pour les années < 1582 ou > 9 999 ; anomalie de l'oracle `NOV` pour `DOWNAME3.` du jour −138 061 ; jours de l'année (`JULDAY`, `JULIAN`) après l'an 4 000 décalés d'un jour.
5. **Heures très grandes** (`HHMM`, `HOUR`, `MMSS` au-delà de 100 000 secondes).
6. **Décimales sur `HHMM`, `MMSS`** (`hhmm5.2`, `mmss8.1`).

- Preuve : `evidence/verification_affichage.txt`, `evidence/fmt_display_*.txt` (sorties brutes de l'oracle).
- Confiance : inféré (4 992 cas identiques sur 5 875) ; inconnu pour les familles non reproduites.

---

## 5. Catalogues de formats utilisateur `.sas7bcat`

**Partie partiellement bloquée.** `PROC FORMAT` est absent de `sas_small` (`ERROR: PROCEDURE FORMAT not found`). Le catalogue du corpus provient du SAS complet : `corpus/files/catalogue/formats.sas7bcat` (24 576 octets, créé par `proc format library=fmtlib; value ff 1='un' 2='deux' 3-high='plus' other='autre'; invalue inf 'a'=1 'b'=2; picture pp low-high='000.00'`), avec la référence `catalogue_cntlout` (`CNTLOUT=`, 7 lignes). Sont aussi disponibles trois catalogues publics (pyreadstat, ReadStat). Aucune paire de catalogues ne différant que d'un élément n'a pu être produite (tâche 5.2 non faite) ; `src/sashelp/core.sas7bcat` n'a pas été ouvert.

### 5.1 En-tête (même famille que `.sas7bdat`)

Le catalogue a le même en-tête que `10_format_sas7bdat.md` section 2, avec : octet 15 du magic = `0x63` (`0x60` pour une table, `0x65` pour un index), type de fichier à l'offset 156 = `CATALOG ` ; `header_length` 8 192 et `page_length` 4 096 (champs aux mêmes offsets que pour une table) ; 4 pages dans les 3 catalogues examinés. La signature de chaque page est `file_code XOR (k + 1)` comme pour une table (vérifié sur 3 catalogues).
- Preuve : `evidence/catalogue_dump_partiel.txt`
- Confiance : inféré

### 5.2 Répertoire
La page 0 (type de page `0x1000`, `block_count` 5 et `subheader_count` 4 pour le catalogue du corpus) se termine par un répertoire d'entrées de 56 octets en fin de page (offsets `0x0F20`, `0x0F58`, `0x0F90`, `0x0FC8`), chacune commençant par `[type : 8 octets ASCII complétés par des espaces][nom : 8 octets]` : `SYSRESR PGBITMAP`, `INFMT   INF     `, `FORMAT  PP      `, `FORMAT  FF      `. Le type vaut `FORMAT` (format numérique `VALUE`/`PICTURE`), `INFMT` (informat) ; `FORMATC`/`INFMTC` (caractère) ne sont pas observés dans le corpus du SAS complet. Le reste de l'entrée (adresse de l'entrée sur les pages suivantes, tailles) n'est pas décodé.
- Confiance : inféré (types et noms) ; inconnu (reste de l'entrée)

### 5.3 Entrées
Chaque format est un bloc sur la page 3 (offsets `0x0048` pour `FF`, `0x0244` pour `@INF`, `0x0440` pour `PP`), commençant par le nom sur 8 octets, un horodatage `f64` de dernière modification et une série de champs de 16 bits **big endian** ; les bornes numériques sont stockées en doubles dont les bits sont modifiés (par exemple `bf f0` pour 1,0 : l'octet de poids fort est complémenté), les libellés par une longueur sur 2 octets puis le texte (`06 00 | 02 00 | un`). La clause `OTHER`, les plages `3-HIGH`, les options (`LENGTH`, `FUZZ`, `MULT`, `FILL`, `DECSEP`…) du `CNTLOUT` ne sont pas décodées. Les libellés `un`, `deux`, `plus`, `autre` sont lus par `rea_output/tools/catalog_dump.py`.
- Confiance : inconnu (structure des entrées) ; inféré (libellés)

### 5.4 Ce qui manque
Le critère « lecteur de référence restituant un catalogue à l'identique de `CNTLOUT=` » n'est pas atteint. Il faut soit la décompilation ciblée des modules de catalogue, soit davantage de catalogues du SAS complet ne différant que d'un élément (une plage, un libellé, une option). Inconnues : `60_inconnues.md`, entrées `F07` à `F09`.

---

## 6. Encodages

### 6.1 Table code d'en-tête → nom SAS → encodage standard

Le code est l'octet 70 de l'en-tête (`10_format_sas7bdat.md` 2.1) ; une copie à l'offset 8 du premier bloc de texte (`00 00 00 <code>`). La table vient de trois sources **publiques** (hypothèses) : ReadStat (74 codes), pandas (65), Parso (111). Les colonnes ReadStat/pandas/Parso donnent le nom que chacune associe au code ; « accord » dit si les sources donnant un nom s'accordent après normalisation des alias. La colonne « standard » donne l'encodage `encoding_rs` (nom WHATWG) ou signale l'absence d'équivalent. Colonne « fichiers du corpus » : nombre de fichiers du corpus portant ce code.

Points d'attention pour le driver :
- **`encoding_rs` n'a pas ISO-8859-1 ni US-ASCII stricts** : le code 29 (`latin1`) et le code 28 (`ascii`) se décodent avec `windows-1252` ; les octets 0x80 à 0x9F diffèrent de la lecture ISO-8859-1 (contrôles C1) ; le code 28 n'interdit pas les octets ≥ 0x80.
- **Pas d'équivalent WHATWG** pour les pages de code IBM (437, 850, 852, 857, 858…), les Mac autres que `macintosh` et `x-mac-cyrillic`, EUC-TW, CP1381, ISO-2022-KR/CN, EBCDIC : le driver doit signaler l'encodage non pris en charge ou utiliser une autre bibliothèque.
- **Désaccords entre sources publiques** (colonne « accord » = NON) : codes 39, 43 à 47, 51, 58, 125, 136, 248 ; les codes 43 à 47, 51 et 58 sont décalés de deux rangs entre ReadStat et pandas d'une part, Parso d'autre part ; ils restent `inféré` et non utilisables sans fichier d'essai.
- **Code 0** : « encodage non spécifié » ; pandas lit `cp1252` (SAS se replie sur wlatin1) ; Parso et ReadStat n'ont pas d'entrée. Le driver doit permettre de forcer l'encodage.
- **Code 204** : « any » (aucun transcodage) ; Parso/ReadStat le traitent comme l'encodage par défaut.
- **Fiabilité du code** (rappel de `10_format_sas7bdat.md` 2.1) : écrire avec `OUTREP=` pose 29 ou 62 même pour des données UTF-8 (`datareader/test16`, `test19`) ; seul le contenu des octets permet de trancher.

| code | ReadStat | pandas | Parso | accord des 3 sources | standard (WHATWG / `encoding_rs`) | fichiers du corpus | confiance |
|---|---|---|---|---|---|---|---|
| 0 | — | cp1252 | — | binaire : emplacement sans nom | `windows-1252` | 2 fichier(s), ex. `external/pandas/cars.sas7bdat` | inféré (binaire `tknlsenc.so`, `tknls_einfo@0x35c0e0` (`evidence/decompile/formats_encodages_findings.md`)) |
| 20 | UTF-8 | utf-8 | UTF-8 | oui (3 sources) | `utf-8` | 172 fichier(s), ex. `external/pandas/many_columns.sas7bdat` | inféré (fichiers + sources publiques) |
| 28 | US-ASCII | ascii | US-ASCII | oui (3 sources) | `windows-1252` (ASCII strict absent de WHATWG : le label « ascii » donne windows-1252) | aucun | inféré (sources publiques seules) |
| 29 | ISO-8859-1 | latin1 | ISO-8859-1 | oui (3 sources) | `windows-1252` (WHATWG remplace ISO-8859-1 par windows-1252 (octets 0x80 à 0x9F différents)) | 36 fichier(s), ex. `external/datareader/test10.sas7bdat` | inféré (fichiers + sources publiques) |
| 30 | ISO-8859-2 | latin2 | ISO-8859-2 | oui (3 sources) | `iso-8859-2` | aucun | inféré (sources publiques seules) |
| 31 | ISO-8859-3 | latin3 | ISO-8859-3 | oui (3 sources) | `iso-8859-3` | aucun | inféré (sources publiques seules) |
| 32 | ISO-8859-4 | latin4 | ISO-8859-4 | oui (3 sources) | `iso-8859-4` | aucun | inféré (sources publiques seules) |
| 33 | ISO-8859-5 | cyrillic | ISO-8859-5 | oui (3 sources) | `iso-8859-5` | aucun | inféré (sources publiques seules) |
| 34 | ISO-8859-6 | arabic | ISO-8859-6 | oui (3 sources) | `iso-8859-6` | aucun | inféré (sources publiques seules) |
| 35 | ISO-8859-7 | greek | ISO-8859-7 | oui (3 sources) | `iso-8859-7` | aucun | inféré (sources publiques seules) |
| 36 | ISO-8859-8 | hebrew | ISO-8859-8 | oui (3 sources) | `iso-8859-8` | aucun | inféré (sources publiques seules) |
| 37 | ISO-8859-9 | latin5 | ISO-8859-9 | oui (3 sources) | `windows-1254` (WHATWG remplace ISO-8859-9 par windows-1254) | aucun | inféré (sources publiques seules) |
| 38 | ISO-8859-10 | latin6 | — | oui (2 sources) | `iso-8859-10` | aucun | inféré (sources publiques seules) |
| 39 | ISO-8859-11 | cp874 | x-iso-8859-11 | binaire : même encodage (TIS-620 / ISO-8859-11, proche de windows-874) | `windows-874` (WHATWG remplace ISO-8859-11 par windows-874) | aucun | inféré (binaire `tknlsenc.so`, `tknls_einfo@0x35c0e0` (`evidence/decompile/formats_encodages_findings.md`); aucun fichier du corpus) |
| 40 | ISO-8859-15 | latin9 | ISO-8859-15 | oui (3 sources) | `iso-8859-15` | 2 fichier(s), ex. `external/pandas/0x40controlbyte.sas7bdat` | inféré (fichiers, contenu non discriminant) |
| 41 | CP437 | cp437 | — | oui (2 sources) | aucun équivalent WHATWG (IANA : CP437) | aucun | inféré (sources publiques seules) |
| 42 | CP850 | cp850 | — | oui (2 sources) | aucun équivalent WHATWG (IANA : CP850) | aucun | inféré (sources publiques seules) |
| 43 | CP852 | cp852 | IBM437 | binaire : cp852 (ReadStat et pandas) ; Parso erroné | aucun équivalent WHATWG (IANA : CP852) | aucun | inféré (binaire `tknlsenc.so`, `tknls_einfo@0x35c0e0` (`evidence/decompile/formats_encodages_findings.md`); aucun fichier du corpus) |
| 44 | CP857 | cp857 | IBM850 | binaire : cp857 (ReadStat et pandas) ; Parso erroné | aucun équivalent WHATWG (IANA : CP857) | aucun | inféré (binaire `tknlsenc.so`, `tknls_einfo@0x35c0e0` (`evidence/decompile/formats_encodages_findings.md`); aucun fichier du corpus) |
| 45 | CP858 | cp858 | IBM852 | binaire : cp858 (ReadStat et pandas) ; Parso erroné | aucun équivalent WHATWG (IANA : CP858) | aucun | inféré (binaire `tknlsenc.so`, `tknls_einfo@0x35c0e0` (`evidence/decompile/formats_encodages_findings.md`); aucun fichier du corpus) |
| 46 | CP862 | cp862 | IBM00858 | binaire : cp862 (ReadStat et pandas) ; Parso erroné | aucun équivalent WHATWG (IANA : CP862) | aucun | inféré (binaire `tknlsenc.so`, `tknls_einfo@0x35c0e0` (`evidence/decompile/formats_encodages_findings.md`); aucun fichier du corpus) |
| 47 | CP864 | cp864 | IBM862 | binaire : cp864 (ReadStat et pandas) ; Parso erroné | aucun équivalent WHATWG (IANA : CP864) | aucun | inféré (binaire `tknlsenc.so`, `tknls_einfo@0x35c0e0` (`evidence/decompile/formats_encodages_findings.md`); aucun fichier du corpus) |
| 48 | CP865 | cp865 | — | oui (2 sources) | aucun équivalent WHATWG (IANA : CP865) | aucun | inféré (sources publiques seules) |
| 49 | CP866 | cp866 | — | oui (2 sources) | `ibm866` | aucun | inféré (sources publiques seules) |
| 50 | CP869 | cp869 | — | oui (2 sources) | aucun équivalent WHATWG (IANA : CP869) | aucun | inféré (sources publiques seules) |
| 51 | CP874 | cp874 | IBM866 | binaire : cp874 / windows-874 (ReadStat et pandas) ; Parso erroné | `windows-874` | aucun | inféré (binaire `tknlsenc.so`, `tknls_einfo@0x35c0e0` (`evidence/decompile/formats_encodages_findings.md`); aucun fichier du corpus) |
| 52 | CP921 | — | — | 1 source seulement | aucun équivalent WHATWG (IANA : CP921) | aucun | inféré (sources publiques seules) |
| 53 | CP922 | — | — | 1 source seulement | aucun équivalent WHATWG (IANA : CP922) | aucun | inféré (sources publiques seules) |
| 54 | CP1129 | — | — | 1 source seulement | aucun équivalent WHATWG (IANA : CP1129) | aucun | inféré (sources publiques seules) |
| 55 | CP720 | cp720 | — | oui (2 sources) | aucun équivalent WHATWG (IANA : CP720) | aucun | inféré (sources publiques seules) |
| 56 | CP737 | cp737 | — | oui (2 sources) | aucun équivalent WHATWG (IANA : CP737) | aucun | inféré (sources publiques seules) |
| 57 | CP775 | cp775 | — | oui (2 sources) | aucun équivalent WHATWG (IANA : CP775) | aucun | inféré (sources publiques seules) |
| 58 | CP860 | cp860 | IBM857 | binaire : cp860 (ReadStat et pandas) ; Parso erroné | aucun équivalent WHATWG (IANA : CP860) | aucun | inféré (binaire `tknlsenc.so`, `tknls_einfo@0x35c0e0` (`evidence/decompile/formats_encodages_findings.md`); aucun fichier du corpus) |
| 59 | CP863 | cp863 | — | oui (2 sources) | aucun équivalent WHATWG (IANA : CP863) | aucun | inféré (sources publiques seules) |
| 60 | WINDOWS-1250 | cp1250 | windows-1250 | oui (3 sources) | `windows-1250` | aucun | inféré (sources publiques seules) |
| 61 | WINDOWS-1251 | cp1251 | windows-1251 | oui (3 sources) | `windows-1251` | aucun | inféré (sources publiques seules) |
| 62 | WINDOWS-1252 | cp1252 | windows-1252 | oui (3 sources) | `windows-1252` | 19 fichier(s), ex. `external/datareader/test1.sas7bdat` | inféré (fichiers + sources publiques) |
| 63 | WINDOWS-1253 | cp1253 | windows-1253 | oui (3 sources) | `windows-1253` | aucun | inféré (sources publiques seules) |
| 64 | WINDOWS-1254 | cp1254 | windows-1254 | oui (3 sources) | `windows-1254` | aucun | inféré (sources publiques seules) |
| 65 | WINDOWS-1255 | cp1255 | windows-1255 | oui (3 sources) | `windows-1255` | aucun | inféré (sources publiques seules) |
| 66 | WINDOWS-1256 | cp1256 | windows-1256 | oui (3 sources) | `windows-1256` | aucun | inféré (sources publiques seules) |
| 67 | WINDOWS-1257 | cp1257 | windows-1257 | oui (3 sources) | `windows-1257` | aucun | inféré (sources publiques seules) |
| 68 | WINDOWS-1258 | cp1258 | windows-1258 | oui (3 sources) | `windows-1258` | aucun | inféré (sources publiques seules) |
| 69 | MACROMAN | mac_roman | x-MacRoman | oui (3 sources) | `macintosh` | aucun | inféré (sources publiques seules) |
| 70 | MACARABIC | mac_arabic | x-MacArabic | oui (3 sources) | aucun équivalent WHATWG (IANA : MACARABIC) | aucun | inféré (sources publiques seules) |
| 71 | MACHEBREW | — | x-MacHebrew | oui (2 sources) | aucun équivalent WHATWG (IANA : MACHEBREW) | aucun | inféré (sources publiques seules) |
| 72 | MACGREEK | mac_greek | x-MacGreek | oui (3 sources) | aucun équivalent WHATWG (IANA : MACGREEK) | aucun | inféré (sources publiques seules) |
| 73 | MACTHAI | — | x-MacThai | oui (2 sources) | aucun équivalent WHATWG (IANA : MACTHAI) | aucun | inféré (sources publiques seules) |
| 75 | MACTURKISH | mac_turkish | x-MacTurkish | oui (3 sources) | aucun équivalent WHATWG (IANA : MACTURKISH) | aucun | inféré (sources publiques seules) |
| 76 | MACUKRAINE | — | x-MacUkraine | oui (2 sources) | aucun équivalent WHATWG (IANA : MACUKRAINE) | aucun | inféré (sources publiques seules) |
| 78 | — | — | IBM037 | 1 source seulement | aucun équivalent WHATWG (IANA : IBM037) | aucun | inféré (sources publiques seules) |
| 87 | — | — | IBM424 | 1 source seulement | aucun équivalent WHATWG (IANA : IBM424) | aucun | inféré (sources publiques seules) |
| 88 | — | — | IBM500 | 1 source seulement | aucun équivalent WHATWG (IANA : IBM500) | aucun | inféré (sources publiques seules) |
| 89 | — | — | IBM-Thai | 1 source seulement | aucun équivalent WHATWG (IANA : IBM-Thai) | aucun | inféré (sources publiques seules) |
| 90 | — | — | IBM870 | 1 source seulement | aucun équivalent WHATWG (IANA : IBM870) | aucun | inféré (sources publiques seules) |
| 91 | — | — | x-IBM875 | 1 source seulement | aucun équivalent WHATWG (IANA : x-IBM875) | aucun | inféré (sources publiques seules) |
| 95 | — | — | x-IBM1025 | 1 source seulement | aucun équivalent WHATWG (IANA : x-IBM1025) | aucun | inféré (sources publiques seules) |
| 98 | — | — | x-IBM1112 | 1 source seulement | aucun équivalent WHATWG (IANA : x-IBM1112) | aucun | inféré (sources publiques seules) |
| 99 | — | — | x-IBM1122 | 1 source seulement | aucun équivalent WHATWG (IANA : x-IBM1122) | aucun | inféré (sources publiques seules) |
| 102 | — | — | IBM424 | 1 source seulement | aucun équivalent WHATWG (IANA : IBM424) | aucun | inféré (sources publiques seules) |
| 103 | — | — | IBM-Thai | 1 source seulement | aucun équivalent WHATWG (IANA : IBM-Thai) | aucun | inféré (sources publiques seules) |
| 104 | — | — | IBM870 | 1 source seulement | aucun équivalent WHATWG (IANA : IBM870) | aucun | inféré (sources publiques seules) |
| 105 | — | — | x-IBM875 | 1 source seulement | aucun équivalent WHATWG (IANA : x-IBM875) | aucun | inféré (sources publiques seules) |
| 108 | — | — | x-IBM1025 | 1 source seulement | aucun équivalent WHATWG (IANA : x-IBM1025) | aucun | inféré (sources publiques seules) |
| 109 | — | — | IBM1026 | 1 source seulement | aucun équivalent WHATWG (IANA : IBM1026) | aucun | inféré (sources publiques seules) |
| 110 | — | — | IBM1047 | 1 source seulement | aucun équivalent WHATWG (IANA : IBM1047) | aucun | inféré (sources publiques seules) |
| 111 | — | — | x-IBM1112 | 1 source seulement | aucun équivalent WHATWG (IANA : x-IBM1112) | aucun | inféré (sources publiques seules) |
| 112 | — | — | x-IBM1122 | 1 source seulement | aucun équivalent WHATWG (IANA : x-IBM1122) | aucun | inféré (sources publiques seules) |
| 117 | — | — | x-IBM937 | 1 source seulement | aucun équivalent WHATWG (IANA : x-IBM937) | aucun | inféré (sources publiques seules) |
| 118 | CP950 | cp950 | x-windows-950 | oui (3 sources) | `big5` (Big5 de WHATWG = cp950 étendu) | aucun | inféré (sources publiques seules) |
| 119 | EUC-TW | — | x-EUC-TW | oui (2 sources) | aucun équivalent WHATWG (IANA : EUC-TW) | aucun | inféré (sources publiques seules) |
| 123 | BIG-5 | big5 | Big5 | oui (3 sources) | `big5` | aucun | inféré (sources publiques seules) |
| 124 | — | — | x-IBM935 | 1 source seulement | aucun équivalent WHATWG (IANA : x-IBM935) | aucun | inféré (sources publiques seules) |
| 125 | GB18030 | gb18030 | GBK | binaire : GBK (ReadStat et pandas erronés ; GB18030 est le code 205) | `gbk` (binaire) | aucun | inféré (binaire `tknlsenc.so`, `tknls_einfo@0x35c0e0` (`evidence/decompile/formats_encodages_findings.md`); aucun fichier du corpus) |
| 126 | WINDOWS-936 | cp936 | x-mswin-936 | oui (3 sources) | `gbk` (gbk de WHATWG = cp936 étendu) | aucun | inféré (sources publiques seules) |
| 128 | CP1381 | — | x-IBM1381 | oui (2 sources) | aucun équivalent WHATWG (IANA : CP1381) | aucun | inféré (sources publiques seules) |
| 129 | — | — | x-IBM939 | 1 source seulement | aucun équivalent WHATWG (IANA : x-IBM939) | aucun | inféré (sources publiques seules) |
| 130 | — | — | x-IBM930 | 1 source seulement | aucun équivalent WHATWG (IANA : x-IBM930) | aucun | inféré (sources publiques seules) |
| 134 | EUC-JP | euc_jp | EUC-JP | oui (3 sources) | `euc-jp` | aucun | inféré (sources publiques seules) |
| 136 | CP932 | cp932 | x-windows-iso2022jp | binaire : cp932 / windows-31j (ReadStat et pandas) ; Parso erroné | `shift_jis` (Shift_JIS de WHATWG = windows-31j (cp932)) | aucun | inféré (binaire `tknlsenc.so`, `tknls_einfo@0x35c0e0` (`evidence/decompile/formats_encodages_findings.md`); aucun fichier du corpus) |
| 137 | CP942 | — | x-IBM942 | oui (2 sources) | aucun équivalent WHATWG (IANA : CP942) | aucun | inféré (sources publiques seules) |
| 138 | CP932 | shift_jis | Shift_JIS | oui (3 sources) | `shift_jis` (Shift_JIS de WHATWG = windows-31j (cp932)) | 1 fichier(s), ex. `files/encodage/encodage_sjis_no.sas7bdat` | inféré (fichiers + sources publiques) |
| 139 | — | — | x-IBM933 | 1 source seulement | aucun équivalent WHATWG (IANA : x-IBM933) | aucun | inféré (sources publiques seules) |
| 140 | EUC-KR | euc-kr | EUC-KR | oui (3 sources) | `euc-kr` (euc-kr de WHATWG = cp949) | aucun | inféré (sources publiques seules) |
| 141 | CP949 | cp949 | x-windows-949 | oui (3 sources) | `euc-kr` (euc-kr de WHATWG = cp949) | aucun | inféré (sources publiques seules) |
| 142 | CP949 | cp949 | x-IBM949 | oui (3 sources) | `euc-kr` (euc-kr de WHATWG = cp949) | aucun | inféré (sources publiques seules) |
| 163 | MACICELAND | mac_iceland | x-MacIceland | oui (3 sources) | aucun équivalent WHATWG (IANA : MACICELAND) | aucun | inféré (sources publiques seules) |
| 167 | ISO-2022-JP | iso2022_jp | ISO-2022-JP | oui (3 sources) | `iso-2022-jp` | aucun | inféré (sources publiques seules) |
| 168 | ISO-2022-KR | iso2022_kr | ISO-2022-KR | oui (3 sources) | aucun équivalent WHATWG (IANA : ISO-2022-KR) | aucun | inféré (sources publiques seules) |
| 169 | ISO-2022-CN | — | x-ISO2022-CN-GB | oui (2 sources) | aucun équivalent WHATWG (IANA : ISO-2022-CN) | aucun | inféré (sources publiques seules) |
| 172 | ISO-2022-CN-EXT | — | x-ISO2022-CN-CNS | oui (2 sources) | aucun équivalent WHATWG (IANA : ISO-2022-CN-EXT) | aucun | inféré (sources publiques seules) |
| 173 | — | — | IBM037 | 1 source seulement | aucun équivalent WHATWG (IANA : IBM037) | aucun | inféré (sources publiques seules) |
| 183 | — | — | IBM01140 | 1 source seulement | aucun équivalent WHATWG (IANA : IBM01140) | aucun | inféré (sources publiques seules) |
| 184 | — | — | IBM01141 | 1 source seulement | aucun équivalent WHATWG (IANA : IBM01141) | aucun | inféré (sources publiques seules) |
| 185 | — | — | IBM01142 | 1 source seulement | aucun équivalent WHATWG (IANA : IBM01142) | aucun | inféré (sources publiques seules) |
| 186 | — | — | IBM01143 | 1 source seulement | aucun équivalent WHATWG (IANA : IBM01143) | aucun | inféré (sources publiques seules) |
| 187 | — | — | IBM01144 | 1 source seulement | aucun équivalent WHATWG (IANA : IBM01144) | aucun | inféré (sources publiques seules) |
| 188 | — | — | IBM01145 | 1 source seulement | aucun équivalent WHATWG (IANA : IBM01145) | aucun | inféré (sources publiques seules) |
| 189 | — | — | IBM01146 | 1 source seulement | aucun équivalent WHATWG (IANA : IBM01146) | aucun | inféré (sources publiques seules) |
| 190 | — | — | IBM01147 | 1 source seulement | aucun équivalent WHATWG (IANA : IBM01147) | aucun | inféré (sources publiques seules) |
| 191 | — | — | IBM01148 | 1 source seulement | aucun équivalent WHATWG (IANA : IBM01148) | aucun | inféré (sources publiques seules) |
| 192 | — | — | IBM01140 | 1 source seulement | aucun équivalent WHATWG (IANA : IBM01140) | aucun | inféré (sources publiques seules) |
| 193 | — | — | IBM01141 | 1 source seulement | aucun équivalent WHATWG (IANA : IBM01141) | aucun | inféré (sources publiques seules) |
| 194 | — | — | IBM01142 | 1 source seulement | aucun équivalent WHATWG (IANA : IBM01142) | aucun | inféré (sources publiques seules) |
| 195 | — | — | IBM01143 | 1 source seulement | aucun équivalent WHATWG (IANA : IBM01143) | aucun | inféré (sources publiques seules) |
| 196 | — | — | IBM01144 | 1 source seulement | aucun équivalent WHATWG (IANA : IBM01144) | aucun | inféré (sources publiques seules) |
| 197 | — | — | IBM01145 | 1 source seulement | aucun équivalent WHATWG (IANA : IBM01145) | aucun | inféré (sources publiques seules) |
| 198 | — | — | IBM01146 | 1 source seulement | aucun équivalent WHATWG (IANA : IBM01146) | aucun | inféré (sources publiques seules) |
| 199 | — | — | IBM01147 | 1 source seulement | aucun équivalent WHATWG (IANA : IBM01147) | aucun | inféré (sources publiques seules) |
| 200 | — | — | IBM01148 | 1 source seulement | aucun équivalent WHATWG (IANA : IBM01148) | aucun | inféré (sources publiques seules) |
| 204 | any | cp1252 | — | binaire : nom SAS `any`, aucun transcodage | `windows-1252` | aucun | inféré (binaire `tknlsenc.so`, `tknls_einfo@0x35c0e0` (`evidence/decompile/formats_encodages_findings.md`); aucun fichier du corpus) |
| 205 | GB18030 | gb18030 | GB18030 | oui (3 sources) | `gb18030` | aucun | inféré (sources publiques seules) |
| 207 | — | — | x-IBM1097 | 1 source seulement | aucun équivalent WHATWG (IANA : x-IBM1097) | aucun | inféré (sources publiques seules) |
| 208 | — | — | x-IBM1097 | 1 source seulement | aucun équivalent WHATWG (IANA : x-IBM1097) | aucun | inféré (sources publiques seules) |
| 211 | — | — | IBM01149 | 1 source seulement | aucun équivalent WHATWG (IANA : IBM01149) | aucun | inféré (sources publiques seules) |
| 212 | — | — | IBM01149 | 1 source seulement | aucun équivalent WHATWG (IANA : IBM01149) | aucun | inféré (sources publiques seules) |
| 227 | ISO-8859-14 | latin8 | — | oui (2 sources) | `iso-8859-14` | aucun | inféré (sources publiques seules) |
| 234 | — | — | x-IBM930 | 1 source seulement | aucun équivalent WHATWG (IANA : x-IBM930) | aucun | inféré (sources publiques seules) |
| 235 | — | — | x-IBM933 | 1 source seulement | aucun équivalent WHATWG (IANA : x-IBM933) | aucun | inféré (sources publiques seules) |
| 236 | — | — | x-IBM935 | 1 source seulement | aucun équivalent WHATWG (IANA : x-IBM935) | aucun | inféré (sources publiques seules) |
| 237 | — | — | x-IBM937 | 1 source seulement | aucun équivalent WHATWG (IANA : x-IBM937) | aucun | inféré (sources publiques seules) |
| 238 | — | — | x-IBM939 | 1 source seulement | aucun équivalent WHATWG (IANA : x-IBM939) | aucun | inféré (sources publiques seules) |
| 242 | ISO-8859-13 | iso8859_13 | ISO-8859-13 | oui (3 sources) | `iso-8859-13` | aucun | inféré (sources publiques seules) |
| 245 | MACCROATIAN | mac_croatian | x-MacCroatian | oui (3 sources) | aucun équivalent WHATWG (IANA : MACCROATIAN) | aucun | inféré (sources publiques seules) |
| 246 | MACCYRILLIC | mac_cyrillic | x-MacCyrillic | oui (3 sources) | `x-mac-cyrillic` | aucun | inféré (sources publiques seules) |
| 247 | MACROMANIA | mac_romanian | x-MacRomania | binaire : même encodage (différence d'orthographe) | aucun équivalent WHATWG (IANA : MACROMANIA) | aucun | inféré (binaire `tknlsenc.so`, `tknls_einfo@0x35c0e0` (`evidence/decompile/formats_encodages_findings.md`); aucun fichier du corpus) |
| 248 | SHIFT_JISX0213 | shift_jisx0213 | JIS_X0201 | binaire : Shift_JISx0213 (équivalence déduite de la description) ; Parso erroné | aucun équivalent WHATWG (IANA : SHIFT_JISX0213) | aucun | inféré (binaire `tknlsenc.so`, `tknls_einfo@0x35c0e0` (`evidence/decompile/formats_encodages_findings.md`); aucun fichier du corpus) |

- Preuve : `evidence/table_encodages.md` (générée par `rea_output/tools/make_encoding_table.py`, qui échoue si un code observé dans le corpus manque), `evidence/encodages_sources_publiques.tsv`.
- Confiance : inféré. Vérifié par fichier : 20, 29, 62, 138 ; 40 et 0 présents mais contenu non discriminant.

**Mise en œuvre (jalon J3, 2026-11-04).** Le cœur Rust embarque la table de ce paragraphe et du 6.3 (`crates/sas7bdat/src/encoding_table.rs`, générée par `ci/gen_tables.py` : 226 codes, dont le code 0 sans nom) : 43 codes ont un encodage `encoding_rs`, les autres n'ont pas d'équivalent et ne se lisent qu'en octets bruts. La table de classification de la section 1 et de 1.1 est générée de la même façon (`format_table.rs`, 629 couples nom/sens). Texte relu identique aux CSV de SAS pour **50 fichiers non UTF-8** du corpus.

### 6.2 Octets invalides et troncature multi-octets

Expériences dans l'oracle (session UTF-8, tables UTF-8, `corpus/gen/probe/enc/`) :

#### octets_invalides
- Emplacement : valeur d'une colonne caractère d'un fichier lu par SAS
- Taille / type : octets quelconques dans un champ de `largeur` octets
- Sémantique : lire une table UTF-8 en session UTF-8 contenant des octets invalides (`FF` isolé, continuation `80` isolée, début de caractère de 3 octets coupé `E6 97`) donne la valeur **telle quelle**, sans substitution, sans erreur et sans avertissement dans le journal ; `PUT` écrit les mêmes octets (`61 62 FF 64 65 66`).
- Valeurs observées : trois lignes modifiées de `enc_inv` (`evidence/enc_lecture_octets_invalides.bin`)
- Écriture : le driver doit conserver les octets bruts (ou les signaler à l'utilisateur) et ne pas échouer
- Preuve : sortie runtime de `corpus/gen/probe/enc/r.sas`
- Confiance : inféré (uniquement UTF-8 → UTF-8)

#### troncature_multi_octets
- Emplacement : affectation d'une chaîne à une colonne caractère trop étroite (écriture par SAS)
- Taille / type : coupure à une limite d'octets
- Sémantique : SAS **coupe à l'octet** sans respecter la limite de caractère : `c = "aé"` dans une colonne de longueur 2 est stockée `61 C3` (caractère `é` coupé après son premier octet) ; `d = "é"` dans une colonne de longueur 1 est stockée `C3` ; `e = "日"` dans une colonne de longueur 3 tient (`E6 97 A5`). La lecture restitue ces octets sans correction.
- Valeurs observées : `evidence/enc_ecriture_troncature.txt`, `evidence/enc_lecture_troncature.bin`
- Écriture : un écrivain peut tronquer à la limite de caractère (plus sûr) ; SAS ne le fait pas
- Preuve : sortie runtime de `corpus/gen/probe/enc/w.sas` puis lecture de la table obtenue
- Confiance : inféré

**Non vérifié** : octets invalides dans un fichier latin1, wlatin1 ou Shift-JIS lu avec transcodage (l'oracle ne gère que UTF-8, latin1, wlatin1 et n'a pas de fichier Shift-JIS à relire) ; comportement de SAS en cas de caractère non représentable lors du transcodage vers UTF-8.


### 6.3 Table native des encodages (décompilation, 2026-10-30)

Source : `tknlsenc.so`, tableau `tknls_einfo@0x35c0e0` (adresse Ghidra = adresse ELF + `0x100000`), 265 enregistrements de `0x420` octets **indexés directement par le code de l'octet 70** de l'en-tête `.sas7bdat` ; chaque enregistrement donne le nom SAS, un nom de type iconv, un ancien nom Java, le CCSID IBM, la page Windows, un nom canonique Java, l'octet blanc (`0x20` ou `0x40`), l'octet de substitution et la famille. Pièces : `evidence/decompile/encodages_natifs.tsv` (265 emplacements, 225 codes nommés), `tknlsenc_bundle.json`. Confiance : `confirmé` pour les codes 20, 29, 62 et 138 (cohérents avec les fichiers du corpus), `inféré` pour les autres (aucun fichier).

- Les 126 codes nommés de la table publique de 6.1 sont **tous** dans le binaire ; le binaire en nomme **99 de plus** (UTF-16/32 : 19, 24 à 27 ; EBCDIC et OpenEdition ; Fujitsu, Hitachi, NEC, DEC ; `saslatin1`, `saslatin2`, `teralatin` ; `latin10`…). La table complète est dans `encodages_natifs.tsv`.
- **Désaccords résolus** : 43 = CP852, 44 = CP857, 45 = CP858, 46 = CP862, 47 = CP864, 51 = CP874, 58 = CP860 (ReadStat et pandas ont raison, Parso est décalé de deux rangs) ; 136 = CP932 (windows-31j) ; **125 = GBK** (et non GB18030, qui est le code 205) ; 248 = Shift_JISx0213 ; 39 = thaï (TIS-620 / ISO-8859-11, proche de windows-874) ; 247 : simple orthographe.
- **Code 204 = `any`** : aucun transcodage (`NLSNeedsTranscoding@0x10ddc0` répond « pas de transcodage » dès qu'un des deux codes vaut 204 ; inféré). **Code 0** : emplacement sans nom et sans règle de compatibilité ; un driver doit le traiter comme « encodage non déclaré », forçable par l'utilisateur, avec repli sur `windows-1252` (code de repli 62 dans le binaire ; inféré). Le `cp1252` de pandas pour le code 0 est une convention du lecteur.
- **Transcodage nécessaire ou non** : même code des deux côtés → non (confirmé pour UTF-8 → UTF-8) ; 29 ↔ 40 et 29 ↔ 62 → non ; 28 (ASCII) avec un encodage compatible ASCII → non (inféré).

### 6.4 Inventaire natif des formats et informats (décompilation, 2026-10-30)

Source : `tkefmf*.so` et `tkefmi*.so` (9 modules ; `tkefmt.so` et `tkefmprf.so` n'ont pas de table de noms). Chaque module exporte un tableau `funcinfo` d'enregistrements de **192 octets** : nom (32 caractères UTF-32 ; préfixe `@` pour un informat), types d'entrée et de sortie, largeur minimale, maximale et par défaut, pointeur de fonction. Pièces : `evidence/decompile/formats_natifs.tsv` (574 lignes : 396 formats et 178 informats, 416 noms distincts). Les 20 largeurs par défaut de `tools/sasfmt.py` concordent avec le binaire (sauf `JULDAY`, absent des modules).

- Sur les 241 couples nom/sens de la section 1 : **186 sont dans les binaires** (130 aussi dans l'oracle : `confirmé` ; 56 hors oracle : `inféré` devient binaire) ; **55 couples (26 noms) n'y sont pas** : informats de date et d'heure (`DATE`, `MMDDYY`, `YYMMDD*`, `ANYDT*`, `E8601*`, `B8601*`…), `$VARYING`, `$EBCDIC1047`, `JULDAY`, `ROMAN`, `EURDF*` ; 17 d'entre eux existent dans l'oracle : ils sont implémentés dans un autre module, hors périmètre (non résolu).
- **263 noms des binaires ne sont pas dans la section 1** (388 couples) : devises `NLMNI*` et `NLMNL*` (40 devises), `EURFR*` et `EURTO*` (50 noms), `IS8601*`, `S370F*` supplémentaires, `$LOGVS*`, `$UUID`, `$BASE64X`, `TODSTAMP`, `SMFSTAMP`… (liste complète dans le TSV). Ils doivent être ajoutés à la table de classification lors de l'implémentation (`formats::classify`, jalon J3).
- Hors périmètre, non traités : algorithmes d'affichage (champs à partir de `+0xA0` des enregistrements, TF02 à TF04), catalogues.

### 6.5 Transcodage sur octets invalides (décompilation partielle, 2026-10-30)

Source : `tknlsicv.so`. UTF-8 vers monooctet : un octet ou une séquence invalide donne U+FFFD puis un octet de substitution (`0x1A` en ASCII, `0x3F` en EBCDIC), la conversion **continue** et renvoie un statut d'erreur (`0x803FE808`) et un drapeau à l'appelant ; variantes d'écriture `\uXXXX` ou `?`. Monooctet vers UTF-8 : table de 256 points de code, pas d'erreur par octet. Non traité : sources multioctets (Shift-JIS) et contenu des tables. Tout est `inféré`, sauf UTF-8 → UTF-8 (`confirmé`, octets conservés sans substitution, 6.2).

---

## 7. Cibles de décompilation

À traiter quand la confirmation NULL sera consignée ; modules : `tkefmt.so`, `tkefmf*.so`, `tkefmi*.so`, `tknlsenc.so`, `tknlsicv.so`, `src/nls/`, modules de catalogue.

| N° | Question |
|---|---|
| TF01 | Inventaire des formats et informats natifs : **extrait 2026-10-30** (6.4, `evidence/decompile/formats_natifs.tsv`), 55 couples de la section 1 absents des 11 modules |
| TF02 | Algorithmes de repli en notation E (`BEST`, `w.d`) et règles de 14 chiffres de `BEST` |
| TF03 | Formats absents de l'oracle : `E8601*`, `B8601*`, `NL*`, `EURDF*`, `NENGO`, `WORDS`, `FRACT`, `HEX`, `BINARY`… |
| TF04 | Structure des entrées d'un catalogue (plages, bornes, `OTHER`, `PICTURE`, options, `FORMATC`, `INFMTC`) |
| TF05 | Table des encodages : **résolu 2026-10-30** (6.3) ; codes 0 et 204 résolus en partie |
| TF06 | Transcodage sur octets invalides : **partiel 2026-10-30** (6.5) |

---

## 8. Renvois
- Écriture d'un `.sas7bdat` (noms de formats et encodage à écrire) → `specify-sas7bdat-writing`.
- Offsets des champs format, informat, label et code d'encodage dans le fichier → `10_format_sas7bdat.md`.
- Type Arrow et schéma → `design-rust-driver` (qui lit cette table de classification).
