## Why

Le driver Rust visé doit **écrire** des `.sas7bdat`, mais l'état de l'art public (ReadStat, Parso, pandas, PDF de Shotwell) est centré sur la lecture : de nombreux champs d'en-tête et de sous-en-tête ne sont décrits que par « valeur observée », sans règle de calcul. Sans une spécification d'écriture validée par SAS lui-même, tout fichier produit risque d'être rejeté ou mal interprété par SAS ou par les autres lecteurs. Ce changement (prompt 05) s'appuie sur la spec de lecture (02) et de compression (03) déjà produites pour combler ce manque avant la conception Rust (06).

## What Changes

- Spécification d'une procédure d'écriture pas à pas d'un `.sas7bdat` que **SAS relit sans `ERROR` ni `WARNING`**, ainsi que ReadStat (`pyreadstat`), pandas et Parso (si disponible), avec données et métadonnées identiques à l'attendu. L'identité octet à octet avec SAS n'est **pas** un objectif (horodatages, champs libres).
- Classification de **chaque** champ d'en-tête et de sous-en-tête en `calculé` (formule), `constante` (valeur) ou `libre`, établie par mutation de fichiers valides ; décompilation de `sase7opn` / `sase7io` réservée aux rejets que les mutations n'expliquent pas.
- Règles de construction : ordre des sous-en-têtes, taille de page et d'en-tête, stratégie de remplissage des pages, mise à jour des compteurs, limites (noms 32 octets, labels 256, char 32 767, nombre de colonnes).
- Montée en complexité par cas validés un à un : colonnes num/char multiples, longueurs num 3–8, missing spéciaux, labels, formats/informats, noms longs, non ASCII, pages multiples (data et mix), nombreuses colonnes, 0 ligne, gros volume (> 2 Go si faisable), `COMPRESS=CHAR` et `BINARY`, encodage autre qu'UTF-8.
- Livrables :
  - `rea_output/20_ecriture.md` (procédure, table des champs, limites, cas non supportés) ;
  - `rea_output/tools/ref_writer.py` (écrivain de référence, outil d'analyse — pas le driver) ;
  - `rea_output/tools/write_matrix.md` (matrice cas × lecteur → OK/KO + log) ;
  - mises à jour de `rea_output/60_inconnues.md`, `rea_output/evidence/` et `rea_output/STATE.md`.
- Hors priorité (documenté seulement si observable) : ajout de lignes à un fichier existant, lignes supprimées, index, contraintes d'intégrité, chiffrement.

## Capabilities

### New Capabilities
- `sas7bdat-writing` : spécification vérifiée de la production de `.sas7bdat` valides (procédure, classification des champs, règles de construction, limites, matrice de validation multi-lecteurs).

### Modified Capabilities
<!-- Aucune : les capacités sas7bdat-file-format (02) et sas7bdat-compression (03) sont consommées, pas modifiées ; les corrections qu'elles nécessitent sont tracées dans leurs documents rea_output/. -->

## Impact

- **Dépendances** : requiert `specify-sas7bdat-read-format` (02, `rea_output/10_format_sas7bdat.md`) et `specify-sas7bdat-compression` (03, `rea_output/15_compression.md`, `rea_output/tools/ref_compress.py`) ; `specify-sas-formats-encodings` (04, `rea_output/30_formats_encodages.md`) conseillé ; s'appuie sur `setup-sas-oracle-corpus` (01 : SAS exécutable dans un Linux x86-64 + `corpus/`) et sur `survey-sas7bdat-prior-art` (00 : manques en écriture).
- **Débloque** : `design-rust-driver` (06), qui lira `rea_output/20_ecriture.md` (jamais `evidence/`).
- **Systèmes** : exécution de SAS (oracle de relecture) hors macOS ; lecteurs tiers en Python (`pyreadstat`, pandas) et Java (Parso) en option.
- **Licence** : analyse limitée aux informations non publiées, à des fins d'interopérabilité, sous licence SAS NULL ; confirmation interne à consigner dans `rea_output/STATE.md`.
- `src/` reste en lecture seule ; aucun code du driver n'est écrit dans ce changement.
