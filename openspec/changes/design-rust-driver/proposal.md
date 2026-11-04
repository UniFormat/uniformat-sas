## Why

Les changements 02 à 05 produisent une spécification du format `.sas7bdat` (lecture, compression, formats/encodages, écriture) et la liste des inconnus. Il faut maintenant concevoir, **à partir de cette seule spécification**, un driver Rust qui lit et écrit ces fichiers et qui sert de socle à Arrow, Python, R, SQL (DataFusion) et plus tard Arrow Flight / Dremio. Cette conception doit être faite dans une phase séparée de l'analyse pour que le code Rust soit écrit d'après une description du format, et non d'après le code SAS.

## What Changes

- Production d'un document d'architecture `docs/architecture.md` : vue d'ensemble, découpage en crates, flux de données, API publique esquissée (signatures uniquement, aucune implémentation).
- Production d'une ADR par décision dans `docs/adr/NNNN-*.md` :
  - découpage en crates (`sas7bdat` cœur sans I/O, `-arrow`, `-datafusion`, `-py`, `-r`, `-cli`) et dépendances autorisées dans le cœur ;
  - modèle d'I/O (`Read + Seek`, mmap, `object_store` S3/HTTP pour la lecture par plages ; async ou non) ;
  - mapping des types depuis la table de classification de `rea_output/30_formats_encodages.md` (num → Float64 / Date32 / Timestamp / Time64 / Decimal ; char → Utf8 avec option sur les blancs finaux ; missing → null avec conservation du code de missing spécial) ;
  - sémantique SQL propre au driver (comparaison de chaînes avec espaces finaux, ordre des missing, lignes supprimées invisibles) ;
  - API d'écriture (depuis Arrow, depuis un itérateur de lignes), choix de compression, validation des limites de `rea_output/20_ecriture.md`, comportement sur valeur non représentable ;
  - gestion des inconnus de `rea_output/60_inconnues.md` (erreur, avertissement ou tolérance) ;
  - erreurs (`thiserror`), politique `unsafe`, MSRV, licence du driver (selon `rea_output/00_etat_de_l_art.md`).
- Description de la stratégie de tests : corpus `corpus/` en tests d'intégration (CSV de référence), round-trip (driver, SAS, ReadStat, pandas), proptest, fuzzing `cargo-fuzz`, benchmarks contre ReadStat/pyreadstat et pandas.
- Production de `docs/roadmap.md` : jalons lecture non compressée → compression → métadonnées/formats → Arrow/DataFusion → Python/R → écriture → Flight, chacun avec un critère de fin.
- Mise à jour de `rea_output/STATE.md` en fin de session.
- Contraintes de séparation : la session ne lit que `prompts/README.md`, `rea_output/STATE.md`, `rea_output/00_etat_de_l_art.md`, `10_format_sas7bdat.md`, `15_compression.md`, `20_ecriture.md`, `30_formats_encodages.md`, `60_inconnues.md` et `corpus/MANIFEST.md` ; lecture de `src/`, `rea_output/evidence/` et de toute sortie de décompilation interdite.

Hors périmètre : écrire du code d'implémentation Rust (aucun `Cargo.toml`, aucun `.rs`), lancer SAS, utiliser la skill REA, modifier `src/`, compléter la spécification du format (rôle de 02 à 05).

## Capabilities

### New Capabilities
- `sas7bdat-rust-driver` : conception du driver Rust `sas7bdat` — architecture, crates, ADR, mapping des types, sémantique SQL, écriture, traitement des inconnus, stratégie de tests et feuille de route, produites à partir de la seule spécification.

### Modified Capabilities
<!-- Aucune : aucune spec existante sous openspec/specs/. -->

## Impact

- Nouveaux fichiers : `docs/architecture.md`, `docs/adr/NNNN-*.md`, `docs/roadmap.md` ; mise à jour de `rea_output/STATE.md`.
- Dépendances (changements à terminer avant) : `specify-sas7bdat-read-format` (02, `10_format_sas7bdat.md`), `specify-sas7bdat-compression` (03, `15_compression.md`), `specify-sas-formats-encodings` (04, `30_formats_encodages.md`), `specify-sas7bdat-writing` (05, `20_ecriture.md`) ; indirectement `survey-sas7bdat-prior-art` (00, licences et état de l'art) et `setup-sas-oracle-corpus` (01, `corpus/MANIFEST.md`).
- Écosystème visé par la conception (non installé ici) : Apache Arrow, DataFusion, PyO3/maturin, extendr, `object_store`, `thiserror`, proptest, cargo-fuzz.
- Aucun impact sur `src/` (lecture seule, et interdit de lecture dans cette phase).
