# Spécification du format `.sas7bdat` non compressé (lecture)

> Changement OpenSpec : `specify-sas7bdat-read-format` (prompt 02). Dernière mise à jour : 2026-10-30.
> Objet : décrire assez précisément la structure d'un fichier `.sas7bdat` **non compressé** pour écrire un lecteur indépendant de SAS. Ce document est la seule source lue par `design-rust-driver` (avec `60_inconnues.md` et `corpus/MANIFEST.md`) ; les preuves sont dans `rea_output/evidence/`.

## 0. Statut, méthode et limites

**Statut (honnête).** Cette spécification a été produite **sans décompilation** : la confirmation interne NULL de l'analyse d'interopérabilité n'est pas consignée dans `rea_output/STATE.md` (blocage ouvert). En conséquence :

- **aucune entrée n'est `confirmé`** : la définition de `confirmé` (`prompts/README.md`) exige une preuve binaire (`fonction@adresse`) **et** une preuve fichier ; seule la seconde existe. Toute entrée est `inféré` (preuve fichier, souvent concordante avec l'état de l'art public) ou `inconnu` ;
- la section 8 liste les **cibles de décompilation** (champs restés `inféré`/`inconnu`) pour le jour où la confirmation sera consignée ;
- la section 7 (contrôles effectués par SAS à l'ouverture) est **vide** : elle ne peut pas être établie sans décompilation ni test de mutation (renvoi `specify-sas7bdat-writing`).

**Méthode appliquée** (ordre imposé par la spec OpenSpec) : (1) structure de `00_etat_de_l_art.md` comme hypothèse `inféré` ; (2) analyse différentielle du corpus (paires de fichiers ne différant que par un paramètre : `rea_output/evidence/paires_differentielles.md`) ; (3) décompilation : **non exécutée** ; (4) vérification par un lecteur de référence Python écrit à partir de ce seul document (`rea_output/tools/ref_reader.py`, bibliothèque standard seulement).

**Résultats de vérification** (rejouables : `sh rea_output/tools/run_checks.sh`) :

| Vérification | Résultat | Preuve |
|---|---|---|
| Valeurs de tous les fichiers non compressés du corpus généré (165 tables : LE/BE, 32/64 bits, formats `UTF-8`/`latin1`/`wlatin1`/`Shift-JIS`, manquants spéciaux, lignes supprimées, tables de 0 à 50 000 lignes, 1 à 5 000 colonnes) | identiques aux CSV de référence, 0 écart ; 30 fichiers compressés ignorés (renvoi à `specify-sas7bdat-compression`) | `evidence/compare_csv_files.txt` |
| Fichiers publics non compressés (datareader `test1/4/7/10/13/16/19`, pandas `test10/13/many_columns`) | identiques, 0 écart ; 15 fichiers compressés ignorés | `evidence/compare_csv_external.txt` |
| Métadonnées de colonnes (numéro, nom, type, longueur, format, informat, label) contre `PROC CONTENTS` | 195 tables sur 195, 0 écart (les labels de 256 caractères sont tronqués par le listing, `linesize=200`) | `evidence/check_contents_files.txt` |
| Couverture octet par octet de l'en-tête (232 fichiers dont 37 publics) | 0 octet non nul hors champ décrit | `evidence/header_coverage.txt` |
| Zones de sous-en-têtes non décodées | listées avec leurs valeurs observées | `evidence/zones_non_decodees.md`, `60_inconnues.md` |
| **Lecteur indépendant** écrit par un agent n'ayant lu que ce document (`rea_output/tools/ref_reader_spec_only.py`, bibliothèque standard) | 0 écart sur les mêmes 173 fichiers et 195 `PROC CONTENTS`, après correction de 3 écarts initiaux (manquants Windows) ; il a signalé 18 lacunes ou erreurs de la version précédente de ce document, corrigées ici (lignes 4.1, 4.2 `row_marker`, 5.3, 5.4, 3.5, 6) | `evidence/lecteur_independant_resultats.txt`, `evidence/lecteur_independant_rapport_lacunes.md` |

**Limites.** Le corpus généré provient d'un seul build (SAS 9.4 TS1M7, Linux x86-64) ; les variantes big-endian, 32 bits et Windows sont produites par `OUTREP=` (pas par l'OS d'origine). Les fichiers publics (SAS 9.4 M1 à M9, Windows, Linux, AIX) comblent en partie ce manque mais sans paires différentielles. Les lignes supprimées ne sont observées que sur 1 table (10 lignes sur 100, page mixte). Aucun fichier chiffré, aucun index lu, pas de page `META2` ni de table de plus de 5 000 colonnes.

### 0.1 Fichiers et paires différentielles

Liste complète (165 tables générées non compressées + fichiers publics) : `rea_output/evidence/paires_differentielles.md` (générée depuis `corpus/gen/tables.tsv` et `corpus/out_full/compression_check.tsv`). Paires principales utilisées ci-dessous (« variante → référence ») :

| Paramètre varié | Paires |
|---|---|
| nombre de lignes | `base_n0/n1/n2/n3000_mix_no` → `base_n100_mix_no` ; `taille_n50000_no` |
| type et longueur d'une variable | `types_num3..num8_no`, `types_char1/8/200/32767_no` → `metadonnees_ref_no` |
| valeurs (manquants, extrêmes, chaînes) | `valeurs_manquants_no` (28 codes), `valeurs_extremes_no`, `valeurs_chaines_no` |
| nom, label, format, informat | `metadonnees_nom1/nom32/lab_ascii/lab256/lab_utf8/tablabel/fmt_*/infmt_*_no` → `metadonnees_ref_no` |
| taille de page, largeur de ligne | `taille_buf4096/8192/65536/131072_no` ; `taille_wide300/wide5000_no` |
| encodage | `encodage_latin1/wlatin1/sjis_no` → `encodage_utf8_no` |
| représentation des données | `plateforme_{linux_32,windows_32,windows_64,solaris_64,hp_ux_32,aix_64}_no` → `plateforme_linux_x86_64_no` |
| lignes supprimées | `suppression_n100_del10_no` → `base_n100_mix_no` |
| index, catalogue | `index_n100_ix_no`, `catalogue_fmtuser_no` → `base_n100_mix_no` |

**Réserve sur `OUTREP=`.** Écrire avec `OUTREP=` change aussi le code d'encodage (octet 70 : 29 ou 62 au lieu de 20) et les octets 47 et 71 (0 au lieu de 20) : dans `plateforme_*`, le paramètre « représentation » n'est donc pas seul à varier par rapport à `base_*`. La référence de la famille `plateforme` est `plateforme_linux_x86_64_no` (même option), pas `base_*`.

### 0.2 Notation

- `w` = taille d'un entier « d'adresse » du fichier : **8** si le fichier est de format 64 bits (octet 32 = `0x33`), **4** sinon.
- `a1` = 4 si l'octet 35 vaut `0x33`, sinon 0 ; `a2` = 4 si l'octet 32 vaut `0x33`, sinon 0 ; `t` = a1 + a2. Ces décalages déplacent tous les champs de l'en-tête situés après eux.
- Endianness `E` : octet 37 = `1` → little endian (LE), `0` → big endian (BE). Tout entier multi-octets est dans l'endianness `E`, **y compris** dans les pages et sous-en-têtes.
- `u16`, `u32`, `u64`, `f64` = entiers non signés et flottant IEEE 754 binaire 64 dans l'endianness `E`.
- Les offsets sont en octets depuis le début de la structure citée (fichier, page ou sous-en-tête). « 32 bits / 64 bits » désigne le format du fichier (`w`), pas l'OS.
- Un champ « dont les octets ne sont jamais non nuls dans le corpus » reste décrit (réservé, valeur observée 0) et figure aussi dans `60_inconnues.md`.

---

## 1. Vue d'ensemble

Un `.sas7bdat` non compressé est une suite de blocs de même taille :

```
[ en-tête : header_length octets ] [ page 0 ] [ page 1 ] ... [ page page_count-1 ]
```

- `taille du fichier = header_length + page_length × page_count` : vérifié sur les 232 fichiers (aucune exception).
- Dans 230 des 232 fichiers, `header_length = page_length` (4 096 à 131 072 ; 65 536 par défaut) ; les 2 exceptions sont des fichiers publics anciens où les deux tailles diffèrent. Taille de page paramétrable par `bufsize=` (`taille_buf4096/8192/131072_no`) ; SAS l'agrandit quand une ligne ne tient pas (`taille_wide5000_no` : 122 880 ; `types_char32767_no` : 106 496).
- Les métadonnées (description des colonnes) sont des **sous-en-têtes** rangés dans la ou les premières pages (pages « meta » ou « mix ») ; les lignes sont dans les pages « data » et « mix ». Une page « mix » contient à la fois des sous-en-têtes et des lignes.
- Une ligne est un enregistrement de longueur fixe `row_length` (octets) ; chaque colonne y occupe `[offset, offset+longueur)`.

Procédure de lecture complète : section 6.

---

## 2. En-tête de fichier

L'en-tête occupe `header_length` octets ; aucun octet non nul n'existe hors des champs ci-dessous dans les 232 fichiers (`evidence/header_coverage.txt`).

### 2.1 Octets 0 à 83 : identification et descripteur de format

#### magic
- Emplacement : fichier + 0x00
- Taille / type : 32 octets, constante `00 00 00 00 00 00 00 00 00 00 00 00 C2 EA 81 60 B3 14 11 CF BD 92 08 00 09 C7 31 8C 18 1F 10 11`
- Sémantique : signature de format. L'octet 15 vaut `0x60` pour une table de données ; il vaut `0x65` dans le fichier d'index `index_n100_ix_no.sas7bndx` (le type de fichier à l'offset 156 y est `INDEX   `).
- Valeurs observées : les 232 fichiers `.sas7bdat` portent la valeur ci-dessus ; `.sas7bndx` : octet 15 = `0x65`. Les `.sas7bcat` ne sont pas décrits ici.
- Écriture : recopier la constante (octet 15 = `0x60`).
- Preuve : `evidence/header_coverage.txt` ; `diff_pair` sur `index_n100_ix_no.sas7bndx` (octet 15 seul).
- Confiance : inféré

#### u64_flag (octet 32)
- Emplacement : fichier + 32
- Taille / type : 1 octet
- Sémantique : `0x33` = fichier de format 64 bits (`w = 8`, `a2 = 4`) ; `0x22` = format 32 bits (`w = 4`).
- Valeurs observées : `0x33` pour `base_*`, `plateforme_{linux_x86_64,solaris_64,aix_64}` ; `0x22` pour `plateforme_{linux_32,windows_32,hp_ux_32}` **et `windows_64`**. `OUTREP=WINDOWS_64` produit donc un fichier au format 32 bits (`RowSize` de 480 octets, pointeurs de 12 octets) : le nom « 64 bits » désigne l'architecture du système, pas le format du fichier. Confirmé par `evidence/diff_plateforme_linux64_vs_windows32.txt` et par le fichier public `datareader/test4` (WINDOWS_64, lu sans écart).
- Écriture : `0x33` si on écrit en format 64 bits.
- Preuve : `plateforme_*_no` (octet 32, longueur des sous-en-têtes) ; `evidence/compare_csv_external.txt`
- Confiance : inféré

#### const_33_34
- Emplacement : fichier + 33, 2 octets
- Taille / type : 2 octets constants `0x22 0x00`
- Sémantique : constante de format, présente avec le magic.
- Valeurs observées : `22 00` dans 231 fichiers ; `00 00` dans 1 fichier public ancien (release `9.0000M0`).
- Écriture : `22 00`.
- Preuve : `evidence/header_coverage.txt`, tables de valeurs par plateforme (section 0.1)
- Confiance : inféré

#### a1_flag (octet 35)
- Emplacement : fichier + 35
- Taille / type : 1 octet
- Sémantique : `0x33` → `a1 = 4` (les champs situés après l'offset 164 sont décalés de 4 octets) ; sinon `a1 = 0`.
- Valeurs observées : `0x33` (220 fichiers), `0x32` (11 : `plateforme_{linux_32,windows_32}` et fichiers publics Windows 32), `0x22` (1 fichier public ancien).
- Écriture : `0x33` (format 64 bits ou 32 bits avec `a1 = 4`) ; non reproduit pour `0x32` dans ce document.
- Preuve : les champs `created`, `header_length` se retrouvent à `164 + a1`, `196 + a1` dans les 232 fichiers (`evidence/header_coverage.txt`).
- Confiance : inféré

#### flag_36
- Emplacement : fichier + 36
- Taille / type : 1 octet
- Sémantique : inconnue ; vaut `0x33` pour les formats 64 bits **et** pour `windows_64`, `0x22` pour les autres formats 32 bits.
- Valeurs observées : `0x33` (213), `0x22` (18), `0x00` (1 fichier public ancien).
- Écriture : recopier la valeur du format cible (`0x33` en 64 bits).
- Preuve : `plateforme_*_no`, `evidence/header_coverage.txt`
- Confiance : inconnu

#### endian (octet 37)
- Emplacement : fichier + 37
- Taille / type : 1 octet
- Sémantique : `1` = little endian ; `0` = big endian. Gouverne tous les entiers et flottants du fichier.
- Valeurs observées : `1` (215), `0` (17 : `plateforme_{solaris_64,aix_64,hp_ux_32}`, datareader `test10..15`, pandas `test10/13`).
- Écriture : `1` pour x86.
- Preuve : lecture sans écart des 7 fichiers BE non compressés (`evidence/compare_csv_files.txt`, `compare_csv_external.txt`)
- Confiance : inféré

#### const_38
- Emplacement : fichier + 38
- Taille / type : 1 octet constant `0x02`
- Sémantique : constante de format.
- Valeurs observées : `0x02` (231), `0x00` (1 fichier public ancien).
- Écriture : `0x02`.
- Preuve : `evidence/header_coverage.txt`
- Confiance : inféré

#### platform_char et platform_code (octets 39 et 40)
- Emplacement : fichier + 39 et + 40
- Taille / type : 1 octet chacun
- Sémantique : famille de système de l'écrivain : `'1'` (`0x31`) avec code `0x01` = Unix ; `'2'` (`0x32`) avec code `0x04` = Windows. Écrits en double aux octets 63 et 64.
- Valeurs observées : Unix (214 fichiers) ; Windows (18 : `plateforme_{windows_32,windows_64}`, datareader `test1..6,19..21`). Le fichier produit par `OUTREP=WINDOWS_*` sur Linux porte le code Windows tout en gardant `host = Linux` plus loin (voir `host`).
- Écriture : `'1'` / `0x01` pour Unix.
- Preuve : `evidence/diff_plateforme_linux64_vs_windows32.txt`
- Confiance : inféré

#### session_code_47 et session_code_71 (octets 47 et 71)
- Emplacement : fichier + 47 et + 71 (deux copies)
- Taille / type : 1 octet chacun
- Sémantique : inconnue ; **identique aux deux endroits dans tous les fichiers**. Vaut `0x14` (20) dans les fichiers écrits par une session SAS UTF-8 sans `OUTREP=` ; `0x00` quand `OUTREP=` est utilisé et pour les fichiers publics antérieurs ; `0x10` pour `encodage_sjis_no` (exécuté dans une autre session). Il **n'est pas** la copie du code d'encodage de la table : `encodage_latin1_no` a `0x1d` à l'octet 70 et `0x14` aux octets 47 et 71 (hypothèse : encodage de la session écrivante, `inféré` au mieux).
- Valeurs observées : `0x14` (177), `0x00` (54), `0x10` (1).
- Écriture : copier la valeur du code d'encodage UTF-8 (`0x14`) si on écrit en UTF-8.
- Preuve : `evidence/diff_encodage_utf8_vs_latin1.txt`, `evidence/header_coverage.txt`
- Confiance : inconnu

#### desc_50_51 et desc_74_75
- Emplacement : fichier + 50 (2 octets) et + 74 (2 octets)
- Taille / type : 2 octets chacun, deux copies identiques
- Sémantique : inconnue. `03 01` dans 228 fichiers, `02 01` (3 : `plateforme_linux_32_*`), `00 00` (1 fichier public ancien).
- Valeurs observées : `03 01` (228 fichiers), `02 01` (3 : `plateforme_linux_32_*`), `00 00` (1 fichier ancien).
- Écriture : `03 01` (`02 01` pour `LINUX_32`).
- Preuve : `plateforme_linux_32_no` contre `plateforme_windows_32_no`
- Confiance : inconnu

#### magic_tail_copy
- Emplacement : fichier + 52, 4 octets
- Taille / type : constante `18 1F 10 11` (copie des 4 derniers octets du magic).
- Sémantique : recopie des quatre derniers octets du magic.
- Valeurs observées : constante dans les 232 fichiers.
- Écriture : `18 1F 10 11`.
- Preuve : `evidence/header_coverage.txt`
- Confiance : inféré

#### descriptor_copy_56_64 et desc_65_68
- Emplacement : fichier + 56, 9 octets ; fichier + 65, 4 octets
- Taille / type : 9 octets : copie exacte des octets 32 à 40 ; puis 4 octets (`33 01 23 33` en LE64, `32 01 22 22` en LE32 sans `a1`, `33 01 23 22` en BE32 avec `a1`).
- Sémantique : second exemplaire du descripteur de format ; les octets 65 à 68 reprennent l'indicateur `a1` (octet 65 = octet 35), la constante `01`, un octet `0x23` si `a1 = 4` sinon `0x22`, et l'octet 36 (octet 68).
- Valeurs observées : voir le tableau par plateforme (`evidence/diff_plateforme_linux64_vs_windows32.txt`) ; la copie est identique à l'original dans tous les fichiers générés.
- Écriture : recopier les octets 32 à 40 aux octets 56 à 64, puis `[octet35, 01, (a1 ? 23 : 22), octet36]`.
- Preuve : `plateforme_*_no`
- Confiance : inféré

#### encoding (octet 70)
- Emplacement : fichier + 70
- Taille / type : 1 octet
- Sémantique : code d'encodage de la table (correspondance code → nom : changement `specify-sas-formats-encodings`).
- Valeurs observées dans le corpus : `0x14` = 20 (UTF-8, 172 fichiers), `0x1d` = 29 (latin1, 36), `0x3e` = 62 (wlatin1 / windows-1252, 19), `0x8a` = 138 (Shift-JIS, 1), `0x28` = 40 (2 fichiers publics pandas), `0x00` (2 fichiers publics anciens). Le code est **posé par `OUTREP=`** : les tables `plateforme_*` écrites sous une session UTF-8 portent 29 (Unix) ou 62 (Windows). **Attention** : `datareader/test16` et `test19` annoncent 29 et 62 mais leurs octets de chaînes sont de l'UTF-8 (le CSV de référence est en UTF-8) : le code d'encodage n'est donc pas fiable pour décoder une chaîne si le fichier vient d'une conversion `OUTREP=`.
- Valeurs observées : voir ci-dessus : 20 (172 fichiers), 29 (36), 62 (19), 138 (1), 40 (2), 0 (2).
- Correspondance minimale utilisée par le lecteur de référence (la table complète relève de `specify-sas-formats-encodings`) : 20 → `utf-8`, 29 → `latin-1`, 62 → `cp1252`, 138 → `shift_jis` ; 40 (2 fichiers pandas, `iso-8859-15` selon les sources publiques) et 0 (fichiers anciens, session par défaut) : **non établis**, le lecteur doit alors demander l'encodage ou utiliser `latin-1`.
- Écriture : 20 pour UTF-8.
- Preuve : `evidence/diff_encodage_utf8_vs_latin1.txt` ; `compare_csv_external.txt` (notes sur test16/test19)
- Confiance : inféré

#### byte_73
- Emplacement : fichier + 73
- Taille / type : 1 octet
- Sémantique : inconnue ; `0x20` pour Unix 64 bits et BE, `0x10` pour Windows, `0x00` pour `hp_ux_32`.
- Valeurs observées : `0x20` (206), `0x10` (18), `0x00` (8).
- Écriture : `0x20` pour Unix.
- Preuve : `plateforme_*_no`
- Confiance : inconnu

#### file_label_sas_file
- Emplacement : fichier + 84
- Taille / type : 8 octets ASCII `SAS FILE`
- Sémantique : libellé de famille de fichier.
- Valeurs observées : `SAS FILE` dans les 232 fichiers.
- Écriture : recopier.
- Preuve : `evidence/header_coverage.txt`
- Confiance : inféré

#### table_name
- Emplacement : fichier + 92
- Taille / type : 64 octets ASCII complétés par des espaces (puis éventuellement des zéros)
- Sémantique : nom du membre. **Vide** (espaces) dans les 195 tables du corpus généré (écriture d'une étape DATA vers une bibliothèque) ; renseigné (`TEST1`, `TEST10`…) dans les fichiers publics datareader. Les fichiers générés portent donc une signature « nom vide » que SAS accepte.
- Valeurs observées : vide dans les 195 tables du corpus généré ; `TEST1`…`TEST21` dans datareader.
- Écriture : espaces (nom vide) ou le nom en majuscules.
- Preuve : `evidence/header_coverage.txt` ; `datareader/test1`
- Confiance : inféré

#### file_type
- Emplacement : fichier + 156
- Taille / type : 8 octets ASCII
- Sémantique : type de membre SAS (`DATA` = table, `INDEX` = index).
- Valeurs observées : `DATA    ` (table), `INDEX   ` (fichier d'index).
- Écriture : `DATA    `.
- Preuve : `index_n100_ix_no.sas7bndx`
- Confiance : inféré

### 2.2 Horodatages, tailles, versions

#### created et modified
- Emplacement : fichier + 164 + a1 et + 172 + a1
- Taille / type : 8 octets chacun, `f64` : secondes écoulées depuis le 1er janvier 1960 00:00:00 (epoch SAS).
- Sémantique : date et heure de création et de dernière modification du fichier.
- Valeurs observées : deux valeurs identiques dans les fichiers générés (ex. `0x41DF6D21A3AEFA72`) ; valeurs distinctes possibles ailleurs.
- Écriture : l'instant courant en secondes SAS.
- Preuve : lecture des deux champs ; concordance avec `Created`/`Last Modified` de `PROC CONTENTS` non vérifiée octet par octet.
- Confiance : inféré

#### tz_offset_created et tz_offset_modified
- Emplacement : fichier + 180 + a1 et + 188 + a1
- Taille / type : 8 octets chacun, `f64`
- Sémantique : décalage UTC en secondes (+7200 = UTC+2, −18000 = UTC−5).
- Valeurs observées : `0.0` dans les fichiers générés (session en UTC) ; `7200.0` (pandas `cars`, `zero_rows`), `−18000.0` (pandas `test10`), `−14400.0` (datareader `test1`), `10800.0` (readstat `datetime`). Les deux champs sont toujours égaux.
- Écriture : `0.0` (UTC).
- Preuve : lecture sur 5 fichiers publics ; `evidence/header_coverage.txt`
- Confiance : inféré

#### header_length
- Emplacement : fichier + 196 + a1
- Taille / type : `u32`
- Sémantique : taille de l'en-tête en octets ; début de la page 0.
- Valeurs observées : 4 096 à 131 072 ; égale à `page_length` dans 230 fichiers sur 232 (2 fichiers publics anciens font exception).
- Écriture : égale à `page_length` (comportement des fichiers SAS 9.4M7 générés).
- Preuve : `taille du fichier = header_length + page_length × page_count` sur 232 fichiers.
- Confiance : inféré

#### page_length
- Emplacement : fichier + 200 + a1
- Taille / type : `u32`
- Sémantique : taille d'une page. Valeurs observées : 4 096, 8 192, 65 536, 98 304, 106 496, 122 880, 131 072 (`bufsize=` est respecté ; SAS agrandit la page quand une ligne ne tient pas : `taille_wide5000_no` 122 880, `types_char32767_no` 106 496).
- Valeurs observées : 4 096, 8 192, 65 536 (défaut), 98 304, 106 496, 122 880, 131 072.
- Écriture : multiple de 8 ; la règle de choix de SAS n'est pas établie (`inconnu`).
- Preuve : `taille_buf*`, `taille_wide*`, `types_char32767`
- Confiance : inféré

#### page_count
- Emplacement : fichier + 204 + a1
- Taille / type : `u32` (format 32 bits) ou `u64` (format 64 bits) : `w` octets
- Sémantique : nombre de pages après l'en-tête.
- Valeurs observées : 1 (`base_n100_mix_no`), 2 (`base_n3000_mix_no`), 6 (`taille_wide5000_no`), 19 (`taille_n50000_no`).
- Écriture : nombre de pages écrites.
- Preuve : `taille_n50000_no` (19 pages), longueur du fichier
- Confiance : inféré

#### after_page_count
- Emplacement : fichier + 208 + a1 + a2
- Taille / type : `u32`
- Sémantique : inconnue. Vaut 0 dans les fichiers SAS 9.4M7 générés ; vaut 1 dans les fichiers publics 9.4M1 (datareader, pandas `test10`/`test13`).
- Valeurs observées : 0 (SAS 9.4M7 généré), 1 (fichiers publics 9.4M1).
- Écriture : 0.
- Preuve : `evidence/header_coverage.txt`
- Confiance : inconnu

#### sas_release, host, os_version, os_name, os_machine
- Emplacement : fichier + 216 + t (8 octets), + 224 + t (16), + 240 + t (16), + 256 + t (16), + 272 + t (16)
- Taille / type : chaînes ASCII complétées par des zéros
- Sémantique : version de SAS (`9.0401M7` = SAS 9.4 TS1M7), famille de système (`Linux`, `WIN`, `W32_VSPRO`, `X64_SRV16`, `WIN_PRO`), version du noyau (`6.18.48-107.148.` tronquée à 16 octets), nom de l'OS, architecture (`x86_64`). Ils décrivent la machine qui a **écrit** le fichier : les fichiers `OUTREP=WINDOWS_*` produits sur Linux portent encore `Linux`.
- Valeurs observées : release `9.0401M7` (196 fichiers), `9.0401M1` (23), `9.0401M9` (4), `9.0401M6` (3), `9.0401M3` (2), `9.0000M0` (1) ; hôte `Linux/x86_64` (228) ou une des valeurs Windows ci-dessus (4).
- Écriture : libellés libres ; ReadStat écrit `9.0401M5`, `Linux`, `3.10.0-1160.11.1`, `x86_64` (hypothèse non vérifiée ici).
- Preuve : `evidence/header_coverage.txt`
- Confiance : inféré

#### created_low_copy et created_low_xor
- Emplacement : fichier + 288 + t (4 octets), puis + 292 + t, + 296 + t, + 300 + t (4 octets chacun)
- Taille / type : 4 × `u32`
- Sémantique : le mot de 32 bits de poids faible de `created`, puis trois fois ce même mot dont chaque octet est **XOR l'octet correspondant de `"LNoe"`** (`4C 4E 6F 65`). Le mot de poids faible est écrit **octets de poids faible en premier, même dans un fichier big endian** (les 18 fichiers BE de l'analyse ne diffèrent que par cet ordre) ; la relation XOR est vérifiée sur 231 des 232 fichiers (exception : le fichier public ancien `9.0000M0`).
- Valeurs observées : relation constante dans tous les fichiers générés.
- Écriture : calculer comme ci-dessus.
- Preuve : `rea_output/tools/fields.py` (champs) et contrôle de relation sur les 232 fichiers (session 02) ; `evidence/header_coverage.txt`
- Confiance : inféré

#### file_code
- Emplacement : fichier + 320 + t
- Taille / type : `u32`
- Sémantique : valeur de 32 bits propre au fichier (différente à chaque création, corrélée à l'horodatage sans lui être égale : sur 6 fichiers, `mot bas ^ mot haut ^ valeur` a ses 16 bits de poids fort nuls). **Chaque page commence par 4 octets égaux à `file_code XOR (numéro de page + 1)`** (page 0 : `file_code ^ 1`) : vérifié pour 266 des 269 premières pages de fichiers LE64 (3 exceptions non analysées). La formule exacte de `file_code` n'est pas établie.
- Valeurs observées : fichiers LE64 ; relation page/code sur les pages 0 à 2.
- Écriture : tirer une valeur de 32 bits et dériver les signatures de pages ; SAS accepte-t-il une valeur arbitraire ? `inconnu` (renvoi `specify-sas7bdat-writing`).
- Preuve : analyse des signatures de page (`rea_output/evidence/structure_base_n100_mix_no.txt` pour la page 0)
- Confiance : inféré

#### created_copy
- Emplacement : fichier + 328 + t
- Taille / type : `f64`
- Sémantique : copie de `created` (identique dans les 232 fichiers).
- Valeurs observées : identique à `created` dans les 232 fichiers.
- Écriture : recopier `created`.
- Preuve : `evidence/header_coverage.txt`
- Confiance : inféré

### 2.3 Résultats de décompilation de l'en-tête de fichier (2026-10-30)

Source : `evidence/decompile/sas_header_findings.md` et `sas_header_bundle.json`, exécutable `src/sasexe/sas` (confirmation NULL et extension du périmètre du 2026-10-30, `STATE.md`). Le noyau ne traite que le bloc d'en-tête de 512 octets (« UHB ») : les pages sont des blocs opaques pour lui. Les fonctions s'appellent `wcopen` (`wcopen.V@0x1b0010`), `wcclose`, `wcread`, `wctrunc`, `vc_read_uhb`, `vc_write_uhb`, `vc_convert_uhb`, `hc_disk_read_uhb`, `hc_disk_write_uhb`, `hc_disk_validate_member.V` ; leur correspondance avec les primitives `yzb*` des modules `sase7*` est **inférée** par le nom et le rôle. Confirmé = lu dans le code et cohérent avec les 178 fichiers LE64 du corpus ou avec `evidence/write/mutations.tsv`.

| Octets | Constat | Confiance |
|---|---|---|
| 8 à 11 | écrits à 0 ; à la lecture, octet 9 ≤ 1 et octet 11 = 0 exigés (sinon « créé par une version plus récente ») | confirmé |
| 12 à 27 | identifiant (GUID) ; **non vérifié à la lecture** ; à la création, octet 15 = `0x60 + indice du type de membre` (DATA `0x60`, CATALOG `0x63`, INDEX `0x65`…) ; le type de membre est déduit de l'extension du fichier, pas de ces octets | confirmé |
| 28 à 51 | copie héritée dérivée du vecteur 52-75, **ignorée à la lecture** (différente aux octets 41-44, 46 et 49 selon les fichiers) | inféré |
| 52 à 75 | vecteur de représentation des données, comparé à celui de l'hôte sous un masque : comparés 52-54, 56, 59, 61-64, 67 (quartet haut), 69-71, 74-75 ; **ignorés** 55, 57-58, 60, 65-66, 68, 72-73. Les plages admises des octets 73 et 74-75 dépendent d'une fonction de l'extension du noyau hors de `sas` (`wzxncap`) : inconnues | confirmé (masque) / inconnu (plages) |
| 84 à 91 | `SAS FILE` en ASCII **ou en EBCDIC**, sinon rejet (code `0x80403825`) | confirmé |
| 92 à 155 | nom de table : 32 espaces (92-123) puis 32 zéros (124-155) | confirmé |
| 168 à 183 (et 184, 192) | dates de création et de modification en **heure locale** (UTC + décalage), égales à la création ; `modified` est réécrite à la fermeture (`wcclose`) ; les décalages (184, 192) sont des `f64` | confirmé |
| 196+`a1` et 200+`a1` | `page_length` doit être entre **1 024 et 2³⁰**, sinon rejet (`0x80403825`) ; si la valeur demandée est ≤ 0 : 8 192. `header_length` = `page_length` pour DATA et UTILITY lorsqu'un indicateur global est actif, sinon **8 192** (INDEX et CATALOG du corpus : 8 192) | confirmé |
| 216 (`after_page_count`) | bit 0 = vecteur de représentation demandé différent du vecteur par défaut de l'hôte (168 fichiers à 0, 10 à 1) ; bit 1 = bloc d'extension (offset en 344, longueur en 348) | confirmé (bit 0) / inféré (bit 1) |
| 296 à 311 | **clé** `K` = octets 296-299 (copie de 168-171 à la création) ; mots en 300, 304, 308 = `mot de passe haché XOR K XOR "LNoe"` (haché = 0 sans mot de passe) ; à la lecture, tout mot décodé non nul = membre protégé ; toute clé `K` est acceptée si les mots valent `K XOR "LNoe"` (aucun lien avec 168) | confirmé |
| 320 à 327 | `YZBENC01` si la table est chiffrée | confirmé |
| 328 à 331 (`file_code`) | fourni par le module appelant, recopié sans contrôle par `sas` | confirmé (absence de contrôle dans `sas`) |
| 336 | copie de `created` faite par le noyau à chaque écriture d'en-tête | confirmé |
| 360 à `header_length` | non écrits par le noyau (zéros) | confirmé |
| signature de page | `file_code XOR N`, `N` à partir de 1 : 368 pages du corpus, 0 écart ; aucun contrôle de ce type dans `sas` (le contrôle « Expecting page N, got page M » est dans un autre module, non analysé) | inféré |

Ordre des contrôles à l'ouverture et codes de retour : `0x80403825`, `0x80403835`, `0x8040381A`, `0x0040383C`, `0x80000801`, `0x80403827` (détail et correspondance avec les mutations de `evidence/write/mutations.tsv` : `sas_header_findings.md` 1.6). Reste **hors du périmètre analysé** : la page marqueur `0x9000`, le plafond de 255 lignes compressées, les champs de `RowSize` posés par la couche de données (`+0x10`, `+0x58`, `+0x60`, `+0x250`..), `ColumnList`, les drapeaux de nom, la règle de choix de la taille de page et le message « Expecting page N » : le code correspondant n'est ni dans `sas` ni dans les modules `sase7*` déjà vus (probablement dans `sasyz*`/`sase7`, voir `STATE.md`).

---

## 3. Pages

Chaque page occupe `page_length` octets, à l'offset `header_length + k × page_length` (k = 0 … `page_count − 1`). Soit `po = 32` (format 64 bits) ou `16` (format 32 bits). Les offsets ci-dessous sont relatifs au début de la page.

### 3.1 En-tête de page (`po + 8` octets : 40 en 64 bits, 24 en 32 bits)

#### page_signature
- Emplacement : page + 0
- Taille / type : 4 octets (`u32` dans l'endianness du fichier)
- Sémantique : `file_code XOR (k + 1)` où `k` est le numéro de page (voir `file_code`, section 2.2).
- Valeurs observées : relation vérifiée sur 266 des 269 premières pages de fichiers LE64 et sur les pages 0 des fichiers BE (`plateforme_{aix_64,solaris_64,hp_ux_32}_no` : `xor = 1`). La même valeur est recopiée dans RowSize (`page_signature_copy`).
- Écriture : voir `file_code` (renvoi `specify-sas7bdat-writing`).
- Preuve : `evidence/structure_base_n100_mix_no.txt`, `evidence/header_coverage.txt`
- Confiance : inféré

#### page_reserved_4_7
- Emplacement : page + 4, 4 octets
- Taille / type : 4 octets
- Sémantique : inconnue. 0 dans les pages des fichiers LE ; dans les pages des fichiers 64 bits big endian (`plateforme_{aix_64,solaris_64}`), les octets 4 à 7 contiennent des restes de mémoire (par exemple `B8 7F 00 00`, motif d'un pointeur) : valeur non significative.
- Valeurs observées : `00 00 00 00` (fichiers LE) ; restes de mémoire (`B8 7F 00 00`…) en big endian 64 bits.
- Écriture : 0.
- Preuve : `evidence/zones_non_decodees.md`
- Confiance : inconnu

#### free_bytes
- Emplacement : page + 24 (64 bits) / + 12 (32 bits)
- Taille / type : `w` octets (`u64` ou `u32`)
- Sémantique : espace libre résiduel de la page, calculé en réservant en fin de zone de lignes un **bitmap de suppression** (voir 3.5) de `⌈capacité / 8⌉` octets. Pour une page de données : `page_length − (po + 8) − nb_lignes × row_length − ⌈r_max / 8⌉`, avec `r_max` le plus grand nombre de lignes tel que `r_max × row_length + ⌈r_max / 8⌉ + po + 8 ≤ page_length`. Pour une page mixte : `offset du premier octet du sous-en-tête le plus bas − fin des lignes − ⌈mix_capacity / 8⌉` (`mix_capacity` = champ de RowSize).
- Valeurs observées : formule exacte sur 150 pages de données sur 154 et 178 pages mixtes sur 179 ; les 5 exceptions viennent de deux fichiers publics anciens (`cars`, `many_columns` : valeur 0 ou sans réservation du bitmap). En BE 64 bits le champ occupe bien 8 octets (valeur dans les derniers octets).
- Écriture : appliquer la formule.
- Preuve : contrôle sur les 333 pages de données et mixtes non compressées du corpus (session 02, voir `STATE.md`), `evidence/structure_*`
- Confiance : inféré

#### page_type
- Emplacement : page + `po`
- Taille / type : `u16`
- Sémantique : type de page. Le **drapeau `0x0080`** signale qu'un bitmap de lignes supprimées est significatif (3.5). Types observés :

| Valeur | Nom | Contenu | Observé dans |
|---|---|---|---|
| `0x0000` | meta | sous-en-têtes seulement | 12 pages de fichiers non compressés (`taille_wide5000_no` : 5 pages meta) |
| `0x0100` | data | lignes seulement | 154 pages de fichiers non compressés |
| `0x0200` | mix | sous-en-têtes puis lignes | 175 pages ; page 0 de presque toutes les tables |
| `0x0280` | mix + lignes supprimées | idem avec bitmap significatif | 4 pages ; `suppression_n100_del10_*` |
| `0x0400` | amd | un sous-en-tête de signature `FFFFFFFA` | `index_n100_ix_no` (fichier non compressé) ; 5 fichiers compressés publics (`readstat/compression_type_0x*`) |
| `0x4000` | meta2 | non rencontré dans le corpus généré (rôle `inconnu`, pas décodé dans les modules `sase7*`) | `pandas/test_meta2_page` (fichier compressé, page non analysée) |
| `0x9000` | comp | page de lignes compressées | fichiers compressés (hors périmètre, `specify-sas7bdat-compression`) |

- Valeurs observées : `0x0000`, `0x0100`, `0x0200`, `0x0280`, `0x0400` (tableau ci-dessus) ; `0x4000` et `0x9000` dans des fichiers compressés.
- Lecture : les lignes ne sont lues que dans les pages dont `page_type & 0x0F00` vaut `0x0100` ou `0x0200` ; les pages `0x0000`, `0x0400`, `0x4000` ne portent que des sous-en-têtes ; les pages `0x9000` sont compressées (hors périmètre) ; tout autre type est ignoré.
- Écriture : `0x0200` pour la page 0 (sous-en-têtes + premières lignes), `0x0100` pour les suivantes.
- Preuve : `evidence/structure_*.txt`, `evidence/diff_base_vs_suppression.txt` (seul le type passe de `0x0200` à `0x0280`)
- Confiance : inféré

#### block_count
- Emplacement : page + `po` + 2
- Taille / type : `u16`
- Sémantique : nombre de « blocs » de la page = nombre de sous-en-têtes (pointeurs, terminal inclus) **plus** nombre de lignes de la page. Pour une page de données : nombre de lignes. Pour une page mixte : `block_count − subheader_count` = nombre de lignes de la page.
- Valeurs observées : 111 pour la page 0 de `base_n100_mix_no` (11 sous-en-têtes, 100 lignes) ; 2 637 pour `base_n3000_mix_no` (2 626 lignes sur la page mixte, 374 sur la page de données suivante).
- Écriture : sous-en-têtes + lignes.
- Preuve : lecture sans écart de 165 fichiers (`evidence/compare_csv_files.txt`) avec `lignes = block_count − subheader_count` sur les pages mixtes et `block_count` sur les pages de données.
- Confiance : inféré

#### subheader_count
- Emplacement : page + `po` + 4
- Taille / type : `u16`
- Sémantique : nombre de pointeurs de sous-en-têtes. Le dernier est un pointeur terminal (3.2) qui compte dans ce total.
- Valeurs observées : 11 (`base_n100_mix_no`), 10 (`plateforme_*`), 107 (`datareader/test1`, 100 colonnes), 0 (pages de données).
- Écriture : voir 3.2.
- Preuve : idem
- Confiance : inféré

#### page_reserved_after_count
- Emplacement : page + `po` + 6, `u16`
- Taille / type : `u16`
- Sémantique : inconnue. 0 dans toutes les pages non compressées.
- Valeurs observées : 0 dans toutes les pages non compressées.
- Écriture : 0.
- Preuve : `evidence/zones_non_decodees.md`
- Confiance : inconnu

Les octets 8 à `free_bytes − 1` de l'en-tête de page sont à 0 dans tout le corpus non compressé (`inconnu` : zone réservée).

### 3.2 Pointeurs de sous-en-têtes

Les pointeurs commencent à `po + 8` et se suivent sans trou, un par sous-en-tête : `ps = 24` octets (64 bits) ou `12` (32 bits).

#### subheader_pointer
- Emplacement : page + `po` + 8 + `i × ps`, i = 0 … `subheader_count − 1`
- Taille / type : `[offset : w][length : w][compression : u8][type : u8][réservé : ps − 2w − 2 octets, toujours 0]`
- Sémantique : `offset` = position du sous-en-tête depuis le début de la **page** ; `length` = sa longueur ; `compression` : `0` sous-en-tête ordinaire, `1` pointeur terminal (longueur 0, ou description de la zone libre dans une page amd), `4` ligne compressée (hors périmètre) ; `type` : `1` pour ColumnText, ColumnName, ColumnAttributes, ColumnList, sous-en-tête amd ; `0` pour RowSize, ColumnSize, SubheaderCounts, FormatAndLabel et le pointeur terminal des pages mixtes.
- Valeurs observées : voir `evidence/structure_base_n100_mix_no.txt`. Les sous-en-têtes sont **empilés depuis la fin de la page vers le début** : le premier se termine à `page_length` (RowSize : offset `0xFCD8` + longueur `0x328` = `0x10000`), chaque suivant se termine là où commence le précédent. Le pointeur terminal répète l'offset du dernier sous-en-tête avec `length = 0`, `compression = 1`, `type = 0`.
- Écriture : empiler les sous-en-têtes dans cet ordre, puis écrire le pointeur terminal.
- Preuve : `evidence/structure_*.txt` ; réservé nul vérifié sur toutes les pages non compressées
- Confiance : inféré

Ordre observé sur la page 0 d'une table de `n` colonnes (`n ≥ 1`) : RowSize, ColumnSize, SubheaderCounts, ColumnText (1 ou plusieurs), ColumnName (1 ou plusieurs), ColumnAttributes (1 ou plusieurs), [ColumnList], FormatAndLabel × `n`, pointeur terminal. Quand les métadonnées dépassent une page (`taille_wide5000_no`), elles s'étendent sur des pages `0x0000` successives et le premier ColumnText/ColumnName/ColumnAttributes se répète par blocs (le lecteur de référence les concatène dans l'ordre d'apparition). Le nombre de FormatAndLabel est égal au nombre de colonnes dans toutes les tables non compressées examinées ; ColumnList est présent sur 36 tables de 3 colonnes et absent sur 13 : **la condition de présence de ColumnList n'est pas établie** (`inconnu`).

### 3.3 Position et nombre des lignes

- **Page de données (`0x0100`)** : les `block_count` lignes sont contiguës à partir de `po + 8` (pas de pointeurs). La ligne `i` occupe `[po + 8 + i × row_length, po + 8 + (i+1) × row_length)`.
- **Page mixte (`0x0200`/`0x0280`)** : les lignes commencent à `⌈(po + 8 + subheader_count × ps) / 8⌉ × 8` (premier multiple de 8 après la table des pointeurs ; le décalage est nul en 64 bits, et de 0 ou 4 octets en 32 bits selon la parité du nombre de pointeurs : vérifié sur `datareader/test1` avec 107 pointeurs). Elles sont au nombre de `block_count − subheader_count`.
- Les pages sont lues dans l'ordre ; on s'arrête quand `row_count` lignes (RowSize) ont été lues, lignes supprimées comprises.
- Il n'y a pas de remplissage entre deux lignes. `row_length` est un multiple de 8 quand la ligne contient au moins une variable numérique (24 pour `c $5` + 2 numériques), sinon la somme des longueurs (5 pour `c $5` seul).
- Preuve : lecture sans écart de `base_n3000_mix_no` (page mixte de 2 626 lignes puis page de données de 374), `taille_n50000_no` (19 pages), `datareader/test1`.
- Confiance : inféré

### 3.4 Pages d'index (amd)

La page `0x0400` de `index_n100_ix_no` contient un sous-en-tête de signature `FFFFFFFA` (44 octets : `FA FF FF FF FF FF FF FF | 01 00 00 00 00 00 00 00 | 00 00 00 00 00 00 10 00 | 00 00 05 00 01 00 02 00 | 00 …`) et un pointeur terminal décrivant la zone libre (`offset 0x58`, `length 0xFF7C`, `compression = 1`, `type = 1`). Le contenu de ce sous-en-tête est `inconnu` ; le fichier d'index `.sas7bndx` n'est pas décrit (hors périmètre). La table indexée garde son format ordinaire et le drapeau `0x0001` du champ `name_flags` de la colonne indexée (4.7). Le compteur de SubheaderCounts pour la signature `FFFFFFFA` vaut `(2, 1, 2, 1)` : page 2, position 1.

- Preuve : `evidence/structure_index_n100_ix_no.txt`
- Confiance : inconnu

### 3.5 Lignes supprimées

#### deleted_rows_bitmap
- Emplacement : page + (`free_bytes` + fin des lignes de la page), où « fin des lignes » = offset du début des lignes + `nb_lignes × row_length`
- Taille / type : `⌈nb_lignes / 8⌉` octets significatifs (la zone réservée fait `⌈capacité / 8⌉` octets)
- Sémantique : un bit par ligne de la page, **bit de poids fort en premier** ; `1` = ligne supprimée. N'est lu que si `page_type & 0x0080`. Les lignes supprimées restent dans le fichier (elles comptent dans `row_count`) ; un lecteur « comme SAS » les exclut. `RowSize.deleted_count` donne leur nombre total.
- Précisions : « nb_lignes » est le nombre de lignes **physiques** de la page (`block_count − subheader_count` pour une page mixte), suppressions comprises ; `free_bytes` est le champ de la page (section 3.1) ; le plafond `row_count` s'applique après la lecture du bitmap.
- Valeurs observées : `suppression_n100_del10_no` : lignes 3, 13, …, 93 (index 0-based 2, 12, …, 92) supprimées, bitmap `20 08 02 00 80 20 08 02 00 80 20 08 …` (13 octets significatifs, à l'offset 63 331 de la page, 329 octets réservés = ⌈2 626 / 8⌉) ; 90 lignes lues contre 90 `Observations` de `PROC CONTENTS` et le CSV de référence.
- Écriture : poser `0x0080`, écrire le bitmap à l'endroit réservé, mettre à jour `deleted_count`.
- Preuve : `suppression_n100_del10_no` (un seul fichier, page mixte) ; `evidence/compare_csv_files.txt` ; `evidence/diff_base_vs_suppression.txt` (seuls le type, `deleted_count`, le bitmap et `field_2` de RowSize changent). **Pas de contre-exemple** : l'emplacement pour une page de données (`0x0180`), un fichier multipage et un fichier 32 bits n'ont pas été observés.
- Confiance : inféré

---

## 4. Sous-en-têtes

Un sous-en-tête est une suite d'octets à l'offset indiqué par son pointeur (3.2). Il commence par une **signature** de `w` octets. Dans les tableaux, « mot » = entier de `w` octets (`u64` ou `u32`) dans l'endianness du fichier et `m(k)` = mot d'indice `k` (offset `k × w`).

### 4.1 Signatures

La valeur de signature est un entier de 32 bits ; en 64 bits elle est étendue sur 8 octets (étendue par des `FF` pour les valeurs `FFFFxxxx`, par des zéros pour `F7F7F7F7`/`F6F6F6F6`), sauf en big endian 64 bits pour RowSize et ColumnSize.

| Structure | Valeur | Octets en 32 bits LE | Octets en 64 bits LE | Octets en 64 bits BE | Type de pointeur |
|---|---|---|---|---|---|
| RowSize | `F7F7F7F7` | `F7 F7 F7 F7` | `F7 F7 F7 F7 00 00 00 00` | `F7 F7 F7 F7 FF FF FB FE` | 0 |
| ColumnSize | `F6F6F6F6` | `F6 F6 F6 F6` | `F6 F6 F6 F6 00 00 00 00` | `F6 F6 F6 F6 FF FF FB FE` | 0 |
| SubheaderCounts | `FFFFFC00` | `00 FC FF FF` | `00 FC FF FF FF FF FF FF` | `FF FF FF FF FF FF FC 00` | 0 |
| ColumnText | `FFFFFFFD` | `FD FF FF FF` | `FD FF FF FF FF FF FF FF` | `FF…FF FD` | 1 |
| ColumnName | `FFFFFFFF` | `FF FF FF FF` | `FF` × 8 | `FF` × 8 | 1 |
| ColumnAttributes | `FFFFFFFC` | `FC FF FF FF` | `FC FF FF FF FF FF FF FF` | `FF…FF FC` | 1 |
| FormatAndLabel | `FFFFFBFE` | `FE FB FF FF` | `FE FB FF FF FF FF FF FF` | `FF…FB FE` | 0 |
| ColumnList | `FFFFFFFE` | `FE FF FF FF` | `FE FF FF FF FF FF FF FF` | `FF…FF FE` | 1 |
| (amd) | `FFFFFFFA` | — | `FA FF FF FF FF FF FF FF` | — | 1 |

**Règle de lecture des signatures.** En 32 bits : `u32` dans l'endianness du fichier. En 64 bits little endian : `u32` à l'offset 0, les 4 octets suivants valant `00 00 00 00` ou `FF FF FF FF`. En 64 bits big endian : si les 4 premiers octets sont `F7 F7 F7 F7` ou `F6 F6 F6 F6` (RowSize, ColumnSize), la valeur est ce `u32` (les 4 octets suivants valent `FF FF FB FE`) ; sinon la valeur est le `u32` formé par les 4 derniers octets du mot de 8 octets (`FF FF FF FF` en tête). En 32 bits big endian, le `u32` est lu en big endian (`FF FF FC 00` pour SubheaderCounts : observé sur `plateforme_hp_ux_32_no`).

Signatures `FFFFFFFB` et `FFFFFFF9` n'apparaissent que comme entrées de SubheaderCounts (jamais comme sous-en-tête dans le corpus). Les signatures de ce tableau sont vérifiées par `evidence/structure_*.txt` ; le tableau d'ensemble des signatures par variante est `inféré` pour tout ce qui n'a pas été observé (32 bits BE complet, 64 bits BE pour les structures autres que RowSize, ColumnSize, SubheaderCounts, ColumnText, ColumnName, ColumnAttributes, FormatAndLabel).

- Preuve : signatures observées sur 232 fichiers (`evidence/zones_non_decodees.md` pour les zones restantes)
- Confiance : inféré

### 4.2 RowSize (signature `F7F7F7F7`)

Longueur 808 octets (64 bits) ou 480 octets (32 bits) dans tout le corpus.

#### row_marker — `m(1)`
- Emplacement : RowSize + `w`
- Taille / type : mot
- Sémantique : marqueur dont la valeur dépend de la release et du format, **sans lien fiable avec la compression** : un lecteur ne doit pas l'utiliser pour détecter un fichier compressé (seul le littéral `SASYZCRL`/`SASYZCR2` du premier bloc de texte le fait, section 4.5).
- Valeurs observées : `0xF0` (240) dans 169 fichiers non compressés SAS 9.4M7 (32 et 64 bits, LE et BE) ; `0x131` (305) dans les fichiers compressés SAS 9.4M7 64 bits ; `0x28B` (651) dans les fichiers 32 bits des fichiers publics 9.4M1 (compressés ou non : `datareader/test1`, `test10`…) ; 205 (`pandas/cars`) ; 0 (`pandas/many_columns`).
- Écriture : `0xF0` (valeur des fichiers non compressés SAS 9.4M7).
- Preuve : tableau des RowSize du corpus (232 fichiers), lecture indépendante par un second lecteur écrit à partir de ce document (session 02, `STATE.md`)
- Confiance : inféré

#### row_field_2 — `m(2)`
- Emplacement : RowSize + `2w`
- Taille / type : mot
- Sémantique : inconnue ; croît avec le nombre de colonnes (10 pour 1 colonne, 13 pour 3 colonnes, 311 pour 301 colonnes, 5 016 pour `taille_wide5000_no`) et vaut 24 pour `suppression_n100_del10_no` contre 13 pour la même table sans suppression. Hypothèse : taille cumulée d'un ensemble de structures (`inconnu` tant que non établi).
- Valeurs observées : 10, 12, 13, 14, 15, 24, 31, 36, 311, 5 016 selon les tables.
- Écriture : calculer comme SAS (non établi) ; une valeur arbitraire est-elle acceptée ? `inconnu`.
- Preuve : `evidence/diff_base_vs_suppression.txt`
- Confiance : inconnu

#### row_flags — `m(4)`
- Emplacement : RowSize + `4w`
- Taille / type : mot
- Sémantique : `0x00223011` (non compressé) ou `0x0022303A` (compressé, `catalogue_cntlout`). Bits non décodés.
- Valeurs observées : idem.
- Écriture : `0x00223011`.
- Preuve : tableau des RowSize du corpus (session 02)
- Confiance : inféré

#### row_length — `m(5)`
- Emplacement : RowSize + `5w`
- Taille / type : mot
- Sémantique : longueur d'une ligne en octets (24 pour `c $5` + 2 numériques ; 5 pour `c $5` seul ; 8 pour une colonne numérique ; 40 008 pour `taille_wide5000_no`).
- Valeurs observées : 5, 8, 16, 20, 24, 208, 304, 512, 1 128, 2 408, 32 776, 40 008 octets selon les tables.
- Écriture : voir 5.1 (multiple de 8 quand il y a une variable numérique).
- Preuve : lecture sans écart de 165 fichiers
- Confiance : inféré

#### row_count — `m(6)`
- Emplacement : RowSize + `6w`
- Taille / type : mot
- Sémantique : nombre de lignes **physiques**, lignes supprimées comprises. Répété en fin de structure (`row_count_copy`, ci-dessous).
- Valeurs observées : 0, 1, 2, 3, 20, 100, 200, 3 000, 50 000 selon les tables.
- Écriture : nombre de lignes écrites.
- Preuve : idem
- Confiance : inféré

#### deleted_count — `m(7)`
- Emplacement : RowSize + `7w`
- Taille / type : mot
- Sémantique : nombre de lignes marquées supprimées dans tout le fichier. 10 pour `suppression_n100_del10_*`, 0 ailleurs. Répété en fin de structure (`deleted_count_copy`).
- Valeurs observées : 0 (194 tables), 10 (`suppression_n100_del10_*`).
- Écriture : voir 3.5.
- Preuve : `evidence/diff_base_vs_suppression.txt` (`+0x038 : 00 / 0a`)
- Confiance : inféré

#### col_count_p1 et col_count_p2 — `m(9)`, `m(10)`
- Emplacement : RowSize + `9w` et `10w`
- Taille / type : 2 mots
- Sémantique : nombre de colonnes (p1) et complément (p2). `p1` = nombre de colonnes pour toutes les tables jusqu'à 301 colonnes (1, 2, 3, 4, 5, 21, 100, 301) ; pour `taille_wide5000_no` (5 001 colonnes d'après ColumnSize) `p1 = 801` et `p2 = 1 395` : **la signification n'est pas établie au-delà d'un seul groupe de métadonnées** (sans doute le nombre de colonnes décrites sur la première page ou dans le premier bloc).
- Valeurs observées : `p1` = nombre de colonnes jusqu'à 301 ; `p2` = 0 sauf `taille_wide5000_no` (801 et 1 395).
- Écriture : p1 = nombre de colonnes, p2 = 0 pour moins de 300 colonnes.
- Preuve : tableau des RowSize du corpus
- Confiance : inféré (≤ 301 colonnes) / inconnu (au-delà)

#### row_field_11, row_field_12 — `m(11)`, `m(12)`
- Emplacement : RowSize + `11w` et `12w`
- Taille / type : 2 mots
- Sémantique : inconnue. `m(12)` = nombre de colonnes sauf `metadonnees_nom32_*` (valeur 32 = longueur maximale d'un nom de variable) ; `m(11)` vaut 0, 26, 28, 32, 80, 816, 13 064 selon les tables.
- Valeurs observées : `m(12)` : 1, 2, 3, 4… (nombre de colonnes), 32 (`metadonnees_nom32_*`) ; `m(11)` : 0, 26, 28, 32, 80, 816, 13 064.
- Écriture : voir 5.1.
- Preuve : tableau des RowSize du corpus
- Confiance : inconnu

#### page_length_copy — `m(13)`
- Emplacement : RowSize + `13w`
- Taille / type : mot
- Sémantique : taille de page (égale à `page_length`).
- Valeurs observées : égale à `page_length` dans tous les fichiers non compressés (4 096, 8 192, 65 536, 98 304, 106 496, 122 880, 131 072).
- Écriture : `page_length`.
- Preuve : tableau des RowSize du corpus
- Confiance : inféré

#### mix_capacity — `m(15)`
- Emplacement : RowSize + `15w`
- Taille / type : mot
- Sémantique : nombre maximum de lignes que SAS pense pouvoir placer sur une page mixte (2 626 pour 24 octets par ligne, 3 936 pour 16, 7 836 pour 8, 12 423 pour 5…). Sert à dimensionner le bitmap de suppression réservé (3.1, `free_bytes`). **La formule n'est pas établie** : elle dépend de la taille des métadonnées et de `row_length`, mais l'estimation `(offset du premier sous-en-tête − début des lignes) / row_length` s'écarte de 13 à 310 lignes.
- Valeurs observées : 2 626 (24 octets/ligne), 3 936 (16), 7 836 (8), 12 423 (5), 304 (208), 79 (`taille_buf4096_no`), 249 (`taille_buf8192_no`).
- Écriture : copier la formule de SAS (non établie) ; le lecteur n'utilise pas ce champ (il utilise `block_count − subheader_count`).
- Preuve : tableau des RowSize du corpus, `evidence/structure_*`
- Confiance : inconnu

#### row_ff_fill — `m(16)`, `m(17)`
- Emplacement : RowSize + `16w`, 2 mots
- Taille / type : 2 mots (`2w` octets)
- Sémantique : remplissage `FF` sur `2w` octets, constant.
- Valeurs observées : `FF` × `2w` dans tous les fichiers.
- Écriture : `FF` × `2w`.
- Preuve : `evidence/zones_non_decodees.md`
- Confiance : inféré

#### page_signature_copy — `m(55)` (4 octets)
- Emplacement : RowSize + `55w`, 4 octets
- Taille / type : 4 octets (dans l'endianness du fichier)
- Sémantique : copie des 4 premiers octets de la page 0 (`page_signature` de la page 0). Vérifié sur 172 fichiers.
- Valeurs observées : égal aux 4 premiers octets de la page 0 dans 172 fichiers sur 172.
- Écriture : recopier.
- Preuve : tableau des RowSize du corpus
- Confiance : inféré

#### locators — début à `512` (64 bits) ou `264` (32 bits), 10 mots
- Emplacement : RowSize + 512 (64 bits) / + 264 (32 bits), 5 couples de 2 mots
- Taille / type : 10 mots (5 couples de 2 mots, `u16` au début de chaque emplacement)
- Sémantique : cinq couples `(1, position)` : numéro de page de métadonnées (1) et position (1-based, dans la table de pointeurs) de : ColumnSize, **dernier** FormatAndLabel, `subheader_count + 1`, **dernier bloc** de la page (`block_count`), **premier** FormatAndLabel. Chaque valeur est un `u16` placé au début d'un emplacement de `w` octets (en BE le `u16` est dans les 2 premiers octets de l'emplacement). Établi sur les tables dont toutes les métadonnées tiennent sur la page 0.
- Valeurs observées : `(1,2) (1,10) (1,12) (1,111) (1,8)` pour `base_n100_mix_no` ; `(1,2) (1,9) (1,11) (1,110) (1,7)` pour `plateforme_linux_x86_64_no`.
- Écriture : recalculer.
- Preuve : `evidence/diff_base_vs_n2` (champ `+0x238 : 6f / 0d` = `block_count`) ; tableau des RowSize
- Confiance : inféré

#### row_count_copy et deleted_count_copy — `longueur − 32` et `longueur − 24`
- Emplacement : RowSize + (longueur du sous-en-tête − 32) et (longueur − 24), 8 octets (`u64`, y compris en format 32 bits)
- Taille / type : 2 × `u64`
- Sémantique : copies de `row_count` et de `deleted_count`.
- Valeurs observées : `0x0308` = 100 et `0x0310` = 0 (ou 10) pour `base_n100_mix_no` / `suppression_n100_del10_no` ; en 32 bits BE la valeur est dans les 4 derniers octets du `u64`.
- Écriture : recopier.
- Preuve : `evidence/diff_base_vs_n2.txt` (`+0x308 : 64 / 02`), `evidence/diff_base_vs_suppression.txt` (`+0x310 : 00 / 0a`)
- Confiance : inféré

#### row_tail
- Emplacement : RowSize + 520 … + 807 (64 bits) ou + 344 … + 447 (32 bits), hors `locators`, `row_count_copy` et `deleted_count_copy` ; et les mots 18 à 54 et 56 à 63
- Taille / type : suite de `u16` placés au début d'emplacements de `w` octets (valeurs 0 pour la plupart)
- Sémantique : références `(index de bloc de texte, offset, longueur)` vers le libellé du fichier, la procédure créatrice (`DATASTEP`) et les littéraux de compression ; compteurs. Non décodées.
- Valeurs observées : 40 valeurs distinctes sur 161 sous-en-têtes LE64 (`evidence/zones_non_decodees.md`, `RowSize … 0x250, 216`) ; les mots 18 à 54 et 56 à 63 sont nuls dans tout le corpus.
- Écriture : recopier une valeur d'un fichier de même forme (identifiants `U12`, `U13` de `60_inconnues.md`).
- Preuve : `evidence/zones_non_decodees.md`
- Confiance : inconnu

### 4.3 ColumnSize (signature `F6F6F6F6`)

#### column_count — `m(1)`
- Emplacement : ColumnSize + `w`
- Taille / type : mot
- Sémantique : nombre de colonnes (variables) de la table. 1, 2, 3, 4, 5, 9, 21, 100, 301, 392, 5 001 dans le corpus. Un second mot (`m(2)`) est à 0 sauf 5 fichiers (valeur 1, `inconnu`).
- Valeurs observées : 1, 2, 3, 4, 5, 9, 21, 100, 301, 392, 5 001 selon les tables.
- Écriture : nombre de colonnes.
- Preuve : lecture sans écart de 165 fichiers ; `check_contents` : 195 tables
- Confiance : inféré

### 4.4 SubheaderCounts (signature `FFFFFC00`)

Longueur 600 octets (64 bits) ou 304 (32 bits). Structure : `m(0)` signature, `m(1)` ≈ taille du premier bloc ColumnText (écart de 0 pour 126 fichiers, de 8 pour 46, autres valeurs pour 6 fichiers : règle non établie), `m(2)` = nombre total de sous-en-têtes de signatures `FFFFFFFC`, `FFFFFFFD`, `FFFFFFFF`, `FFFFFFFE`, `FFFFFFFB`, `FFFFFFFA`, `FFFFFFF9` présents dans le fichier (3 sans ColumnList, 4 avec, 5 avec un index, 9 pour `taille_wide5000_no` ; **confirmé** `yzrhset@0x114b30`, 0 écart sur 178 fichiers LE64), `m(3)` = 7 (nombre d'entrées), `m(14)` = deux octets : `0x0C` (12 emplacements : `600 = 120 + 12 × 40`) puis `0x07` (7 utilisés), d'où la constante `0x070C` (**confirmé** `yzrhset@0x114b30` ; les entrées préchargées ne sont lues que si l'octet des emplacements dépasse 6), puis 7 entrées de `5w` octets à partir de `15w` : `[signature][page du premier][position du premier][page du dernier][position du dernier]`.

Entrées observées, dans l'ordre : `FFFFFFFC` (ColumnAttributes), `FFFFFFFD` (ColumnText), `FFFFFFFF` (ColumnName), `FFFFFFFE` (ColumnList), `FFFFFFFB` (5ᵉ entrée ; **ce n'est pas FormatAndLabel**, qui n'est pas suivi dans SubheaderCounts ; compteurs à 0), `FFFFFFFA`, `FFFFFFF9` (compteurs à 0, sauf `FFFFFFFA` = `(2,1,2,1)` pour le fichier à index). Exemple pour `base_n100_mix_no` : ColumnAttributes `(1,6,1,6)`, ColumnText `(1,4,1,4)`, ColumnName `(1,5,1,5)`, ColumnList `(1,7,1,7)`.

- Preuve : `evidence/zones_non_decodees.md`, lecture des 156 SubheaderCounts LE64
- Confiance : inféré (positions des entrées) / inconnu (`m(2)`, mots restants)

### 4.5 ColumnText (signature `FFFFFFFD`)

#### column_text_block
- Emplacement : ColumnText + `w`
- Taille / type : `[text_size : u16][texte : text_size − 2 octets]`, suivi d'un reliquat nul. Le **bloc** est la suite des `text_size` octets qui commencent à l'offset `w` ; toutes les références `(index, offset, longueur)` (noms, formats, labels) visent un bloc et leurs offsets sont comptés depuis le début de ce bloc, le champ `text_size` étant à l'offset 0. `index` = rang du ColumnText (0-based) dans l'ordre d'apparition dans le fichier.
- Précision : `text_size` compte les octets du bloc **y compris le champ `text_size` lui-même** (bloc = `[w, w + text_size)`). Noms, formats, informats et labels sont décodés avec l'encodage de l'octet 70 (vérifié sur les labels UTF-8 de `metadonnees_lab_utf8_no` ; non vérifié pour latin1 ou Shift-JIS).
- Constat du jalon J1 (2026-11-03) : dans `pandas/many_columns` (release `9.0101M0`), le sous-en-tête ColumnText fait 3 436 octets pour `text_size = 3 416` et 160 références de format (par exemple `(0, 3416, 4)` = `TIME`) visent des octets **après** `[w, w + text_size)`, dans le reste du sous-en-tête. `ref_reader.py` les lisait comme vides (hors bloc) ; le lecteur Rust prend pour bloc tout le reste du sous-en-tête (de `w` à sa fin). Aucun fichier SAS 9.4M7 du corpus n'a de référence au-delà de `text_size` (195 `PROC CONTENTS` concordants), donc l'écart ne touche que cet ancien fichier public ; la signification de `text_size` dans cette release reste `inconnu`.
- Contenu du premier bloc (offsets du bloc) : `[8..12)` `00 00 00 <code d'encodage>` (recopie de l'octet 70 ; `00 00 00 00` dans les fichiers publics anciens) ; `[12..20)` 8 octets : espaces, ou le littéral de compression `SASYZCRL` (RLE, `COMPRESS=CHAR`) ou `SASYZCR2` (RDC, `COMPRESS=BINARY`) ; `[20..28)` espaces ; `[28..36)` `DATASTEP` (procédure créatrice) ou espaces ; puis les textes : noms, formats, informats, labels, concaténés sans séparateur, dans l'ordre de leur création.
- Sémantique : bloc de texte partagé par les noms, formats, informats et labels de colonnes.
- Valeurs observées : 147 fichiers avec encodage 20 et littéral vide ; littéraux `SASYZCRL`/`SASYZCR2` dans tous les fichiers compressés ; créateur `DATASTEP` (177 fichiers).
- Écriture : `00 00 00 <enc>`, 8 espaces, 8 espaces, `DATASTEP`, puis les textes.
- Preuve : `evidence/zones_non_decodees.md` ; décodage correct des noms/labels de 195 tables (`evidence/check_contents_files.txt`)
- Confiance : inféré

### 4.6 ColumnName (signature `FFFFFFFF`)

#### column_name_entry
- Emplacement : ColumnName + `w + 8` + 8 × j, j = 0 … (nombre d'entrées − 1) ; nombre d'entrées = `(longueur − 2w − 12) / 8`
- Taille / type : 8 octets : `[index de bloc : u16][offset dans le bloc : u16][longueur : u16][réservé : u16 = 0]` ; entre la signature et les entrées : `[size : u16]` puis 6 octets nuls ; `size = 8 + 8 × nombre d'entrées` ; reliquat nul à la fin.
- Sémantique : référence au nom de la colonne `j` (dans l'ordre global des colonnes, tous ColumnName concaténés). L'ordre des colonnes est **l'ordre de création des variables** (`#` de `PROC CONTENTS`), vérifié sur 165 tables ; les noms de plus d'un bloc ou d'un chunk sont lus dans l'ordre d'apparition.
- Valeurs observées : noms de 1 à 32 octets.
- Écriture : une entrée par variable dans l'ordre de création.
- Preuve : `evidence/check_contents_files.txt` (numéro, nom pour 195 tables)
- Confiance : inféré

### 4.7 ColumnAttributes (signature `FFFFFFFC`)

#### column_attribute_entry
- Emplacement : ColumnAttributes + `w + 8` + (`w + 8`) × j ; nombre d'entrées = `(longueur − 2w − 12) / (w + 8)`
- Taille / type : `[offset dans la ligne : w][largeur : u32][name_flags : u16][type : u8][réservé : u8 = 0]` ; entre la signature et les entrées : `[size : u16]` + 6 octets nuls ; reliquat nul à la fin.
- Sémantique : `type` : 1 = numérique, 2 = caractère. `largeur` = longueur de stockage (3 à 8 pour un numérique, 1 à 32 767 pour un caractère). `offset` = position de la valeur dans la ligne. `name_flags` : `0x0400` pour les noms de 1 à 8 octets (5 301 colonnes), `0x0800` pour un nom de 32 octets (3 colonnes) ; **`0x0401` pour la colonne indexée** (`index_n100_ix_no`, colonne `i`) : le bit 0 marque une colonne indexée. La règle pour les noms de 9 à 31 octets n'a pas été observée.
- Valeurs observées : type 1 (5 526 colonnes), type 2 (92) ; largeurs 1 à 32 767 ; `name_flags` `0x0400`, `0x0800`, `0x0401`.
- Écriture : voir 5.1 pour les offsets ; `0x0400` si nom ≤ 8 octets.
- Preuve : `evidence/check_contents_files.txt` (type, longueur) ; décompte des `name_flags` (session 02)
- Confiance : inféré (offset, largeur, type) / inconnu (`name_flags` hors 0x0400, 0x0800, 0x0401)

### 4.8 FormatAndLabel (signature `FFFFFBFE`)

#### format_and_label_entry
- Emplacement : un sous-en-tête par colonne, dans l'ordre des colonnes ; longueur 64 (64 bits) ou 52 (32 bits)
- Taille / type : en 64 bits : `format_width u16` à `+0x18`, `format_decimals u16` à `+0x1A`, `informat_width u16` à `+0x1C`, `informat_decimals u16` à `+0x1E`, `informat_ref` (3 × u16 : index, offset, longueur) à `+0x28`, `format_ref` à `+0x2E`, `label_ref` à `+0x34`. En 32 bits : `+0x0C`, `+0x0E`, `+0x10`, `+0x12`, `+0x1C`, `+0x22`, `+0x28` (soit les offsets 64 bits − 12 pour les largeurs, − 12 pour les références).
- Sémantique : le **nom** de format ou d'informat (sans largeur ni point) est dans le texte (`DATE`, `DOLLAR`, `COMMA`, `$CHAR`, `MMDDYY`, `BEST`, `$`) ; la largeur et le nombre de décimales sont dans les `u16`. `PROC CONTENTS` affiche `NOM` + largeur + `.` + décimales. Longueur nulle = pas de format. Le label est un texte de 0 à 256 octets (256 vérifié).
- Valeurs observées : formats `DATE9.`, `DATETIME19.`, `TIME8.`, `DOLLAR12.2`, `BEST12.`, `COMMA10.2`, `PERCENT8.1`, `E10.`, `$CHAR20.` ; informats `DATE9.`, `DATETIME19.`, `TIME8.`, `COMMA10.2`, `$CHAR20.` ; labels ASCII, 256 caractères, UTF-8 (`Libellé accentué é ü ñ — 日本`) ; 195 tables sans écart avec `PROC CONTENTS` (format, informat, label, 64 bits).
- Écriture : écrire largeur et décimales dans les `u16`, ajouter le nom au bloc de texte et noter la référence.
- Preuve : `evidence/check_contents_files.txt` (64 bits : 195 tables dont toutes celles qui ont un format, un informat ou un label) ; en 32 bits, `check_contents` confirme nom, type et longueur (tables `plateforme_*`, sans format ni label) mais **les offsets des champs format, informat et label en 32 bits ne sont vérifiés que par `datareader/test1`** (formats et labels lus plausibles, non comparés à `PROC CONTENTS`)
- Confiance : inféré (64 bits) / inféré, preuve faible (32 bits)

### 4.9 ColumnList (signature `FFFFFFFE`)

Présent pour 36 tables de 3 colonnes mais absent pour 13 autres ; longueur de 52 à 13 092 octets. Le contenu observé (3 colonnes) : `[size u16 = 0x24][u16 0x7FC8][4 octets nuls]`, `[u16 0x1C][6 nuls]`, `[3, 3, 1, 3]` (4 `u16`), `[6 octets d'adresse mémoire, ex. 30 F5 1B 05 DA 7F][3]`, `[FFFF][2]` : on y voit un nombre de colonnes, une liste de `u16` et **un pointeur mémoire non initialisé** (`7F…` en poids fort, différent à chaque fichier). Les mots dépendant de la mémoire sont les seuls octets non reproductibles entre deux écritures d'une même table. Rôle : table de hachage ou de permutation des colonnes, selon PDF Shotwell/ReadStat (hypothèse).
- Écriture : l'omettre ? `inconnu` (SAS sait-il ouvrir un fichier sans ColumnList ? Il existe des tables de 3 colonnes sans ColumnList écrites par SAS lui-même).
- Preuve : `evidence/zones_non_decodees.md` (`ColumnList` : 17 valeurs distinctes pour la même longueur)
- Confiance : inconnu

---

## 5. Lignes et valeurs

### 5.1 Disposition d'une ligne

Une ligne occupe `row_length` octets ; la colonne `j` y occupe `[offset_j, offset_j + largeur_j)` (ColumnAttributes). Aucun octet de ligne n'est interprété hors des colonnes. Observations sur les fichiers écrits par SAS :

- les colonnes numériques viennent d'abord, en ordre de création, à partir de l'offset 0 (alignées sur 8 quand leur largeur est 8) ; les colonnes caractères suivent, sans alignement (`datareader/test1` : 75 numériques sur 600 octets, puis des caractères aux offsets 600, 609, 618, 627…) ;
- `row_length` = somme des largeurs, arrondie au multiple de 8 supérieur dès qu'il existe au moins une variable numérique ; sans variable numérique, la somme brute (5 pour un seul `$5`) ;
- les octets de remplissage de fin de ligne ne portent pas d'information.

- Écriture : numériques (8 octets) puis caractères ; `row_length` arrondie.
- Preuve : lecture sans écart de 165 fichiers non compressés
- Confiance : inféré

### 5.2 Numériques

- Largeur 8 : double IEEE 754 binaire 64 dans l'endianness du fichier.
- Largeur 3 à 7 : seuls les `n` octets de **poids fort** du double sont stockés ; en little endian ce sont les `n` derniers octets de la représentation 8 octets, en big endian les `n` premiers. Pour reconstituer la valeur : compléter les octets de poids faible par des zéros. Vérifié pour les largeurs 3, 4, 5, 6, 7 et 8 (`types_num3…num8_no`, 0 écart).
- Largeurs 1 et 2 : non observées (SAS sous Linux ne les produit pas) ; Parso et pandas divergent (D15 de `00_etat_de_l_art.md`) : `inconnu`.
- Valeurs extrêmes : `0`, `±1`, `±1e-300`, `±1e300`, `2.2250738585072e-308` (plus petit normalisé), `1.7976931348623157e308`, `123456789012345`, `0.1`, `2^53` vérifiés (`valeurs_extremes_no`). Les valeurs de `types_num3…7` (`1.25`, `2.5`…) sont représentables sur 3 octets : le comportement pour une valeur **non** représentable (troncature de la mantisse) n'est pas observé dans le corpus. Le CSV de référence les écrit avec 14 chiffres significatifs (`1.7976931348623E308`) ; la comparaison tolère un écart relatif de `1e-13`.
- Infinis et NaN ordinaires : SAS ne sait pas en produire, non observés.
- Dates, heures, dates-heures : doubles ordinaires ; l'interprétation vient du format (nombre de jours, de secondes depuis le 1er janvier 1960). Hors périmètre (`specify-sas-formats-encodings`).
- Preuve : `evidence/compare_csv_files.txt`
- Confiance : inféré

### 5.3 Valeurs manquantes

Un manquant est un NaN dont un octet de la représentation porte le code. Le motif dépend de la **famille de système** (octet 39) et de l'endianness :

| Famille / endianness | `.` | `._` | `.A` … `.Z` | Observé dans |
|---|---|---|---|---|
| Unix, little endian | `00 00 00 00 00 FE FF FF` | `… 00 FF FF FF` (code `0xFF`) | code `0xFD` (`.A`) … `0xE4` (`.Z`) | `valeurs_manquants_no` (28 valeurs), `datareader/test7`/`test16`, `pyreadstat/missing_test` |
| Unix, big endian | `FF FF FE 00 00 00 00 00` (octets du cas précédent en ordre inverse) | non observé | non observé | `datareader/test10`/`test13`, `pandas/test10`/`test13` (`.` seulement) |
| Windows, little endian (`OUTREP=WINDOWS_*`, SAS Windows) | `00 00 00 00 00 D1 FF FF` | non observé | non observé | `datareader/test1`/`test4`/`test19`, `readstat/datetime` (`.` seulement) |
| ancien fichier public (release `9.0000M0`) | `00 00 00 00 00 FF F8 7F` (NaN IEEE silencieux) | non observé | non observé | `pandas/many_columns` (553 valeurs `.`) |

Règle de lecture : tout NaN dont le code est dans la plage Unix `0xE4`…`0xFF` (octet de rang 5 en little endian, de rang 2 en big endian) donne le code du tableau ; **tout autre NaN est lu comme `.`**. Le CSV de référence des fichiers publics écrit une cellule vide pour tous ces cas : les codes `._` et `.A`-`.Z` ne sont donc vérifiés que pour la représentation Unix little endian. Pour les largeurs 3 à 7, le code est dans les octets conservés (le lecteur complète avant décodage).
- Écriture : motif de la famille cible (Unix little endian pour un fichier x86 Linux).
- Preuve : `valeurs_manquants_no` ; `evidence/compare_csv_external.txt` ; analyse des motifs NaN des 14 fichiers publics (session 02)
- Confiance : inféré

### 5.4 Caractères

- Les `largeur` octets contiennent le texte dans l'encodage de la table (octet 70), complété à droite par des espaces (`0x20`). La valeur lue est le contenu sans espaces de fin ; les espaces de tête sont conservés (`valeurs_chaines_no`).
- Un champ **entièrement nul** (`00` × largeur) est lu comme vide (fichiers publics anciens, `many_columns`). Des octets nuls ailleurs sont des données (`compression_runs_no` : suites de `00`) ; le cas d'un champ partiellement rempli terminé par des `00` n'est **pas observé** (`inconnu`). Un lecteur qui rend des octets bruts retire, comme pour le texte, les espaces de fin seulement.
- Une chaîne vide est stockée en espaces. Il n'y a pas de terminateur.
- Encodage : voir le code (octet 70) et la **réserve** de l'octet 70 : `datareader/test16` et `test19` portent 29 et 62 alors que leurs chaînes sont de l'UTF-8 ; un lecteur doit pouvoir forcer l'encodage de décodage.
- Preuve : `evidence/compare_csv_files.txt` (UTF-8, latin1, wlatin1, Shift-JIS) ; `compare_csv_external.txt`
- Confiance : inféré

### 5.5 Lignes supprimées

Voir 3.5 et 4.2 (`deleted_count`). Lecture : ignorer toute ligne dont le bit de suppression est à 1. `row_count` compte les lignes physiques ; le nombre de lignes lues vaut `row_count − deleted_count` (90 pour `suppression_n100_del10_no`, accord avec `PROC CONTENTS` : « Observations 90, Deleted Observations 10 »).
- Confiance : inféré

---

## 6. Procédure de lecture

1. **En-tête.** Lire 32 octets, comparer au `magic` ; refuser les autres types (les fichiers `.sas7bcat` et `.sas7bndx` ont un octet 15 différent, `0x65` pour l'index). L'en-tête précède la page 0 : la page `k` est à `header_length + k × page_length` même si `header_length ≠ page_length` (cas de deux fichiers publics anciens, 1 024 contre 4 096). Déduire `w` (octet 32), `a1`, `a2`, l'endianness (octet 37), le code d'encodage (octet 70), `header_length`, `page_length`, `page_count`. Refuser si la taille du fichier est inférieure à `header_length + page_length × page_count`.
2. **Métadonnées (première passe).** Pour chaque page `k` : lire `page_type`, `block_count`, `subheader_count`. Si `page_type & 0xFF00 ∈ {0x0000, 0x0200, 0x0400}` (ou `0x4000`), lire les pointeurs ; ignorer ceux de longueur 0 ou de compression ≠ 0 ; lire la signature de chacun et enregistrer RowSize (`row_length`, `row_count`, `deleted_count`), ColumnSize (`column_count`), chaque ColumnText (bloc), ColumnName (entrées), ColumnAttributes (entrées), FormatAndLabel (dans l'ordre d'apparition) ; ignorer SubheaderCounts, ColumnList et les signatures inconnues.
3. **Colonnes.** La colonne `j` combine : nom (ColumnName `j` → bloc de texte), `offset`/`largeur`/`type` (ColumnAttributes `j`), format, informat et label (FormatAndLabel `j`). Si un littéral `SASYZCRL` ou `SASYZCR2` figure dans un bloc de texte, le fichier est compressé : arrêter (hors périmètre). Un fichier créé avec `COMPRESS=CHAR`/`BINARY` dont SAS a désactivé la compression n'a pas de littéral et se lit comme un fichier ordinaire (`Compressed=NO`).
4. **Lignes (seconde passe).** Pour chaque page de type data ou mix, calculer l'offset du début des lignes et leur nombre (3.3) ; si `page_type & 0x0080`, lire le bitmap (3.5) ; décoder chaque ligne non supprimée colonne par colonne (5.2 à 5.4) ; s'arrêter quand `row_count` lignes physiques ont été parcourues.
5. **Sortie.** Colonnes dans l'ordre de création, lignes dans l'ordre du fichier.

L'implémentation de référence (`rea_output/tools/ref_reader.py`, environ 260 lignes, bibliothèque standard) suit cette procédure.

---

## 7. Contrôles effectués par SAS à l'ouverture

### 7.0 Résultats de décompilation des modules du format (2026-10-30)

Périmètre : `sase7io`, `sase7opn`, `sase7xgt` (confirmation NULL du 2026-10-30, `STATE.md`). Pièces : `evidence/decompile/sase7io_findings.md` et les bundles `sase7io_bundle.json`, `sase7opn_bundle.json`, `sase7xgt_bundle.json`. **Limite essentielle : le code de l'en-tête de fichier (octets 0 à 331, nombre magique, `SAS FILE`, zone 296-311, `file_code`) et des signatures de page n'est dans aucun de ces modules ; il est dans le noyau hôte (`src/sasexe/sas`), hors du périmètre confirmé et non analysé.** Les modules ne font que l'appeler (primitives `yzbopen`, `yzbread`, `yzrdhdr`, `yzrgepg`…). Confirmé = lu dans le code et cohérent avec les fichiers du corpus ou avec les mutations de `evidence/write/mutations.tsv`.

| Objet | Constat | Confiance |
|---|---|---|
| RowSize `+0x08` (`row_marker`) | niveau de format ; accepté si `(valeur & 0x3FF)` est dans `[205, 775]` ; un fichier compressé exige une valeur **supérieure à `0x130`** (sinon code de retour `0x8042109F`) ; `0x28B` correspond à l'option `EXTENDOBSCOUNTER=YES` ; `0xF0`, `0x131` sont les valeurs de SAS 9.4M7 (`yzrdopn@0x113c50`, module `sase7opn`) | confirmé |
| RowSize longueur | 0x328 octets (808) | confirmé |
| RowSize `+0x31B` | octet de remplissage + 1, toujours 1 | confirmé |
| RowSize `+0x1B8` | copie de la signature de la page 1 | confirmé |
| RowSize `+0x78` (`mix_capacity`) | formule de `20_ecriture.md` 4.1 ; plafonnée à `65 535 − block_count` ; `+0x2FE` = `block_count + capacité` (`yzrload@0x115e50`) | confirmé |
| RowSize `+0x18` | **mot d'état**, non réservé : bits 0-1 « à réparer », bits 2-3 « endommagé » (codes `0x80421040`, `0x81421040`) ; vaut 0 dans le corpus | inféré (à tester par mutation) |
| Localisateurs `+0x200`..`+0x24F` | `+0x200` : ColumnSize ; `+0x210`/`+0x218` : **fin des métadonnées** (borne des accès positionnés, code `0x8041F80F` au-delà) ; `+0x220`/`+0x228` : première ligne (écrit par `yzrload`) ; `+0x230`/`+0x238` : **dernière page de lignes**, comparée au nombre de pages de l'en-tête (balayage arrière qui saute les pages dont `type & 0x8400` ≠ 0) ; `+0x240` : premier FormatAndLabel | confirmé |
| Pointeur de sous-en-tête | signature du pointeur 0 : `F7F7F7F7` ou le texte `YZRH`, sinon code `0x8041F80D` ; octet de compression : bits 0-1 = 1 → code `0x8041F831`, = 2 → `0x8041F80F`, 0 et 3 acceptés | confirmé (explique la mutation `ptr.comp`) |
| Type de page | champ de bits : `0x0200` mixte, `0x0100` données, `0x0400` et `0x8000` (donc `0x9000 = 0x8000 \| 0x1000`) = pages **sans lignes**, ignorées pour trouver la dernière page de lignes ; `0x2000` posé si la taille de page diffère de `RowSize.P` ; `0x0080` = lignes supprimées | confirmé |
| « Invalid record id » | index de ligne ≥ `block_count` (code `0x8041F80F`) ; le texte du message est inféré | confirmé (condition) |
| Bitmap de suppression | page de données : les `⌈rmax/8⌉` derniers octets, soit `[P − ⌈rmax/8⌉, P)` ; page mixte : `[minoff − ⌈cap/8⌉, minoff)` ; lignes dès l'offset 40 (données) ou `40 + 24 × sc` arrondi à 8 (mixte) | confirmé pour le calcul ; cas multipage et 32 bits non observés |
| SubheaderCounts | voir 4.4 | confirmé |
| Taille de page | minimum `max(2 048, 1 544, une ligne)`, environ 80 lignes visées, 65 535 lignes au plus par page ; pas d'arrondi ni de plancher lus | inféré |

**Reste `inconnu`** : en-tête de fichier et signatures de page (noyau hôte), `ColumnList`, drapeaux de nom de ColumnAttributes, `RowSize+0x10`, `+0x58`, `+0x60`, `+0x250`..`+0x2C7`, pages `0x0800` et `0x4000`, contenu de la page `0x9000` et plafond de 255 lignes compressées (W01, W02). Les modules `sase7xgt`, `sase7xop`, `sase7xrt` sont la couche d'index (`yzi*`, `yi*`), `sase7l`, `sase7ld`, `sase7lu` des catalogues (`yl*`) : ils n'ont pas été décompilés plus avant (hors question de lecture des lignes).

**Non établis.** Les contrôles (champ, condition de rejet, message de `src/sasmsg/`) exigent soit la décompilation des modules `sase7*` (bloquée, voir section 0), soit des essais d'ouverture par SAS de fichiers modifiés (tests de mutation, `specify-sas7bdat-writing`). Constats indirects uniquement : SAS ouvre et relit sans erreur des fichiers dont le nom de table est vide, dont les octets 47 et 71 valent 0, et dont ColumnList est absent ; SAS écrit des fichiers à `after_page_count = 0` et `1` selon la release. Aucune condition de rejet n'a été observée.

---

## 8. Cibles de décompilation

Champs restés `inféré` à faible preuve ou `inconnu`, à traiter (dans cet ordre) quand la confirmation de licence sera consignée ; modules visés : `sase7io`, `sase7opn`, `sase7l`, `sase7ld`, `sase7lu`, `sase7xgt`, `sase7xop`, `sase7xrt`, `src/dbcs/sasexe/sase7`, `sase7yse`.

| N° | Structure et champ | Offset (64 / 32 bits) | Question |
|---|---|---|---|
| T01 | En-tête : `flag_36`, `byte_73`, `desc_50_51`, `desc_74_75`, `desc_65_68` | 36, 73, 50, 74, 65 | rôle de chaque octet ; lesquels sont contrôlés à l'ouverture |
| T02 | En-tête : `session_code_47/71` | 47, 71 | encodage de session ? effet d'`OUTREP=` |
| T03 | En-tête : `file_code`, signatures de page | 320 + t ; page + 0 | formule de `file_code` ; SAS rejette-t-il une valeur arbitraire |
| T04 | En-tête : `after_page_count` | 208 + a1 + a2 | rôle (0 en M7, 1 en M1) |
| T05 | En-tête : `page_length` | 200 + a1 | règle de choix de la taille de page de SAS |
| T06 | RowSize : `row_field_2`, `row_field_11/12`, `mix_capacity` | 2w, 11w, 12w, 15w | formules ; effet sur l'ouverture |
| T07 | RowSize : `col_count_p1/p2` au-delà de 301 colonnes | 9w, 10w | sens exact |
| T08 | RowSize : zone `0x250`-`0x327` (216 octets, 64 bits) | 592 / 344 | références de texte et compteurs |
| T09 | ColumnAttributes : `name_flags` pour les noms de 9 à 31 octets | + (w + 4) | règle des drapeaux ; bit 0 = indexé |
| T10 | ColumnList : contenu, condition de présence | tout le sous-en-tête | rôle ; absence tolérée |
| T11 | SubheaderCounts : mots `m(1)`, `m(2)` et entrées `FFFFFFFB`, `FFFFFFFA`, `FFFFFFF9` | 1w, 2w, 15w… | règle de calcul |
| T12 | Page amd (`0x0400`) : sous-en-tête `FFFFFFFA` | page index + 0xFFD4 | contenu (index) |
| T13 | `free_bytes` et bitmap de suppression | page + 24 / + 12 | emplacement pour les pages de données, les fichiers multipages et 32 bits ; dimension de la zone |
| T14 | Pages `0x4000` (meta2) et `0x0080`/`0x0180` | page + `po` | rôle et pointeurs |
| T15 | Manquants spéciaux en big endian ; numériques de largeur 1 et 2 | — | ordre des octets, largeur minimale |
| T16 | Contrôles à l'ouverture (section 7) | — | champs contrôlés, messages `src/sasmsg/` |

---

## 9. Couverture et complétude

- **En-tête** : chaque octet de l'en-tête des 232 fichiers analysés (variantes 64 bits LE/BE et 32 bits LE/BE) appartient à un champ de la section 2 ou est nul (`evidence/header_coverage.txt`, table `rea_output/tools/fields.py`).
- **Pages** : chaque octet des 40 (24) premiers octets des pages et des pointeurs des fichiers non compressés appartient à un champ de la section 3 ou est nul (même sortie). Les pages `0x9000` des fichiers compressés sont hors périmètre.
- **Sous-en-têtes** : les zones d'octets non décodées sont **énumérées** dans `evidence/zones_non_decodees.md` (valeurs observées pour chaque structure et variante) et reprises dans `60_inconnues.md`. Les zones à valeur unique `00*` sont des champs réservés nuls ; les autres sont des inconnus nommés.
- **Valeurs** : 165 tables générées et 8 fichiers publics lus sans écart.

---

## 10. Renvois (hors périmètre)

- Compression `CHAR` (RLE) et `BINARY` (RDC), lignes stockées comme sous-en-têtes de type `comp = 4`, pages `0x9000`, littéraux `SASYZCRL`/`SASYZCR2` → `specify-sas7bdat-compression`.
- Correspondance code d'encodage → nom, formats et informats natifs, catalogues `.sas7bcat` → `specify-sas-formats-encodings`.
- Écriture, constantes d'en-tête, calcul de `file_code`, contrôles d'ouverture → `specify-sas7bdat-writing`.
- Index `.sas7bndx` et chiffrement : non couverts.

