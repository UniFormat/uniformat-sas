# Spécification de la compression des lignes `.sas7bdat` (`COMPRESS=CHAR` et `COMPRESS=BINARY`)

> Changement OpenSpec : `specify-sas7bdat-compression` (prompt 03). Dernière mise à jour : 2026-10-30.
> Objet : décrire les deux compressions de lignes assez précisément pour écrire un décompresseur et un compresseur sans lire le code SAS. Ce document complète `rea_output/10_format_sas7bdat.md` (structure d'un fichier non compressé : en-tête, pages, pointeurs de sous-en-têtes, notations `w`, `a1`, `a2`, `t`, `po`) et se lit sans `rea_output/evidence/`.

## 0. Statut, méthode et limites

**Statut (honnête).** Comme pour le changement 02, **aucune décompilation n'a été faite** : la confirmation interne NULL de l'analyse d'interopérabilité n'est pas consignée dans `rea_output/STATE.md`. Aucune entrée n'est donc `confirmé` (la définition exige une preuve binaire `fonction@adresse` en plus de la preuve fichier). Les liens avec les modules `src/sasexe/sasyzcrl` et `sasyzcr2` ne sont pas établis (section 1). Les règles de décision du chemin d'écriture de SAS (seuils, choix brut / compressé, fenêtre RDC) relèvent de la décompilation : elles sont documentées ici **telles qu'observées dans les fichiers** et marquées `inconnu` quand les fichiers ne les déterminent pas (section 5).

**Méthode** : (1) hypothèses publiques (ReadStat, Parso, pandas, PDF de Shotwell) prises comme points de départ `inféré` ; (2) comparaison de chaque fichier `_char` / `_binary` à sa table `_no` de même nom produite par le même SAS (la ligne brute de référence est connue octet pour octet) ; (3) vérification par décompression de toutes les lignes ; (4) deux décompresseurs écrits indépendamment (section 7).

**Corpus exploitable.** Sur les 126 tables `CHAR`/`BINARY` générées, SAS n'a réellement compressé que 30 tables (`Compressed ≠ NO`, liste dans `corpus/out_full/compression_check.tsv`) : 15 en RLE et 15 en RDC ; les 96 autres sont des tables ordinaires dont SAS a désactivé la compression (« would increase the size of the data set ») et se lisent comme les fichiers non compressés. Vingt-et-un fichiers publics compressés complètent (datareader `test2/3/5/6/8/9/11/12/14/15/17/18/20/21`, pandas `0x40controlbyte` et `test_meta2_page`, pyreadstat `sample_bincompressed`, readstat `compression_type_0x02/0x06/0x09/0x0d`). Une 52e table, `catalogue_cntlout` (export `CNTLOUT=` de `PROC FORMAT`, 7 lignes), est aussi en RLE sans que `COMPRESS=` ait été demandé : `PROC FORMAT` compresse par défaut ; ses 7 lignes sont décompressées (`evidence/compression_litteraux.txt` liste les 52 fichiers à littéral). L'oracle `sas_small` **ne sait pas écrire** de table compressée : aucun vecteur n'a pu être produit en dehors de ce corpus (section 6).

**Résultats de vérification** (rejouables avec `sh rea_output/tools/run_checks.sh`) :

| Vérification | Résultat | Preuve |
|---|---|---|
| Décompression RLE de toutes les lignes `CHAR` du corpus généré (15 tables, 1 476 lignes) comparées à la ligne brute de la table `_no` | 1 476 sur 1 476 identiques | `evidence/compression_rle_corpus.txt` |
| Décompression RDC de toutes les lignes `BINARY` du corpus généré (15 tables : 1 256 lignes compressées + 220 lignes stockées brutes) | 1 256 sur 1 256 identiques ; 220 lignes brutes identiques | `evidence/compression_rdc_corpus.txt` |
| Fichiers publics compressés (21 fichiers, RLE et RDC) relus par `ref_reader.py`, comparés au CSV de référence quand il existe | 15 fichiers avec CSV : 0 écart ; 6 sans CSV : nombre de lignes égal à `row_count` (dont 5 lignes sur 5 pour les quatre fichiers `readstat`) | `evidence/compression_externes.txt` |
| Opcodes observés | RLE : 13 classes ; RDC : commandes 0, 1, 2 et 3 à 9, 13, 14, 15 | sections 3 et 4 |

---

## 1. Identification des algorithmes

#### lien_char
- Emplacement : bloc de texte 0 du premier ColumnText (voir `10_format_sas7bdat.md`, 4.5), octets 12 à 19 du bloc
- Taille / type : 8 octets ASCII
- Sémantique : littéral `SASYZCRL` ⇒ les lignes sont compressées par l'algorithme **RLE** décrit en section 3 (celui de `COMPRESS=CHAR`). À 8 espaces, aucune compression.
- Valeurs observées : `SASYZCRL` dans les 15 tables `CHAR` effectivement compressées du corpus (`Compressed=CHAR`) et dans les 7 fichiers publics datareader en RLE ; jamais dans une table `NO`. La décompression par l'algorithme de la section 3 redonne la ligne brute de chacune des 1 476 lignes.
- Écriture : écrire `SASYZCRL` à cet endroit pour une table dont les lignes sont en RLE.
- Preuve : chaîne dans chaque fichier concerné (octet 12 du bloc 0, soit ColumnText + `w + 12`) ; décompression exacte du corpus. **Pas de preuve binaire** (le module `src/sasexe/sasyzcrl` n'a pas été ouvert).
- Confiance : inféré (lien littéral ↔ algorithme RLE) ; inconnu (lien avec le module `sasyzcrl`)

#### lien_binary
- Emplacement : même emplacement que `lien_char`
- Taille / type : 8 octets ASCII
- Sémantique : littéral `SASYZCR2` ⇒ lignes compressées par l'algorithme **RDC** (Ross Data Compression : octets littéraux et références arrière pilotés par des mots de contrôle de 16 bits), section 4 (celui de `COMPRESS=BINARY`).
- Valeurs observées : `SASYZCR2` dans les 15 tables `BINARY` effectivement compressées et dans les fichiers publics RDC ; la décompression par l'algorithme de la section 4 redonne la ligne brute de chacune des 1 256 lignes compressées.
- Écriture : écrire `SASYZCR2` pour une table RDC.
- Preuve : idem ; **pas de preuve binaire** (module `sasyzcr2` non ouvert)
- Confiance : inféré (lien littéral ↔ RDC) ; inconnu (lien avec le module `sasyzcr2`)

#### compression_demandee_et_effective
- Emplacement : option `COMPRESS=` de la table, observée dans `Compressed` de `PROC CONTENTS` et dans le littéral de `lien_char` / `lien_binary`
- Taille / type : comportement de SAS à l'écriture
- Sémantique : demander `CHAR` ou `BINARY` ne suffit pas : SAS désactive la compression de la table quand elle augmente la taille (note du journal « Compression was disabled … »). Le fichier est alors **identique en structure** à un fichier non compressé : pas de littéral, lignes dans des pages `0x0100`/`0x0200`, `Compressed=NO`. **Le critère n'est pas établi**, mais une observation le contraint : les mêmes 100 lignes de 21 octets sont compressées pour `plateforme_{linux_32,windows_32,windows_64,hp_ux_32}` (format 32 bits, pointeur de 12 octets par ligne) et ne le sont pas pour `plateforme_{linux_x86_64,solaris_64,aix_64}` (format 64 bits, pointeur de 24 octets) : le surcoût du pointeur par ligne entre dans la comparaison de SAS. Les tables à lignes courtes (moins de 21 octets) ne sont jamais compressées ; `types_char200`, `taille_wide300`, `taille_wide5000` et `compression_*` le sont.
- Valeurs observées : 96 tables sur 126 demandées en `CHAR`/`BINARY` ne sont pas compressées, 30 le sont (`corpus/out_full/compression_check.tsv`). Corpus complémentaire (lignes de 68 à 5 000 octets) : 45 tables sur 45 sont compressées, **même quand la note du journal indique « increased size by x percent »** (tables `big_*`, `edge_*`) : la décision se fait donc sur la longueur de ligne (C02), pas sur la taille obtenue : refus de 16 à 24 octets par ligne, compression de 28 à 64 octets (25 à 27 non mesurés)
- Écriture : libre : l'écrivain décide de compresser ou non ; en cas de compression, tout le reste du document s'applique
- Preuve : `corpus/out_full/compression_check.tsv` (126 tables)
- Confiance : inféré

---

## 2. Détection des lignes compressées et stockage

### 2.1 Unité compressée : la ligne

Chaque ligne est compressée **indépendamment** (aucune fenêtre ni état partagé entre lignes) : la ligne `i` du fichier est décompressée seule, et sa longueur décompressée est `row_length` (champ RowSize, `10_format_sas7bdat.md` 4.2). Elle n'est plus rangée dans une page de données mais **comme un sous-en-tête** : un pointeur par ligne dans la table de pointeurs d'une page `0x0000`.

#### compressed_row_pointer
- Emplacement : page `0x0000` + `po + 8` + `i × ps`, pointeur de sous-en-tête (`10_format_sas7bdat.md` 3.2)
- Taille / type : `[offset : w][length : w][compression : u8 = 4][type : u8 = 1][réservé]`
- Sémantique : `compression = 4` et `type = 1` ⇒ la ligne est stockée **compressée** : ses `length` octets, à `offset` depuis le début de la page, sont le flot RLE ou RDC (selon le littéral du fichier). L'ordre des pointeurs est l'ordre des lignes.
- Valeurs observées : 200 pointeurs pour 200 lignes (`compression_pat2_char`) ; longueurs de 8 octets (`compression_blank200_char`) à 20 004 octets (`taille_wide5000_char`, ligne brute de 40 008 octets) ; la ligne compressée est plus courte que la ligne brute dans tous les cas observés.
- Écriture : un pointeur par ligne, `compression = 4`, `type = 1`.
- Preuve : `evidence/compression_pointeurs.txt` (14 fichiers), tous les fichiers du corpus compressé
- Confiance : inféré

#### compression_byte
- Emplacement : octet de compression du pointeur de sous-en-tête (`10_format_sas7bdat.md` 3.2), octet `2w` du pointeur
- Taille / type : `u8`
- Sémantique : le **bit 2 (`0x04`)** indique une ligne compressée (RLE ou RDC selon le littéral). Valeurs observées : `0` (sous-en-tête ou ligne brute), `1` (pointeur terminal / zone libre), `4` (ligne compressée), `6` et `13` (lignes compressées RDC), `2` et `9` (lignes **brutes** de `row_length` octets), `3` (un pointeur de 1 octet `00`, non interprété). Les valeurs `2`, `3`, `6`, `9`, `13` ne se rencontrent que dans les quatre fichiers `readstat/compression_type_0x02`, `0x06`, `0x09`, `0x0d`, dont le nom reprend la valeur : chacun contient 4 lignes en compression 4 et une cinquième ligne avec cette valeur. Le **bit 3 (`0x08`)** est corrélé à une ligne supprimée : les fichiers `0x09` et `0x0d` ont `deleted_count = 1`, `0x02` et `0x06` ont 0 (jalon J2, `inféré`, voir C09 dans `60_inconnues.md`) ; les autres bits n'ont pas de sens établi.
- Valeurs observées : voir ci-dessus ; les 30 tables SAS 9.4M7 n'utilisent que `0`, `1` et `4`
- Écriture : n'écrire que `0` (sous-en-tête ou ligne brute), `1` (terminal) et `4` (ligne compressée)
- Preuve : `evidence/compression_pointeurs.txt` ; lecture de 5 lignes sur 5 dans les quatre fichiers `readstat`
- Confiance : inféré (bit 2) / inconnu (autres bits)

#### raw_row_pointer
- Emplacement : même emplacement que `compressed_row_pointer`
- Taille / type : `[offset : w][length : w][compression : u8 = 0][type : u8 = 1][réservé]`
- Sémantique : `type = 1`, octet de compression sans le bit 2 (valeurs `0`, `2`, `9` ; jamais `1`) et `length = row_length` ⇒ la ligne est stockée **brute** (ses `row_length` octets sont la ligne) dans une table compressée. Se distingue des sous-en-têtes de métadonnées (même `compression = 0`, `type = 1` dans une table compressée) par la **position** : les métadonnées (RowSize, ColumnSize, SubheaderCounts, ColumnText, ColumnName, ColumnAttributes, [ColumnList], FormatAndLabel × `n`) précèdent les lignes, et par la signature (aucune des huit signatures de `10_format_sas7bdat.md` 4.1).
- Valeurs observées : 226 lignes brutes dans 5 fichiers RDC : `compression_pat2_binary` (200 lignes), `compression_noise_binary` (20 lignes), `pyreadstat/sample_bincompressed` (4 lignes sur 5), `readstat/compression_type_0x02` et `0x09` (1 ligne sur 5). Aucune ligne brute dans les fichiers RLE.
- Écriture : voir 5.3.
- Preuve : `evidence/compression_pointeurs.txt`
- Confiance : inféré

#### free_area_pointer
- Emplacement : dernier pointeur d'une page `0x0000` contenant des lignes
- Taille / type : `[offset : w][length : w][compression = 1][type = 1]`
- Sémantique : décrit la **zone libre** de la page, entre la fin de la table de pointeurs et le sous-en-tête de plus bas offset (`offset + length` = offset du dernier sous-en-tête). Ne désigne pas une ligne (et dans une page non compressée le pointeur terminal a `length = 0`).
- Valeurs observées : `offset = 0x13D8`, `length = 0x4485` pour la page 0 de `compression_pat2_char` (`0x13D8 + 0x4485 = 0x585D`, offset de la dernière ligne).
- Écriture : un pointeur terminal par page.
- Preuve : `evidence/compression_pointeurs.txt`
- Confiance : inféré

#### page_comp_0x9000
- Emplacement : une page de type `0x9000` par fichier compressé, placée **juste après la première page qui contient des lignes** : page 1 quand les métadonnées tiennent sur la page 0 (tous les fichiers de `corpus/files_complement`), page 5 pour `taille_wide5000_char` dont les 5 000 colonnes occupent les pages 0 à 4 et dont les lignes commencent à la page 4.
- Taille / type : en-tête de page de 40 octets (`[0..3]` signature de page, `[24..31] = 1`, `[32..33] = 0x9000`, `[34..35] = n + 1`, `[36..37] = n`, `[38..39] = A − 18·n − 48`), `n` pointeurs de 24 octets entièrement nuls, puis un tableau de `n` enregistrements de 18 octets qui occupe `[A − 18·n, A)`. `n` est le nombre de pages qui contiennent des lignes (le marqueur et les pages de métadonnées seules n'y comptent pas) ; `A = min(P, 32 767)` où `P` est la taille de page.
- Enregistrements : le premier en mémoire décrit la **dernière** page de lignes, le dernier la première. Chacun : `u64` **big endian** = nombre de lignes cumulé jusqu'à cette page incluse (le premier porte donc le total de la table) ; `u64` little endian = numéro 1-based de la page dans le fichier (le marqueur est compté) ; `u16` little endian = indice du dernier sous-en-tête de la page (nombre de blocs − 1, pointeur terminal compris, ColumnList compris). Les octets `BD 7F` de `po + 6` (0x7FBD = 32 701) sont le champ `A − 18·n − 48` pour `n = 1`.
- Sémantique : page « marqueur » **sans ligne**, ignorée d'un lecteur. SAS la lit : mutations en `evidence/write/mutations_marqueur.tsv` (table à trois pages de lignes) : `[24..31]` autre que 1 rejeté (« Page header information invalid ») ; champ `[38..39]` toléré sauf `0xFFFF` ; le plus grand cumul doit être **supérieur ou égal** au nombre de lignes de la table (sinon `WARNING: End of file` après la dernière ligne comptée) ; numéros de page, derniers indices, cumuls intermédiaires et `block_count ± 1` ignorés ; `subheader_count` diminué de 1 rejeté (SAS lit alors moins d'enregistrements), augmenté de 1 ignoré. La limite de **255 lignes** de la première version de l'écrivain (W01) venait d'un cumul écrit sur un octet ; il n'y a **pas de limite à 65 535** (le cumul est sur 8 octets).
- Valeurs observées : 155 fichiers compressés du SAS complet (`rea_output/tools/check_marker.py` : 155 OK, 0 écart ; 1 à 99 enregistrements, `P` = 4 096, 8 192, 16 384 et 65 536, de 1 à 70 000 lignes dont 65 535, 65 536 et 70 000) ; fichiers de `corpus/files` et `readstat/compression_type_0x02` (autre release, structure différente : voir W03).
- Écriture : écrire le tableau avec les valeurs de SAS.
- Preuve : `rea_output/tools/check_marker.py`, `mutate_marker.py`, `compare_multipage.py` ; relecture par `sas_small` des cas `w11_*` de `write_matrix.md` (256, 1 000, 10 000, 70 000 lignes, pages de 4 096 octets). Aucune preuve binaire (code du noyau `sas` non étudié pour cette page).
- Confiance : inféré (conforme à 155 fichiers de SAS et accepté par SAS ; les champs « ignorés » n'ont pas d'autre rôle observé)

#### compressed_file_signs
- Emplacement : fichier entier (pages, RowSize, pointeurs de métadonnées)
- Taille / type : ensemble d'indices
- Sémantique : dans un fichier réellement compressé : les pages de lignes sont de type `0x0000` (aucune page mixte ni de données) ; les sous-en-têtes de métadonnées ont le **type de pointeur 1** (RowSize, ColumnSize, SubheaderCounts, FormatAndLabel…, qui valent 0 sans compression) ; `row_length` est la **somme des largeurs** de colonnes **sans arrondi à 8** (508 pour `compression_pat4`, 512 dans la table `_no`) ; `RowSize.row_marker` vaut 305 (`0x131`) dans les fichiers SAS 9.4M7 (32 et 64 bits) et 651 (`0x28B`) dans les fichiers publics 9.4M1 en 32 bits, mais **cette valeur ne sert pas à détecter la compression** (voir `10_format_sas7bdat.md` 4.2). La détection fiable est le littéral de `lien_char` / `lien_binary`.
- Valeurs observées : les 30 tables compressées du corpus généré et les 21 fichiers publics
- Écriture : respecter ces indices (ils sont la conséquence d'une écriture ligne par ligne en sous-en-têtes)
- Preuve : comparaison des RowSize des 30 tables compressées et de leurs tables `_no`
- Confiance : inféré

---

## 3. RLE (`COMPRESS=CHAR`, littéral `SASYZCRL`)

**Format.** Le flot est une suite de commandes. Chaque commande commence par un **octet de contrôle** `c` ; on note `hi = c & 0xF0` et `lo = c & 0x0F`. Une commande produit des octets en sortie et consomme `k` octets de données après `c` ; les données se lisent **dans l'ordre, immédiatement après l'octet de contrôle** : `d0` désigne l'octet qui suit `c`, et pour `0x40` le deuxième octet de données est l'octet à répéter. Le flot est décompressé jusqu'à épuisement de l'entrée ; la sortie fait alors exactement `row_length` octets (vérifié sur 1 476 lignes). Les entiers multi-octets ne sont pas utilisés (les longueurs sont en `lo` et l'octet suivant) : le format ne dépend pas de l'endianness du fichier.

| `hi` | Action | Longueur `n` de la sortie | Données lues (`k`) | Octets produits | Observé (min – max, nombre) | Preuve | Confiance |
|---|---|---|---|---|---|---|---|
| `0x00` à `0x3F` | copie littérale longue | `n = d0 + 64 + 256 × (c & 0x3F)` (64 – 16 447) ; les bits 4 et 5 de `c` prolongent la longueur | `d0` (1 octet) puis `n` octets | les `n` octets lus | 65 – 503, 622 | corpus, 1 476 lignes | confirmé (`sasyzcrl@0x101d10`, évidence `ev_6c55e8e3519c`) |
| `0x40` | répétition d'un octet (longue) | `n = 256 × lo + d0 + 18` | `d0` puis 1 octet `b` | `b` répété `n` fois | 19 – 500, 31 | corpus (lignes `compression_runs_char`, `pandas/0x40controlbyte`) | confirmé (`sasyzcrl@0x101d10`, évidence `ev_6c55e8e3519c`) |
| `0x50` | répétition de `@` (`0x40`) | `n = 256 × lo + d0 + 17` | `d0` | `@` × `n` | 18 – 501, 16 | corpus | confirmé (`sasyzcrl@0x101d10`, évidence `ev_6c55e8e3519c`) |
| `0x60` | répétition d'espace (`0x20`) | `n = 256 × lo + d0 + 17` | `d0` | espace × `n` | 18 – 1 040, 1 104 | corpus | confirmé (`sasyzcrl@0x101d10`, évidence `ev_6c55e8e3519c`) |
| `0x70` | répétition de nul (`0x00`) | `n = 256 × lo + d0 + 17` | `d0` | `0x00` × `n` | 18 – 500, 115 | corpus | confirmé (`sasyzcrl@0x101d10`, évidence `ev_6c55e8e3519c`) |
| `0x80` | copie littérale courte | `n = lo + 1` (1 – 16) | `n` octets | les `n` octets lus | 1 – 16, 22 516 | corpus | confirmé (`sasyzcrl@0x101d10`, évidence `ev_6c55e8e3519c`) |
| `0x90` | copie littérale | `n = lo + 17` (17 – 32) | `n` octets | idem | 17 – 32, 114 | corpus | confirmé (`sasyzcrl@0x101d10`, évidence `ev_6c55e8e3519c`) |
| `0xA0` | copie littérale | `n = lo + 33` (33 – 48) | `n` octets | idem | 34 – 48, 30 | corpus | confirmé (`sasyzcrl@0x101d10`, évidence `ev_6c55e8e3519c`) |
| `0xB0` | copie littérale | `n = lo + 49` (49 – 64) | `n` octets | idem | 49, 2 | corpus (peu d'occurrences) | confirmé (`sasyzcrl@0x101d10`, évidence `ev_6c55e8e3519c`) |
| `0xC0` | répétition d'un octet (courte) | `n = lo + 3` (3 – 18) | 1 octet `b` | `b` × `n` | 3 – 18, 453 | corpus | confirmé (`sasyzcrl@0x101d10`, évidence `ev_6c55e8e3519c`) |
| `0xD0` | répétition de `@` (courte) | `n = lo + 2` (2 – 17) | aucune | `@` × `n` | 2 – 17, 87 | corpus | confirmé (`sasyzcrl@0x101d10`, évidence `ev_6c55e8e3519c`) |
| `0xE0` | répétition d'espace (courte) | `n = lo + 2` (2 – 17) | aucune | espace × `n` | 2 – 17, 1 254 | corpus | confirmé (`sasyzcrl@0x101d10`, évidence `ev_6c55e8e3519c`) |
| `0xF0` | répétition de nul (courte) | `n = lo + 2` (2 – 17) | aucune | `0x00` × `n` | 2 – 17, 21 658 | corpus | confirmé (`sasyzcrl@0x101d10`, évidence `ev_6c55e8e3519c`) |

Notes :
- **Résolution des désaccords publics** (D5 à D7 de `00_etat_de_l_art.md`) : le facteur de la commande `0x40` est **256** (et non 16, comme le décrit Parso) : confirmé par les lignes à suites de 300 et 500 octets de `compression_runs_char` (`41 1A 78` = 256 + 26 + 18 = 300 octets `x`). Les commandes `0x50`, `0x90`, `0xA0`, `0xB0` existent et sont utilisées. Les commandes `0x10`, `0x20`, `0x30` ne sont rencontrées nulle part. Les longueurs de `0x00`, `0x40`, `0x50`, `0x60`, `0x70` utilisent `lo` comme octet de poids fort (`lo ≠ 0` observé : 503 = 183 + 64 + 256).
- **Validité d'un flot** : il est invalide si des données manquent (les octets de contrôle `0x10`…`0x3F` sont **valides** : ils prolongent la longueur de la copie longue, voir la section 3.1), si la sortie dépasse `row_length` ou si, l'entrée épuisée, la sortie est plus courte que `row_length`. Un lecteur refuse la ligne (comportement de SAS sur un flot invalide : `inconnu`). `row_length` est la valeur de RowSize du fichier compressé (somme des largeurs de colonnes **sans arrondi à 8**, section 2). Les formules de longueur donnent les valeurs légales ; les « minimum – maximum observés » du tableau n'en sont que la partie rencontrée (par exemple `0x60` avec `d0 = 0` donne 17 espaces, légal d'après la formule mais jamais vu, car SAS préfère `0xE0 + 15` jusqu'à 17).
- Exemple (ligne de 208 octets de `compression_blank200_char` : `i` = 1,0 sur 8 octets, `abc`, 197 espaces) : flot `F4 84 F0 3F 61 62 63 60 B4` : `F4` = 6 nuls, `84` = copie de 5 octets (`F0 3F 61 62 63`), `60 B4` = `0xB4 + 17 = 197` espaces ; total 6 + 5 + 197 = 208 octets.
- Preuve générale : décompression exacte de 1 476 lignes de 13 tables et de 7 fichiers publics (`datareader/test2,5,9,12,15,18,21`, `pandas/0x40controlbyte`), `evidence/compression_rle_corpus.txt`.


### 3.1 Confirmation par décompilation de `sasyzcrl` (2026-10-30)

Fonction `sasyzcrl@0x101d10` (module `src/sasexe/sasyzcrl`, SHA-256 `ccdb0081…38f3`), évidence `ev_6c55e8e3519c6793900201d1f38a49eb02ea281dd4897ba3f8e2d1ba6290f8cd`, export `evidence/decompile/sasyzcrl_bundle.json`. La fonction contient le décodeur (mode `3`) et l'encodeur (autres modes ; les modes `1` et `4` ne font rien). Interface déduite : tampon de paramètres avec longueur de sortie attendue (`row_length`), longueur d'entrée, pointeur d'entrée, pointeur de sortie, mode.

**Décodeur (confirmé, coïncide avec le tableau de la section 3 sauf les points marqués ★)** :
- `0x80`…`0xBF` : copie littérale de `(c & 0x7F) + 1` octets (1 à 64).
- `0xC0`…`0xCF` : `(c & 0x0F) + 3` copies de l'octet suivant ; `0xD0`…`0xDF` : `(c & 0x0F) + 2` fois `@` ; `0xE0`…`0xEF` : espaces ; `0xF0`…`0xFF` : nuls.
- `0x00`…`0x3F` : copie littérale de `(((c & 0x3F) << 8) | d0) + 0x40` octets. ★ Les octets `0x10`, `0x20`, `0x30` (bits 4 et 5) **sont utilisés** : ils prolongent la longueur (jusqu'à 16 447), contrairement à ce que disait la version précédente de ce document (« non utilisés », « flot invalide »).
- `0x40`…`0x7F` : `L = ((c & 0x3F) << 8) | d0` ; bits 4-5 de `c` : `00` → `L + 0x12` copies de l'octet `d1` ; `01` → `L' + 0x11` fois `@` ; `10` → espaces ; `11` → nuls (`L'` = `L` privé du bit de type).
- Le flot doit être **consommé exactement** : entrée non épuisée ou sortie qui dépasserait `row_length` → erreur interne de SAS (message « an internal error has occurred while reading a compressed file », code de retour `0x8041F817`). ★ La fonction ne vérifie **pas** que la sortie atteint `row_length` : elle rend la longueur produite ; l'exigence « sortie complète » de la version précédente est une règle de ce document, pas de ce module.
- Si la longueur stockée est **égale** à `row_length`, les octets sont copiés tels quels (ligne brute) et le code de retour est `0x0041F817` (valeur de succès « non compressé »).

**Encodeur (confirmé)** :
- Une suite est reconnue dès 2 octets pour l'espace, `@` et nul, dès 3 octets pour tout autre octet. ★ **Exception en fin de ligne** (corpus complémentaire, 2026-11-02) : une suite qui va jusqu'au dernier octet de la ligne exige un octet de plus (3 pour espace, `@`, nul ; 3 ou 4 pour les autres octets, non départagé) ; sinon ses octets sont écrits en littéral. Vérifié sur 14 628 lignes `CHAR` : flots identiques à SAS (`ref_compress.rle_compress`). L'encodeur Rust (`sas7bdat::compress::rle_compress`, jalon J2) découpe comme SAS : suites d'espace, `@` ou nul de 1 040 octets au plus par commande, autre octet 1 041, copie littérale 16 447 ; il reproduit les **260 flots** CHAR des vecteurs de `compression_vectors/char.tsv` (le compresseur Python en reproduit 193 : il découpe à 4 112 et 4 159).
- Octets non répétés : jusqu'à 64 en commande `0x80` + `n − 1` ; 65 et plus en copie longue `0x00` par blocs d'au plus **16 447** octets (`0x403F`).
- Suites : de 2 à 17 (espace, `@`, nul) en commande courte `0xD0`/`0xE0`/`0xF0` ; de 3 à 18 pour un autre octet en `0xC0` ; au-delà : espace, `@`, nul de 18 à **1 040** en commande `0x50`/`0x60`/`0x70` (2 octets), autre octet de 19 à **1 041** en commande `0x40` (3 octets) ; les suites plus longues sont découpées. L'encodeur n'utilise donc jamais les longueurs maximales que le format sait coder (4 112).
- Un seul octet répété est écrit en copie `0x80 b` ; deux octets identiques non spéciaux en `0x81 b b`.
- **Repli brut** : si la sortie encodée atteindrait `row_length` octets (égalité comprise), l'encodeur abandonne, recopie l'entrée telle quelle et renvoie `0x0041F817`. Une ligne n'est donc stockée compressée que si le flot est **strictement plus court** que `row_length` : règle R11 confirmée pour RLE.

Reste `inconnu` : la politique de l'appelant (qui décide de la compression de la table entière, C02) ne se trouve pas dans ce module.

---

## 4. RDC (`COMPRESS=BINARY`, littéral `SASYZCR2`)

**Format.** Le flot est une suite de **groupes**. Chaque groupe commence par un **mot de contrôle de 16 bits** écrit sur 2 octets en **big endian, quelle que soit l'endianness du fichier** (vérifié sur des fichiers little endian et big endian), suivi de 16 éléments au plus. Pour le bit `j` du mot, de `j = 0` (bit de poids fort, `0x8000`) à `j = 15` (bit de poids faible) :

- bit à **0** : l'élément est un **octet littéral** (1 octet lu, 1 octet produit) ;
- bit à **1** : l'élément est une **commande** (voir ci-dessous) qui commence par un octet `u` : `cmd = u >> 4`, `cnt = u & 0x0F`.

Le décodage s'arrête quand l'entrée est épuisée, même au milieu d'un groupe ; les bits non utilisés du dernier mot de contrôle sont à 0 (335 groupes finals vérifiés). La sortie fait alors exactement `row_length` octets. **Validité** : un flot est invalide s'il reste 1 octet à la place d'un mot de contrôle complet, si les données d'une commande manquent, si une référence arrière a un décalage supérieur au nombre d'octets déjà produits, si la sortie dépasse `row_length`, ou si, l'entrée épuisée, elle est plus courte que `row_length` (comportement de SAS : `inconnu`). Une ligne de longueur nulle n'existe pas dans le corpus (flot vide ou mot `00 00` : `inconnu`).

| `cmd` | Action | Longueur produite | Données lues après `u` | Observé | Preuve | Confiance |
|---|---|---|---|---|---|---|
| `0` | répétition d'un octet (courte) | `cnt + 3` (3 – 18) | 1 octet `b` | 3 – 18, 6 416 | corpus | confirmé (`sasyzcr2@0x101d50`, évidence `ev_f59950f89e6b`) |
| `1` | répétition d'un octet (longue) | `cnt + 19 + 16 × d0` (19 – 4 114) | `d0` puis 1 octet `b` | 19 – 4 114, 699 | corpus | confirmé (`sasyzcr2@0x101d50`, évidence `ev_f59950f89e6b`) |
| `2` | copie arrière longue | `d1 + 16` (16 – 271) | `d0` puis `d1` (ordre : `d0` en premier) ; décalage `ofs = cnt + 3 + 16 × d0` | longueurs 17 – 56, décalages 18 – 303 ; 95 occurrences, uniquement dans 8 fichiers publics (`datareader/test3,6,8,11,14,17,20`, `pandas/test_meta2_page`) | fichiers publics (avec CSV pour 7 d'entre eux) | confirmé (`sasyzcr2@0x101d50`, évidence `ev_f59950f89e6b`) |
| `3` à `15` | copie arrière courte | `cmd` (3 – 15) | `d0` ; décalage `ofs = cnt + 3 + 16 × d0` | commandes 3 à 9, 13, 14, 15 observées ; décalages 4 – 552 ; 70 616 occurrences | corpus | confirmé (`sasyzcr2@0x101d50`, évidence `ev_f59950f89e6b`) |

Une **copie arrière** de longueur `L` et de décalage `ofs` ajoute `L` octets, chacun étant l'octet situé `ofs` positions avant la fin courante de la sortie (copie octet par octet : les zones se recouvrent, ce qui produit des répétitions de motifs de `ofs` octets, par exemple `ofs = 4` et `L = 15`). `ofs ≥ 3` (le décalage est stocké diminué de 3) ; `ofs` est au plus 4 098 ; `ofs` supérieur au nombre d'octets déjà produits est invalide.

**Exemple pas à pas** (ligne de 208 octets de `compression_blank200_binary` : `i` = 1,0 sur 8 octets, `abc`, 197 espaces) : flot `82 00 03 00 F0 3F 61 62 63 12 0B 20` ; mot de contrôle `82 00` = `1000 0010 0000 0000` ; élément 0 (bit à 1) : `u = 03` → `cmd = 0`, `cnt = 3`, octet `00` : `cnt + 3 = 6` nuls ; éléments 1 à 5 (bits à 0) : littéraux `F0 3F 61 62 63` ; élément 6 (bit à 1) : `u = 12` → `cmd = 1`, `cnt = 2`, `d0 = 0B`, octet `20` : `2 + 19 + 16 × 11 = 197` espaces ; l'entrée est épuisée, la sortie compte 6 + 5 + 197 = 208 octets.

Notes :
- Les commandes `10`, `11` et `12` (copie arrière de 10 à 12 octets) suivent la même règle que `3` à `9`, `13` à `15` mais ne sont pas rencontrées dans le corpus ; la règle commune est `inféré`.
- Fenêtre : les plus grands décalages observés sont 552 (corpus généré) ; la valeur maximale codable est 4 098 ; la taille de fenêtre utilisée par le compresseur de SAS n'est pas établie (section 5).
- La commande `2` n'apparaît que dans des fichiers publics ; son ordre `d0`, `d1` et sa longueur `d1 + 16` sont vérifiés par la **comparaison au CSV de référence** de 7 fichiers `datareader` (`test3,6,8,11,14,17,20`, 0 écart), pas par un vecteur à ligne brute connue.
- Preuve générale : décompression exacte de 1 256 lignes de 15 tables et des fichiers publics RDC (`datareader/test3,6,8,11,14,17,20`, `pyreadstat/sample_bincompressed`, `readstat/compression_type_0x02,0x06,0x09,0x0d`, `pandas/test_meta2_page`), `evidence/compression_rdc_corpus.txt`.


### 4.1 Confirmation par décompilation de `sasyzcr2` (2026-10-30)

Fonction `sasyzcr2@0x101d50` (module `src/sasexe/sasyzcr2`, SHA-256 `e8e2d0b7…0d2c`), évidence `ev_f59950f89e6baae7cae7903b61756499e023ce5dac1f0d09e6eb53ff1188ae58`, export `evidence/decompile/sasyzcr2_bundle.json`. Modes (champ à l'offset `0xBC` du bloc de paramètres) : `1` initialise la table de hachage (32 768 octets, 4 096 entrées de 8 octets), `4` la libère, `2` compresse, `3` décompresse.

**Décodeur (confirmé)** : conforme à la section 4 : mot de contrôle de 16 bits big endian lu bit de poids fort en premier ; bit `0` = octet littéral ; bit `1` : `hi = u >> 4`, `lo = u & 0x0F` : `hi = 0` → `lo + 3` copies de l'octet suivant ; `hi = 1` → `16 × d0 + 19 + lo` copies de `d1` ; `hi = 2` → copie arrière de `d1 + 16` octets au décalage `16 × d0 + 3 + lo` ; `hi = 3…15` → copie arrière de `hi` octets, même décalage. Tout dépassement de la longueur de sortie (`row_length`) renvoie l'erreur `0x8041F817`. ★ Le décodeur **ne vérifie pas** que le décalage reste dans la sortie déjà produite ni que l'entrée contient les octets annoncés, et il **ne vérifie pas** que la sortie atteint `row_length` : ces contrôles sont, dans ce document, des règles de lecture sûre (les implémenter est obligatoire pour un lecteur exposé à des fichiers non fiables). La copie arrière utilise `memcpy` : le comportement sur zones recouvrantes n'est pas défini par ce module, mais l'encodeur de SAS ne produit jamais de chevauchement.

**Encodeur (confirmé)** :
- Une table de hachage de 4 096 entrées (une adresse par entrée) associe à 3 octets consécutifs (clé : `((b0 << 8) | b1) XOR ((b0 >> 4) | (b2 << 4))`, 12 bits) leur **dernière occurrence**. L'entrée est mise à jour à chaque position testée qui n'est pas dans une suite de 3 octets identiques ou plus, avant le test de la candidate. La table n'est pas remise à zéro d'une ligne à l'autre : une candidate antérieure au début de l'entrée est ignorée, une candidate plus récente que la position courante donne une longueur nulle ; le flot produit peut donc varier selon l'historique des lignes (propriété à ne pas tester par comparaison d'octets).
- Suite d'un même octet de 3 octets ou plus : commande `0` jusqu'à 18, commande `1` de 19 à **4 114** (longueur maximale du format).
- Sinon, candidate à distance au plus **4 098** (le maximum codable) : longueur comparée jusqu'à **271**, sans jamais dépasser le début de l'élément courant (pas de chevauchement) ; longueur < 3 → octet littéral ; 3 à 15 → commande `3…15` ; 16 à 271 → commande `2`.
- Restent littéraux : tout octet sans suite ni candidate, et les 2 derniers octets de la ligne (moins de 3 octets restants).
- Une ligne de **moins de 19 octets** n'est jamais compressée. Au début de chaque nouveau groupe de 16 éléments, si la position d'écriture dépasse `row_length − 50`, l'encodeur abandonne (repli brut, code de retour `0x0041F817`, longueur stockée = `row_length`).
- Aucune commande ne manque : toutes les longueurs de copie 3 à 15 et la commande `2` sont émises par l'encodeur (les commandes `10`, `11`, `12` de la section 4 sont donc des commandes valides que SAS peut écrire).

---

## 5. Règles du compresseur de SAS et classement `obligatoire` / `libre`

**Principe de classement.** Le critère d'écriture retenu est la **décodabilité par SAS**, pas l'identité des octets avec ceux de SAS (le driver n'a pas à reproduire SAS). Une règle est **obligatoire** si le format l'impose (un lecteur, SAS compris, décode mal sans elle) ; **libre** si le compresseur peut choisir autrement (taille de sortie différente, flot toujours valide). **Aucune règle n'a été vérifiée par une relecture SAS** (impossible : `sas_small` ne lit pas ces fichiers en écriture ; la vérification est reportée au changement `specify-sas7bdat-writing`, section 7) : le classement repose sur la structure du format et sur les 2 732 lignes décodées dans le corpus généré.

| N° | Règle | Classement | Justification / preuve | Confiance |
|---|---|---|---|---|
| R1 | Un flot RLE est une suite d'octets de contrôle de la table de la section 3 suivis de leurs données | obligatoire | forme du format | inféré |
| R2 | Pour RLE, `n` est borné par la formule de la commande (par exemple 17 à 4 112 pour `0x60`) ; une suite plus longue se découpe en plusieurs commandes, le reste (inférieur au minimum d'une commande à longueur variable) s'écrivant avec une commande courte ou en littéral. Les longueurs maximales (4 112, 4 113, 4 159) sont des extrapolations des formules : le corpus s'arrête à 1 040 octets | obligatoire (formules) / libre (découpage) | formules de longueur | inféré |
| R3 | Pour RDC, mot de contrôle de 16 bits big endian précédant 16 éléments, bit de poids fort en premier | obligatoire | forme du format | inféré |
| R4 | Pour RDC, `ofs ≥ 3` et `ofs` ≤ nombre d'octets déjà produits | obligatoire | forme du format | inféré |
| R5 | La sortie décompressée fait `row_length` octets | obligatoire | longueur des lignes de RowSize | inféré |
| R6 | Écrire `compression = 4`, `type = 1` pour une ligne compressée ; littéral `SASYZCRL`/`SASYZCR2` dans le bloc de texte | obligatoire | détection par les lecteurs publics et par `ref_reader` | inféré |
| R7 | Seuils de RLE : SAS utilise les commandes courtes `0xC0`/`0xD0`/`0xE0`/`0xF0` pour des suites de 2 ou 3 à 18 octets, les commandes longues au-delà ; observé : suites d'espace ou de nul dès 2 octets, suites d'octet quelconque dès 3 | libre | un compresseur peut n'utiliser que des copies littérales (`0x80`…) : le flot reste décodable | inféré |
| R8 | Choix entre copie courte (`0x80`, jusqu'à 16 octets) et copie longue (`0x90`…`0xB0`, `0x00`) : SAS emploie surtout `0x80` (22 516 occurrences) et la copie longue `0x00` (≥ 65 octets) ; `0x90`/`0xA0`/`0xB0` apparaissent dans les fichiers publics | libre | idem | inféré |
| R9 | Pour RDC, une référence arrière exige une longueur d'au moins 3 octets et ne chevauche jamais sa propre source (longueur ≤ décalage) : un motif de période 2 (`ab` répété) ne donne donc aucune référence et la ligne est stockée brute (`compression_pat2_binary`) | libre | **confirmé** par `sasyzcr2@0x101d50` (encodeur : recherche d'une occurrence antérieure par table de hachage sur 3 octets, boucle de comparaison arrêtée au début de l'élément courant) | confirmé |
| R10 | Fenêtre RDC de l'encodeur de SAS : décalage maximal **4 098** (`0x1002`), longueur de copie maximale **271**, répétition d'un octet jusqu'à **4 114** (les maxima que le format sait coder) | libre | **confirmé** par `sasyzcr2@0x101d50` (voir 4.1) | confirmé |
| R11 | Choix ligne brute / compressée. RLE : compressée seulement si le flot est strictement plus court que `row_length` (`sasyzcrl@0x101d10`). RDC : brute si `row_length < 19`, ou si, au début d'un groupe de 16 éléments, la position d'écriture dépasse `row_length − 50` octets ; sinon compressée (`sasyzcr2@0x101d50`, voir 4.1). Une ligne RDC doit donc gagner environ 48 octets au moins pour être stockée compressée | libre (le lecteur accepte les deux formes) | **confirmé** par les deux modules | confirmé |
| R12 | Désactivation de la compression d'une table entière quand le surcoût dépasse le gain (note du journal SAS) | libre | un écrivain peut décider de compresser ou non ; si la table est compressée, tous les points précédents s'appliquent | inféré |

Le compresseur de référence (`ref_compress.py`) laisse à l'appelant le choix brut / compressé (aucun critère établi : R11) ; ses flots ne cherchent pas à reproduire ceux de SAS (sur les vecteurs RDC, 5 868 octets au total contre 43 020 pour SAS, car il cherche les références plus largement).

Cas « compression sans gain » : (a) au niveau de la **table**, SAS n'écrit pas de littéral et la table est ordinaire (96 tables du corpus) ; (b) au niveau de la **ligne** (RDC seulement), la ligne est stockée brute avec un octet de compression sans bit 2 (`0`, parfois `2` ou `9`), `type = 1`, `length = row_length` (226 lignes).

---

## 6. Vecteurs de test

`rea_output/tools/compression_vectors/` (généré par `rea_output/tools/make_vectors.py` à partir du corpus) :

- `char.tsv` : 338 vecteurs RLE ; `binary.tsv` : 367 vecteurs RDC ; colonnes `id, algorithme, fichier, ligne, stockage (compressed|raw), categorie, longueur_ligne, brut_hex, stocke_hex` (description détaillée : `compression_vectors/README.md`).
- Catégories : `corpus` (lignes ordinaires, dont 7 fichiers publics et leurs variantes), `bornes-de-longueur` (110 lignes par algorithme : `compression_runs`, suites de 2 à 500 octets pour 5 valeurs d'octet `0x78`, `0x40`, `0x00`, `0xFF`, `0x20`, aux bornes 2, 3, 4, 5, 15 à 20, 30, 63 à 65, 127 à 129, 255 à 257, 300, 500), `incompressible` (`compression_noise` : 20 lignes de 200 caractères pseudo-aléatoires), `ligne-quasi-uniforme` (`compression_zeros`, `compression_blank200`), `externe-sans-brut` (fichiers publics sans table `_no` : seule la décompression sans erreur est vérifiée).
- Les vecteurs sont un **échantillon** : au plus 12 lignes par fichier (toutes celles de `compression_runs`), d'où 81 occurrences de la commande 2 dans les vecteurs contre 95 dans l'ensemble des fichiers publics ; `catalogue_cntlout` (table écrite par `PROC FORMAT`, RLE) en fait partie ; les 4 vecteurs `raw` sans `brut_hex` sont des lignes brutes de `sample_bincompressed` (test : égalité avec `stocke_hex`, qui est la ligne).
- **Cas limites non couverts** : une ligne de **longueur nulle**, une ligne **strictement identique** sur toute sa longueur (nuls seulement, sans colonne `i`) et les bornes des **commandes RDC** (longueurs 18 et 19 des commandes 0 et 1, longueur 16 de la commande 2, décalages 3 et 4 098) ne figurent pas dans le corpus ; `sas_small` ne produit pas de fichier compressé. Ils exigent le SAS complet (`prompts/01b_extraction_sas_complet.md`, à étendre) : action ouverte dans `STATE.md`.

---

## 7. Décompresseur et compresseur de référence

- `rea_output/tools/ref_reader.py` : décompresseurs RLE et RDC intégrés, écrits par un agent n'ayant lu que ce document (section 0, `STATE.md`).
- `rea_output/tools/ref_compress.py` : compresseurs RLE et RDC écrits depuis ce document ; critère : `decompress(ref_compress(x)) == x` pour chaque vecteur brut, et `decompress(stocké par SAS) == brut` pour chaque vecteur du corpus.
- **Écriture indépendante** : décompresseurs et compresseurs écrits par un agent n'ayant lu que ce document et les fichiers de vecteurs (`rea_output/evidence/compression_lecteur_independant_rapport_lacunes.md`, 18 lacunes signalées dont les points utiles sont corrigés dans cette version : ordre des données, validité des flots, exemple RDC, décomptes).
- **Résultats** (`rea_output/tools/test_compression.py`, `evidence/compression_vecteurs.txt`) : `decompress(stocké par SAS) == brut` pour 260 vecteurs RLE et 236 vecteurs RDC ; décompression sans erreur des vecteurs sans ligne brute (78 RLE, 101 RDC) ; `decompress(compress(brut)) == brut` pour les 260 lignes brutes connues par algorithme et pour 298 cas synthétiques par algorithme (lignes vide et d'un octet, suites de 2 à 20, 60 à 70, 255 à 260 et 4 100 à 4 120 octets pour cinq valeurs d'octet, motifs de période 3, 4 et 7, mélanges aléatoires). Le compresseur RDC émet les commandes 0, 1, 2, 3 à 9 et 12.
- **Vérification par SAS** de la relecture des lignes produites par `ref_compress.py` : **reportée** au changement `specify-sas7bdat-writing` (elle exige un écrivain de fichier complet) et consignée dans `rea_output/STATE.md`.

---

## 8. Cibles de décompilation (inconnues restantes)

À traiter quand la confirmation NULL sera consignée ; modules : `src/sasexe/sasyzcrl`, `src/sasexe/sasyzcr2`.

| N° | Question | Section |
|---|---|---|
| TC01 | Lien des littéraux `SASYZCRL`/`SASYZCR2` avec les modules `sasyzcrl`/`sasyzcr2` (preuve binaire) | 1 |
| TC02 | Critère de désactivation de la compression d'une table (surcoût contre gain) | 1 |
| TC03 | Seuils RLE : longueurs minimales par commande, choix entre commandes équivalentes | 5, R7 R8 |
| TC04 | RDC : algorithme de recherche des références (table de hachage), fenêtre maximale, longueur minimale, cas du décalage 2 | 5, R9 R10 |
| TC05 | Choix ligne brute / compressée (RDC) et seuil | 5, R11 |
| TC06 | Commandes RLE `0x10`, `0x20`, `0x30` et RDC `10`-`12` : existent-elles | 3, 4 |
| TC07 | Contenu de la page `0x9000` (12 derniers octets) et de la zone libre | 2 |
| TC08 | Comportement de SAS sur un flot invalide | 3, 4 |
| TC09 | Bits 0, 1 et 3 de l'octet de compression de pointeur ; pointeur de 1 octet de compression 3 | 2 |

Les inconnues correspondantes sont dans `60_inconnues.md` (entrées `C01` à `C09`).

## Note J6 (2026-11-04) — copies RDC recouvrantes et lecteurs

Les décodeurs de pandas, de Parso et de `ref_reader.py` acceptent une copie arrière dont la longueur dépasse le décalage (la source est alors recouverte par la sortie, comme en LZ77). **ReadStat** (`sas7bdat_parse_subheader_rdc`, donc `pyreadstat` et `haven`) la refuse : `copy_len > back_offset` ⇒ erreur d'analyse. Un encodeur qui vise tous les lecteurs ne doit produire que des copies avec `longueur ≤ décalage` ; les suites d'un même octet passent alors par la commande de répétition (commandes 0 et 1). Observation sur les fichiers de SAS : non établie (aucune copie recouvrante n'a été cherchée dans les 1 256 lignes RDC du corpus). Confiance : inféré (source de ReadStat lue, essai sur `pyreadstat` 1.3.7).

