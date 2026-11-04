# 20 — Écriture de fichiers `.sas7bdat` relus par SAS

Spécification issue du changement `specify-sas7bdat-writing` (prompt 05), date 2026-10-30. Elle s'appuie sur `10_format_sas7bdat.md` (format de lecture), `15_compression.md` (compression) et `30_formats_encodages.md` (formats, encodages) ; les noms de sous-en-têtes, de champs et le pseudo-code sont en anglais comme dans ces documents. Le document est autosuffisant : les preuves sont citées par identifiant (journal, fichier du corpus + offset, ligne de `evidence/write/mutations.tsv`), jamais par extrait de code décompilé.

## 0. Statut, portée et niveaux de confiance

- **Cible** : fichiers de SAS 9.4 TS1M7 (`9.0401M7`), Linux x86-64 (64 bits, little endian), moteur V9. Les familles 32 bits, big endian, Windows, HP-UX… se lisent (`10_format_sas7bdat.md`) mais ne sont pas écrites ici.
- **Objectif** : un fichier que SAS relit sans `ERROR` ni `WARNING` (`PROC CONTENTS`, `PROC PRINT`, étape DATA) et que pandas, ReadStat (pyreadstat), Parso relisent avec les mêmes données et métadonnées. L'identité octet à octet avec SAS n'est pas visée.
- **Aucune décompilation n'a été faite** : la confirmation interne NULL de l'analyse d'interopérabilité (directive 2009/24/CE, art. 6) n'est pas consignée dans `STATE.md`. Aucune règle n'est donc `confirmé` (la définition exige une preuve binaire `fonction@adresse` en plus de la preuve fichier). Les règles portent `inféré` (écart nul sur les fichiers du corpus et acceptation par le relecteur SAS) ou `inconnu`. Les points non résolus sont listés en section 9 et dans `60_inconnues.md` (entrées W01 à W09).
- **SAS n'est utilisé que comme relecteur** : l'oracle `sas_small` (SAS 9.4M7 restreint, conteneur Linux amd64) lit les fichiers écrits par `rea_output/tools/ref_writer.py`. Il ne sait pas écrire de table compressée, ni charger les informats (`WARNING: The informat @X could not be loaded`, avertissement identique pour un fichier écrit par le SAS complet : cas `OK*` de la matrice).
- **Portée des preuves « libre »** : un champ est dit `libre` quand SAS 9.4M7 (`sas_small`) relit le fichier sans changement de comportement après mutation, pour `PROC CONTENTS`, `PROC PRINT` et une étape DATA en lecture. Les autres procédures et les autres versions de SAS (ajout de lignes, `PROC DATASETS`, tri, versions 9.3 ou Viya) n'ont pas été essayées : le doute est consigné (inconnue W06).

## 1. Lecteurs et résultats

| Lecteur | Version | Usage |
|---|---|---|
| SAS | 9.4 TS1M7 (`sas_small`, conteneur Docker linux/amd64) | `PROC CONTENTS`, `PROC PRINT`, `data _null_; set lib.t` écrivant un CSV ; journal sans `ERROR`/`WARNING` |
| pandas | 2.3.3 (`pandas.read_sas`) | données et valeurs manquantes ; une chaîne vide est rendue `NaN` (comportement de pandas, accepté par la comparaison) |
| pyreadstat | 1.2.9 (ReadStat) | données et métadonnées |
| Parso | 2.0.14 (JVM 26, `ParsoDump.java`) | données ; dates rendues en `java.util.Date` : cas de dates non applicables |
| `ref_reader.py` | écrit depuis `10_format_sas7bdat.md` | lecteur de référence du projet |

Matrice complète : `rea_output/tools/write_matrix.md` (cas × lecteur → OK/KO + journal SAS). Dernière rejouée complète (tâche 5.12, après la dernière correction de la spec) : **49 cas × 5 lecteurs = 245 cellules : 224 OK, 9 KO connus (limites documentées), 2 non applicables (dates dans Parso), 10 non faisables (2 cas), 0 KO.** Aucun cas précédemment OK n'est passé KO. Points à retenir :

- tous les cas non compressés (colonnes num 3 à 8, manquants spéciaux, labels, formats, informats, noms de 32 octets, valeurs non ASCII, 3 000 lignes sur plusieurs pages, pages de 4 096 à 1 048 576 octets, 700 colonnes, 0 ligne, latin1, wlatin1) passent les lecteurs disponibles ;
- les tables compressées de 0 à 10 000 lignes, `CHAR` et `BINARY`, sur une ou plusieurs pages (pages de 4 096 et 65 536 octets), passent les cinq lecteurs, `sas_small` compris (cas `w11_*`, 57 cas au total, 0 KO) ; la limite de 255 lignes de la première version de l'écrivain venait d'un champ de la page marqueur écrit sur un octet au lieu de deux (W01, résolue le 2026-11-02, section 4.2 point 7) ;
- limites propres à un lecteur ou à SAS (« KO connu ») : pyreadstat refuse le code d'encodage 29 (`w12_latin1`) et les colonnes de 32 768 octets ; SAS tronque ou refuse un nom de 33 octets, un label de 257 octets et une colonne de 32 768 octets (section 8) ;
- non faisables : table de plus de 2 Go (`w15_gt2gb`) et page de 1 024 octets (`w13_page1024`, métadonnées trop grandes pour la page 0).

## 2. Notation

- Offsets en décimal ou `0x`, relatifs au début de l'objet nommé ; `u16/u32/u64` little endian ; `[a, b)` = intervalle semi-ouvert.
- `P` = taille de page (défaut 65 536) ; `HL` = taille d'en-tête de fichier (**HL = P** sur les 265 fichiers 64 bits du corpus, section 4.1).
- `n` = nombre de colonnes, `N` = nombre de lignes, `RL` = `row_length`.
- Classes de champ : `calculé` (formule), `constante` (valeur fixe), `libre` (SAS ignore la valeur ; l'écrivain donne la valeur conseillée). Confiance : `inféré` ou `inconnu` (jamais `confirmé`, section 0).

## 3. Entrées de l'écrivain

Une table décrite par : encodage cible (code d'octet 70 : 20 UTF-8, 29 latin1, 62 wlatin1 ; seuls ces trois sont écrits, F10 de `60_inconnues.md`) ; taille de page `P` ; liste ordonnée de colonnes `(name, type, length, label, format, informat)` ; lignes.

Contraintes vérifiées par SAS (section 8) : `name` ≤ 32 octets dans l'encodage cible ; `label` ≤ 256 octets ; numérique : longueur 3 à 8 ; caractère : longueur 1 à 32 767 ; un nom de format ou d'informat est écrit tel quel (SAS signale un format utilisateur introuvable à l'affichage).

## 4. Procédure d'écriture, pas à pas

### 4.1 Table non compressée

1. **Disposition des colonnes.** Les colonnes numériques d'abord, triées par longueur décroissante (ordre de création pour les égalités), puis les colonnes caractère dans l'ordre de création. Les offsets sont cumulés à partir de 0. `RL` = somme des longueurs, **arrondie au multiple de 8 supérieur s'il existe au moins une colonne numérique** (sinon la somme). Les valeurs numériques de longueur `k < 8` sont les `k` octets de poids fort du double IEEE little endian (`raw[8-k:]`). Un manquant spécial s'écrit `00 00 00 00 00 t FF FF` avec `t` = `FE` pour `.`, `FF` pour `_`, `FD − (lettre − 'A')` pour `A` à `Z` (`10_format_sas7bdat.md`). Une valeur caractère est remplie à droite par des blancs (`0x20`).
2. **Textes.** Tous les textes (noms, noms de formats, d'informats, labels) sont placés dans des blocs `ColumnText` ; un texte vide a la référence `(0, 0, 0)`. Le premier bloc commence par un préfixe de 36 octets : octets 0 à 7 = longueur du bloc (u16) puis zéros, octets 8 à 11 = `00 00 00 <code d'encodage>`, octets 12 à 27 = 16 blancs, octets 28 à 35 = `DATASTEP`. Chaque texte, encodé dans l'encodage cible, est écrit à la suite sur un multiple de 4 octets (remplissage par zéros) ; sa référence est `(index du bloc, offset dans le bloc, longueur en octets)`. Un nouveau bloc (préfixe de 8 octets) est ouvert quand le bloc courant dépasserait 32 740 octets. Ordre d'ajout : pour chaque colonne, son nom, son format, son informat, son label.
3. **Sous-en-têtes de métadonnées**, dans cet ordre : `RowSize`, `ColumnSize`, `SubheaderCounts`, un `ColumnText` par bloc, `ColumnName`, `ColumnAttributes`, puis un `FormatAndLabel` par colonne. **Le sous-en-tête `ColumnList` n'est pas écrit** : SAS l'écrit, mais les cinq lecteurs relisent sans lui (inconnue W05). Contenu octet par octet : section 5.
4. **Page 0, de type mixte (`0x0200`)** : en-tête de page de 40 octets, table de `sc = S + 1` pointeurs de 24 octets (`S` sous-en-têtes de métadonnées dans l'ordre ci-dessus, plus un pointeur terminal), zone de lignes, zone libre, puis les sous-en-têtes empilés depuis la fin de la page (le premier, `RowSize`, est le plus proche de la fin ; le suivant le précède, etc.).
   - `start = 40 + 24·sc` (début des lignes, multiple de 8) ; `minoff = P − Σ longueur(sous-en-têtes)` ; `avail = minoff − start`.
   - Capacité en lignes de la page 0 : `cap = max { c : c·RL + ⌈c/8⌉ ≤ avail }` (le terme `⌈c/8⌉` réserve la zone d'indicateurs de lignes supprimées). La formule redonne la valeur écrite par SAS dans `RowSize+0x78` sur les 61 fichiers 64 bits non compressés du corpus où elle se teste (0 écart).
   - La page 0 reçoit `n0 = min(N, cap)` lignes à partir de l'offset `start`, rangées l'une après l'autre.
   - `free_bytes` = `minoff − (start + n0·RL) − ⌈cap/8⌉` (valeur de SAS ; ignorée par SAS à la relecture, section 5.2).
   - `block_count = sc + n0`, `subheader_count = sc`.
   - Pointeurs : pour chaque sous-en-tête `(offset, longueur, 0, type)` avec `type = 1` pour `ColumnText`, `ColumnName`, `ColumnAttributes` et `type = 0` pour `RowSize`, `ColumnSize`, `SubheaderCounts`, `FormatAndLabel` ; octets 18 à 23 à zéro. Pointeur terminal : `(minoff, 0, 1, 0)`.
5. **Pages suivantes, de type données (`0x0100`)** : `rmax = max { r : r·RL + ⌈r/8⌉ + 40 ≤ P }` lignes par page (formule vérifiée sur les mêmes 61 fichiers, y compris pages de 4 096 et 8 192 octets). En-tête de 40 octets, lignes à partir de l'offset 40, `block_count` = nombre de lignes de la page, `subheader_count` = 0, `free_bytes = P − 40 − r·RL − ⌈rmax/8⌉` où `r` est le nombre de lignes de la page. Les lignes restantes sont réparties dans l'ordre, `rmax` par page, la dernière page recevant le reste.
6. **Signatures de page.** Soit `file_code` un u32 quelconque (octets 328 à 331 de l'en-tête de fichier). Les 4 premiers octets de la page `k` (`k` = 0 pour la première) valent `file_code XOR (k + 1)` (u32 little endian).
7. **Champs de `RowSize` qui dépendent des pages** (après avoir réparti les lignes) : `+0x78 = cap` ; cinq paires de localisateurs `(page, position)` à partir de `+0x200` (une paire = deux u64) : `(1, 2)`, `(1, S_fl_dernier + 1)`, `(1, sc + 1)`, `(nombre de pages, nombre de blocs de la dernière page)`, `(1, S_fl_premier + 1)`, où `S_fl_premier` et `S_fl_dernier` sont les indices 0-based du premier et du dernier `FormatAndLabel` et « nombre de blocs de la dernière page » vaut `sc + n0` pour une table d'une seule page et le nombre de lignes de la dernière page sinon ; `+0x2FE` (u16) = `cap + sc` si toutes les lignes tiennent sur la page 0, sinon `rmax` (valeur de SAS, ignorée par SAS à la relecture) ; `+0x1B8` (u32) = signature de la page 0. Les paires de localisateurs sont **vérifiées** par SAS (section 5.4).
8. **En-tête de fichier** (`HL = P` octets, zéros sauf les champs de la section 5.1).

### 4.2 Table compressée (`COMPRESS=CHAR` ou `COMPRESS=BINARY`)

Même démarche, avec les différences suivantes (toutes vérifiées par la relecture SAS pour 1 à 10 000 lignes) :

1. **Décision.** Si la somme des longueurs de colonnes est inférieure à 16 octets, la table est écrite **sans compression** (minimum technique : une ligne brute de moins de 16 octets serait prise pour un sous-en-tête par les lecteurs et un flot compressé plus long que la ligne est refusé par SAS, cas `w11_*_mini` et `w11_*_len15`). SAS lui-même ne compresse jamais les tables de moins de 21 octets par ligne et désactive la compression quand elle n'apporte pas de gain (C02 de `60_inconnues.md`, R12 de `15_compression.md`) : un driver qui veut imiter SAS fixe son seuil à 21.
2. **`RL` = somme des longueurs de colonnes sans arrondi.** Les colonnes sont disposées comme en 4.1.
3. **Texte** : le préfixe du premier bloc `ColumnText` fait **44 octets** : le littéral `SASYZCRL` (CHAR) ou `SASYZCR2` (BINARY) s'insère aux octets 12 à 19, les 16 blancs puis `DATASTEP` se décalent de 8 octets (blancs en 20 à 35, `DATASTEP` en 36 à 43). Tous les textes suivants sont donc décalés de 8 octets.
4. **Lignes** : chaque ligne devient un sous-en-tête. `ref_compress.rle_compress` (CHAR) ou `rdc_compress` (BINARY) donne le flot ; si le flot est strictement plus court que la ligne il est stocké avec l'octet de compression `4`, sinon la ligne brute est stockée avec l'octet `0` (jamais pour une ligne de moins de 16 octets ni dont le début est l'une des signatures de sous-en-têtes : on stocke alors le flot, même plus long). Le pointeur d'une ligne est `(offset, longueur stockée, octet de compression, 1)`.
5. **Pointeurs de métadonnées** : tous de type `1` (octet de compression `0`).
6. **Pages** : toutes de type `0x0000`. La page 0 porte les métadonnées puis autant de lignes que possible : une ligne est ajoutée si `données_utilisées + longueur + 24·(pointeurs + 2) + 40 ≤ P` (le `+2` compte le pointeur de la ligne et le pointeur terminal). Les pages suivantes ne portent que des lignes. Chaque page se termine par un pointeur terminal `(40 + 24·sc, zone libre, 1, 1)`, `sc` comptant ce pointeur ; `free_bytes` = longueur de la zone libre. `block_count = subheader_count = sc`.
7. **Page marqueur `0x9000`** (obligatoire pour `sas_small`, section 5.6) : insérée juste après la première page qui contient des lignes (page 0 en général). En-tête de page de 40 octets : `[0..3]` signature de page, `[24..31] = 1`, `[32..33] = 0x9000`, `[34..35] = n + 1`, `[36..37] = n`, `[38..39] = A − 18·n − 48` avec `A = min(P, 32 767)` et `n` = nombre de pages qui contiennent des lignes ; puis `n` pointeurs de 24 octets entièrement nuls ; puis, dans `[A − 18·n, A)`, `n` enregistrements de 18 octets **dans l'ordre décroissant des pages** : `u64` **big endian** = lignes cumulées jusqu'à cette page incluse, `u64` = numéro de page 1-based dans le fichier (le marqueur compte), `u16` = indice du dernier sous-en-tête de la page (blocs − 1, terminal compris).
8. **`RowSize`** : `+0x08 = 0x131`, `+0x20 = 0x22303A`, `+0x40 = 2`, `+0x24A = 1`, zone de références `+0x2A0..+0x2C7` propre aux tables compressées (qui contient la référence `(0, 12, 8)` du littéral) ; `+0x10 = N + S + 2` ; localisateurs : `(1, 2)`, `(1, S_fl_dernier + 1)`, `(1, S + 1)`, `(numéro de la dernière page de lignes, pointeurs de cette page sans le terminal)`, `(1, S_fl_premier + 1)` ; `+0x78 = cap` calculé comme en 4.1 avec le `RL` non arrondi ; `+0x2FE = 0`.
9. **Limite** : aucune limite de lignes propre à la compression (70 000 lignes relues par SAS ; le cumul est sur 8 octets). La limite de 255 lignes de la première version (W01) venait d'un cumul écrit sur un octet.

### 4.3 Ordre de contrôle avant écriture

Refuser (avec message) : nom de plus de 32 octets, label de plus de 256 octets, colonne caractère de plus de 32 767 octets, métadonnées qui ne tiennent pas sur la page 0 (`Σ sous-en-têtes + 24·sc + 40 > P`, section 8), encodage hors {20, 29, 62}.

## 5. Table des champs : `calculé`, `constante`, `libre`

Chaque ligne a été **mutée** dans un fichier valide (tout à zéro, octets aléatoires, tout à `0xFF`) puis relue par SAS (`PROC CONTENTS` + étape DATA) : `rea_output/tools/mutate_write.py`, résultats `rea_output/evidence/write/mutations.tsv` (une ligne par champ et mutation, journal SAS de chacune dans `evidence/write/mut/`). Pour un champ qui vaut déjà zéro, la mutation « zéro » est remplacée par « valeur + 1 ». Réaction : « ignoré » = journal propre et mêmes données et métadonnées ; « modifié » = journal propre mais données ou métadonnées différentes ; « rejeté » = `ERROR`, `WARNING`, exception, ou SAS ne termine pas. Classes :

- `calculé` : une formule donne la valeur ; SAS rejette ou relit autrement une valeur fausse.
- `constante` : valeur fixe observée dans tous les fichiers ; SAS rejette (ou n'accepte que zéro) toute autre valeur, ou le champ est lu par les lecteurs publics.
- `libre` : SAS relit sans changement après mutation ; l'écrivain donne la valeur conseillée (celle de SAS, pour rester proche des fichiers réels).

Confiance de toutes les lignes : `inféré` (preuve fichier + mutation, pas de preuve binaire, section 0). La **vérification d'exhaustivité** (tâche 2.2) est `rea_output/tools/diff_recopies.py` : il réécrit 13 fichiers du corpus (non compressés, compressés CHAR et BINARY) à partir de leurs lignes et métadonnées et compare octet à octet ; toute différence tombe dans une plage listée ci-dessus (0 « recopié sans explication », `evidence/write/diff_recopies.txt`).

### 5.1 En-tête de fichier (64 bits, little endian)

| offset | taille | champ | valeur ou formule | classe | mutations (SAS) | remarque |
|---|---|---|---|---|---|---|
| 0 | 32 | magic number | `00`×12 puis `c2 ea 81 60 b3 14 11 cf bd 92 08 00 09 c7 31 8c 18 1f 10 11` | **constante** | `h.magic` 0/alea/FF : ignoré/rejeté/rejeté | tout zéro toléré, valeurs aléatoires rejetées (« created by a more recent release ») |
| 32 | 1 | drapeau 64 bits | `33` | **constante** | `h.32` 0/alea/FF : ignoré/ignoré/ignoré | ignoré par SAS (qui lit la copie 56 à 63) mais lu par les lecteurs publics |
| 33 | 2 | — | `22 00` | **libre** | `h.33_34` 0/alea/FF : ignoré/ignoré/ignoré | valeur conseillée `22 00` |
| 35 | 1 | alignement `a1` = 4 | `33` | **constante** | `h.35` 0/alea/FF : ignoré/ignoré/ignoré | ignoré par SAS (copie en 59), lu par les lecteurs publics |
| 36 | 1 | — | `33` | **libre** | `h.36` 0/alea/FF : ignoré/ignoré/ignoré | valeur conseillée `33` |
| 37 | 1 | boutisme (`01` = little endian) | `01` | **constante** | `h.37` 0/alea/FF : ignoré/ignoré/ignoré | ignoré par SAS (copie en 61), lu par les lecteurs publics |
| 38 | 1 | — | `02` | **libre** | `h.38` 0/alea/FF : ignoré/ignoré/ignoré | valeur conseillée `02` |
| 39 | 1 | — | `31` | **libre** | `h.39` 0/alea/FF : ignoré/ignoré/ignoré | valeur conseillée `31` |
| 40 | 1 | code de plateforme | `01` | **libre** | `h.platform` 0/alea/FF : ignoré/ignoré/ignoré | `01` = Unix ; la représentation affichée (`LINUX_X86_64`…) ne dépend pas de cet octet |
| 47 | 1 | — | `14` | **libre** | `h.47` 0/alea/FF : ignoré/ignoré/ignoré | valeur conseillée `14` |
| 50 | 2 | — | `03 01` | **libre** | `h.50_51` 0/alea/FF : ignoré/ignoré/ignoré | valeur conseillée `03 01` |
| 52 | 4 | — | `18 1f 10 11` | **constante** | `h.52_55` 0/alea/FF : ignoré/rejeté/rejeté | zéro toléré, valeurs aléatoires rejetées (« not a SAS data set ») |
| 56 | 8 | copie des octets 32 à 39 | = octets 32 à 39 | **calculé** | `h.56_63` 0/alea/FF : rejeté/rejeté/rejeté | c'est la copie que SAS lit : toute différence est rejetée |
| 64 | 1 | représentation des données | `01` | **constante** | `h.64` 0/alea/FF : ignoré/modifié/modifié | une autre valeur change `Data Representation` (affiche `Default`) |
| 65 | 4 | — | `33 01 23 33` | **constante** | `h.65_68` 0/alea/FF : ignoré/rejeté/rejeté | zéro toléré, valeurs aléatoires rejetées |
| 70 | 1 | code d'encodage | `20` (UTF-8), `29` (latin1), `62` (wlatin1) | **calculé** | `h.enc` 0/alea/FF : ignoré/modifié/rejeté | choisit l'encodage affiché ; 0 est lu comme l'encodage de la session (UTF-8), une valeur aléatoire donne par exemple `pcoem850`, `FF` est rejeté |
| 71 | 1 | — | `14` | **libre** | `h.71` 0/alea/FF : ignoré/ignoré/ignoré | valeur conseillée `14` |
| 73 | 1 | — | `20` | **constante** | `h.73` 0/alea/FF : ignoré/rejeté/rejeté | zéro toléré, valeurs aléatoires rejetées (« created by a more recent release ») |
| 74 | 2 | — | `03 01` | **constante** | `h.74_75` 0/alea/FF : rejeté/rejeté/rejeté | toute autre valeur rejetée (« a bug in SAS » ou « more recent release ») |
| 84 | 8 | type de fichier | `SAS FILE` | **constante** | `h.sas_file` 0/alea/FF : rejeté/rejeté/rejeté | toute autre valeur rejetée |
| 92 | 32 | nom de table | 32 blancs | **libre** | `h.name` 0/alea/FF : ignoré/ignoré/ignoré | SAS affiche le nom du fichier |
| 156 | 8 | type de membre | `DATA    ` | **libre** | `h.type` 0/alea/FF : ignoré/ignoré/ignoré | ignoré à la relecture |
| 168 | 8 | date de création (double, secondes depuis 1960-01-01) | heure d'écriture + 315 619 200 | **libre** | `h.dates` 0/alea/FF : ignoré/ignoré/ignoré | 168 à 183 : création et modification |
| 176 | 8 | date de modification | = date de création | **libre** | `h.dates` 0/alea/FF : ignoré/ignoré/ignoré | — |
| 200 | 4 | taille de l'en-tête `HL` | `P` | **calculé** | `h.hdrlen` 0/alea/FF : rejeté/rejeté/rejeté | HL = P sur les 265 fichiers 64 bits du corpus ; toute autre valeur rejetée |
| 204 | 4 | taille de page `P` | `P` | **calculé** | `h.pagelen` 0/alea/FF : rejeté/rejeté/rejeté | puissance de 2 ou non (5 000 essayé, section 8) |
| 208 | 8 | nombre de pages | nombre de pages écrites (marqueur compris) | **calculé** | `h.pagecount` 0/alea/FF : rejeté/rejeté/rejeté | rejeté s'il ne correspond pas à la longueur du fichier |
| 224 | 8 | release de création | `9.0401M7` | **libre** | `h.release` 0/alea/FF : modifié/modifié/modifié | texte affiché dans `Release Created` ; toute valeur relue |
| 232 | 5 | hôte de création | `Linux` | **libre** | `h.host` 0/alea/FF : modifié/modifié/modifié | texte affiché dans `Host Created` |
| 248 | 16 | version du système | `6.18.48-107.148.` | **libre** | `h.os_version` 0/alea/FF : ignoré/ignoré/ignoré | — |
| 280 | 6 | machine | `x86_64` | **libre** | `h.machine` 0/alea/FF : ignoré/ignoré/ignoré | — |
| 296 | 16 | zone dérivée de la date | octets 296 à 299 = 4 octets de poids faible de la date de création ; 300 à 311 = 3 fois ces 4 octets XOR `4c 4e 6f 65` | **calculé** | `h.tail_296` 0/alea/FF : rejeté/rejeté/rejeté | toute mutation est rejetée (« invalid or missing READ password ») : contrôle d'intégrité |
| 328 | 4 | code de fichier `file_code` | u32 aléatoire | **calculé** | `h.file_code` 0/alea/FF : ignoré/rejeté/rejeté | zéro désactive le contrôle des signatures de page ; sinon page `k` signée `file_code XOR (k+1)` |
| 336 | 8 | date | = date de création | **libre** | `h.date_336` 0/alea/FF : ignoré/ignoré/ignoré | — |

`HL` octets (= `P`) : les octets non listés sont à zéro.

### 5.2 En-tête de la page (octets 0 à 39 de chaque page)

| offset | taille | champ | valeur ou formule | classe | mutations (SAS) | remarque |
|---|---|---|---|---|---|---|
| 0 | 4 | signature de la page | `file_code XOR (k+1)` | **calculé** | `p.sig` 0/alea/FF : rejeté/rejeté/rejeté | rejeté si faux (« Expecting page 1, got page -1 ») |
| 4 | 20 | — | zéros | **libre** | `pg.4_23` 0/alea/FF : ignoré/ignoré/ignoré | — |
| 24 | 8 | `free_bytes` | formules 4.1 et 4.2 | **libre** | `pg.free` 0/alea/FF : ignoré/ignoré/ignoré | ignoré par SAS ; valeur conseillée = valeur de SAS |
| 32 | 2 | type de page | `0x0200` (mixte), `0x0100` (données), `0x0000` (compressée), `0x9000` (marqueur) | **calculé** | `pg.type` 0/alea/FF : rejeté/rejeté/rejeté | — |
| 34 | 2 | `block_count` | section 4 | **calculé** | `pg.bc` 0/alea/FF : rejeté/rejeté/rejeté | rejeté si faux |
| 36 | 2 | `subheader_count` | section 4 | **libre** | `pg.sc` 0/alea/FF : ignoré/ignoré/ignoré | ignoré par SAS ; à écrire exact (les lecteurs publics l'utilisent) |
| 38 | 2 | — | zéros | **libre** | `pg.38_39` 0/alea/FF : ignoré/ignoré/ignoré | la page marqueur de SAS y porte des restes de mémoire (`BD 7F`) |

### 5.3 Pointeurs de sous-en-têtes (24 octets chacun, à partir de l'offset 40 de la page)

| offset | taille | champ | valeur ou formule | classe | mutations (SAS) | remarque |
|---|---|---|---|---|---|---|
| +0 | 8 | offset du sous-en-tête | position dans la page | **calculé** | — | pas de mutation isolée : couvert par les mutations de contenu |
| +8 | 8 | longueur | longueur du sous-en-tête | **calculé** | — | — |
| +16 | 1 | octet de compression | `0` ; `4` ligne compressée ; `1` pointeur terminal | **constante** | `ptr.comp` 0/alea/FF : rejeté/ignoré/ignoré | testé sur le pointeur de `ColumnText` : la valeur 1 est rejetée, `FF` et une valeur aléatoire sont tolérées ; écrire 0 |
| +17 | 1 | type | `1` pour `ColumnText`, `ColumnName`, `ColumnAttributes` (non compressé), `0` pour les autres ; `1` pour tous si la table est compressée | **libre** | `ptr.type_rowsize` 0/alea/FF : ignoré/ignoré/ignoré ; `ptr.type_text` 0/alea/FF : ignoré/ignoré/ignoré | SAS reconnaît les sous-en-têtes par leur signature ; valeurs conseillées = valeurs de SAS |
| +18 | 6 | — | zéros | **libre** | `ptr.pad` 0/alea/FF : ignoré/ignoré/ignoré | — |

### 5.4 Sous-en-tête `RowSize` (808 octets)

Sens confirmé par décompilation (`10_format_sas7bdat.md` 7.0, 2026-10-30) : `+0x08` est un niveau de format (compressé : valeur supérieure à `0x130`), les localisateurs `+0x200`..`+0x240` désignent ColumnSize, la fin des métadonnées, la première ligne, la dernière page de lignes et le premier FormatAndLabel ; `+0x18` est un mot d'état (écrire 0).

| offset | taille | champ | valeur ou formule | classe | mutations (SAS) | remarque |
|---|---|---|---|---|---|---|
| 0x00 | 8 | signature | `f7 f7 f7 f7 00 00 00 00` | **constante** | — | reconnaissance du sous-en-tête |
| 0x08 | 8 | marqueur de format | `0xF0` ; `0x131` si compressée | **constante** | `rs.marker` 0/alea/FF : rejeté/rejeté/rejeté | tout autre valeur rejetée (« not compatible with this SAS version ») |
| 0x10 | 8 | compteur | `n + 9` ; compressée : `N + S + 2` | **libre** | `rs.field2` 0/alea/FF : ignoré/ignoré/ignoré ; `zc.rs_field2` 0/alea/FF : ignoré/ignoré/ignoré | ignoré ; valeur conseillée = valeur de SAS |
| 0x20 | 8 | drapeaux | `0x223011` ; `0x22303A` si compressée | **constante** | `rs.flags` 0/alea/FF : rejeté/rejeté/rejeté | toute mutation rejetée |
| 0x28 | 8 | `row_length` | `RL` | **calculé** | `rs.40_rowlen` 0/alea/FF : modifié/rejeté/modifié | mutation = données lues différemment ou rejet |
| 0x30 | 8 | `row_count` | `N` | **calculé** | `rs.nobs` 0/alea/FF : modifié/modifié/modifié | affiché dans `Observations` |
| 0x40 | 8 | — | `0` ; `2` si compressée | **constante** | `zc.rs_40` 0/alea/FF : rejeté/rejeté/rejeté | rejeté si compressée et ≠ 2 |
| 0x48 | 8 | nombre de colonnes | `n` | **calculé** | `rs.72_n` 0/alea/FF : rejeté/ignoré/rejeté | — |
| 0x58 | 8 | valeur de SAS non reproduite | écrit à `0` | **constante** | `rs.58` 0/alea/FF : rejeté/rejeté/ignoré | SAS y écrit une valeur dépendant des colonnes (26 pour une ligne de 208 octets, 28 pour 24, 0 pour une table à une seule colonne) ; `0` est relu ; la valeur 1 et une valeur aléatoire sont rejetées (erreur de mémoire), `FF…FF` est toléré (inconnue W07) |
| 0x60 | 8 | nombre de colonnes (copie) | `n` | **libre** | `rs.96_n` 0/alea/FF : ignoré/ignoré/ignoré | ignoré |
| 0x68 | 8 | taille de page | `P` | **calculé** | `rs.104_page` 0/alea/FF : modifié/modifié/modifié | affiché dans `Data Set Page Size` |
| 0x78 | 8 | capacité de la page mixte | `cap` (4.1) | **calculé** | `rs.cap` 0/alea/FF : rejeté/rejeté/rejeté | toute mutation rejetée ; la formule redonne la valeur de SAS sur 61 fichiers |
| 0x80 | 16 | — | `FF`×16 | **libre** | `rs.128_136` 0/alea/FF : ignoré/ignoré/ignoré | — |
| 0x1B8 | 4 | signature de la page 0 | `file_code XOR 1` | **libre** | `rs.page_sig` 0/alea/FF : ignoré/ignoré/ignoré | ignoré |
| 0x200 | 32 | localisateurs 0 et 1 | `(1, 2)`, `(1, S_fl_dernier + 1)` | **calculé** | `rs.loc0_1` 0/alea/FF : rejeté/rejeté/rejeté | toute mutation rejetée |
| 0x220 | 16 | localisateur 2 | `(1, sc + 1)` ; compressée : `(1, S + 1)` | **calculé** | `rs.loc2` 0/alea/FF : modifié/rejeté/rejeté ; `zc.rs_loc2` 0/alea/FF : modifié/modifié/rejeté | mutation = données différentes ou rejet |
| 0x230 | 16 | localisateur 3 (dernière page) | `(nombre de pages, blocs de la dernière page)` | **calculé** | `rs.loc3` 0/alea/FF : rejeté/ignoré/ignoré ; `zc.rs_loc3` 0/alea/FF : rejeté/ignoré/ignoré | zéro rejeté, valeurs grandes tolérées (borne supérieure) |
| 0x240 | 16 | localisateur 4 | `(1, S_fl_premier + 1)` | **calculé** | `rs.loc4` 0/alea/FF : rejeté/rejeté/rejeté | toute mutation rejetée (« invalid namestr ») |
| 0x24A | 1 | — | `0` ; `1` si compressée | **libre** | `zc.rs_24a` 0/alea/FF : ignoré/ignoré/ignoré | — |
| 0x2A0 | 40 | références de texte | voir `ROWSIZE_TAIL_WORDS` et `COMPRESSED_TAIL_WORDS` de `ref_writer.py` | **constante** | `rs.tail_refs` 0/alea/FF : ignoré/rejeté/rejeté ; `zc.rs_tail` 0/alea/FF : rejeté/rejeté/rejeté | zéro toléré (non compressé), valeurs aléatoires rejetées |
| 0x2E8 | 8 | compteurs | `04 00 01 00 01 00 01 00` | **constante** | `rs.tail_2e8` 0/alea/FF : rejeté/modifié/rejeté | — |
| 0x2EE | 2 | longueur maximale des noms | max des longueurs de noms en octets | **libre** | `rs.name_len` 0/alea/FF : ignoré/ignoré/ignoré | SAS écrit la longueur maximale ; ignoré |
| 0x2FE | 2 | capacité + pointeurs ou `rmax` | `cap + sc` si toutes les lignes tiennent sur la page 0, sinon `rmax` ; compressée : `0` | **libre** | `rs.fe` 0/alea/FF : ignoré/ignoré/ignoré | ignoré |
| 0x308 | 8 | `row_count` (copie) | `N` | **libre** | `rs.776_nobs` 0/alea/FF : ignoré/ignoré/ignoré | ignoré |
| 0x318 | 12 | — | `00 00 00 01 00 00 00 00` puis `00 01` en 0x31A | **constante** | `rs.tail_318` 0/alea/FF : rejeté/rejeté/rejeté | toute mutation rejetée |

### 5.5 Autres sous-en-têtes

| zone | offset | taille | champ | valeur ou formule | classe | mutations (SAS) | remarque |
|---|---|---|---|---|---|---|---|
| ColumnSize | +8 | 8 | nombre de colonnes | `n` | **calculé** | `cs.m1` 0/alea/FF : rejeté/rejeté/rejeté | mutation rejetée |
| ColumnSize | +16 | 8 | — | zéros | **constante** | `cs.pad` 0/alea/FF : rejeté/rejeté/rejeté | mutation rejetée |
| SubheaderCounts | +8 | 8 | taille du premier bloc de texte | `len(bloc 0)` | **libre** | `cn.m1` 0/alea/FF : ignoré/ignoré/ignoré | ignoré |
| SubheaderCounts | +16 | 16 | compteurs d'en-tête | nombre réel de sous-en-têtes de signatures `FC`, `FD`, `FF`, `FE`, `FB`, `FA`, `F9` écrits (3 : ColumnList n'est pas écrit), puis `7` (confirmé par décompilation, `10` §4.4 ; 0 et 4 sont tolérés par SAS) | **constante** | `cn.hdr` 0/alea/FF : ignoré/rejeté/rejeté | zéro toléré, valeurs aléatoires rejetées |
| SubheaderCounts | +112 | 8 | — | `0x070C` | **constante** | `cn.const_112` 0/alea/FF : ignoré/rejeté/ignoré | valeurs aléatoires rejetées |
| SubheaderCounts | +120 + 40·k | 40 | entrées k = 0..6 : signature sur 64 bits (`FC FD FF FE FB FA F9` + `FF`×7) puis 4 u64 | entrées 0 à 2 : `(1, position+1, 1, position+1)` de ColumnAttributes, ColumnText (premier bloc), ColumnName ; entrées 3 à 6 : zéros | **calculé** | `cn.entry0` 0/alea/FF : ignoré/ignoré/rejeté ; `cn.entry3_6` 0/alea/FF : ignoré/ignoré/ignoré | entrée 0 : seule la valeur 0xFF…FF est rejetée ; entrées 3 à 6 ignorées |
| ColumnText | +16 | 4 | code d'encodage dans le préfixe | `00 00 00 <code>` | **libre** | `tx.enc` 0/alea/FF : ignoré/ignoré/ignoré | ignoré (SAS lit l'octet 70 de l'en-tête) |
| ColumnText | +20 | 16 | blancs du préfixe | 16 × `0x20` (compressée : décalés de 8 octets) | **libre** | `tx.spaces` 0/alea/FF : ignoré/ignoré/ignoré | — |
| ColumnText | +36 | 8 | `DATASTEP` | `DATASTEP` (compressée : en +44) | **libre** | `tx.datastep` 0/alea/FF : ignoré/ignoré/ignoré | — |
| ColumnText (compressée) | +20 | 8 | littéral de l'algorithme | `SASYZCRL` (CHAR) ou `SASYZCR2` (BINARY) | **constante** | `zc.literal` 0/alea/FF : rejeté/rejeté/rejeté | toute mutation rejetée |
| ColumnName | +8 | 8 | longueur et zéros | `8 + 8·n` (u16) puis 6 zéros | **calculé** | `nm.hdr` 0/alea/FF : rejeté/rejeté/rejeté | mutation rejetée |
| ColumnName | entrée +6 | 2 | 4e mot de l'entrée (nom) | `0` | **libre** | `nm.entry_pad` 0/alea/FF : ignoré/ignoré/ignoré | — |
| ColumnAttributes | +8 | 8 | longueur et zéros | `8 + 16·n` (u16) puis 6 zéros | **calculé** | `at.hdr` 0/alea/FF : rejeté/rejeté/rejeté | mutation rejetée (une valeur aléatoire fait boucler SAS) |
| ColumnAttributes | entrée +12 | 2 | drapeaux du nom | `0x0400` si le nom fait 8 octets au plus, `0x0800` sinon | **calculé** | `at.flags` 0/alea/FF : ignoré/modifié/rejeté | valeur aléatoire : métadonnées modifiées ; `FF` : rejet |
| ColumnAttributes | entrée +15 | 1 | octet de remplissage | `0` | **calculé** | `at.pad` 0/alea/FF : modifié/ignoré/modifié | mutation = données lues différemment |
| FormatAndLabel | +8 | 16 | zéros | `0` | **libre** | `fl.head` 0/alea/FF : ignoré/ignoré/ignoré | — |
| FormatAndLabel | +0x18 | 8 | largeur et décimales du format, de l'informat | 4 × u16 : `(fw, fd, iw, id)` | **calculé** | `fl.widths` 0/alea/FF : modifié/modifié/modifié | mutation = format affiché différent |
| FormatAndLabel | +0x28 | 18 | références de texte informat, format, label | 3 × `(bloc, offset, longueur)` en u16 | **calculé** | `fl.refs` 0/alea/FF : modifié/modifié/modifié | mutation = textes affichés différents |
| FormatAndLabel | +0x3A | 6 | — | zéros | **libre** | `fl.tail` 0/alea/FF : ignoré/ignoré/ignoré | — |

### 5.6 Page marqueur `0x9000` (tables compressées de SAS 9.4M7)

| zone | offset dans la page | taille | valeur ou formule | classe | mutations (SAS) | remarque |
|---|---|---|---|---|---|---|
| 0x9000 — en-tête | 24 à 39 | 16 | `[24..31] = 1`, type `0x9000`, `(bc, sc) = (n + 1, n)`, `[38..39] = A − 48 − 18·n` | **calculé** | `hdr24` 0 / 2 : rejeté (« Page header information invalid ») ; `blocs ±1` : ignoré ; `sous-en-têtes −1` : rejeté (End of file) ; `sous-en-têtes +1` : ignoré ; `champ38` 0, ±1 : ignoré, `0xFFFF` : rejeté | n = pages qui contiennent des lignes ; mutations en `evidence/write/mutations_marqueur.tsv` |
| 0x9000 — lignes cumulées | `R + 18·j` (u64 big endian), `R = A − 18·n`, `j` = 0 pour la dernière page de lignes | 8 | lignes cumulées jusqu'à cette page incluse (le premier enregistrement porte le total) | **calculé** | cumul −1, 0 sur le premier enregistrement : rejeté (End of file) ; +1, `0xFFFF` : ignoré ; enregistrements 1 et 2 : toute valeur ignorée | le plus grand cumul doit être ≥ nombre de lignes de la table |
| 0x9000 — numéro de page | `R + 18·j + 8` | 8 | numéro 1-based de la page dans le fichier | **libre** | −1, +1, 0 sur les 3 enregistrements : ignoré | ignoré (valeur de SAS conseillée) |
| 0x9000 — dernier indice | `R + 18·j + 16` | 2 | blocs de la page − 1 (terminal compris) | **libre** | −1, +1, 0 sur les 3 enregistrements : ignoré | ignoré (valeur de SAS conseillée) |

## 6. Règles de construction

### 6.1 Ordre des sous-en-têtes, taille de page, taille d'en-tête

- **Ordre** : `RowSize`, `ColumnSize`, `SubheaderCounts`, `ColumnText` (un ou plusieurs blocs), `ColumnName`, `ColumnAttributes`, `FormatAndLabel` × `n` (SAS ajoute `ColumnList` entre `ColumnAttributes` et le premier `FormatAndLabel` ; il est omis ici). La reconnaissance se fait par la signature (8 octets) : l'ordre exact n'a pas été mis à l'épreuve par permutation (inférence : l'ordre de SAS est conservé et relu partout), mais le champ `Counts` et les localisateurs de `RowSize` désignent des positions dans cet ordre (sections 5.4 et 5.5).
- **Taille d'en-tête** : `HL = P` (265 fichiers 64 bits du corpus, 0 exception). Mutation : `HL ≠ P` est rejeté (`h.hdrlen`).
- **Taille de page `P`** : relues par SAS sans écart : 4 096 (`w08_pages_buf`), 5 000 (pas une puissance de 2), 8 192, 65 536 (défaut), 131 072, 1 048 576 (`w13_page*` de la matrice). **Taille minimale** : la page 0 doit contenir `Σ longueur(sous-en-têtes) + 24·sc + 40` octets (environ 1 900 octets pour deux colonnes, plus ≈ 120 octets par colonne : `FormatAndLabel` 64, pointeur 24, entrées de noms 8 et d'attributs 16, texte du nom). Une page de 1 024 octets ne convient donc pas (cas `w13_page1024` : non faisable).
- **Nombre maximal de colonnes sur la page 0** (colonnes numériques à noms courts) : 20 pour `P = 4 096`, 549 pour 65 536, 1 110 pour 131 072, 4 000 pour 1 048 576 (limite posée par l'écrivain : les longueurs de `ColumnName` et `ColumnAttributes` sont des u16). SAS écrit au-delà de ces valeurs sur plusieurs pages de métadonnées (par exemple 5 000 colonnes sur 7 pages de 131 072 octets, `taille_wide5000_char`) : non écrit ici (inconnue W04).

### 6.2 Répartition des lignes dans les pages et compteurs

- Table non compressée : la page 0 (mixte) reçoit `min(N, cap)` lignes, les pages suivantes (données) `rmax` lignes chacune, dans l'ordre ; formules en 4.1. Comparaison avec le corpus (`evidence/write/repartition_lignes.txt` ; `base_n3000_mix_no`, `taille_buf4096_no`, `taille_buf8192_no`, `taille_buf131072_no`, `base_n100_mix_no`) : mêmes nombres de pages (2, 19, 10, 1, 1) ; nombre de lignes par page de données identique (168 et 337 lignes pour 4 096 et 8 192 octets) ; la page 0 contient 3 lignes de plus que dans les fichiers de SAS (2 629 contre 2 626 lignes pour `base_n3000_mix_no`) parce que `ColumnList` (54 octets + 24 d'un pointeur) n'est pas écrit, et la dernière page a donc 3 lignes de moins (371 contre 374) : c'est le seul écart.
- Compteurs : `block_count` (page mixte) = `sc + n0`, `block_count` (page de données) = lignes de la page ; `subheader_count` = `sc` (page 0) ou 0 ; `RowSize` : `row_count`, `cap`, localisateur de dernière page, nombre de pages de l'en-tête de fichier ; `SubheaderCounts` : positions 1-based de `ColumnAttributes`, `ColumnText`, `ColumnName` ; tous recalculés après la répartition et le contenu de `RowSize` **réécrit dans la page 0 après coup** (l'écrivain de référence le copie dans la page avant de connaître `cap`).
- Table compressée : remplissage glouton page par page (4.2) ; vérifié sur 4 fichiers de SAS (`compression_pat4_char`, `pat7_char`, `taille_wide5000_char`, `pat2_char`) : une ligne qui ne tient pas ouvre une nouvelle page (restes observés de 459, 170, 36, 64 octets contre des lignes suivantes de 506, 504, 64, 64 octets plus 24 octets de pointeur). Sur 60 changements de page du corpus complémentaire (SAS complet, `pg*`, `comp_n10000_*`, `big_*`) la marge minimale entre la ligne suivante (flot + 24 octets de pointeur) et la place libre est de **1 octet** : la règle `libre ≥ 0 après ajout` est exacte (W02, section 9).

### 6.3 Champs numériques et manquants

- Longueur `k` (3 à 8) : octets de poids fort du double (`raw[8−k:]`) ; relecture exacte par les cinq lecteurs pour 1,25, 2,5, −0,5, 100, 0 (`w03_num3` à `w03_num8`).
- Manquants `.`, `_`, `.A` à `.Z` (28 valeurs) : octet de rang 5 de la valeur à 8 octets = code ci-dessus ; relecture par les cinq lecteurs conforme à l'attendu (`w04_missing`) : SAS affiche `.`, `_`, `A`…`Z` ; pandas, pyreadstat et Parso rendent des manquants.

### 6.4 Textes et encodages

- Les noms, formats, informats et labels sont écrits dans l'encodage du fichier (`codec` de l'écrivain) : UTF-8 (`w06_names`, noms et valeurs non ASCII), latin1 (code 29, `w12_latin1`), wlatin1 (code 62, `w12_wlatin1`, caractères `0x80` à `0x9F` : `€`, `œ`, guillemets courbes). Une valeur qui ne se code pas dans l'encodage cible est une erreur de l'appelant (non gérée).
- Relecture SAS dans une session UTF-8 d'un fichier latin1 ou wlatin1 : `NOTE` seulement, pas de `WARNING` (journaux `evidence/write/w12_*_sas.log`).
- pyreadstat 1.2.9 refuse le code 29 (« unsupported character set ») ; il accepte 20 et 62. Le driver Rust devrait écrire 62 plutôt que 29 pour de l'ouest-européen sans caractère propre à ISO-8859-1 (décision de conception, hors de cette spec).

## 7. Cas non supportés

Ce qui suit est **hors priorité** (prompt 05) ; seul ce qui est observable est noté.

| Cas | Statut | Ce que l'on sait |
|---|---|---|
| Ajout de lignes à un fichier existant (`PROC APPEND`, `MODIFY`) | non supporté, non essayé | `sas_small` ne sait pas modifier une table permanente (`STATE.md`, capacités) ; un fichier produit par l'écrivain n'a pas été essayé en ajout |
| Lignes supprimées | non supporté | page `0x0280` (bitmap de lignes supprimées) lue par `10_format_sas7bdat.md` ; `sas_small` échoue à supprimer (`BRIDGE FAILURE`) ; l'écrivain réserve la zone `⌈cap/8⌉` mais ne l'utilise pas |
| Index (`.sas7bndx`) | non supporté | `PROC DATASETS` échoue dans `sas_small` ; fichiers d'index lus uniquement (`corpus/files/index/`) |
| Contraintes d'intégrité | non supporté | non observables avec `sas_small` |
| Chiffrement (`ENCRYPT=`, mot de passe) | non supporté | un contrôle de type « mot de passe » existe à l'octet 296 de l'en-tête (section 5.1) : sa mutation produit `Invalid or missing READ password` ; ce n'est pas le chiffrement de données |
| Compression, nombre de lignes | pas de limite propre | 70 000 lignes relues par SAS (`w11_*_70000`) | — |
| Métadonnées sur plusieurs pages (plus de 549 colonnes pour `P = 65 536`) | non supporté | inconnue W04 |
| Plateformes 32 bits, big endian, Windows, HP-UX | non écrites | lues (`10_format_sas7bdat.md`) |
| `ColumnList` | non écrit | inconnue W05 |

## 8. Limites observées contre SAS

| Limite | Valeur | Preuve | Comportement hors limite |
|---|---|---|---|
| Nom de colonne | 32 octets (dans l'encodage du fichier) | `w06_names` (32 octets, OK), `w14_name33` | SAS tronque à 32 octets et signale `ERROR: The variable named … contains more than 32 bytes` |
| Label | 256 octets | `w14_label256` (OK), `w14_label257` | SAS tronque à 256 octets et signale `WARNING: Some character data was lost during transcoding` |
| Colonne caractère | 32 767 octets | `w14_char32767` (OK, ligne de 32 767 octets relue) | 32 768 octets : SAS `ERROR: insufficient memory` ; pyreadstat refuse aussi |
| Colonne numérique | 3 à 8 octets | `w03_num3` à `w03_num8` | non essayé en dehors de la plage |
| Nombre de colonnes (métadonnées sur la page 0) | 549 (`P = 65 536`) à 4 000 (`P = 1 048 576`) | `w09_manycols` (300), `w14_cols700` (700, `P = 131 072`) | l'écrivain refuse (`ValueError`) |
| Taille de page | 4 096 à 1 048 576 relus ; 1 024 impossible (métadonnées) | `w13_page*` | — |
| Nombre de lignes (non compressé) | 3 000 essayé | `w07_pages`, `w08_pages_buf` (500 lignes, `P = 4 096`) | — |
| Nombre de lignes (compressé) | 70 000 essayé sous SAS, sans limite connue | `w11_*_256`, `w11_*_1000`, `w11_*_10000`, `w11_*_70000`, `w11_*_pages`, `w11_*_pg4096` | — |
| Volume | 3 000 lignes de 24 octets essayé ; > 2 Go non essayé | `w15_gt2gb` : non faisable | — |

## 9. Inconnues (détail dans `60_inconnues.md`, entrées W01 à W11)

- **W01** — *résolue (2026-11-02)* : la limite de 255 lignes venait d'un champ de lignes cumulées écrit sur un octet ; SAS lit 8 octets big endian (corpus complémentaire du SAS complet, 155 fichiers conformes, `sas_small` relit 256, 1 000, 10 000 et 70 000 lignes).
- **W02** — *résolue (2026-11-02)* : un enregistrement par page de lignes, 18 octets, tableau terminé à `A = min(P, 32 767)` ; condition de remplissage `libre ≥ 0 après ajout` (marge minimale observée : 1 octet). Reste le comportement quand les métadonnées occupent plusieurs pages (W04). W11 (plus de 65 535 lignes) est résolue : cumul sur 8 octets.
- **W03** — pages de types `0x4000` (9.4M6) et `0x0400` (`compression_type_0x02`) : rôle inconnu, jamais écrites.
- **W04** — métadonnées sur plusieurs pages et sous-en-têtes `ColumnName` / `ColumnAttributes` multiples.
- **W05** — `ColumnList` omis : relu par les cinq lecteurs ; rôle hors lecture inconnu.
- **W06** — portée des champs `libre` : lecture seule par `sas_small`.
- **W07** — `RowSize+0x58` : valeur de SAS non reproduite.
- **W08** — fichiers de plus de 2 Go non essayés.
- **W09** — champs `libre` de l'en-tête dont SAS affiche la valeur (release, hôte) : le driver doit y mettre des valeurs plausibles (`9.0401M7`, `Linux`) ; l'effet sur d'autres versions de SAS est inconnu.

## 10. Reproduction et traçabilité

```bash
S=<scratchpad> ; export VENV_PYTHON=$S/venv/bin/python PARSO_CP=$S/parso/classes:$S/parso/parso.jar:$S/parso/slf4j-api.jar
python3 -I rea_output/tools/validate_write.py              # matrice complète -> rea_output/tools/write_matrix.md
python3 -I rea_output/tools/mutate_write.py [champs]      # mutations -> rea_output/evidence/write/mutations.tsv
python3 -I rea_output/tools/diff_recopies.py <fichiers>   # comparaison octet à octet avec le corpus
```

| Identifiant | Contenu |
|---|---|
| `tools/ref_writer.py` | écrivain de référence ; liste `RECOPIES` des champs recopiés avec fichier et offset d'origine |
| `tools/write_cases.py`, `validate_write.py`, `readers_dump.py` | cas, boucle de validation, relecture pandas / pyreadstat |
| `tools/write_matrix.md` | matrice cas × lecteur |
| `evidence/write/<cas>_sas.log`, `.lst` | journal et listing SAS de chaque cas |
| `evidence/write/mutations.tsv`, `evidence/write/mut/` | 3 mutations par champ et journaux |
| `evidence/write/diff_recopies.txt` | écart octet à octet par fichier du corpus |
| `evidence/write/repartition_lignes.txt` | pages et lignes par page : fichiers de SAS contre fichiers réécrits (6.2) |
| `evidence/write/control_ok_sas.log`, `control_ko_sas.log` | contrôle de la relecture SAS : fichier du corpus OK, copie tronquée KO (`tools/control_sas.py`) |
| `evidence/write/compression_256_lignes.md` | expériences sur la page marqueur et le seuil de 255 lignes |
| `evidence/write_informat_oracle_ref.log` | même avertissement d'informat pour un fichier écrit par le SAS complet |
