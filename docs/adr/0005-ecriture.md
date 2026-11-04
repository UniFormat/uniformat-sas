# ADR 0005 — Écriture : API, compression, validation des limites, valeurs non représentables

- Statut : proposée (2026-10-30)
- Spécification liée : `20` §3 à §9 ; `15` §5 (R1–R12) ; `30` §6.2

## Contexte

`20_ecriture.md` décrit un écrivain de référence validé sur 57 cas (SAS 9.4M7 via `sas_small`, pandas, pyreadstat, Parso, 0 KO) avec ces contraintes établies : 64 bits little endian, métadonnées sur la page 0 seulement, `ColumnList` omis, encodages 20/29/62, `HL = P`, pages 4 096 à 1 048 576 octets, nom ≤ 32 octets, label ≤ 256 octets, caractère ≤ 32 767 octets, numérique de 3 à 8 octets, 549 colonnes au plus pour `P = 65 536` (1 110 pour 131 072, 4 000 pour 1 048 576), tables compressées sans limite de lignes connue (70 000 lignes relues par SAS ; W01 et W11 résolues par le corpus du SAS complet du 2026-11-02), compression refusée sous 16 octets par ligne (SAS : 21), lignes brutes réservées aux lignes de 16 octets ou plus, `RowSize` et en-tête dépendant des totaux (réécrits en fin d'écriture). Les champs sont classés `calculé` / `constante` / `libre` (`20` §5).

## Décision

### 1. API d'écriture

Deux entrées, un seul chemin :

- **Depuis Arrow** : `ArrowWriter::try_new(sink, schema, ArrowWriteOptions)` valide le schéma **avant le premier octet** puis `write(&RecordBatch)` / `close()`. Types acceptés : `Float64`/`Float32`/`Int8…Int64`/`UInt8…UInt32` → numérique ; `Date32` → numérique + format `DATE9.` si aucun format n'est fourni ; `Timestamp` → numérique + `DATETIME20.` ; `Time32/64` → numérique + `TIME8.` ; `Utf8`/`LargeUtf8` → caractère ; `Boolean` → numérique 0/1 ; `Decimal128` → numérique (conversion en `f64`, voir §4) ; `Binary` → caractère (octets bruts, encodage ignoré). Autres types : `SchemaError::UnsupportedType`.
- **Depuis un itérateur de lignes** : `SasWriter::new(sink, WriteSchema, WriteOptions)` + `write_row(&[CellValue])` ; `CellValue` = `Num(f64)` | `Missing(MissingKind)` | `Text(&str)` | `Bytes(&[u8])` | `Null`.
- Métadonnées par colonne via le schéma Arrow (clés `sas.label`, `sas.format`, `sas.informat`, `sas.length`, ADR 0003) ou via `WriteSchema` ; métadonnées de table via `sas.table_name`, `sas.table_label` et `WriteOptions`.
- Le sink est `Write + Seek` (réécriture d'`RowSize`, de la page 0 et de l'en-tête à la fin, ADR 0002) ; sans `Seek`, `write_to_memory` renvoie un `Vec<u8>`.
- Tampon : lignes accumulées par page (jamais plus d'une page de données + la page 0 en mémoire, hors compression).

### 2. Choix de compression

`Compression::{None (défaut), Char, Binary, Auto}`.

- `None` : table ordinaire, toujours relue par SAS dans les cas validés.
- `Char` (RLE, littéral `SASYZCRL`) et `Binary` (RDC, littéral `SASYZCR2`) : écrits comme en `20` §4.2 (`compress_char`/`compress_binary` du cœur). Contraintes appliquées avant écriture : somme des largeurs de colonnes ≥ 16 octets (sinon la table est écrite sans compression, avec un avertissement `compression_disabled_small_rows`) ; toute taille de page (la page marqueur est ancrée à `min(P, 32 767)`, `20` §5.6) ; **aucune limite de lignes** (cumul de la page marqueur sur 8 octets). La page marqueur est écrite à la fermeture (un enregistrement par page de lignes, `20` §4.2 point 7) : le flux de lignes doit donc être terminé avant que le fichier soit complet (écriture transactionnelle déjà prévue). Pas de `WriteError::CompressionRowLimit`. L'écriture vers un fichier est **transactionnelle** : fichier temporaire dans le même dossier, renommage à `close()`, suppression en cas d'erreur ; aucun fichier partiel n'est laissé. Un sink arbitraire reçoit l'erreur et doit être considéré comme invalide.
- `Auto` : décide `None` si le gain estimé est nul (lignes de moins de 28 octets : SAS refuse jusqu'à 24, compresse à partir de 28, 25 à 27 non mesurés, C02) ; sinon `Char` si une ligne type contient surtout des blancs/zéros, `Binary` autrement. L'heuristique est de qualité de vie, pas de conformité (R11, R12 libres).
- Ligne brute dans une table compressée : uniquement si elle fait ≥ 16 octets, si le flot compressé n'est pas strictement plus court et si ses 4 premiers octets ne sont pas une signature de sous-en-tête ; sinon le flot (même plus long) est écrit (`20` §4.2 point 4, C05).
- Choix `Char` par défaut quand l'utilisateur dit « compression » sans algorithme : `Char` (le seul à ne jamais produire de lignes brutes, simplifie la validation).

### 3. Validation des limites avant écriture

`WriteSchema::validate()` s'exécute avant tout octet et renvoie `SchemaError` typée (jamais un `panic`) :

| Contrôle (`20` §8, §4.3) | Erreur |
|---|---|
| nom de colonne > 32 octets dans l'encodage cible, vide ou doublon (insensible à la casse comme SAS : non vérifié par la spécification → avertissement `name_case_collision`) | `NameTooLong`, `DuplicateName` |
| label > 256 octets | `LabelTooLong` |
| colonne caractère > 32 767 octets (ou `Utf8` dont la longueur maximale déclarée dépasse) | `CharTooLong` |
| longueur numérique hors 3 à 8 | `BadNumericLength` |
| métadonnées > page 0 (`Σ sous-en-têtes + 24·sc + 40 > P`) : le cœur essaie d'abord d'agrandir `P` (multiple de 8, jusqu'à 1 Mio) puis refuse | `TooManyColumns { max_for_page }` (limite 549 / 1 110 / 4 000, W04) |
| encodage hors {20, 29, 62} | `UnsupportedEncoding` |
| `ColumnName`/`ColumnAttributes` > 65 535 octets (4 000 colonnes) | `TooManyColumns` |
| ligne plus longue que ce que la page autorise (`RL + 48 > P`) | page agrandie, sinon `RowTooLong` |
| compression : voir §2 | `CompressionRowLimit`, `CompressionPageSize` |

Les noms de colonne Arrow sont écrits tels quels (SAS impose des règles de nom — lettres, chiffres, soulignement — non vérifiées par la spécification ; avertissement `name_not_sas_valid`, pas d'erreur). Les limites hors spécification (fichier > 2 Go, W08) ne sont pas bloquantes : les offsets sont des `u64`, avertissement `untested_large_file`.

### 4. Valeurs non représentables : un comportement unique et configurable

`WriteOptions::on_unrepresentable: Unrepresentable::{Error (défaut), Replace, Truncate}` s'applique uniformément à tous les cas ci-dessous ; `Error` abandonne l'écriture avec la position (colonne, ligne) et rend le sink inutilisable (fichier temporaire supprimé). Le comportement est le même pour l'API Arrow et pour l'itérateur de lignes.

| Cas | `Error` | `Replace` | `Truncate` |
|---|---|---|---|
| texte plus long que la colonne | erreur | — | tronque **à la limite de caractère** (jamais au milieu d'un caractère multi-octets, contrairement à SAS qui coupe à l'octet, `30` §6.2) + `truncated_values` |
| caractère non encodable dans l'encodage cible | erreur | remplace par `?` + `replaced_chars` | — |
| entier `|n| > 2^53` converti en double | erreur | arrondi au double le plus proche + `inexact_integers` | — |
| `NaN` flottant | écrit comme `.` (représentation unique d'un manquant, documenté) dans tous les modes | idem | idem |
| `±Inf` | erreur | `.` + `replaced_values` | — |
| `Decimal128` non exactement représentable en `f64` | erreur | arrondi + `inexact_decimals` | — |
| date/datetime hors 1582-01-01…20 000 | erreur | `.` | — |
| `Timestamp` plus fin que la seconde SAS | écrit en secondes fractionnaires (représentables), aucun cas d'erreur | idem | idem |
| numérique 3 à 7 octets dont la mantisse perd des bits | erreur | troncature de poids faible (comportement de SAS) + `narrowed_values` | idem |

`NULL` : numérique → manquant `.` (ou le code de la colonne annexe `x__missing`, ADR 0003) ; caractère → blancs (SAS ne distingue pas vide et manquant caractère, `10` §5.4).

### 5. Contenu écrit

Conforme à `20` §4 et §5 : valeurs `constante` recopiées, `libre` aux valeurs conseillées (release `9.0401M7`, hôte `Linux`, surchargeables par `WriteOptions`, W09) ; `file_code` tiré aléatoirement (déterministe avec `WriteOptions::seed`) ; `ColumnList` omis (W05). Les cas non supportés (ajout, suppression de lignes, index, contraintes, chiffrement, 32 bits, big endian) renvoient `WriteError::Unsupported(feature)` (`20` §7).

## Options étudiées

| Sujet | Options | Choix |
|---|---|---|
| API | (a) uniquement Arrow ; (b) uniquement lignes ; (c) les deux sur un cœur commun (retenue) | (c) : Arrow pour les lots, lignes pour R et les petits usages |
| sink | (a) `Write` pur avec tampon total ; (b) `Write + Seek` avec réécriture (retenue) ; (c) deux passes sur la source | (b) ; (a) possible via `Vec<u8>` |
| compression par défaut | (a) activée si utile ; (b) désactivée (retenue) | (b) : `Auto` à la demande : la compression n'apporte pas de gain garanti (SAS lui-même produit des fichiers plus gros, « increased size by 28.57 percent » sur `big_*`) et son seuil de longueur de ligne n'est connu qu'à 25–27 octets près (C02) |
| valeur non représentable | (a) toujours erreur ; (b) toujours remplacer ; (c) option unique (retenue) | (c) avec `Error` par défaut |
| troncature | (a) à l'octet comme SAS ; (b) au caractère (retenue) | (b) : ne produit jamais d'octets invalides |

## Conséquences

- (+) Aucun fichier partiel silencieusement invalide : validation avant écriture, écriture transactionnelle, rapport d'avertissements.
- (+) La matrice `write_matrix.md` devient une suite de tests de non-régression (ADR/architecture §6.2).
- (−) L'encodeur RDC n'est pas octet par octet celui de SAS (150 flots sur 390 identiques), seul l'encodeur RLE l'est.
- (−) `Replace`/`Truncate` changent les données : toujours comptés dans `WriteReport` (jamais silencieux).
- Questions ouvertes : règle de validité des noms SAS (lettres/chiffres/soulignement, longueur 32) à confirmer par essai SAS ; écriture `Binary` sur colonnes `Utf8` à encodage 29.

## Mise en œuvre (J6, 2026-11-04)

- **Couches** : le cœur `uniformat_sas::write` (sans entrée/sortie) porte `WriteSchema::validate` (appelée par `FileBuilder::new`), `FileBuilder` (`push_row`, `take_pages`, `finish`), `WriteOptions`, `CellValue`, `Unrepresentable`, `WriteReport` ; `uniformat-sas-io` ajoute `SasWriter<W: Write + Seek>` et `AtomicFile` (fichier temporaire renommé par `commit`, supprimé à l'abandon) ; `uniformat-sas-arrow` ajoute `ArrowWriter` et `write_batches` ; les liaisons exposent `write_sas` (Python) et `write_sas7bdat` (R). Les pages de données sortent au fil de l'eau ; la page 0, la page marqueur et l'en-tête (qui dépendent des totaux) sortent à la fin et sont écrites par `seek`.
- **Fidélité** : le cœur est un port de §4 de `20` ; ses fichiers sont **identiques octet pour octet** à ceux de `ref_writer.py` sur la matrice (52 cas sur 53 écrits par les deux).
- **Réalisé** : validation avant le premier octet (nom, label, longueurs, doublons, encodage, nombre de colonnes, page), agrandissement de la page (`grow_page`, puissance de 2 jusqu'à 1 Mio) ou erreur `TooManyColumns { max_for_page }` / `RowTooLong`, politique unique `Unrepresentable::{Error, Replace, Truncate}` avec compteurs, colonne annexe `x__missing`, formats par défaut `DATE9.` / `DATETIME20.` / `TIME8.`, types Arrow `Float*`, entiers, `Boolean`, `Decimal128`, `Date32/64`, `Timestamp`, `Time32/64`, `Duration`, `Utf8*`, `Binary*`, `Dictionary`, compression `Char` et `Binary`, désactivée sous 16 octets par ligne (avertissement `compression_disabled_small_rows`).
- **Non réalisé** : `Compression::Auto` (§2) ; label de table ; contrôle des règles de nom SAS ; `Unrepresentable` pour les décimaux autrement que par arrondi ; plus de 1 800 pages de lignes compressées (page marqueur, inconnue W12) ; avertissement `untested_large_file`.
- **Écart avec §3** : `Timestamp` plus fin que la seconde est écrit en secondes fractionnaires (aucune erreur), `NaN` en `.` dans tous les modes, comme prévu ; une longueur numérique de 3 à 7 octets dont la mantisse perdrait des bits est une **erreur** par défaut (ADR : `Error`), troncature seulement avec `Replace` ou `Truncate`.
- **Constat** : ReadStat refuse les copies RDC dont la longueur dépasse le décalage ; l'encodeur du cœur n'en produit plus (voir `docs/roadmap.md`, J6 (a)). La consigne « l'encodeur RDC n'est pas octet par octet celui de SAS » (Conséquences) reste vraie.

