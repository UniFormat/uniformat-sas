## Why

Les phases d'analyse du format (02 à 05) et la validation du futur driver Rust exigent un **oracle** : un SAS qui s'exécute réellement, pour générer des fichiers `.sas7bdat` de référence et relire ceux qu'on écrira. Or `src/sas_small` est un binaire ELF x86-64 Linux, alors que l'hôte est un Mac ; et il n'existe aujourd'hui aucun corpus contrôlé où un seul paramètre varie à la fois, condition nécessaire à la méthode différentielle (comparaison d'octets) des prompts suivants.

## What Changes

- Mise en place d'un environnement d'exécution documenté pour `sas_small` (ou, à défaut, le SAS complet d'origine) sur l'hôte macOS : détection de l'architecture (`uname -m`), présentation des options dans l'ordre fixé (conteneur Docker `--platform linux/amd64`, VM Linux x86-64 UTM/Parallels, machine Linux distante, SAS complet d'origine) avec commande exacte et limites, puis **choix validé par l'utilisateur avant toute installation**.
- Exécution d'un programme SAS minimal depuis une commande documentée ; arrêt et signalement en cas d'échec lié à la licence (`elssrv`, SETINIT, nom d'hôte), sans aucun contournement.
- Inventaire des capacités réelles de `sas_small` (table permanente, `COMPRESS=CHAR|BINARY`, `PROC FORMAT` + `FMTLIB`, `PROC SQL`, `PROC EXPORT`, `ENCODING=` en sortie, `PROC DATASETS` avec index et `MODIFY`/`REMOVE`), consigné dans `rea_output/STATE.md`.
- Création du corpus de référence sous `corpus/` :
  - `corpus/gen/*.sas` : un programme par famille (base, types, valeurs, métadonnées, taille, compression, suppression, encodage) et un script qui les exécute tous ;
  - pour chaque table : le `.sas7bdat`, la sortie texte de `PROC CONTENTS`, celle de `PROC PRINT` et un CSV de référence (via `PROC EXPORT` ou, à défaut, une étape DATA) ;
  - variantes `NO` / `CHAR` / `BINARY` pour chaque famille, ou justification de l'absence ;
  - `corpus/external/` : fichiers publics (ReadStat, Parso, pandas) pour les cas impossibles avec cet oracle (big endian, 32 bits, chiffrement, autres OS), avec source et licence ;
  - `corpus/MANIFEST.md` : une ligne par fichier (nom, famille, paramètre varié, source, outil, taille, SHA-256).
- Mise à jour finale de `rea_output/STATE.md` (environnement retenu, capacités testées, blocages, prochaine action).
- Hors périmètre : aucune analyse des octets, aucune décompilation, aucune modification de `src/`.

## Capabilities

### New Capabilities
- `sas-oracle-environment` : environnement d'exécution de SAS (`sas_small` ou SAS complet) sur l'hôte macOS via Linux x86-64, choix d'option validé par l'utilisateur, commande de lancement documentée, respect strict de la licence et inventaire des capacités de l'oracle.
- `sas7bdat-reference-corpus` : corpus de fichiers `.sas7bdat` de référence généré par familles en faisant varier un seul paramètre à la fois, avec sorties de contrôle (CONTENTS, PRINT, CSV), fichiers externes sourcés et manifeste avec empreintes SHA-256.

### Modified Capabilities
<!-- Aucune : aucune spec n'existe encore sous openspec/specs/. -->

## Impact

- **Nouveaux répertoires et fichiers** : `corpus/` (`gen/`, fichiers générés, `external/`, `MANIFEST.md`) ; `rea_output/STATE.md` mis à jour.
- **`src/`** : lecture seule ; `sas_small` est exécuté mais jamais modifié. Le lanceur écrit `WORK`/`SASUSER` sous `${TMPDIR:-/tmp}/sas_small.$USER`.
- **Outillage hôte** : selon l'option choisie, Docker (avec Rosetta ou qemu), un hyperviseur (UTM, Parallels) ou un accès à une machine Linux distante.
- **Licence** : l'exécution repose sur le SETINIT de la licence SAS 9.4 TS1M7 de NULL embarquée ; aucun contournement.
- **Dépendances entre changements** :
  - dépend de `survey-sas7bdat-prior-art` (00) : lecture de `rea_output/00_etat_de_l_art.md` pour cibler les familles du corpus sur les manques identifiés ;
  - débloque `specify-sas7bdat-read-format` (02), `specify-sas7bdat-compression` (03), `specify-sas-formats-encodings` (04) et `specify-sas7bdat-writing` (05), qui exploitent l'oracle et le corpus ;
  - `corpus/MANIFEST.md` fait partie des seuls documents lus par `design-rust-driver` (06).
