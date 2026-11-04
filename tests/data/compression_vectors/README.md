# Vecteurs de test de compression `.sas7bdat`

Générés par `python3 -I rea_output/tools/make_vectors.py <dossier corpus> rea_output/tools/compression_vectors` à partir de `corpus/files/` et `corpus/external/` (copie locale conseillée : le corpus est volumineux). Spécification : `rea_output/15_compression.md`.

## Fichiers

| Fichier | Algorithme | Vecteurs |
|---|---|---|
| `char.tsv` | `COMPRESS=CHAR`, RLE (littéral `SASYZCRL`) | 338 |
| `binary.tsv` | `COMPRESS=BINARY`, RDC (littéral `SASYZCR2`) | 367 |

## Format (TSV, UTF-8, première ligne = en-tête, séparateur tabulation)

| Colonne | Contenu |
|---|---|
| `id` | `char-NNNN` ou `binary-NNNN` (numérotation par ordre de génération, non stable si le corpus change) |
| `algorithme` | `CHAR` ou `BINARY` |
| `fichier` | chemin relatif à `corpus/` du fichier compressé source |
| `ligne` | rang de la ligne dans le fichier (0-based, ordre des pointeurs) |
| `stockage` | `compressed` : `stocke_hex` est le flot compressé par SAS (pointeur dont l'octet de compression a le bit 2 : 4, 6, 13) ; `raw` : ligne stockée brute (pointeur de type 1, octet de compression 0, 2 ou 9), `stocke_hex` = ligne brute |
| `categorie` | `corpus` ; `bornes-de-longueur` (`compression_runs` : suites de 2 à 500 octets identiques pour les octets `0x78`, `0x40`, `0x00`, `0xFF`, `0x20`) ; `incompressible` (`compression_noise`) ; `ligne-quasi-uniforme` (`compression_zeros`, `compression_blank200`) ; `externe-sans-brut` (fichiers publics sans table de référence) |
| `longueur_ligne` | longueur d'une ligne décompressée (`row_length` du fichier compressé, **sans** arrondi à 8) |
| `brut_hex` | ligne brute de référence en hexadécimal, prise dans la table `_no` de même nom (tronquée à `longueur_ligne`) ; vide pour `externe-sans-brut` |
| `stocke_hex` | octets stockés dans le fichier pour cette ligne |

## Contrat de test

- Pour tout vecteur `compressed` avec `brut_hex` non vide : `decompress(stocke_hex, longueur_ligne) == brut_hex`.
- Pour tout vecteur : `decompress(compress(brut_hex), longueur_ligne) == brut_hex` (aller-retour du compresseur de référence).
- Pour `externe-sans-brut` : la décompression réussit et produit `longueur_ligne` octets.

## Couverture et limites

- Chaque algorithme dispose de vecteurs du corpus (au plus 12 lignes par fichier, toutes les lignes de `compression_runs`), de `bornes-de-longueur`, d'`incompressible` et de `ligne-quasi-uniforme`.
- **Non couvert** (l'oracle `sas_small` ne sait pas écrire de table compressée, voir `rea_output/STATE.md`) : ligne de longueur nulle, ligne strictement uniforme sur toute sa longueur, bornes des commandes RDC (longueurs 16, 18, 19, décalages 3 et 4 098). À produire avec le SAS complet (`prompts/01b_extraction_sas_complet.md`).
- Les lignes `raw` ne concernent que RDC (226 lignes sur l'ensemble du corpus, 30 dans ce jeu).
