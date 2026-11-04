# Registre des inconnues

> Toute zone d'octets ou tout comportement du format non expliqué est consigné ici (une entrée par inconnue, avec le modèle de `prompts/README.md`) et n'est jamais masqué. Les entrées sont ajoutées par les changements 02 à 05 (`record_unknown` quand REA est utilisé).
>
> Les manques identifiés avant tout reverse engineering sont dans `rea_output/00_etat_de_l_art.md`, section 4 (M01 à M26) ; ils ne sont pas des inconnues « observées » tant qu'aucune analyse de fichier ou de binaire ne les a rencontrées.

## Entrées du changement `specify-sas7bdat-read-format` (2026-10-30)

Statut : **aucune décompilation n'a été faite** (confirmation interne NULL non consignée) ; chaque entrée est un champ ou un comportement observé dans les fichiers mais non expliqué. La colonne « cible » renvoie à la section 8 de `10_format_sas7bdat.md`. Valeurs observées complètes : `evidence/zones_non_decodees.md` (zones de sous-en-têtes) et `evidence/header_coverage.txt`.

#### U01 — flag_36 et octets de descripteur 65 à 68
- Emplacement : fichier + 36 ; fichier + 65 à 68
- Taille / type : 1 octet ; 4 octets
- Sémantique : l'octet 36 vaut `0x33` en format 64 bits et pour `OUTREP=WINDOWS_64`, `0x22` sinon ; les octets 65-68 recopient `a1`, une constante `01`, `0x23` si `a1 = 4`, et l'octet 36. Rôle exact inconnu.
- Valeurs observées : octet 36 : `0x33` (213), `0x22` (18), `0x00` (1) ; octets 65-68 : `33012333` (213), `32012222` (11), `33012322` (7), `00000000` (1)
- Écriture : recopier selon le format cible
- Preuve : `evidence/header_coverage.txt`, `evidence/diff_plateforme_linux64_vs_windows32.txt` ; cible T01
- Confiance : confirmé (partiel)
- Résolution partielle (2026-10-30, décompilation de `sas`) : octets 52-75 = vecteur de représentation comparé sous un masque (octets 65-68 : 65-66 et 68 ignorés, 67 quartet haut comparé) ; voir `10` 2.3.

#### U02 — desc_50_51 / desc_74_75
- Emplacement : fichier + 50 et + 74, 2 octets chacun
- Taille / type : 2 octets (deux copies)
- Sémantique : couple `03 01` (228 fichiers), `02 01` (`LINUX_32`, 3 fichiers), `00 00` (1 fichier ancien) ; deux copies identiques.
- Valeurs observées : voir Sémantique ; liste exhaustive dans `evidence/zones_non_decodees.md` et `evidence/header_coverage.txt`.
- Écriture : `03 01`
- Preuve : `plateforme_linux_32_no` ; cible T01
- Confiance : confirmé (partiel)
- Résolution partielle (2026-10-30, décompilation de `sas`) : octets 74-75 comparés au vecteur de l'hôte ; plages admises dépendant de `wzxncap` (hors `sas`) ; voir `10` 2.3.

#### U03 — byte_73
- Emplacement : fichier + 73, 1 octet
- Taille / type : 1 octet
- Sémantique : `0x20` (Unix 64 bits et big endian, 206 fichiers), `0x10` (Windows, 18), `0x00` (`hp_ux_32`, 8). Lié à la famille de système ? Non établi.
- Valeurs observées : voir Sémantique ; liste exhaustive dans `evidence/zones_non_decodees.md` et `evidence/header_coverage.txt`.
- Écriture : `0x20`
- Preuve : `plateforme_*_no` ; cible T01
- Confiance : inconnu
- Résolution partielle (2026-10-30, décompilation de `sas`) : octet 73 ignoré par la comparaison de représentation ; plage admise dépendant de `wzxncap` hors `sas` ; la mutation « rejeté pour valeurs aléatoires » reste expliquée par cette fonction (`10` 2.3).

#### U04 — session_code_47 / session_code_71
- Emplacement : fichier + 47 et + 71
- Taille / type : 1 octet (deux copies)
- Sémantique : identiques entre eux ; `0x14` (écriture par une session UTF-8), `0x00` (avec `OUTREP=`, fichiers anciens), `0x10` (`encodage_sjis_no`). Ne reprend **pas** l'octet 70 (`encodage_latin1_no` : 47/71 = `0x14`, 70 = `0x1d`).
- Valeurs observées : voir Sémantique ; liste exhaustive dans `evidence/zones_non_decodees.md` et `evidence/header_coverage.txt`.
- Écriture : `0x14` pour UTF-8
- Preuve : `evidence/diff_encodage_utf8_vs_latin1.txt` ; cible T02
- Confiance : confirmé (partiel)
- Résolution partielle (2026-10-30, décompilation de `sas`) : octets 47 et 71 : 71 comparé (vecteur 52-75 pour 52+) ; 28-51 copie héritée ignorée ; voir `10` 2.3.

#### U05 — after_page_count
- Emplacement : fichier + 208 + a1 + a2, `u32`
- Taille / type : `u32`
- Sémantique : 0 pour SAS 9.4M7, 1 pour les fichiers publics 9.4M1 (datareader, pandas `test10`/`test13`).
- Valeurs observées : voir Sémantique ; liste exhaustive dans `evidence/zones_non_decodees.md` et `evidence/header_coverage.txt`.
- Écriture : 0
- Preuve : `evidence/header_coverage.txt` ; cible T04
- Confiance : confirmé
- Résolution partielle (2026-10-30, décompilation de `sas`) : octet 216 : bit 0 = vecteur de représentation différent du vecteur par défaut de l'hôte ; bit 1 = bloc d'extension (offset 344, longueur 348, inféré) ; voir `10` 2.3.

#### U06 — formule de file_code
- Emplacement : fichier + 320 + t, `u32`
- Taille / type : `u32`
- Sémantique : sur 6 fichiers, `mot bas(created) ^ mot haut(created) ^ file_code` a ses 16 bits de poids fort nuls et ses 16 bits de poids faible variables (non reproduits). Les signatures de page en dérivent (`file_code ^ (k+1)`).
- Valeurs observées : voir Sémantique ; liste exhaustive dans `evidence/zones_non_decodees.md` et `evidence/header_coverage.txt`.
- Écriture : tirer une valeur de 32 bits ; accepter ou non une valeur arbitraire : inconnu.
- Preuve : analyse de 63 fichiers LE64 ; cible T03
- Confiance : confirmé (partiel)
- Résolution partielle (2026-10-30, décompilation de `sas`) : `file_code` fourni par le module appelant et recopié sans contrôle par `sas` ; la formule de tirage reste inconnue (module appelant non analysé) ; signature de page `file_code XOR N` (N ≥ 1) cohérente sur 368 pages.

#### U07 — octets réservés de page
- Emplacement : page + 4 (4 octets) ; page + `po` + 6 (2 octets) ; page + 8 … `free_bytes − 1`
- Taille / type : 4 + 2 + (`free_bytes − 8`) octets
- Sémantique : 0 dans les pages non compressées des fichiers LE ; octets 4 à 7 = restes de mémoire (`B8 7F 00 00`…) dans les fichiers big endian 64 bits (`plateforme_{aix_64,solaris_64}`).
- Valeurs observées : voir Sémantique ; liste exhaustive dans `evidence/zones_non_decodees.md` et `evidence/header_coverage.txt`.
- Écriture : 0
- Preuve : `evidence/zones_non_decodees.md` ; cible T14
- Confiance : inconnu

#### U08 — RowSize : row_field_2 (`m(2)`)
- Emplacement : RowSize + `2w`
- Taille / type : mot
- Sémantique : 10 (1 colonne), 12, 13 (3 colonnes), 14, 15, 24 (`suppression_n100_del10_*`), 31, 36, 311 (301 colonnes), 5 016 (`taille_wide5000_no`). Croît avec le nombre de colonnes et change quand une ligne est supprimée.
- Valeurs observées : voir Sémantique ; liste exhaustive dans `evidence/zones_non_decodees.md` et `evidence/header_coverage.txt`.
- Écriture : formule inconnue ; SAS accepte-t-il une valeur arbitraire ? inconnu.
- Preuve : tableau des RowSize du corpus (session 02), `evidence/diff_base_vs_suppression.txt` ; cible T06
- Confiance : inconnu

#### U09 — RowSize : row_field_11 et row_field_12
- Emplacement : RowSize + `11w` et `12w`
- Taille / type : 2 mots
- Sémantique : `m(12)` = nombre de colonnes (1, 2, 3, 4…) sauf 32 pour `metadonnees_nom32_*` (peut-être max(nombre de colonnes, longueur maximale d'un nom)) ; `m(11)` : 0, 26, 28, 32, 80, 816, 13 064.
- Valeurs observées : voir Sémantique ; liste exhaustive dans `evidence/zones_non_decodees.md` et `evidence/header_coverage.txt`.
- Écriture : formule inconnue
- Preuve : tableau des RowSize du corpus ; cible T06
- Confiance : inconnu

#### U10 — RowSize : mix_capacity
- Emplacement : RowSize + `15w`
- Taille / type : mot
- Sémantique : capacité en lignes d'une page mixte (2 626 pour 24 octets/ligne, 3 936 pour 16, 7 836 pour 8, 12 423 pour 5, 304 pour 208…) ; utilisée pour dimensionner le bitmap réservé (`free_bytes`). Formule non établie : l'estimation `(offset du premier sous-en-tête − début des lignes) / row_length` s'écarte de 13 à 310 lignes.
- Valeurs observées : voir Sémantique ; liste exhaustive dans `evidence/zones_non_decodees.md` et `evidence/header_coverage.txt`.
- Écriture : formule inconnue
- Preuve : `evidence/structure_*` ; cible T06
- Confiance : confirmé
- Résolution partielle (2026-10-30, décompilation) : formule de `mix_capacity` lue dans `yzrload@0x115e50` : identique à `20` 4.1 (voir `10` 7.0).

#### U11 — RowSize : col_count_p1 et col_count_p2 au-delà de 301 colonnes
- Emplacement : RowSize + `9w` et `10w`
- Taille / type : 2 mots
- Sémantique : pour `taille_wide5000_no` (5 001 colonnes), `p1 = 801` et `p2 = 1 395`.
- Valeurs observées : voir Sémantique ; liste exhaustive dans `evidence/zones_non_decodees.md` et `evidence/header_coverage.txt`.
- Écriture : p1 = nombre de colonnes si ≤ 301, p2 = 0 ; au-delà inconnu.
- Preuve : `taille_wide5000_no` ; cible T07
- Confiance : inconnu

#### U12 — RowSize : zone de références et de compteurs
- Emplacement : RowSize + 592 … + 807 (64 bits), + 344 … + 447 (32 bits), hors `locators`, `row_count_copy`, `deleted_count_copy`
- Taille / type : suite de `u16` (placés au début d'emplacements de `w` octets)
- Sémantique : références `(index de bloc de texte, offset, longueur)` vers les littéraux et textes du premier bloc ColumnText (libellé du fichier, procédure `DATASTEP`, littéral de compression), compteurs. 40 valeurs distinctes dans 161 sous-en-têtes LE64 ; non décodées.
- Valeurs observées : 40 valeurs distinctes sur 161 sous-en-têtes LE64 (`evidence/zones_non_decodees.md`).
- Écriture : recopier une valeur d'un fichier de même forme ; inconnu sinon.
- Preuve : `evidence/zones_non_decodees.md` (`RowSize … 0x250, 216`) ; cible T08
- Confiance : inconnu

#### U13 — RowSize : zones réservées nulles
- Emplacement : RowSize + `0x18` (8 octets), `0x70` (8), `0x90` (296), `0x1BC` (68) en 64 bits ; équivalents en 32 bits (`0xC`, `0x20`, `0x38`, `0x48`, `0xE0`, `0x130`)
- Taille / type : zones d'octets (voir Emplacement)
- Sémantique : toujours nulles dans les 161 RowSize LE64, les 8 BE64 et les 11 en 32 bits.
- Valeurs observées : voir Sémantique ; liste exhaustive dans `evidence/zones_non_decodees.md` et `evidence/header_coverage.txt`.
- Écriture : 0
- Preuve : `evidence/zones_non_decodees.md`
- Confiance : inféré
- Résolution partielle (2026-10-30, décompilation) : `RowSize+0x18` n'est pas un réservé : c'est un mot d'état (bits 0-1 « à réparer », bits 2-3 « endommagé ») nul dans le corpus ; à confirmer par mutation (`10` 7.0).

#### U14 — ColumnSize : second mot
- Emplacement : ColumnSize + `2w`
- Taille / type : mot
- Sémantique : 0 sauf 1 fichier LE64 (valeur 1) ; en BE64 : `0x0B` (11) ou `0x6C` (108) selon le fichier. Rôle inconnu.
- Valeurs observées : voir Sémantique ; liste exhaustive dans `evidence/zones_non_decodees.md` et `evidence/header_coverage.txt`.
- Écriture : 0
- Preuve : `evidence/zones_non_decodees.md`
- Confiance : inconnu

#### U15 — SubheaderCounts : mots d'en-tête et entrées sans compteur
- Emplacement : SubheaderCounts + `1w`, `2w`, + 0x20 (80 octets), + 0x190 (200 octets), compteurs des entrées `FFFFFBFE`, `FFFFFFFA`, `FFFFFFF9`
- Taille / type : mots et zones d'octets
- Sémantique : `m(1)` ≈ taille du premier bloc de texte (écart 0 pour 126 fichiers, 8 pour 46, autres pour 6) ; `m(2)` ∈ {3, 4, 5, 9} ; compteurs de `FFFFFBFE` toujours 0 ; zones 0x20 et 0x190 : nulles sauf 1 fichier ; la constante 1 804 (`0x070C`) à `14w`.
- Valeurs observées : voir Sémantique ; liste exhaustive dans `evidence/zones_non_decodees.md` et `evidence/header_coverage.txt`.
- Écriture : recopier la forme d'un fichier de même structure.
- Preuve : `evidence/zones_non_decodees.md` ; cible T11
- Confiance : confirmé (partiel)
- Résolution partielle (2026-10-30, décompilation) : `m(2)` = nombre de sous-en-têtes de signatures FC, FD, FF, FE, FB, FA, F9 ; octets (12, 7) à `m(14)` ; 5ᵉ entrée = `FFFFFFFB` (`yzrhset@0x114b30`, voir `10` 4.4). Les zones nulles 0x20 et 0x190 restent non décodées.

#### U16 — ColumnList : contenu et présence
- Emplacement : tout le sous-en-tête (52 à 13 092 octets, 64 bits)
- Taille / type : suite d'octets de 52 à 13 092
- Sémantique : mots dépendant de la mémoire (adresse `7F…` différente à chaque fichier) ; présent dans 36 tables de 3 colonnes, absent dans 13, présent à partir de 2 colonnes dans les autres cas ; rôle supposé : table de hachage ou permutation (hypothèse publique non vérifiée).
- Valeurs observées : voir Sémantique ; liste exhaustive dans `evidence/zones_non_decodees.md` et `evidence/header_coverage.txt`.
- Écriture : l'omettre ou le recopier : inconnu.
- Preuve : `evidence/zones_non_decodees.md` ; cible T10
- Confiance : inconnu

#### U17 — ColumnAttributes : name_flags
- Emplacement : entrée de colonne + `w + 4`, `u16`
- Taille / type : `u16`
- Sémantique : `0x0400` (nom de 1 à 8 octets), `0x0800` (nom de 32 octets), `0x0401` (colonne indexée). Règle pour 9 à 31 octets inconnue (valeurs 0x0C00, 0x0A00 citées par les sources publiques, non observées).
- Valeurs observées : voir Sémantique ; liste exhaustive dans `evidence/zones_non_decodees.md` et `evidence/header_coverage.txt`.
- Écriture : `0x0400` si nom ≤ 8 octets.
- Preuve : décompte des drapeaux sur 5 618 entrées (session 02) ; cible T09
- Confiance : inconnu

#### U18 — Sous-en-tête FFFFFFFA de la page amd
- Emplacement : page amd + 0xFFD4 (44 octets)
- Taille / type : 44 octets
- Sémantique : signature, `01 00 00 00 00 00 00 00`, `00 00 00 00 00 00 10 00`, `00 00 05 00 01 00 02 00`, zéros. Lié à l'index `i` (colonne indexée) ; non décodé.
- Valeurs observées : voir Sémantique ; liste exhaustive dans `evidence/zones_non_decodees.md` et `evidence/header_coverage.txt`.
- Écriture : sans objet (index hors périmètre)
- Preuve : `evidence/structure_index_n100_ix_no.txt` ; cible T12
- Confiance : inconnu

#### U19 — Bitmap de suppression : cas non observés
- Emplacement : page + `free_bytes` + fin des lignes
- Taille / type : `⌈nb_lignes / 8⌉` octets
- Sémantique : observé sur une seule page mixte LE64 (10 lignes supprimées sur 100). Non observé : page de données avec suppression (type `0x0180` ?), fichier multipage, format 32 bits, big endian, plusieurs milliers de lignes supprimées. L'emplacement du bitmap pour ces cas est déduit de la règle `free_bytes + fin des lignes` mais jamais vu.
- Valeurs observées : voir Sémantique ; liste exhaustive dans `evidence/zones_non_decodees.md` et `evidence/header_coverage.txt`.
- Écriture : réserver le bitmap comme SAS (`⌈capacité / 8⌉` octets) ; poser `0x0080`.
- Preuve : `suppression_n100_del10_no` ; cible T13
- Confiance : inconnu pour ces cas

#### U20 — Pages meta2 (`0x4000`) et combinaisons de drapeaux
- Emplacement : page + `po`
- Taille / type : `u16`
- Sémantique : page `0x4000` présente dans `pandas/test_meta2_page` (fichier compressé) ; combinaisons `0x0080` + data (`0x0180`), `0x0080` + meta (`0x0080`) citées par les sources publiques mais non observées.
- Valeurs observées : voir Sémantique ; liste exhaustive dans `evidence/zones_non_decodees.md` et `evidence/header_coverage.txt`.
- Écriture : sans objet
- Preuve : `evidence/zones_non_decodees.md` ; cible T14
- Confiance : inconnu
- Résolution partielle (2026-10-30, décompilation) : les bits de type de page sont décodés (`0x0400` et `0x8000` = pages sans lignes, `0x2000` taille de page différente de `RowSize.P`) ; le rôle de `0x4000` et `0x0800` reste non décodé (`10` 7.0).

#### U21 — Manquants spéciaux en big endian ; numériques de largeur 1 et 2
- Emplacement : valeur de colonne numérique
- Taille / type : valeur numérique de 8 octets ou moins
- Sémantique : ordre des octets du code de manquant non observé en big endian ; largeurs 1 et 2 jamais produites par SAS sous Linux.
- Valeurs observées : voir Sémantique ; liste exhaustive dans `evidence/zones_non_decodees.md` et `evidence/header_coverage.txt`.
- Écriture : sans objet
- Preuve : `valeurs_manquants_no` (LE64 uniquement) ; cible T15
- Confiance : inconnu

#### U22 — Fiabilité du code d'encodage avec OUTREP
- Emplacement : fichier + 70
- Taille / type : 1 octet
- Sémantique : `OUTREP=` pose le code 29 (Unix) ou 62 (Windows) même quand la session est en UTF-8 ; dans `datareader/test16` et `test19` (OUTREP) le code annonce 29/62 mais les octets de chaînes sont de l'UTF-8. Dans les tables `plateforme_*` du corpus, tous les caractères sont ASCII : le contenu réel est inconnu.
- Valeurs observées : voir Sémantique ; liste exhaustive dans `evidence/zones_non_decodees.md` et `evidence/header_coverage.txt`.
- Écriture : sans objet
- Preuve : `evidence/compare_csv_external.txt` ; renvoi `specify-sas-formats-encodings` (cible T02)
- Confiance : inconnu

#### U23 — Règle de choix de la taille de page
- Emplacement : fichier + 196 + a1 / 200 + a1
- Taille / type : `u32` (deux champs)
- Sémantique : `bufsize=` est respecté (4 096, 8 192, 131 072) ; SAS agrandit la page quand une ligne ne tient pas (106 496 pour une ligne de 32 776 octets, 122 880 pour 40 008) mais la règle d'arrondi n'est pas établie. Deux fichiers publics anciens ont `header_length ≠ page_length`.
- Valeurs observées : voir Sémantique ; liste exhaustive dans `evidence/zones_non_decodees.md` et `evidence/header_coverage.txt`.
- Écriture : multiple de 8 ; à valider par SAS.
- Preuve : `taille_buf*`, `taille_wide*`, `types_char32767` ; cible T05
- Confiance : inconnu

#### U24 — FormatAndLabel : offsets en 32 bits et zones réservées
- Emplacement : FormatAndLabel + `0x08` (16 octets), `0x20` (8), `0x3A` (6) en 64 bits ; `0x04` (8), `0x14` (8), `0x2E` (6) en 32 bits
- Taille / type : zones d'octets (voir Emplacement)
- Sémantique : nulles dans tout le corpus. Les offsets des champs en 32 bits (`0x0C…0x28`) reposent sur un seul fichier (`datareader/test1`, lu sans écart de valeurs mais métadonnées non comparées à `PROC CONTENTS`).
- Valeurs observées : voir Sémantique ; liste exhaustive dans `evidence/zones_non_decodees.md` et `evidence/header_coverage.txt`.
- Écriture : 0
- Preuve : `evidence/zones_non_decodees.md` ; cible hors liste (à vérifier avec un fichier 32 bits comparé à `PROC CONTENTS`)
- Confiance : inconnu

#### U25 — Contrôles à l'ouverture
- Emplacement : tout le fichier
- Taille / type : sans objet (comportement)
- Sémantique : champs contrôlés par SAS, conditions de rejet et messages de `src/sasmsg/` : non établis (ni décompilation, ni test de mutation).
- Valeurs observées : voir Sémantique ; liste exhaustive dans `evidence/zones_non_decodees.md` et `evidence/header_coverage.txt`.
- Écriture : voir `specify-sas7bdat-writing`
- Preuve : aucune ; cible T16
- Confiance : inféré (partiel)
- Résolution partielle (2026-10-30, décompilation) : contrôles de `RowSize` et des pointeurs lus dans `sase7opn`/`sase7io` (voir `10` 7.0) ; les contrôles de l'en-tête de fichier et des signatures de page sont dans le noyau hôte `src/sasexe/sas`, hors du périmètre confirmé.

## Entrées du changement `specify-sas7bdat-compression` (2026-10-30)

Statut : aucune décompilation (confirmation NULL non consignée). Les cibles TC01 à TC08 sont dans `15_compression.md`, section 8.

#### C01 — Liens avec les modules `sasyzcrl` et `sasyzcr2`
- Emplacement : `src/sasexe/sasyzcrl`, `src/sasexe/sasyzcr2` (binaires non ouverts)
- Taille / type : sans objet
- Sémantique : le lien littéral ↔ algorithme est établi par les fichiers (décompression exacte) mais pas par le binaire : quel module écrit et lit quel algorithme est `inconnu`.
- Valeurs observées : littéraux `SASYZCRL` / `SASYZCR2` dans le premier bloc de texte
- Écriture : sans objet
- Preuve : aucune preuve binaire ; cible TC01
- Confiance : confirmé
- Résolution (2026-10-30, décompilation) : `sasyzcrl@0x101d10` implémente RLE (`COMPRESS=CHAR`, littéral `SASYZCRL`) et `sasyzcr2@0x101d50` RDC (`COMPRESS=BINARY`, littéral `SASYZCR2`) : chacune contient le décodeur et l'encodeur. Évidences `ev_6c55e8e3…`, `ev_f59950f8…` (`evidence/decompile/`).

#### C02 — Critère de désactivation de la compression d'une table
- Emplacement : décision de SAS à l'écriture (note du journal « Compression was disabled … »)
- Taille / type : comportement
- Sémantique : 96 tables sur 126 demandées en `CHAR`/`BINARY` ont été écrites sans compression. Les tables de moins de 21 octets par ligne ne sont jamais compressées ; à 21 octets par ligne, les 100 mêmes lignes sont compressées en format 32 bits (pointeur de 12 octets) et pas en format 64 bits (pointeur de 24 octets) : le surcoût de pointeur compte. Le seuil exact n'est pas établi.
- Valeurs observées : `corpus/out_full/compression_check.tsv`
- Écriture : libre (décider de compresser ou non)
- Preuve : tableau des 126 tables ; cible TC02
- Confiance : inconnu
- Complément (2026-11-02, passage v3 du SAS complet, une colonne caractère de n octets, 200 lignes, `compress=char` ou `binary`) : n = 16 à 24 : compression refusée (« Compression was disabled … because compression overhead would increase the size of the data set ») ; n = 28 à 64 : compression appliquée même quand la taille double (« increased size by 100.00 percent »). Le seuil est donc entre 25 et 27 octets en pointeurs de 24 octets (21 octets en format 32 bits, 01b) ; ce n'est pas la taille finale qui décide. Lignes de 25, 26, 27 octets non produites. Autres tables (68 à 5 000 octets par ligne) : 45 sur 45 compressées. Confiance : inféré.

#### C03 — Seuils et choix des commandes du compresseur RLE
- Emplacement : chemin d'écriture de `sasyzcrl`
- Taille / type : règles de décision
- Sémantique : longueurs minimales (2 pour espace/nul/`@`, 3 pour un octet quelconque dans les fichiers observés), choix entre commandes équivalentes (`0x80` ou `0x90`… pour les copies), jamais d'usage de `0x10`, `0x20`, `0x30`.
- Valeurs observées : section 3 de `15_compression.md`
- Écriture : libre
- Preuve : statistiques d'opcodes du corpus ; cible TC03
- Confiance : confirmé
- Résolution (2026-10-30, décompilation) : Seuils et commandes de l'encodeur RLE lus dans `sasyzcrl@0x101d10` (suites dès 2 octets pour espace, `@`, nul, dès 3 sinon ; copie `0x80` jusqu'à 64 ; copie longue `0x00` dès 65 par blocs de 16 447 ; suites de 18 à 1 040 / 19 à 1 041 ; 15_compression.md 3.1).

#### C04 — Algorithme de recherche des références RDC
- Emplacement : chemin d'écriture de `sasyzcr2`
- Taille / type : règles de décision
- Sémantique : fenêtre, longueur minimale, structure de recherche. Hypothèse (non prouvée) : table de hachage ne gardant que l'occurrence la plus récente de trois octets, ce qui expliquerait que le motif de période 2 (`compression_pat2_binary`) ne soit pas compressé (décalage utile 2 < 3).
- Valeurs observées : décalages 4 à 552 dans le corpus généré, 18 à 303 pour les copies longues des fichiers publics
- Écriture : libre
- Preuve : `compression_pat2_binary`, `compression_pat4_binary`, `compression_pat7_binary` ; cible TC04
- Confiance : confirmé
- Résolution (2026-10-30, décompilation) : Recherche RDC lue dans `sasyzcr2@0x101d50` : table de hachage de 4 096 entrées sur 3 octets, dernière occurrence, décalage ≤ 4 098, longueur 3 à 271 sans chevauchement (15_compression.md 4.1).

#### C05 — Choix ligne brute ou compressée (RDC)
- Emplacement : chemin d'écriture de `sasyzcr2`
- Taille / type : règle de décision
- Sémantique : 224 lignes stockées brutes (compression 0) dans des tables RDC : aucune redondance exploitable (`noise`) ou motif de période 2 (`pat2`), plus 4 lignes sur 5 de `sample_bincompressed`. Seuil de comparaison compressé / brut non établi.
- Valeurs observées : section 5 de `15_compression.md`
- Écriture : libre
- Preuve : vecteurs `stockage = raw` ; cible TC05
- Confiance : confirmé
- Résolution (2026-10-30, décompilation) : RLE : brute si le flot n'est pas strictement plus court que `row_length`. RDC : brute si `row_length < 19` ou si, au début d'un groupe de 16 éléments, l'écriture dépasse `row_length − 50` (15_compression.md R11).

#### C06 — Commandes jamais observées
- Emplacement : RLE `0x10`, `0x20`, `0x30` ; RDC commandes 10 à 12 (copie arrière)
- Taille / type : octet de contrôle
- Sémantique : les trois premières n'apparaissent dans aucun fichier ; les commandes RDC 10 à 12 suivent la règle des commandes voisines mais n'ont pas été vues.
- Valeurs observées : aucune
- Écriture : ne pas les produire
- Preuve : statistiques d'opcodes (corpus et fichiers publics) ; cible TC06
- Confiance : confirmé
- Résolution (2026-10-30, décompilation) : Les commandes RLE `0x10`, `0x20`, `0x30` sont des copies longues (bits de longueur) ; les commandes RDC 10 à 12 sont émises par l'encodeur (15_compression.md 3.1 et 4.1).

#### C07 — Page `0x9000` : contenu du bloc final et octets `BD 7F`
- Emplacement : page `0x9000`, offsets 32 738 à 32 767 (page de 65 536 octets) et `po + 6`
- Taille / type : une dizaine d'octets
- Sémantique : comptes répétés (lignes, pointeurs) déduits des valeurs, non prouvés ; octets `po + 6` = restes de mémoire. Positions non mesurées pour d'autres tailles de page.
- Valeurs observées : section 2 de `15_compression.md`
- Écriture : recopier la forme d'un fichier SAS
- Preuve : `compression_pat2_char`, `compression_pat4_char`, `compression_zeros_binary`, `plateforme_linux_32_char` ; cible TC07
- Confiance : inféré
- Résolution (2026-11-02, corpus complémentaire du SAS complet) : le « bloc final » est un tableau de `n` enregistrements de 18 octets dans `[A − 18·n, A)`, `A = min(P, 32 767)`, `n` = pages contenant des lignes ; chacun : `u64` big endian de lignes cumulées, `u64` de numéro de page, `u16` de dernier indice de sous-en-tête ; `BD 7F` = `A − 18·n − 48` (0x7FBD pour `n = 1`). 155 fichiers conformes (`tools/check_marker.py`), mutations par SAS (`evidence/write/mutations_marqueur.tsv`). Pas de preuve binaire : confiance `inféré`. Voir `15_compression.md`, `page_comp_0x9000`.

#### C08 — Flot invalide et cas limites non produits
- Emplacement : lecteur de SAS
- Taille / type : comportement
- Sémantique : réaction de SAS à un flot invalide (sortie plus longue ou plus courte que la ligne, octet de contrôle inconnu) ; ligne de longueur nulle, ligne strictement uniforme, bornes de longueur RDC non présentes dans le corpus.
- Valeurs observées : aucune
- Écriture : produire seulement des flots valides
- Preuve : aucune ; cible TC08
- Confiance : inconnu
- Complément (2026-11-02, corpus complémentaire) : cas limites produits par le SAS complet et relus sans écart par `ref_reader.py` (60 tables comparées aux CSV de SAS, `tools/check_complement_csv.py` ; les 3 tables de 30 nombres nuls sont vérifiées à zéro) : ligne vide, ligne uniforme de 200 octets, suites d'un même octet de 14 à 21, motifs périodiques de décalage 3, 4, 5, 16 à 20, 100, 4 097, 4 098 et 4 099 (RDC). L'encodeur RLE de référence reproduit les flots de SAS sur 14 628 lignes (règle de fin de ligne, `15_compression.md` 3.1). Restent inconnues : la réaction de SAS à un flot invalide et l'encodage RDC octet par octet (150 flots sur 390 identiques). Confiance de la partie « cas limites produits » : `confirmé` par fichier.

#### C09 — Octets de compression de pointeur autres que 0, 1, 4
- Emplacement : octet de compression des pointeurs de sous-en-têtes (`15_compression.md`, entrée `compression_byte`)
- Taille / type : `u8`
- Sémantique : valeurs `2`, `3`, `6`, `9`, `13` vues uniquement dans les quatre fichiers `readstat/compression_type_0x02`, `0x06`, `0x09`, `0x0d` (SAS d'une release non identifiée) ; le bit 2 signale une ligne compressée, mais le rôle des autres bits (0, 1, 3) n'est pas établi, ni celui du pointeur `3` de 1 octet.
- Valeurs observées : `2`, `9` : ligne brute de 2 028 octets ; `6`, `13` : ligne RDC de 1 995 et 1 981 octets ; `3` : 1 octet `00`
- Complément (2026-11-04, jalon J2) : les deux fichiers dont la cinquième ligne porte `9` ou `13` ont `deleted_count = 1` dans RowSize, les deux dont elle porte `2` ou `6` ont `deleted_count = 0`. Le **bit 3 (`0x08`)** de l'octet de compression marque donc très probablement une ligne **supprimée** dans une table compressée (pas de bitmap pour ces pages). Hypothèse sur 4 fichiers d'une même release, `inféré` à faible preuve : le lecteur Rust l'applique avec l'avertissement `C09` (erreur en mode strict) et rend 4 lignes visibles au lieu de 5. Les bits 0 et 1 restent sans rôle établi.
- Écriture : n'utiliser que `0`, `1`, `4`
- Preuve : `evidence/compression_pointeurs.txt` ; cible TC09
- Confiance : inconnu

## Entrées du changement `specify-sas-formats-encodings` (2026-10-30)

Statut : aucune décompilation (confirmation NULL non consignée). Les cibles TF01 à TF06 sont dans `30_formats_encodages.md`, section 7.

#### F01 — Inventaire exhaustif des formats et informats natifs et module d'origine
- Emplacement : `src/sasexe/tkefmt.so`, `tkefmf*.so`, `tkefmi*.so` (non ouverts)
- Taille / type : tables de noms
- Sémantique : l'inventaire de `30_formats_encodages.md` est une liste de 241 noms de la documentation publique testés dans `sas_small` ; le nombre réel de formats natifs et le module qui fournit chacun sont inconnus. 62 formats et 32 informats documentés sont absents de l'oracle.
- Valeurs observées : `evidence/fmt_existence.log`
- Écriture : sans objet
- Preuve : aucune preuve binaire ; cible TF01
- Confiance : confirmé (partiel)
- Résolution (2026-10-30, décompilation de `tkefm*`/`tknls*`) : inventaire extrait des modules `tkefmf*`/`tkefmi*` (416 noms, 186 des 241 couples de `30` retrouvés ; 55 couples non retrouvés, 263 noms nouveaux) ; voir `30` 6.4.

#### F02 — Repli en notation E des formats numériques
- Emplacement : algorithme d'affichage (`w.d`, `BEST`, `COMMA`, `DOLLAR`, `PERCENT`, `Z`)
- Taille / type : comportement
- Sémantique : quand un nombre est trop grand pour la largeur, SAS affiche une mantisse entière sans point (`123E9` pour 123456789012 en 6 colonnes, `12E3` pour 12345,6789 en 4) ou avec point (`1.23E6`) ; la règle de choix n'est pas établie (plus de chiffres disponibles ne suffisent pas à l'expliquer).
- Valeurs observées : `evidence/verification_affichage.txt` (familles 1 de la section 4.6)
- Écriture : sans objet (affichage)
- Preuve : sorties de l'oracle ; cible TF02
- Confiance : inconnu

#### F03 — Très petites valeurs et nombre de chiffres de BEST
- Emplacement : algorithme `BESTw`
- Taille / type : comportement
- Sémantique : `1E-5` s'écrit `0.00001` en `best8.` à `best32.` ; `BEST` n'affiche que 14 chiffres pour `1/3` et tronque `2/3` à `0.66666666666666` ; seuil de passage à la notation E pour les petites valeurs non établi.
- Valeurs observées : `evidence/fmt_display_num.txt`
- Écriture : sans objet
- Preuve : oracle ; cible TF02
- Confiance : inconnu

#### F04 — Formats absents ou non conformes dans l'oracle
- Emplacement : `E8601*`, `B8601*`, `NL*`, `EURDF*`, `HEX`, `BINARY`, `OCTAL`, `$HEX`, `WORDS`, `WORDF`, `FRACT`, `D`, `BESTD`, `COMMAX`, `DOLLARX`, `NENGO`, `WORDDATX`, `WEEKDATX`
- Taille / type : formats
- Sémantique : les formats ISO 8601 et NLS sont absents de `sas_small` ; `HEX`, `BINARY`, `OCTAL` s'y comportent comme un format numérique ordinaire (pas comme leur nom l'indique) ; `$HEX` ne se charge pas ; leur affichage n'est pas spécifié.
- Valeurs observées : `evidence/fmt_existence.log`, `evidence/fmt_display_num.txt`
- Écriture : sans objet
- Preuve : oracle ; cible TF03
- Confiance : inconnu

#### F05 — Dates hors plage et anomalies de calendrier
- Emplacement : formats de date
- Taille / type : comportement
- Sémantique : avant 1582-01-01 et après l'an 20 000 tout est en astérisques, entre 9 999 et 20 000 l'année à 4 chiffres devient `****` ; pour `WORDDATE`, `WEEKDATE`, `YYQR`, `YYMON` les astérisques ne sont pas tous reproduits ; l'oracle affiche `NOV` pour `DOWNAME3.` du jour −138 061 ; `JULDAY`/`JULIAN` après l'an 4 000 diffèrent d'un jour du calcul avec les années 4 000, 8 000… non bissextiles.
- Valeurs observées : `evidence/fmt_display_date.txt`
- Écriture : sans objet
- Preuve : oracle ; cible TF03
- Confiance : inconnu

#### F06 — Heures très grandes et décimales de HHMM / MMSS
- Emplacement : `HHMM`, `HOUR`, `MMSS`
- Taille / type : comportement
- Sémantique : au-delà de 100 000 secondes les formats passent par une notation en heures ou en E non reproduite ; `hhmm5.2` et `mmss8.1` affichent des fractions (`0:00.`, `0:59.5`) non reproduites.
- Valeurs observées : `evidence/fmt_display_time.txt`, `evidence/fmt_display_dt.txt`
- Écriture : sans objet
- Preuve : oracle
- Confiance : inconnu

#### F07 — Catalogue : reste des entrées du répertoire
- Emplacement : page 0 d'un `.sas7bcat`, 40 octets après `[type][nom]` de chaque entrée de 56 octets
- Taille / type : mots de 16 et 32 bits
- Sémantique : adresses et tailles de l'entrée sur les pages suivantes, non décodées (valeurs `02`, `F0 03 10 08`, `30 06 00 03`…).
- Valeurs observées : `evidence/catalogue_dump_partiel.txt`
- Écriture : recopier une entrée d'un catalogue existant
- Preuve : un catalogue du SAS complet, deux catalogues publics ; cible TF04
- Confiance : inconnu

#### F08 — Catalogue : contenu des entrées (plages, OTHER, PICTURE, options)
- Emplacement : pages d'entrées d'un `.sas7bcat` (page 3 pour `formats.sas7bcat`)
- Taille / type : champs big endian, doubles à bits modifiés
- Sémantique : codage des bornes, de `LOW`/`HIGH`/`OTHER`, des options de `CNTLOUT` (`LENGTH`, `FUZZ`, `MULT`, `FILL`, `DECSEP`, `DIG3SEP`…) non décodé ; seuls les libellés sont lus.
- Valeurs observées : `rea_output/evidence/catalogue_dump_partiel.txt`
- Écriture : impossible sans cette spécification
- Preuve : `catalogue_cntlout` comme référence ; cible TF04
- Confiance : inconnu

#### F09 — Catalogue : FORMATC, INFMTC, plusieurs catalogues, pages d'extension
- Emplacement : répertoire et pages d'entrées
- Taille / type : structures
- Sémantique : aucun format caractère (`$VALUE`, `INVALUE $`) ni catalogue de plus de 4 pages n'a été produit ; le rôle de la page 1 (type `0x4C58`) et de la page 2 est inconnu.
- Valeurs observées : un seul catalogue du SAS complet
- Écriture : sans objet
- Preuve : tâche 5.2 non faite (paires de catalogues) ; cible TF04
- Confiance : inconnu

#### F10 — Désaccords sur les codes d'encodage
- Emplacement : octet 70 de l'en-tête
- Taille / type : `u8`
- Sémantique : les sources publiques ne s'accordent pas sur les codes 39, 43 à 47, 51, 58, 125, 136, 248 ; les codes 43 à 47, 51, 58 sont décalés de deux rangs entre ReadStat/pandas et Parso ; aucune de ces tables n'est vérifiée par un fichier du corpus (seuls 0, 20, 29, 40, 62, 138 sont observés ; 40 et 0 dans des fichiers à contenu ASCII).
- Valeurs observées : `evidence/table_encodages.md`
- Écriture : n'écrire que 20 (UTF-8), 29 (latin1), 62 (wlatin1)
- Preuve : `evidence/encodages_sources_publiques.tsv` ; cible TF05
- Confiance : inféré
- Résolution (2026-10-30, décompilation de `tkefm*`/`tknls*`) : 12 désaccords résolus par `tknls_einfo` (`30` 6.3) : ReadStat et pandas ont raison pour 43–47, 51, 58 ; 125 = GBK ; 136 = CP932 ; 248 = Shift_JISx0213 ; 39 = thaï ; 247 orthographe. Aucun fichier du corpus pour ces codes.

#### F11 — Transcodage de SAS sur octets invalides (encodages non UTF-8)
- Emplacement : lecture d'un fichier latin1, wlatin1 ou Shift-JIS par SAS en session UTF-8
- Taille / type : comportement
- Sémantique : seul le cas UTF-8 vers UTF-8 a été observé (octets conservés sans substitution) ; Shift-JIS (octets de seconde position invalides) et caractères non représentables dans la page de destination n'ont pas été essayés : l'oracle n'a pas de module Shift-JIS.
- Valeurs observées : `evidence/enc_lecture_octets_invalides.bin`
- Écriture : sans objet
- Preuve : aucune ; cible TF06
- Confiance : inféré (partiel)
- Résolution (2026-10-30, décompilation de `tkefm*`/`tknls*`) : octet invalide vers monooctet : U+FFFD puis octet de substitution, conversion poursuivie avec statut d'erreur ; sources multioctets non traitées (`30` 6.5).

#### F12 — Noms SAS des encodages et code 0 / 204
- Emplacement : table de la section 6.1
- Taille / type : noms
- Sémantique : les noms SAS officiels (`wlatin1`, `shift-jis`, `ms-932`…) ne viennent que des commentaires de ReadStat et de la documentation publique ; le sens du code 0 (« non spécifié ») et du code 204 (« any ») n'est pas vérifié.
- Valeurs observées : `evidence/encodages_sources_publiques.tsv`
- Écriture : sans objet
- Preuve : aucune binaire ; cible TF05
- Confiance : inféré (partiel)
- Résolution (2026-10-30, décompilation de `tkefm*`/`tknls*`) : code 204 = `any`, aucun transcodage ; code 0 sans nom, repli `windows-1252` suggéré ; noms SAS officiels dans `encodages_natifs.tsv` (`30` 6.3).

## Entrées du changement `specify-sas7bdat-writing` (2026-10-30)

Statut : aucune décompilation (confirmation NULL non consignée). Toutes les entrées ci-dessous viennent de l'écriture de fichiers relus par `sas_small` (voir `20_ecriture.md`). Le niveau `confirmé` n'est atteint par aucune.

#### W01 — Page marqueur `0x9000` au-delà de 255 lignes compressées
- Emplacement : page `0x9000` des tables compressées de SAS 9.4M7, octets 32 738, 32 747 et 32 756 de la page
- Taille / type : `u8` d'effectif
- Sémantique : `sas_small` exige que l'octet d'effectif (effectif total, ou lignes de la page quand il n'y a qu'une page de lignes) soit supérieur ou égal au nombre de lignes de la table. Un octet ne peut donc pas porter 256 lignes ou plus. Aucune valeur (balayage des 256 valeurs), aucune répartition en plusieurs pages (255 + 45 lignes, 100 lignes par page, avec ou sans page marqueur, pages centrales de type `0x4000`) n'a permis de faire relire par SAS un fichier de 256 lignes ou plus ; pandas, pyreadstat, Parso et `ref_reader.py` relisent ces fichiers (jusqu'à 6 000 lignes). SAS 9.4M6 (fichier public 32 bits de 1 000 lignes) n'a pas de page marqueur ; le codage de SAS 9.4M7 pour 256 lignes et plus n'est pas dans le corpus.
- Valeurs observées : `evidence/write/compression_256_lignes.md`
- Écriture : n'écrire que des tables compressées de 255 lignes au plus pour une relecture par SAS
- Preuve : expériences en `evidence/write/compression_256_lignes.md` ; cible TW01 : tables compressées de 255, 256, 300, 1 000 et 10 000 lignes produites par le SAS complet (`prompts/01b_extraction_sas_complet.md`, à étendre)
- Confiance : inféré
- Résolution (2026-11-02, corpus complémentaire du SAS complet) : l'« octet d'effectif » est un `u64` **big endian** (lignes cumulées) ; la première version de l'écrivain n'écrivait que son octet de poids faible, d'où le plafond de 255. `sas_small` relit les tables compressées de 256, 1 000, 10 000 et 70 000 lignes écrites avec le tableau complet (cas `w11_*_256`, `_1000`, `_10000`, `_70000`, `_pages`, `_pg4096`). Plus de limite à 65 535 lignes (W11 résolue).

#### W02 — Page marqueur : trois pages de lignes ou plus, petites pages, condition de remplissage
- Emplacement : page `0x9000` ; remplissage des pages compressées
- Taille / type : octets et règle
- Sémantique : les octets de la page marqueur ne sont connus que pour une ou deux pages de lignes (4 fichiers, `compression_pat4_char`, `pat7_char`, `pat2_char`, `taille_wide5000_char`) ; avec trois pages de lignes ou plus la forme des octets (première et dernière page seulement ?) est supposée. Pour une page de 4 096 octets (`external/readstat/compression_type_0x02`) la queue est à `P − 11` et non à 32 756. La condition exacte d'ajout d'une ligne (`libre ≥ 0` après l'ajout) n'est pas testée à la limite.
- Valeurs observées : `evidence/write/compression_256_lignes.md`
- Écriture : écrire le motif observé ; page de 65 536 octets recommandée pour les tables compressées
- Preuve : aucune ; cible TW01
- Confiance : inféré
- Résolution (2026-11-02, corpus complémentaire du SAS complet) : un enregistrement de 18 octets par page de lignes (jusqu'à 99 observés), pages de 4 096, 8 192, 16 384 et 65 536 octets (ancrage `min(P, 32 767)`), marqueur après la première page qui contient des lignes (vérifié sur `taille_wide5000_char`, marqueur en page 5). Remplissage des pages : une ligne est ajoutée si l'espace libre reste ≥ 0 (marge minimale observée sur 60 changements de page : 1 octet). Restent hors corpus : les métadonnées sur plusieurs pages écrites par notre écrivain (W04), les fichiers `readstat/compression_type_0x02` (autre release).

#### W03 — Pages de types `0x4000` et `0x0400`
- Emplacement : octets 32 à 33 de l'en-tête de page
- Taille / type : `u16`
- Sémantique : `external/pandas/test_meta2_page.sas7bdat` (SAS 9.4M6, 32 bits) a une page centrale de type `0x4000` portant des lignes compressées ; `external/readstat/compression_type_0x02.sas7bdat` a une page de type `0x0400` ; leur rôle est inconnu. L'écrivain ne produit jamais ces types.
- Valeurs observées : fichiers cités
- Écriture : sans objet
- Preuve : aucune ; cible TW01
- Confiance : inconnu

#### W04 — Métadonnées sur plusieurs pages
- Emplacement : sous-en-têtes `ColumnName`, `ColumnAttributes`, `FormatAndLabel`, `ColumnText` au-delà de la page 0
- Taille / type : disposition
- Sémantique : l'écrivain place toutes les métadonnées sur la page 0 ; au-delà de 549 colonnes (page de 65 536 octets) ou 1 110 colonnes (131 072 octets) il refuse. SAS écrit les métadonnées sur plusieurs pages (5 000 colonnes sur 7 pages de 131 072 octets pour `taille_wide5000_char`) et plusieurs sous-en-têtes `ColumnName`/`ColumnAttributes` ; les localisateurs de `RowSize` et les positions de `SubheaderCounts` pour ce cas ne sont pas résolus.
- Valeurs observées : `corpus/files/taille/taille_wide5000_*`, `taille_wide300_*`
- Écriture : refusé par l'écrivain de référence
- Preuve : aucune ; cible TW02
- Confiance : inconnu

#### W05 — Sous-en-tête `ColumnList` omis
- Emplacement : entre `ColumnAttributes` et le premier `FormatAndLabel` dans les fichiers de SAS
- Taille / type : sous-en-tête de 54 octets (2 colonnes)
- Sémantique : SAS l'écrit toujours ; `sas_small`, pandas, pyreadstat, Parso et `ref_reader.py` relisent sans lui. Son usage hors lecture (tri, index, jointure) est inconnu. Son absence décale de 1 les compteurs de pointeurs et de 3 lignes la capacité de la page 0 (6.2).
- Valeurs observées : `evidence/write/diff_recopies.txt`
- Écriture : omis
- Preuve : relectures de `w01` à `w14` ; cible TW03
- Confiance : inconnu

#### W06 — Portée des champs `libre`
- Emplacement : champs classés `libre` en 5.1 à 5.6 de `20_ecriture.md`
- Taille / type : comportement
- Sémantique : la classification vient de mutations relues par `PROC CONTENTS`, `PROC PRINT` et une étape DATA dans `sas_small` 9.4M7. Une autre procédure (`PROC DATASETS`, `SORT`, `APPEND`), une autre version de SAS, SAS Viya ou le SAS complet peuvent exploiter un champ ignoré ici.
- Valeurs observées : `evidence/write/mutations.tsv`
- Écriture : écrire la valeur conseillée (celle de SAS) pour tout champ `libre`
- Preuve : aucune hors `sas_small` ; cible TW04
- Confiance : inconnu

#### W07 — `RowSize+0x58`
- Emplacement : `RowSize` +0x58
- Taille / type : `u64`
- Sémantique : SAS y écrit une valeur qui dépend des colonnes (26 pour une ligne de 208 octets de 2 colonnes, 28 pour 24 octets de 3 colonnes, 80 pour 138 octets de 21 colonnes, 0 pour une colonne unique) ; l'écrivain écrit 0 et SAS relit ; la valeur 1 et une valeur aléatoire sont rejetées par SAS (erreur de mémoire), `FF…FF` est toléré. La règle de SAS n'est pas reconstituée.
- Valeurs observées : `corpus/files/base/`, `corpus/files/compression/`
- Écriture : 0
- Preuve : `evidence/write/mutations.tsv` (`rs.58`) ; cible TW05
- Confiance : inconnu

#### W08 — Fichiers de plus de 2 Go
- Emplacement : tailles et compteurs de pages
- Taille / type : comportement
- Sémantique : non essayé (écrivain en mémoire, relecture lente sous émulation). Le format garde des `u64` pour les nombres de pages et les offsets ; aucun champ de 32 bits n'a été relevé dans le format 64 bits.
- Valeurs observées : aucune
- Écriture : sans objet
- Preuve : cas `w15_gt2gb` non faisable ; cible TW06
- Confiance : inconnu

#### W09 — Valeurs de release et d'hôte de l'en-tête
- Emplacement : octets 224 à 231 et 232 à 236 de l'en-tête de fichier
- Taille / type : texte
- Sémantique : SAS 9.4M7 affiche ces valeurs dans `Release Created` et `Host Created` et relit n'importe quel texte. L'effet sur une autre version de SAS (refus d'une release inconnue, compatibilité ascendante) est inconnu.
- Valeurs observées : `evidence/write/mutations.tsv` (`h.release`, `h.host`)
- Écriture : `9.0401M7`, `Linux`
- Preuve : aucune hors `sas_small` ; cible TW04
- Confiance : inconnu

#### W10 — Périmètre de décompilation : noyau hôte (`src/sasexe/sas`)
- Emplacement : exécutable principal `src/sasexe/sas` et ses variantes `dbcs` (hors du périmètre confirmé par le responsable des contrats SAS le 2026-10-30)
- Taille / type : comportement et zones d'octets
- Sémantique : les huit modules `sase7*` appellent le noyau hôte pour tout le niveau fichier et page. Le code de l'en-tête de fichier (nombre magique, `SAS FILE`, zone 296-311, `file_code`, octets 52-55, 73-75), des signatures de page, de la page `0x9000` et du plafond de 255 lignes compressées s'y trouve selon un balayage de motifs d'octets (aucun désassemblage). Les inconnues U01 à U07, U22, U25 (en-tête), C07, W01 et W02 en dépendent.
- Valeurs observées : `evidence/decompile/sase7io_findings.md` section 0
- Écriture : sans objet
- Preuve : périmètre étendu le 2026-10-30 (`STATE.md`) ; résultats dans `evidence/decompile/sas_header_findings.md`. Constat : `sas` ne contient pas les symboles `yz*` des modules `sase7*` ; le noyau ne traite que l'en-tête de 512 octets (familles `wc*`, `vc_*`, `hc_*`). La page `0x9000`, le plafond de 255 lignes et les champs posés par la couche de données restent à localiser dans un autre module
- Confiance : inconnu

#### W11 — Tables compressées de plus de 65 535 lignes
- Emplacement : page `0x9000`, champ de lignes cumulées de chaque enregistrement
- Taille / type : `u64` big endian
- Sémantique : le cumul est sur 8 octets : `comp_n65535`, `comp_n65536` et `comp_n70000` du SAS complet portent 65 535, 65 536 et 70 000 (`char` et `binary`). La première lecture de ces fichiers le décodait à tort sur 16 bits (valeur 0 pour 65 536, 4 464 pour 70 000) parce que les octets de poids fort se trouvent à la fin de l'enregistrement précédent.
- Valeurs observées : 53 à 99 enregistrements, `P` = 65 536
- Écriture : aucune limite de lignes
- Preuve : `tools/check_marker.py` (155 fichiers OK) ; `sas_small` relit les cas `w11_*_70000` de l'écrivain
- Résolution : 2026-11-02 (corpus complémentaire v3)
- Confiance : inféré

### W12 — Page marqueur `0x9000` au-delà de 1 800 pages de lignes (J6)
- Observation : la page marqueur tient `n` enregistrements de 18 octets dans `[A − 18·n, A)` avec `A = min(P, 32 767)` ; le driver (J6) refuse une table compressée de plus de 1 800 pages portant des lignes (`WriteError::Unsupported`).
- Inconnu : le comportement de SAS au-delà (plusieurs pages marqueur, enregistrements sur plusieurs pages ?). Les 155 fichiers compressés du corpus complémentaire ont 53 à 99 enregistrements au plus.
- Écriture : table compressée limitée à ≈ 115 Mio de flots compressés (pages de 64 Kio) tant que l'inconnue n'est pas levée ; une table plus grosse s'écrit non compressée.
- Confiance : inconnu

