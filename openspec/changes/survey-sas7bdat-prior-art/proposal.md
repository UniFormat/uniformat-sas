## Why

Le futur driver Rust doit lire et écrire les `.sas7bdat`, mais une grande partie du format est déjà décrite publiquement (ReadStat, Parso, pandas, PDF de Shotwell, etc.). Avant tout reverse engineering, il faut établir ce qui est déjà connu, pour que les phases suivantes (02 à 05) ne portent que sur les vrais manques : c'est une condition de l'exception d'interopérabilité (directive UE 2009/24, art. 6), qui ne couvre que les informations non déjà disponibles, et c'est aussi la façon la plus économique de procéder.

## What Changes

- Production de `rea_output/00_etat_de_l_art.md`, structuré en cinq sections :
  1. inventaire des implémentations et documentations publiques (ReadStat, Parso, pandas `pandas.io.sas`, paquet Python `sas7bdat`, PDF de Matthew Shotwell / paquet R `sas7bdat`, `readstat-rs` et autres crates Rust sur crates.io, implémentations Go/JS pertinentes, documentation SAS officielle publique sur `COMPRESS=`, `ENCODING=` et les formats), avec URL, dernière version / activité et licence vérifiée sur la source ;
  2. matrice des capacités (une ligne par capacité, une colonne par implémentation), chaque case sourcée (fichier source + ligne, ou document) ;
  3. points de désaccord entre implémentations (interprétations divergentes, champs ignorés, constantes « magiques » écrites sans explication, notamment en écriture dans ReadStat) ;
  4. liste des manques, cibles du reverse engineering, chacun avec description, impact (bloquant / dégradé / confort), prompt de rattachement (02 à 05) et méthode envisagée (analyse différentielle ou décompilation) ;
  5. contraintes de licence : sources lisibles sans risque pendant la conception, licence recommandée pour le driver, règle d'usage par source.
- Création de `rea_output/STATE.md` selon le modèle de `prompts/README.md`.
- Création de `rea_output/60_inconnues.md`, vide, avec son en-tête.
- Hors périmètre : aucun accès à `src/`, aucune exécution de SAS, aucune conception Rust, aucune décompilation.

## Capabilities

### New Capabilities
- `sas7bdat-prior-art`: état de l'art public sur le format `.sas7bdat` (inventaire sourcé, matrice des capacités, désaccords, manques rattachés aux changements 02–05, contraintes de licence) et initialisation des fichiers de suivi `rea_output/STATE.md` et `rea_output/60_inconnues.md`.

### Modified Capabilities
<!-- Aucune : premier changement de la série, aucune spec existante. -->

## Impact

- Fichiers créés : `rea_output/00_etat_de_l_art.md`, `rea_output/STATE.md`, `rea_output/60_inconnues.md`. Aucun code, aucune modification de `src/` (lecture seule, non consulté ici).
- Dépendances : aucune en amont. Ce changement conditionne toute la série :
  - `setup-sas-oracle-corpus` (01) s'appuie sur la liste des manques pour cibler le corpus de référence ;
  - `specify-sas7bdat-read-format` (02), `specify-sas7bdat-compression` (03), `specify-sas-formats-encodings` (04) et `specify-sas7bdat-writing` (05) ne traitent que les manques qui leur sont rattachés ici, et partent des descriptions publiques comme hypothèses à vérifier ;
  - `design-rust-driver` (06) reprend la licence recommandée et les règles de lecture des sources.
- Systèmes externes : accès réseau en lecture aux dépôts publics (GitHub, crates.io, PyPI, CRAN, documentation SAS) pour vérifier versions et licences.
