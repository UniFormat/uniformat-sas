# ADR 0007 — Erreurs, politique `unsafe`, MSRV et licence

- Statut : proposée (2026-10-30) ; la licence reste **soumise à la confirmation NULL** (`STATE.md`, blocages ouverts)
- Spécification liée : `00` §5.1 à §5.4 (licences), `10` §7 (contrôles), `60` (inconnus), `20` §7 et §8 (erreurs d'écriture)

## Contexte

Le driver parse des fichiers non fiables (objets S3, Flight) : il ne doit ni paniquer, ni allouer sans borne, ni exécuter de comportement indéfini. Il est exposé à Python (PyO3) et R (extendr), où une panique non rattrapée ou un `unsafe` mal gardé fait tomber l'interpréteur. La section licences de `00_etat_de_l_art.md` (5.1 à 5.4) établit la règle de lecture des sources, recommande une licence de driver et liste quatre points à confirmer en interne chez NULL.

## Décisions

### 1. Erreurs (`thiserror`)

- Un type d'erreur par crate, `#[non_exhaustive]`, dérivé avec `thiserror` : cœur `uniformat_sas::Error` ; `uniformat-sas-io::IoError` (enveloppe `io::Error` et `Error`) ; `uniformat-sas-arrow::ArrowError` (enveloppe aussi `arrow::error::ArrowError`).
- Variantes du cœur (liste de départ) : `NotSasFile { reason }`, `UnsupportedFormat { what }` (par exemple 32 bits big endian en écriture), `Truncated { needed, got }`, `LimitExceeded { what, value, limit }`, `CorruptHeader { field, offset }`, `CorruptPage { page, field }`, `CorruptSubheader { page, signature }`, `CorruptRow { page, row }`, `Unverified { id }` (mode `Strict`, ADR 0006), `UnknownEncoding(code)`, `Encoding { column, row }`, `Unsupported(feature)`, `Schema(SchemaError)`, `Write(WriteError)`.
- Toutes les variantes d'analyse portent l'offset ou la page ; le `Display` ne cite jamais de données de fichier brutes non assainies.
- Convention : **aucune** fonction publique ne panique sur entrée invalide ; `unwrap`/`expect`/indexation directe sont interdits dans le code de production (lint `clippy::unwrap_used`, `clippy::indexing_slicing`, `clippy::arithmetic_side_effects` en `deny` dans le cœur) ; les débordements d'entiers utilisent `checked_*`.
- Les avertissements non fatals ne sont pas des erreurs : ils sont collectés dans `ReadReport`/`WriteReport` (ADR 0006).
- Bindings : Python lève `SasError` (hiérarchie `SasFormatError`, `SasUnsupportedError`, `SasWriteError`) ; R signale une condition `sas7bdat_error` ; les avertissements deviennent `warnings.warn` / `warning()`.

### 2. Politique `unsafe`

- `#![forbid(unsafe_code)]` dans `sas7bdat`, `uniformat-sas-arrow`, `uniformat-sas-datafusion`, `uniformat-sas-flight`, `uniformat-cli`.
- `uniformat-sas-io` : `#![deny(unsafe_code)]` ; une seule exception, le module `mmap` (feature `mmap`, désactivée par défaut), qui encapsule `memmap2::Mmap::map` avec un bloc `unsafe` unique, commenté (`SAFETY` : le fichier peut être modifié par un autre processus, ce qui ne viole pas la sûreté mémoire de Rust mais peut produire des octets incohérents ; le parseur les traite comme un fichier corrompu grâce aux bornes). La lecture mmap n'expose que `&[u8]` avec des vérifications de bornes.
- `uniformat-sas-py` et `uniformat-sas-r` : `unsafe` autorisé uniquement à la frontière FFI générée par PyO3 et extendr, jamais dans le code de décodage ; revue obligatoire de chaque bloc.
- Les crates dépendantes (`arrow`, `memmap2`, `encoding_rs`, `pyo3`…) apportent leur propre `unsafe` : audit par `cargo-deny` + `cargo-vet` (ou `cargo-audit`) en CI.
- Aucun `transmute` ni lecture alignée non vérifiée : lectures d'entiers par `from_le_bytes`/`from_be_bytes` sur des tranches de longueur vérifiée.

### 3. MSRV

- Politique : MSRV = **dernière stable − 2** à la date de la première release, jamais inférieure à ce qu'exigent `arrow`, `datafusion` et `object_store` ; relevée seulement dans une version mineure et annoncée dans le `CHANGELOG`. Valeur de départ à fixer au jalon 0 en lisant la MSRV des dépendances (information qui n'est pas dans la spécification et qui n'est pas figée ici). Édition Rust 2021 au minimum.
- CI : `cargo +<MSRV> check --workspace` en plus de la stable.
- Le cœur vise une MSRV plus basse que les adaptateurs (il n'utilise ni Arrow ni async) : documentée séparément si l'écart est utile.

### 4. Licence

- **Choix : double licence `MIT OR Apache-2.0`** pour tous les crates, le paquet Python et le paquet R (les deux fichiers `LICENSE-MIT` et `LICENSE-APACHE` à la racine ; en-tête SPDX `MIT OR Apache-2.0`).
- **Justification (section 5.2 de `00`)** : (a) CRAN et PyPI acceptent les deux licences, Apache-2.0 est incompatible avec GPL v2 stricte alors que MIT l'est : la double licence laisse choisir ; (b) Arrow, DataFusion, Arrow Flight et Dremio sont en Apache-2.0 : compatibilité naturelle et protection de brevets pour les utilisateurs en entreprise ; (c) aucune source de lecture libre (5.1) n'est sous copyleft, donc aucune dérivation contaminante ; (d) cohérence avec ReadStat (MIT), Parso (Apache-2.0), pandas (BSD-3-Clause), `sas7bdat` Rust (MIT) et `polars-readstat-rs` (Apache-2.0).
- **Conditions** (section 5.4 de `00`) : la décision dépend de NULL sur quatre points ouverts — (1) conditions d'utilisation de sas.com (clause sur le développement d'une offre concurrente) avant d'utiliser la documentation SAS comme source ; (2) confirmation de la licence SAS de `src/` pour l'analyse d'interopérabilité (non consignée) ; (3) cadre de la directive 2009/24/CE art. 6 ; (4) autorisation de publication en logiciel libre. **Tant que (4) n'est pas obtenue, le dépôt reste privé** ; la licence est inscrite dans les fichiers dès le début pour ne pas la décider crate par crate plus tard, et le choix est repris si NULL décide autrement.
- **Règle de lecture pour les contributeurs** (reprise de 5.1 de `00`) : lecture libre de ReadStat (sauf `src/sas/ieee.c`), `pyreadstat`, Parso ≥ 2.0.8 (jamais ≤ 2.0.7, GPL v3), pandas, `sas7bdat` Python, `FredHutch/sas7bdat`, la crate `sas7bdat`, `polars-readstat-rs`, `readstat-rs`, `statfmt`, `datareader`, `richokelly/sas7bdat`, `cpp-sas7bdat`, `SASLib.jl` ; **documentation seule** pour le paquet R `sas7bdat`, `sasquatch` (Rust et Java, EUPL), `sas7bdat-js`, `goreadstat`, `FredHutch/sas7bdat-specification` ; **non lu** pour les sources sans licence (`SAS-reader`, `quick-sas7bdat`, `rust-sas-reader`, `readstat-go`, extensions DuckDB, paquets npm). Le PDF de Shotwell (CC BY-SA 3.0) se lit pour les faits, jamais recopié. Cette règle figure dans `CONTRIBUTING.md`.
- **Fichiers de test** (section 5.3 de `00`) : redistribuables avec attribution (`NOTICE`) : `datareader`, Parso, pandas, `pyreadstat`, ReadStat ; **non redistribuables** : `polars_readstat` et `sas7bdat-js` ; les fichiers du corpus du projet produits par le SAS complet ne sont pas publiés sans accord NULL ; seuls les fichiers générés par le driver lui-même (écrits à partir de la matrice de `20`) peuvent être commités.
- Dépendances : `cargo-deny` interdit toute licence copyleft forte dans le graphe de production (GPL, AGPL, EUPL) ; `MIT`, `Apache-2.0`, `BSD-3-Clause`, `ISC`, `Unicode-3.0` autorisées.

## Options étudiées

| Sujet | Options | Choix et raison |
|---|---|---|
| erreurs | (a) `anyhow` partout ; (b) `thiserror` typé (retenue) ; (c) codes d'erreur entiers | (b) : les bindings et la CLI ont besoin de variantes stables et d'identifiants d'inconnus |
| `unsafe` | (a) libre ; (b) interdit partout, y compris mmap ; (c) interdit sauf mmap isolé et FFI (retenue) | (c) : le gain de `mmap` justifie une zone minuscule et auditée ; (b) priverait de zéro copie local |
| MSRV | (a) stable courante ; (b) stable − 2 (retenue) ; (c) figée | (b) : compromis entre dépendances Arrow et utilisateurs d'entreprise |
| licence | (a) MIT seule ; (b) Apache-2.0 seule ; (c) `MIT OR Apache-2.0` (retenue) ; (d) GPL/copyleft | (c) selon `00` §5.2 ; (d) incompatible avec l'écosystème visé |

## Conséquences

- (+) Fuzzing et audit portent sur un cœur sans `unsafe` ni I/O ; un parseur qui rend une erreur typée à chaque entrée invalide.
- (+) Les trois dépendances Python/R/Arrow restent compatibles avec la licence choisie.
- (−) Les lints `arithmetic_side_effects`/`indexing_slicing` alourdissent l'écriture du parseur ; accepté.
- (−) La publication dépend d'une décision NULL extérieure à l'ingénierie : le calendrier de la roadmap ne prévoit aucune publication avant le jalon 5 et avant cette décision.

## Mise en œuvre (J5, 2026-11-04)

- **Erreurs** : Python lève `SasError` (`SasFormatError`, `SasUnsupportedError`, `SasWriteError`) ou `OSError` (`FileNotFoundError`, `PermissionError`), `ValueError` pour une option ou une colonne invalide, et émet des `SasWarning` ; R signale `sas7bdat_error` (sous-classes `sas7bdat_io_error`, `sas7bdat_format_error`, `sas7bdat_usage_error`, `sas7bdat_internal_error`) et des `warning()`.
- **Pas de plantage** : PyO3 convertirait une panique en `PanicException` et extendr en erreur R ; aucun cas n'a été observé (720 fichiers altérés en Python, 480 en R). Le test refuse toute `PanicException` (défaut du cœur) et toute erreur R « interne ».
- **`unsafe`** : `uniformat-sas-py` est en `#![deny(unsafe_code)]` et n'écrit aucun bloc `unsafe` (le code généré par PyO3 compile sans exception) ; `uniformat-sas-r` n'autorise `unsafe` que sur le module `glue`, compilé avec la feature `r`, et n'écrit aucun bloc non plus. La CLI reste en `forbid(unsafe_code)`.
- **Audit des dépendances** : `BSD-2-Clause` autorisée (`numpy`) ; `RUSTSEC-2024-0436` (`paste`, non maintenu, via `extendr-api`) ignoré dans `deny.toml` avec sa raison.

