# État de l'art public du format `.sas7bdat`

> Changement OpenSpec : `survey-sas7bdat-prior-art` (prompt 00). Date de consultation : **2026-10-29** (certaines requêtes de fin de session sont passées après minuit, le 2026-10-30 ; aucun résultat n'en dépend).
> Périmètre : sources publiques uniquement. Aucun accès à `src/`, aucune exécution de SAS, aucune décompilation, aucune conception du driver.

## Conventions de lecture

- **Niveaux de confiance.** Toute description du format tirée d'une source publique est une **hypothèse** : elle est marquée `inféré` (une source, ou plusieurs sources non indépendantes) ou `inconnu`. Aucune affirmation de ce document n'est `confirmé` : il n'y a ni preuve binaire ni preuve runtime à ce stade (voir `prompts/README.md`).
- **Lecture statique.** Rien n'a été compilé ni exécuté. Dans la matrice, `oui` signifie « du code (ou un passage de documentation) existe à l'endroit cité » ; `non` signifie « absence constatée par recherche textuelle dans les fichiers consultés », pas « échec constaté à l'exécution ».
- **Valeurs de la matrice.** `oui`, `non`, `partiel`, `inconnu`. Chaque case porte sa source (`fichier:ligne`, ou document + page/section). Une case `inconnu` cite ce qui a été consulté.
- **Abréviations de chemins** (légende reprise en section 2) :
  - ReadStat : `rd.c` = `src/sas/readstat_sas7bdat_read.c`, `wr.c` = `src/sas/readstat_sas7bdat_write.c`, `sas.c` = `src/sas/readstat_sas.c`, `sas.h` = `src/sas/readstat_sas.h`, `rle.c` = `src/sas/readstat_sas_rle.c`, `bcat.c` = `src/sas/readstat_sas7bcat_read.c` (HEAD `1434a07`, 2026-10-27).
  - Parso : `SFP` = `parso/src/main/java/com/epam/parso/impl/SasFileParser.java`, `SFC` = `.../impl/SasFileConstants.java` (v2.0.14).
  - pandas (`main` @ `092d588`) : `P7` = `pandas/io/sas/sas7bdat.py`, `PC` = `pandas/io/sas/sas_constants.py`, `PX` = `pandas/_libs/sas.pyx`.
  - `sas7bdat` Python 2.3.0 : `PY` = `sas7bdat.py`. Paquet R 0.8 : `R` = `R/sas7bdat.R`. PDF de Shotwell : `PDF p.N` (page imprimée).
  - Crates et paquets (matrice 2c) : `A:` = `crates/sas7bdat/` de `tkragholm/sas7bdat-parser-rs` ; `B:` = `crates/polars_readstat_rs/` de `jrothbaum/polars_readstat` ; `C:` = `irbisadm/statfmt` ; `D:` = `kshedden/datareader` ; `E:` = `curtisalexander/readstat-rs` (`E-RS:` = ReadStat au commit vendorisé `da9fcaa`).
  - Documentation SAS : `S1`…`S17`, `T1`…`T3` = index des sources de la section 1.4.

---

## 1. Inventaire

### 1.1 Implémentations et documentations de référence

| ID | Implémentation | URL | Version / activité constatée | Licence (SPDX) | Où la licence a été lue | Consulté |
|---|---|---|---|---|---|---|
| I1 | **ReadStat** (C) | https://github.com/WizardMac/ReadStat (l'URL `WizardMike/ReadStat` donne un 404) | Dernier tag `v1.1.9` (2023-02-20) ; HEAD `1434a07` du 2026-10-27, très en avance sur le tag ; `NEWS` sans entrée après 1.1.9 | **MIT** | `LICENSE:1-20` ; `README.md:9` ; aucun en-tête de licence dans `src/sas/*`. Réserve : `src/sas/ieee.c:7-14` (XPORT) reprend des routines d'une publication SAS (TS-140) dont la licence n'est pas précisée | 2026-10-29 |
| I2 | `haven` (R, wrapper de ReadStat) | https://github.com/tidyverse/haven | Tag `v2.5.5` (2025-05-30) ; `DESCRIPTION:3` = `2.5.5.9000` (dev) ; dernier commit 2026-10-21 ; copie vendorisée de ReadStat plus ancienne que le HEAD (commit exact `inconnu`, `NEWS.md:24` cite `0dc2aee`) | **MIT** | `DESCRIPTION:13`, `LICENSE.md:1-3`, `src/readstat/LICENSE` | 2026-10-29 |
| I3 | `pyreadstat` (Python, wrapper de ReadStat) | https://github.com/Roche/pyreadstat | `v1.3.7` (2026-10-21) ; ReadStat vendorisé à `98813868…` (`change_log.md:4`) | **Apache-2.0** (wrapper) + **MIT** (ReadStat embarqué) | `setup.py:154` ; `LICENSE:1-8` | 2026-10-29 |
| I4 | **Parso** (Java, EPAM) | https://github.com/epam/parso ; Maven `com.epam:parso` | `2.0.14` (jar du 2021-02-19) ; dernier commit `master` 2021-03-18 : **maintenance gelée** | **Apache-2.0** (≥ 2.0.8). Le pom de 2.0.7 et avant déclare **GPL v3** | `LICENSE` ; en-têtes de source (`SFP:1-16`) ; `pom.xml:20-26` ; `README.md:80` (« Changed license to ALv2 » en 2.0.8) ; `README.md:241-242` (offre commerciale parallèle) | 2026-10-29 |
| I5 | **pandas** `pandas.io.sas` | https://github.com/pandas-dev/pandas (`pandas/io/sas/`, `pandas/_libs/sas.pyx`) | Release `3.1.0` (2026-10-22) ; analyse sur `main` @ `092d588` (3.2.0.dev0). Le tag `v3.1.0` ne contient **ni** les lignes supprimées **ni** le texte de colonnes sur pages amd, présents sur `main` seulement | **BSD-3-Clause** ; code tiers **MIT** (J. Hobbs) dans le périmètre SAS | `LICENSE` ; `LICENSES/SAS7BDAT_LICENSE` ; `P7:1-4` ; `PX:131-134` | 2026-10-29 |
| I6 | Paquet Python **`sas7bdat`** (Jared Hobbs) | https://bitbucket.org/jaredhobbs/sas7bdat (canonique, git) ; https://pypi.org/project/sas7bdat/ ; `github.com/jaredhobbs/sas7bdat` donne un 404 | `2.3.0` (2026-08-24) après `2.2.3` (2019-07-15) : dépôt relancé après ~7 ans de silence ; sdist PyPI identique au dépôt ; fork `openpharma/sas7bdat` inactif (2020) | **MIT** | `LICENSE` ; `setup.py:20` ; `PKG-INFO` | 2026-10-29 |
| I7 | **PDF de Matthew Shotwell** « SAS7BDAT Database Binary Format » (+ Cummins) | https://cran.r-project.org/web/packages/sas7bdat/vignettes/sas7bdat.pdf (18 p., SHA-256 `41dc2773…`, identique à `inst/doc/sas7bdat.pdf` du tarball CRAN 0.8) | Version 2024-08 ; mise à jour 2013 pour u64, Row Size, RLE | **CC BY-SA 3.0** (PDF) | PDF p.1 ; `vignettes/sas7bdat.rst:19` | 2026-10-29 |
| I8 | Paquet R **`sas7bdat`** (prototype `read.sas7bdat`) | https://cran.r-project.org/package=sas7bdat ; https://github.com/BioStatMatt/sas7bdat (**archivé**) | CRAN `0.8` (2024-08-29) ; dépôt GitHub plus récent que le PDF CRAN (commits 2025-05 et 2026-02/04, vignette passée de 798 à 911 lignes : ajout de « Deleted Row Flags », retrait de « probably not sufficient to write ») | **GPL-2.0-or-later** (`GPL (>= 2)`) | `DESCRIPTION:13` (tarball CRAN et dépôt) ; `R/sas7bdat.R:1-15`. **Aucun fichier `LICENSE`/`COPYING`** ni dans le dépôt ni dans le tarball ; API GitHub `license: null` | 2026-10-29 |
| I9 | `FredHutch/sas7bdat-specification` (fork de la spécification) et `FredHutch/sas7bdat` (écrivain Java) | https://github.com/FredHutch/sas7bdat-specification ; https://github.com/FredHutch/sas7bdat | Spécification : dernier push 2026-09-30 ; écrivain : push 2026-09-17 | Écrivain : **MIT** (`LICENSE` lu en clone). Spécification : **`inconnu`** (pas de licence déclarée côté API GitHub ; fichiers **non lus**) | Écrivain : `LICENSE`. Spécification : API GitHub seulement | 2026-10-29 |

### 1.2 Implémentations Rust, Go, JavaScript et autres langages

| ID | Implémentation | URL | Version / activité | Licence (SPDX) | Où la licence a été lue | Verdict |
|---|---|---|---|---|---|---|
| I10 | Crate **`sas7bdat`** (Rust **pur**, lecture seule) | https://github.com/tkragholm/sas7bdat-parser-rs ; https://crates.io/crates/sas7bdat | `0.9.3` (2026-10-20), 12 versions depuis 2026-01-16, 398 commits, **1 seul mainteneur**, 7 037 téléchargements | **MIT** | `LICENSE` racine ; `crates/sas7bdat/Cargo.toml:6` ; registre crates.io (cohérents). Pas de `LICENSE` dans `crates/sas7bdat/`. Fixtures tierces Apache-2.0/BSD-3 dans `THIRD_PARTY_NOTICES.md` | Retenu (matrice colonne A) |
| I11 | Crate **`polars-readstat-rs`** (Rust **pur**, lecture seule pour SAS) | https://github.com/jrothbaum/polars_readstat ; https://crates.io/crates/polars-readstat-rs | `0.27.0` (2026-10-26), 40 versions, 513 commits, 8 contributeurs ; README du crate : code « nearly completely coded by Claude Code and Codex » | **Divergence** : fichier `LICENSE` et `Cargo.toml` à HEAD (commit `b4cafc8`) = **Apache-2.0** ; **registre crates.io = MIT** pour 0.27.0 et les versions antérieures vérifiées ; GitHub = `NOASSERTION`. Le message du commit `b4cafc8` explique que le fichier est Apache-2.0 depuis le commit initial | `LICENSE`, `crates/polars_readstat_rs/Cargo.toml:7`, API crates.io | Retenu (colonne B) ; à traiter comme **Apache-2.0** |
| I12 | **`readstat-rs`** (workspace `readstat`, `readstat-sys`, `readstat-cli`…, binding FFI de ReadStat) | https://github.com/curtisalexander/readstat-rs | HEAD `792be4a` (2026-09-01) ; versions du workspace `0.30.2` ; **absent de crates.io** : `GET /api/v1/crates/{readstat,readstat-sys,readstat-cli,readstat-iconv-sys}` = 404, index sparse = 404, `docs.rs/readstat` = 404, alors que le README affiche des badges crates.io. Cause `inconnu` | **MIT** | `LICENSE` ; `Cargo.toml:27` ; ReadStat vendorisé (sous-module) sous MIT. Licence de win-iconv `inconnue` (sous-module vide) | Retenu (colonne E) |
| I13 | `saundemanu/sasquatch` (Rust, PyO3) | https://github.com/saundemanu/sasquatch | 1 commit (2026-08-06), « 0.1.0 unreleased », non publié sur crates.io (404), aucun fichier de test | **Divergence** : `LICENSE` = MIT seul ; `Cargo.toml:6` et `pyproject.toml:11` = `"MIT or Apache-2.0"` (expression SPDX non valide) | `LICENSE`, `Cargo.toml:6` | Écarté de la matrice (immature) |
| I14 | `OrganicProgramming/SAS-reader` (Rust, port de `datareader`) | https://github.com/OrganicProgramming/SAS-reader | Dernier commit 2023-01-25, non publié | **`inconnu`** (ni `LICENSE` ni champ `license` ; dérive d'un code BSD-3) | `Cargo.toml`, listing du dépôt | Écarté |
| I15 | `tafia/quick-sas7bdat`, `aaweaver-actuary/rust-sas-reader`, `renatoor/arrow-sas7bdat`, forks vides de `readstat-rs` | GitHub | Abandonnés / squelettes (« nothing is working yet ») | **`inconnu`** (aucune licence) | listings | Écartés |
| I16 | **`@irbisadm/statfmt`** (TypeScript pur, **lecture et écriture** sas7bdat, port de ReadStat) | https://github.com/irbisadm/statfmt ; https://www.npmjs.com/package/@irbisadm/statfmt | `0.1.1` (2026-07-14), 37 commits, 11 915 téléchargements/30 j ; README : fichiers générés « may not open in SAS itself » | **MIT** (avec avis MIT de ReadStat reproduit) | `LICENSE:1-56` ; `package.json:55` ; GitHub = `NOASSERTION` à cause des deux avis | Retenu (colonne C) |
| I17 | **`kshedden/datareader`** (Go **pur**, lecture seule) | https://github.com/kshedden/datareader ; https://pkg.go.dev/github.com/kshedden/datareader | Dernier commit 2021-03-25 : **dormant**, pas de tag ; 10 forks sur pkg.go.dev | **BSD-3-Clause** | `LICENSE.txt` ; pkg.go.dev ; GitHub | Retenu (colonne D) |
| I18 | `dumbmatter/sas7bdat-js` → npm `sas7bdat` (JS pur, lecture seule) | https://github.com/dumbmatter/sas7bdat-js ; https://www.npmjs.com/package/sas7bdat | `0.1.7` (2019-12-03), abandonné | **`Apache-2.0` déclarée seulement dans `package.json:22`/npm ; aucun fichier `LICENSE` dans le dépôt** | `package.json:22`, listing | Écarté de la matrice (statut de licence incertain) |
| I19 | `defineEditor/js-stream-sas7bdat` → npm `js-stream-sas7bdat` (addon Node-API compilant ReadStat) | https://github.com/defineEditor/js-stream-sas7bdat | `0.3.0` (2026-06-29) | **MIT** | `LICENSE`, `package.json:51` | Binding, pas du JS pur |
| I20 | `allensrj/goreadstat` (Go, cgo sur ReadStat) ; `yoogoc/readstat-go` | https://github.com/allensrj/goreadstat | `v0.2.0` (2026-10-17) ; `readstat-go` abandonné (2023) | `goreadstat` : `LICENSE` = licence de pyreadstat (Apache-2.0) + avis ReadStat (MIT), pas un texte propre, GitHub `NOASSERTION`. `readstat-go` : **`inconnu`** (404 sur `/license`) | `LICENSE`, `NOTICE` | Bindings, non retenus |
| I21 | Autres lecteurs notables | `epam/parso` (cf. I4) ; `nbbrd/sasquatch` (Java) ; `richokelly/sas7bdat` (C#) ; `olivia76/cpp-sas7bdat` (C++) ; `tk3369/SASLib.jl` (Julia) ; `saurfang/spark-sas7bdat` (Scala) | poussés entre 2024 et 2026 | `nbbrd/sasquatch` **EUPL-1.2** ; `richokelly/sas7bdat` **Apache-2.0** ; `cpp-sas7bdat` **Apache-2.0** ; `SASLib.jl` **MIT « Expat »** ; `spark-sas7bdat` Apache-2.0 (API GitHub, fichier non relu) | `LICENSE` lus en clone, sauf `spark-sas7bdat` | Mentionnés, non analysés |
| I22 | Extensions DuckDB basées sur ReadStat : `mettekou/duckdb-read-stat`, `KoliStat/the-stats-duck` | GitHub | actives | MIT / Apache-2.0 **d'après l'API GitHub seulement** ; fichiers non relus → **`inconnu`** | API GitHub | Hors périmètre |
| I23 | Paquets npm `readstat-js` (0.1.2, 2023) et `sas7bdat.js` (0.1.6, 2023) | npmjs.com | pas de dépôt, `gypfile: true` | **`inconnu`** (tarballs non téléchargés) | métadonnées npm | Écartés |
| I24 | Crates XPORT (hors sas7bdat) : `sas_xport` (MIT), `xportrs`, `xpttools` (GPL-3.0), `xpt-r` | crates.io | — | selon registre | registre | Hors périmètre (format de transport) |

### 1.3 Absences constatées et requêtes effectuées

- **Aucune implémentation Rust pure qui écrive du `.sas7bdat`** : l'écriture n'existe que dans ReadStat et ses portages (`statfmt` en TypeScript, `goreadstat` en cgo, `FredHutch/sas7bdat` en Java). **Aucun écrivain n'écrit la compression BINARY (RDC).** Aucun écrivain n'a démontré de relecture par SAS (aveux explicites : README de ReadStat, README de `statfmt`).
- **Crate `readstat` sur crates.io : introuvable** (voir I12). **Dépôt `jaredhobbs/sas7bdat` sur GitHub : introuvable** (le dépôt est sur Bitbucket). **`WizardMike/ReadStat` : introuvable** (dépôt canonique `WizardMac`).
- Requêtes : crates.io API `?q=` avec `sas7bdat`, `sas`, `readstat`, `sas+xport`, `sas7bcat`, `sas7`, `parso`, `sas+dataset`, `arrow+sas`, `datafusion+sas`, `polars+sas`, `sas-reader`, `sas_reader`, et accès directs aux fiches `readstat*`, `sas7bdat-rs`, `sasquatch`, `polars-readstat` ; GitHub `gh search repos` avec `sas7bdat` (toutes langues, 100 premiers résultats) et `sas7bdat`/`readstat` combinés à `language:Rust|Go|TypeScript|JavaScript|WebAssembly|Zig`, `sas xpt language:Go` ; `gh search code` sur `sas7bdat` et `polars-readstat-rs` ; pkg.go.dev `?q=sas7bdat`, `?q=readstat`, `?q=sas+dataset` ; registre npm `-/v1/search?text=` avec `sas7bdat`, `readstat`, `sas7bdat parser`, `sas-reader`, `parso`, `sas dataset` ; PyPI et Bitbucket pour `sas7bdat` ; CRAN pour `sas7bdat` ; Maven Central pour `com.epam:parso`.
- Non couverts : le fork `openpharma/sas7bdat`, `FredHutch/sas7bdat-specification`, `byteswap.pyx` de pandas, `pandas/core` (recherche d'un éventuel `to_sas`), le petit lecteur interne de la crate `sniff-rs`.

### 1.4 Documentation SAS officielle publique

Consultée via les PDF officiels de documentation.sas.com / support.sas.com (les pages HTML de documentation.sas.com répondent par un défi anti-robot : URL HTML exacte par sujet `inconnu`).

| ID | Document (version documentée) | Date affichée | URL |
|---|---|---|---|
| S1 | SAS 9.4 Data Set Options: Reference, Fourth Edition (`COMPRESS=`, `ENCODING=`, `OUTREP=`, `REUSE=`, `ENCRYPT=`, `INDEX=`, `OBS=`) | 2026-04-09 | https://go.documentation.sas.com/api/collections/pgmsascdc/9.4_3.5/docsets/ledsoptsref/content/ledsoptsref.pdf?locale=en |
| S2 | SAS 9.4 System Options: Reference, Fifth Edition | 2026-09-15 | https://go.documentation.sas.com/api/collections/pgmsascdc/9.4_3.5/docsets/lesysoptsref/content/lesysoptsref.pdf?locale=en |
| S3 | SAS 9.4 Language Reference: Concepts, Sixth Edition (valeurs manquantes, compression, CEDA, catalogues, limites) | non affichée | https://go.documentation.sas.com/api/collections/pgmsascdc/9.4_3.4/docsets/lrcon/content/lrcon.pdf?locale=en |
| S4 | SAS 9.4 V9 LIBNAME Engine: Reference (extensions, compression, catalogues) ; S4v : même ouvrage Viya (2026-09-03) | 2025-03-21 | https://documentation.sas.com/api/docsets/engsas7bdat/9.4/content/engsas7bdat.pdf?locale=en |
| S5 | SAS 9.4 Programmer's Guide: Essentials (CEDA, table de compatibilité) | 2026-06-26 | https://documentation.sas.com/api/docsets/lepg/9.4/content/lepg.pdf?locale=en |
| S6 | Base SAS 9.4 Procedures Guide, Seventh Edition (`PROC CONTENTS`, `PROC DATASETS`, `PROC FORMAT`, alignement et NPOS) | 2026-10-08 | https://go.documentation.sas.com/api/collections/pgmsascdc/9.4_3.5/docsets/proc/content/proc.pdf?locale=en |
| S7 | SAS 9.4 Formats and Informats: Reference | 2025-12-09 | https://go.documentation.sas.com/api/collections/pgmsascdc/9.4_3.5/docsets/leforinforref/content/leforinforref.pdf?locale=en |
| S8 | SAS 9.4 National Language Support (NLS): Reference Guide, Fifth Edition (encodages, `LOCALE`, tables de noms) | 2026-09-15 | https://go.documentation.sas.com/api/collections/pgmsascdc/9.4_3.5/docsets/nlsref/content/nlsref.pdf?locale=en |
| S9 | Moving and Accessing SAS 9.4 Files, Third Edition (XPORT en annexes) | 2025-05-09 | https://go.documentation.sas.com/api/collections/pgmsascdc/9.4_3.5/docsets/movefile/content/movefile.pdf?locale=en |
| S10, S11 | DATA Step Statements: Reference ; Data Set Options (Viya) | 2026-06-15 ; 2024-05-02 | `…/lestmtsref/…`, `…/v_041/docsets/ledsoptsref/…` |
| S12, S13 | Papiers techniques « Record Layout… SAS Version 8 or 9 / Version 5 or 6 Data Set in SAS Transport Format » (XPORT, ancien TS-140) | oct. 2021 | https://support.sas.com/content/dam/SAS/support/en/technical-papers/record-layout-of-a-sas-version-8-or-9-data-set-in-sas-transport-format.pdf ; https://support.sas.com/techsup/technote/ts140.pdf |
| S14–S16 | SAS Notes 22490 (taille maximale de fichier sous UNIX), 4720 (CPORT et compression), 1722 (z/OS) | 2021-04-12 ; — ; 2023-10-26 | http://support.sas.com/kb/22/490.html ; https://support.sas.com/kb/4/720.html ; http://support.sas.com/kb/1/722.html |
| S17 | Terms of Use de sas.com | révisée 2018-10-18 | https://www.sas.com/en_us/legal/copyright.html |
| T1, T2 | Papiers SGF tiers hébergés par SAS : 258-2013 (compression), 241-2011 (longueur numérique) — non normatifs | 2013 ; 2011 | https://support.sas.com/resources/papers/proceedings13/258-2013.pdf ; http://support.sas.com/resources/papers/proceedings11/241-2011.pdf |

Licence et conditions : droits réservés SAS Institute ; les pages de titre des PDF interdisent la reproduction sans accord ; la page S17 (Terms of Use) est discutée en section 5. **Inaccessibles** : pages HTML de documentation.sas.com, `communities.sas.com` (403), `…/terms-of-use.html` (404), SAS Companions (non téléchargés).

Constat principal : **la structure binaire de `.sas7bdat` n'est pas publiée par SAS** (ni en-tête, ni pages, ni marqueurs de compression, ni `.sas7bndx`/`.sas7bcat`/chiffrement). Sont publiés : le format de transport XPORT V5/V6 et V8/V9 (S12, S13, S9 ; présenté comme non propriétaire), les noms des algorithmes de compression (RLE, RDC), les extensions de fichier, les règles d'alignement des variables dans une observation, les valeurs de « data representation » et les groupes de compatibilité CEDA, les tables de noms d'encodages, les limites de noms et de longueurs. Cette absence est ce qui justifie la suite de la série.

---

## 2. Matrice des capacités

18 capacités × 11 implémentations, présentées en trois tableaux (2a, 2b, 2c) pour rester lisibles ; chaque case porte sa valeur et sa source. Toutes les valeurs sont des lectures statiques de code ou de documentation, soit `inféré` au mieux.

### 2a. ReadStat, Parso, pandas

| # | Capacité | ReadStat (HEAD `1434a07`) | Parso 2.0.14 | pandas `main` |
|---|---|---|---|---|
| 1 | Lecture | oui — `rd.c:1481` | oui — `SFP:602-702` | oui — `P7:1010-1088` |
| 2 | Écriture | oui, mais « non vérifiée contre un SAS réel » (README) — `wr.c:1520`, `README.md:22-27` | non — aucune classe d'écriture, `CSVDataWriter` écrit du CSV (`README.md:155-161`) | non — que des lecteurs (`P7:266`) ; `pandas/core` non consulté |
| 3 | 32 vs 64 bits | oui, les deux, lecture et écriture (défaut 64) — `sas.c:187-192`, `sas.c:534-552` | oui — `SFP:326-329`, `SFC:485-491` | oui — `P7:487-499`, `PC:50-57` |
| 4 | Little vs big endian | lecture : oui ; écriture : partiel (endianness de l'hôte seulement) — `sas.c:193-203`, `wr.c:1218` | oui — `SFP:347-350, 426-429` | oui — `P7:506-513` ; tests BE `test_sas7bdat.py:424-425` |
| 5 | Compression CHAR (RLE) — lecture | oui — `rd.c:662-685`, `rle.c:46-137` | oui — `CharDecompressor.java:58-142` | oui — `PX:135-212` |
| 6 | CHAR — écriture | oui, RLE par ligne, repli sur ligne brute sans gain — `wr.c:1459-1496`, `rle.c:243-286` | non | non |
| 7 | Compression BINARY (RDC) — lecture | oui (détection par le texte `SASYZCR2` du Row Size) ; pas de fichier de test RDC identifié dans `resources/` — `rd.c:575-660, 289` | oui — `BinDecompressor.java:59-122` | oui — `PX:215-291` |
| 8 | BINARY — écriture | non — `wr.c:1513-1515` (`ERROR_UNSUPPORTED_COMPRESSION`) | non | non |
| 9 | Lignes supprimées | lecture : oui (ignorées) ; écriture : toujours 0 — `rd.c:1072-1091`, `wr.c:828` | oui — pages 128/384/640 `SFC:561-592`, bitmap `SFP:804-833` | partiel — oui sur `main` (`PX:745-785`), **absent de la release 3.1.0** |
| 10 | Manquants spéciaux (`.A`–`.Z`, `._`) | oui, lecture et écriture — `rd.c:482-485`, `sas.c:639-658`, `wr.c:1360-1401` | non — tout NaN devient `null` — `SFP:1004-1009` | non — NaN IEEE brut — `PX:966-980` |
| 11 | Labels | oui (colonne, table, nom de table) — `rd.c:419-420, 274-280` | oui — `SFP:1690-1710` | partiel — label de colonne `P7:908-931` ; label de fichier non lu |
| 12 | Formats / informats | oui (nom + largeur + décimales) ; informat = copie du format, « TODO where is the informat saved? » — `rd.c:391-424, 743-780` | partiel — formats oui (`SFP:1682-1707`), informats non lus | partiel — nom du format seul — `P7:915-918` |
| 13 | Encodages | table de ~70 codes → iconv, lecture ; écriture UTF-8 seulement (code 20) — `sas.c:42-129`, `wr.c:1220` | oui, **111** codes → charsets Java ; défaut US-ASCII si absent — `SFC:172-286`, `SFP:174` | oui, **65** codes — `PC:136-219` |
| 14 | Catalogues `.sas7bcat` | oui, lecture et écriture (code présent, contredit le README) — `bcat.c`, `src/sas/readstat_sas7bcat_write.c:267` | non (grep négatif dans `src/main`) | non (grep négatif) |
| 15 | Index `.sas7bndx` | non — grep négatif sur tout le dépôt | non — grep négatif | non — grep négatif |
| 16 | Fichiers chiffrés | non — pas de détection ; le lecteur ne lit pas les champs de mot de passe (`sas.c:169-378`) | non — grep négatif | non — grep négatif |
| 17 | Tailles > 2 Go | partiel — E/S 64 bits OK ; nombre de lignes en `uint32_t` (`rd.c:271`), pages ≤ 16 MiB (`sas.c:276-279`) ; non testé | partiel — page ≤ 10 000 000 octets (`SFP:78`), indice de ligne `int` (`SFP:205`) ; non testé | partiel — indice de ligne `int` C (`PX:571-572`) ; non testé |
| 18 | Streaming | oui — callbacks, `row_limit`/`row_offset` ; source seekable obligatoire (2 passes) ; écriture compressée gardée en mémoire — `rd.c:1570-1588`, `wr.c:1457-1458` | oui — `readNext()` sur `InputStream` (`README.md:132-136`) | oui — `chunksize`/`iterator` (`sasreader.py:131-134`) |

### 2b. `sas7bdat` Python, PDF de Shotwell + paquet R, documentation SAS officielle

Pour la colonne « Doc SAS » : `oui` = explicitement documenté ; `partiel` = la sémantique est documentée mais pas la représentation binaire.

| # | Capacité | `sas7bdat` Python 2.3.0 | PDF Shotwell (p.N) / paquet R 0.8 (`R:`) | Documentation SAS |
|---|---|---|---|---|
| 1 | Lecture | oui — `PY:556-656` | PDF : oui (p.2). R : partiel, non compressé et little-endian — `R:298-564` | partiel — lecture par SAS (moteur V9, CEDA) documentée, aucune spec binaire — S4 p.3-6, S3 p.821-828 |
| 2 | Écriture | non — `convert_file` écrit du CSV (`PY:732-768`) | PDF : non (p.2 « probably not sufficient to write »). R : non (`R:298`). Le dépôt GitHub (HEAD 2026) retire la phrase mais n'ajoute pas de code | partiel — écriture par SAS documentée ; aucune spec hors SAS — S1 p.61-64, S3 p.824 |
| 3 | 32 vs 64 bits | oui — `PY:1363-1368` | PDF : oui (p.3-4, p.16). R : oui — `R:318-337` | partiel — valeurs `WINDOWS_32/64`, `LINUX_32/X86_64`… ; octet indicateur non publié — S1 p.62-63 |
| 4 | Little vs big endian | oui — `PY:1388-1390`, test u64 BE `tests/test_sas7bdat.py:239-244` | PDF : oui (p.2-4, 16). R : non — `stop("big endian…")` `R:339-345` | partiel — notion générale ; pas de table « OUTREP → endianness » ; indicateur d'en-tête non publié — S1 p.62, S12 p.11-12 |
| 5 | CHAR (RLE) — lecture | oui — `PY:93-226` | PDF : oui, sous-ensemble de commandes (p.13-15). R : non — `R:488-494` | partiel — RLE nommé, 2 ou 3 octets par suite ; format des enregistrements non publié — S1 p.16, S3 p.772-774 |
| 6 | CHAR — écriture | non | PDF : non (p.2). R : non | partiel — SAS écrit `COMPRESS=CHAR` ; surcoût 12/24 octets par observation — S3 p.773-774, S4 p.162 |
| 7 | BINARY (RDC) — lecture | oui — `PY:229-306` | PDF : **non** (p.14 « need more test files », p.18 ToDo). R : non | partiel — RDC nommé (RLE + fenêtre glissante) ; paramètres et format non publiés — S1 p.16, S4 p.161-163 |
| 8 | BINARY — écriture | non | non | partiel — SAS écrit `COMPRESS=BINARY` ; rien sur le format — S4 p.161-163 |
| 9 | Lignes supprimées | non — le masque `0xFF00` écarte le flag 0x80 (`PY:1325-1330`) ; les lignes supprimées sont probablement rendues comme valides (déduit) | PDF CRAN : non (p.8, champs « ???? »). Dépôt HEAD : oui (`vignettes/sas7bdat.rst:417-418, 825-828`). R : non | partiel — notion, compteur « Deleted Observations », `REUSE=` ; marqueur au niveau octet non publié — S6 p.678, S3 p.711-712 |
| 10 | Manquants spéciaux | non — `math.isnan` → `None` (`PY:541-542`) | PDF : partiel (p.18, un seul motif de NaN noté). R : non (`R:529`) | partiel — 27 manquants, ordre de tri documentés ; codage XPORT documenté (S12 p.7) ; codage dans `.sas7bdat` non publié — S3 p.105-108 |
| 11 | Labels | oui — `PY:1098-1180` | PDF : oui (p.12-13). R : oui — `R:203-231` | partiel — 256 caractères max ; stockage non publié — S3 Table 3.1/4.1 |
| 12 | Formats / informats | partiel — chaîne de format ; pas d'informat — `PY:1098-1180` | PDF : partiel (p.13, 16). R : partiel — `R:214-219` | partiel — nom, largeur, décimales, justification par variable ; stockage non publié — S6 p.685-688, S7 |
| 13 | Encodages | oui, **65** codes (ensemble identique à pandas) — `PY:355-373` | PDF : partiel, table de 23 codes (p.5-6), ToDo p.18. R : paramètre `encoding` seulement (`R:298`) | partiel — noms d'encodages, alias, `LOCALE`, CEDA ; **aucun code numérique** d'en-tête publié — S8 p.941-956, 1065-1068 |
| 14 | Catalogues `.sas7bcat` | non | non — ni PDF ni R | partiel — entrées `FORMAT/FORMATC/INFMT/INFMTC`, `CNTLOUT` ; structure binaire non publiée — S3 p.803-806, S6 p.1082 |
| 15 | Index `.sas7bndx` | non | non | partiel — extension, index simple/composite ; structure non publiée — S4v Table 1.2, S1 p.42-43 |
| 16 | Fichiers chiffrés | non | PDF : non (p.18 ToDo). Dépôt HEAD : hypothèse non confirmée (`rst:131`) | partiel — `ENCRYPT=YES|AES|AES2` ; mode, KDF, IV non publiés — S1 p.22-25 |
| 17 | Tailles > 2 Go | oui, en théorie (aucun plafond dans le code) ; non testé — `PY:658-677` | PDF : inconnu (p.18). R : non, fichier entier en mémoire — `R:388-397` | partiel — observations jusqu'à 2^63−1 ; taille de fichier bornée par l'OS, pas par le format selon la doc lue — S3 p.708, S14 |
| 18 | Streaming | oui — générateur `readlines` (`PY:556`) ; `to_data_frame` charge tout | PDF : non applicable (p.17). R : non | inconnu — aucune page lue ne traite d'un `.sas7bdat` en flux — S4v Table 1.1 |

### 2c. Implémentations Rust, TypeScript et Go, binding `readstat-rs`

| # | Capacité | A. `sas7bdat` (Rust) | B. `polars-readstat-rs` (Rust) | C. `statfmt` (TS) | D. `datareader` (Go) | E. `readstat-rs` (binding) |
|---|---|---|---|---|---|---|
| 1 | Lecture | oui — `A:src/dataset.rs:140` | oui — `B:README.md:36` | oui — `C:src/highlevel.ts:391` | oui — `D:sas7bdat.go:512, 638` | oui — `E:crates/readstat/README.md:3` |
| 2 | Écriture | non — `A:Cargo.toml:5` | non — `README.md:103` (racine du dépôt), `B:src/sas/writer.rs:32` (produit CSV + script `.sas`) | oui — `C:src/sas/sas7bdat-write.ts:460` ; « may not open in SAS » (`C:README.md:224-228`) | non — `D:README.md:7-10` | partiel — API sûre : CSV/Parquet/Feather/NDJSON ; sas7bdat via FFI brute seulement — `E:crates/readstat/README.md:5, 36-44` |
| 3 | 32 vs 64 bits | oui — `A:src/probe.rs:58` | oui — `B:src/sas/header.rs:19-24` | oui — `C:src/sas/sas-header.ts:134-137` | oui — `D:sas7bdat.go:897-921` | oui — `E-RS:readstat_sas.c:178-182` |
| 4 | Little vs big endian | oui — `A:src/probe.rs:53-55`, `A:src/internal.rs:208-247` | oui — `B:src/sas/header.rs:26-31` | partiel — lecture LE/BE, écriture = hôte, aucun test BE — `C:src/sas/sas-header.ts:140-141, 245` | oui — `D:sas7bdat.go:923-932` | oui — `E-RS:readstat_sas7bdat_read.c:1334` |
| 5 | CHAR (RLE) — lecture | oui — `A:src/compression.rs:23-26, 179` | oui — `B:src/sas/decompressor/rle.rs` | oui — `C:src/sas/sas-rle.ts` | oui — `D:sas7bdat.go:339, 503-504` | oui — `E-RS:readstat_sas7bdat_read.c:650` |
| 6 | CHAR — écriture | non | non | oui — `C:src/sas/sas7bdat-write.ts:137-138, 413` | non | partiel — dans ReadStat (`E-RS:readstat_sas7bdat_write.c:81, 197`), non exposé par l'API sûre |
| 7 | BINARY (RDC) — lecture | oui — `A:src/compression.rs:27-30, 257` | oui — `B:src/sas/decompressor/rdc.rs` (3 tests) | oui, sans test — `C:src/sas/sas7bdat-read.ts:223, 416` | oui — `D:sas7bdat.go:429, 505-506` | oui — `E-RS:readstat_sas7bdat_read.c:280, 554` |
| 8 | BINARY — écriture | non | non | non — seule la signature RLE est écrite (`C:src/sas/sas7bdat-write.ts:21, 138`) | non | non — `E-RS:readstat_sas7bdat_write.c:81, 197` |
| 9 | Lignes supprimées | oui — `A:src/layout.rs:686-698`, `A:tests/deleted_rows.rs:1-9` | non — 0 occurrence dans `B:src/sas/*.rs` | non — 0 occurrence dans `C:src/sas/` | non — 0 occurrence dans `D:sas7bdat.go` | oui — `E-RS:readstat_sas7bdat_read.c:249-263, 530-539` |
| 10 | Manquants spéciaux | partiel — bits conservés mais l'API publique expose `Null` sans tag — `A:src/scan/numeric.rs:161-165`, `A:src/row.rs:4-14` | oui — `B:src/sas/value.rs:62-96` | oui — `C:src/sas/sas7bdat-read.ts:336-345`, `sas7bdat-write.ts:377-380` | non — `D:sas7bdat.go:700-702` | partiel — tag lu par ReadStat, écrasé en `null` par le wrapper — `E:crates/readstat/src/cb.rs:707-716` |
| 11 | Labels | oui — `A:src/layout.rs:585-618`, `A:src/labels.rs:1-20` | oui — `B:src/sas/metadata.rs:655-690`, `catalog.rs:345-350` | partiel — variables oui, valeurs via `.sas7bcat` — `C:src/sas/sas7bdat-read.ts:275` | partiel — colonnes seulement — `D:sas7bdat.go:1469-1488` | partiel — variables oui ; catalogues rejetés — `E:crates/readstat/src/rs_path.rs:13` |
| 12 | Formats / informats | partiel — formats oui, informats non (0 occurrence) — `A:src/metadata.rs:162-168` | partiel — formats oui, informats non — `B:src/sas/metadata.rs:669-672` | partiel — formats oui, informats non — `C:src/sas/sas7bdat-write.ts:206-212` | partiel — formats oui, informats non — `D:sas7bdat.go:1479-1483` | partiel — formats oui, informat = copie du format — `E-RS:readstat_sas7bdat_read.c:754-755` |
| 13 | Encodages | oui — ~70 codes, erreur si inconnu — `A:src/probe.rs:375-448, 92-96` | oui — repli **silencieux** vers WINDOWS-1252 — `B:src/sas/encoding.rs:4-83, 148` | oui — erreur si inconnu — `C:src/sas/sas-header.ts:57, 146-149` | partiel — 7 codes nommés, aucun décodage par défaut — `D:sas7bdat.go:314-316, 527-529` | oui — `E-RS:readstat_sas.c:45` |
| 14 | Catalogues `.sas7bcat` | oui — `A:src/catalog.rs:38, 49` | oui — `B:src/sas/catalog.rs:345-350` | oui, lecture et écriture — `C:src/sas/sas7bcat.ts` | non — 0 occurrence | non côté API sûre (`E:crates/readstat/src/rs_path.rs:13`) ; oui dans ReadStat sous-jacent |
| 15 | Index `.sas7bndx` | non — 0 occurrence | non — 0 occurrence | non — 0 occurrence | non — 0 occurrence | non — 0 occurrence |
| 16 | Fichiers chiffrés | non — 0 occurrence ; comportement non testé | non — 0 occurrence (`encrypt|password|rc4|aes`) | non — 0 occurrence | non — 0 occurrence | non — 0 occurrence |
| 17 | Tailles > 2 Go | inconnu — mmap et compteurs u64 favorables (`A:src/dataset.rs:69-75`, `A:src/probe.rs:166-180`), aucun test | oui, selon l'auteur, non vérifié (fichier de 500 Go) — `B:README.md:111` (racine) | non — entrée entière en mémoire — `C:src/highlevel.ts:391` | inconnu — `Seek(int64)` (`D:sas7bdat.go:542`), aucun test | inconnu — `E-RS:readstat_sas7bdat_read.c:37`, aucun test |
| 18 | Streaming | oui — `A:src/scan/builder.rs:521, 630` ; `from_reader` met tout en mémoire | oui — `B:src/readstat_stream.rs:54-100` | partiel — callbacks mais entrée entière en mémoire — `C:src/parser.ts:51` | oui — `D:sas7bdat.go:638` | oui — `E:crates/readstat/README.md:36` |

Remarques de maturité : `A` n'a qu'un mainteneur et son grand corpus n'est pas versionné ; `B` revendique un code largement produit par assistants IA et un changement de licence en cours ; `C` ne teste que des fichiers générés par ReadStat (aucun fichier SAS natif) ; `D` est figé depuis 2021 mais testé sur 21 fichiers produits par SAS.

---

## 3. Points de désaccord

Toutes les entrées ci-dessous sont `inféré` au mieux : aucun arbitrage n'est possible sans fichiers produits par SAS et sans exécution (renvoyé aux changements 02 à 05).

### 3.1 Interprétations divergentes entre implémentations

| # | Sujet | Interprétations et sources |
|---|---|---|
| D1 | **Code d'encodage 40** | PDF = latin5 (ISO-8859-9) (p.5) ; Parso `0x28` = ISO-8859-15 (`SFC:233`) ; pandas = latin9 (`PC:153`) ; `sas7bdat` Python = iso-8859-15 (`PY:359`). Les trois implémentations s'accordent sur 37 = latin5 : le PDF est isolé |
| D2 | **Pages de code OEM 41 à 51 et 58** | Parso : 43 = IBM437, 44 = IBM850, 45 = IBM852, 46 = IBM00858, 47 = IBM862, 51 = IBM866, 58 = IBM857 (`SFC:264`…). pandas et `sas7bdat` Python : 43 = cp852, 44 = cp857, 45 = cp858, 46 = cp862, 47 = cp864, 51 = cp874, 58 = cp860 (`PC:156`…). Décalage de 2 entre les deux familles ; aucun des deux n'est dans le PDF. Autres écarts : 136 = cp932 (pandas) contre x-windows-iso2022jp (Parso) ; 248 = shift_jisx0213 contre JIS_X0201 ; 39 = ISO-8859-11 (PDF, Parso) contre cp874 (pandas, `sas7bdat` Python) ; 119 (EUC-TW) présent dans PDF et Parso, « not found » chez pandas et `sas7bdat` Python (`PC:190`) |
| D3 | **Code d'encodage 0 et 204** | PDF : encodage de session, généralement Windows-1252 (p.6) ; pandas, `sas7bdat` Python, ReadStat : cp1252 (`PC:139`, `PY:356`, `sas.h:141`) ; **Parso : US-ASCII** (code absent de sa table, `SFP:174, 352-358`). Même écart pour 204 (« any ») et 227 (latin8) |
| D4 | **Octets d'encodage** | Parso, pandas, `sas7bdat` Python et ReadStat ne lisent que l'octet 70 ; le PDF dit « un ou deux octets » (p.5) ; la vignette du dépôt (HEAD) cite les octets 47, 70 et 71 (`rst:207`) ; le writer de ReadStat écrit la même valeur aux octets 47, 70 et 71 (`sas.c:431, 447-448`) |
| D5 | **Commande RLE `0x40`** | Parso : `nibble×16 + octet + 18` (`CharDecompressor.java:80-86`) ; pandas et `sas7bdat` Python 2.3.0 : `nibble×256 + octet + 18` (`PX:154-160`, `PY:140-148`). Le changelog de `sas7bdat` Python 2.3.0 corrige le facteur 16 vers 256. PDF : « non documenté » |
| D6 | **Commandes RLE `0x10`, `0x20`, `0x30`, `0x50`** | PDF : commandes 1 à 5 « not observed » (p.14). pandas : `ValueError` (`PX:209-210`). `sas7bdat` Python 2.3.0 : implémente `0x10`, `0x20`, `0x50` (`PY:120-139`). Parso : `0x10`-`0x30` traités comme `0x00` avec facteur `×256`, `0x50` = insertion de `@` (`CharDecompressor.java:67-92`). ReadStat : commande `3` et codes non définis non rejetés (`rle.c:108-110`, « error out here? ») |
| D7 | **Longueur des commandes RLE `0`, `6`, `7`** | PDF : « L = 1 octet, 0-255 » (p.14) ; Parso, pandas, `sas7bdat` Python : `(nibble bas × 256) + octet suivant + constante` |
| D8 | **Octets 352\|680 et 354\|682 du Row Size** | PDF p.9 : « 12,32\|0 » (inconnu) et longueur du Creator Software (LCS) ; pandas et `sas7bdat` Python : LCS (`P7:690-695`, `PY:863-864`) ; **Parso** : offset et longueur du libellé de fichier (`SFP:1444-1445, 1466-1467`). Mêmes octets, deux sémantiques |
| D9 | **Octets 364\|366 (692\|694 en 64 bits)** | PDF : zéros ; Parso : offset et longueur du littéral de compression (`SFP:1447-1448, 1473-1474`) ; pandas et `sas7bdat` Python : non lus |
| D10 | **Détection de la compression** | PDF : littéral aux 8 premiers octets du premier Column Text (p.11) ; pandas et `sas7bdat` Python : sous-chaîne dans tout le premier bloc de texte (`P7:752-757`, `PY:961-968`) ; Parso : couple (offset, longueur) du Row Size (`SFP:1444-1453`) ; ReadStat : texte du Row Size (`rd.c:282-290`) |
| D11 | **Nombre de lignes supprimées** | Parso le lit à `8 × int` (32\|64) (`SFC:646`) ; la vignette du dépôt (HEAD) le place à 28\|56 et dit que 32\|64 est à zéro (`rst:417-418`) ; le PDF CRAN n'en parle pas ; ReadStat lit un compteur d'un autre emplacement (`rd.c:267-269`) |
| D12 | **Types de pages et suppression** | PDF : meta 0, data 256, mix 512, amd 1024, meta 16384, comp −28672. Parso (comparaison exacte, sans masque) : ajoute 128, 384, 640 (« Probably… » `SFC:585-586`). pandas : masque `0xFF00`, flag `0x0080` = lignes supprimées. `sas7bdat` Python : masque `0xFF00`, mix `[0x0200, 0x0280]`. ReadStat : `AMD` (0x0400) et `META2` (0x4000) définis mais jamais utilisés (`sas.h:112, 116`) ; pages AMD repérées par position (`rd.c:1383-1447`) |
| D13 | **Bitmap de lignes supprimées** | Parso et pandas : entier de 4 octets à l'offset de page 24 (64 bits) ou 12 (32 bits), puis bitmap après les lignes, bit de poids fort en premier (`SFP:804-833`, `PX:745-772`) ; la vignette du dépôt décrit ce champ comme « number of unused bytes on page » (`rst:310`) ; ReadStat lit un bitmap à `rd.c:1072-1091` |
| D14 | **Alignement des pages « mix »** | PDF : `DL = (B+8+SC*SL+7) % 8 * 8` (p.6-7) ; Parso, pandas, `sas7bdat` Python : `align = (B+8+SC*SL) % 8` ajouté ; la vignette (HEAD) a changé la formule (`rst:316, 339`) ; ReadStat : saute 4 octets de rembourrage selon une heuristique « Stat/Transfer » (`rd.c:1285-1297`) |
| D15 | **Largeur numérique ≤ 2** | PDF : 3 à 8 (p.15) ; Parso et `sas7bdat` Python : 1-2 octets lus comme entier court signé (`SFP:892-893`, `PY:700-703`) ; pandas : double tronqué (`P7:415-420`) ; ReadStat : 3 à 8 (`rd.c:731-734`) |
| D16 | **Valeurs manquantes spéciales** | PDF : un seul motif de NaN noté (`0000000000D1FFFF`, p.18) ; ReadStat : tag = complément de l'octet 5 du NaN (`rd.c:482-485`), décodage `.`/`_`/`A`-`Z` (`sas.c:639-658`) ; Parso : tout NaN ou double positif < 1E-300 devient `null` (`SFP:1004-1009`) ; pandas, `sas7bdat` Python : tag perdu ; `sas7bdat` Rust : tag conservé en interne mais non exposé |
| D17 | **Rôle du flag de nom (2 octets avant le type d'une colonne)** | PDF p.12 : « name length flag » 4/1024/2048/2560 ; ReadStat écrit `0x0400`, `0x0800`, `0x0C00` selon la longueur et la protection du nom (`wr.c:633-640`) mais **ignore** ces octets en lecture (`rd.c:343-389`) |
| D18 | **Format du nom de format** | ReadStat HEAD : nom, largeur, chiffres stockés séparément (commit `1434a07`, 2026-10-27) ; les copies de `haven` et `pyreadstat` sont antérieures et se comportent différemment (mode « legacy » si le nom contient déjà un point) — `rd.c:743-780` |
| D19 | **Lignes supprimées dans les oracles** | ReadStat 1.1.9 (donc `haven`, `pyreadstat`) les renvoie comme des données ; la crate `sas7bdat` et ReadStat « dev » (≥ PR #366) les excluent (`A:tests/deleted_rows.rs:1-9`). Deux oracles courants divergent donc sur ce point |
| D20 | **Dates** | Le PDF ne cite que DATE/TIME/DATETIME (p.16) ; Parso et pandas gèrent l'anomalie « années 4000/8000 non bissextiles » de SAS, pas `sas7bdat` Python. La documentation SAS donne deux plages de dates incompatibles : jusqu'à l'an 4000 (S7 p.9-10) et l'an 19 900 (S3 p.138) |
| D21 | **Statut de l'écrivain ReadStat** | Trois affirmations coexistent : README « non vérifié contre un SAS » ; commentaire du writer : la spécification FredHutch serait « verified against SAS » (`wr.c:19`) ; `NEWS.md:8` de `haven` : « SAS can read the files it produces » après correction du checksum d'en-tête. Aucune preuve de test sur un SAS réel dans le dépôt |
| D22 | **Incohérences internes de ReadStat (lecture statique)** | Le writer peut produire des pages > 16 MiB (`wr.c:1170`) que le lecteur rejette (`sas.c:276`) ; en big-endian 64 bits, la disposition des signatures écrites ne correspond probablement pas à celle attendue par le lecteur (`wr.c:142-152` contre `rd.c:946-950`) ; le README dit « pas d'écriture des métadonnées de formats » alors que `readstat_sas7bcat_write.c` existe |
| D23 | **Documentation du PDF contre le code R** | Le PDF (p.17) dit que les fichiers non Windows sont rejetés « for now » ; le code R n'écarte que le big-endian (`R:339-345`) et accepte Unix (`R:347-354`). `VERSION <- "0.5"` dans `R/sas7bdat.R:119` contre `0.8` dans `DESCRIPTION` |

### 3.2 Constantes « magiques » écrites sans explication par ReadStat

Source : chemin d'écriture de ReadStat (HEAD `1434a07`). Les commentaires généraux `wr.c:16-20` (« Fields whose purpose is unknown are set to the constant values that SAS writes ») et `sas.c:402-405` (« Bytes with no known meaning are set to the constant values observed in every file written by SAS 9 ») ne justifient pas chaque valeur. La colonne « Explication » indique l'existence d'une explication **propre à la constante**.

| Champ | Valeur écrite | Emplacement | Explication |
|---|---|---|---|
| Octets 33, 34 d'en-tête | `0x22`, `0x00` | `sas.c:422-423` | non |
| Octet 36 | `0x33` (64 bits) / `0x22` (32 bits) | `sas.c:425` | non |
| Octet 38 | `0x02` | `sas.c:427` | non |
| Octet 39 `file_format` | `'1'` (Unix) | `sas.c:428` | non |
| Octets 40, 64 | `0x01` (64 bits) / `0x04` (32 bits) | `sas.c:429, 441` | non |
| Octets 50-51, 74-75 | `0x03 0x01` | `sas.c:434-435, 451-452` | non |
| Octets 52-55 | `0x18 0x1F 0x10 0x11` | `sas.c:436-439` | non (identiques aux 4 derniers octets du magic number, `sas.c:21-26`) |
| Octets 65-68 | `0x33/0x32`, `0x01`, `0x23/0x22`, `0x33/0x22` | `sas.c:442-445` | non |
| Octet 73 | `0x20` (64 bits) / `0x10` (32 bits) | `sas.c:450` | non |
| Octet 35 `a1` | `0x33` (64 bits) / `0x32` (32 bits) | `sas.c:424`, `wr.c:1217` | oui pour `0x32` (`wr.c:1216`) |
| Hôte, version OS, nom OS | `"Linux"`, `"3.10.0-1160.11.1"`, `"x86_64"` en dur, quel que soit l'OS réel | `sas.c:498-501` | libellés en ligne seulement |
| Release | `"9.0401M5"` (version 9) | `sas.c:491-497` | non |
| Checksum de mot de passe | 4 octets de `creation_time`, puis 3 × (même valeur XOR `"LNoe"`) | `sas.c:503-517` | oui (commentaire `sas.c:503-508`), mais les numéros « 288-303 » ne valent qu'en 32 bits |
| Masque de numéro de page | `0xFEDCBA98` ; numéro de page = masque XOR (page + 1) | `wr.c:31, 967`, `sas.c:521-522` | partiel |
| Taille d'en-tête en 64 bits | égale à la taille de page | `wr.c:1177-1180` | oui (« SAS uses the same size for the header and the pages ») |
| Row Size : flags | `0x00223011` (non compressé) / `0x0022703A` (compressé) | `wr.c:37-38, 825` | non |
| Row Size : champ 1 | `0x131` (compressé) / `0xF0` | `wr.c:823` | non |
| Row Size : champs 16-17 | `0xFF` sur 2 × int_len octets | `wr.c:836` | non |
| Row Size : divers en fin | `len-64 = 4`, `len-62 = 1`, octet `len-13 = 5` (32 bits) / `1` (64 bits) | `wr.c:889-896` | non |
| Subheader Counts : signatures suivies | `0xFFFFFFFC/D/F/E` **et `0xFFFFFFFB`, `0xFFFFFFFA`, `0xFFFFFFF9`** | `wr.c:903-911` | non (les trois dernières ni nommées ni lues) |
| Subheader Counts : constante | **`1804`** à l'offset 56/112 ; 7 vecteurs « suivis » alors que `SAS7BDAT_COUNTS_VECTOR_COUNT` = 12 (défini, jamais utilisé) | `wr.c:942, 912, 35` | non |
| Flag de nom de colonne | `0x0C00` / `0x0400` / `0x0800` | `wr.c:633-640` | non |
| Column List | champ constant `1` ; écrit seulement à partir de 2 colonnes | `wr.c:694-698, 662` | partiel (« SAS only writes the hash table when there is more than one column ») |
| Texte du créateur | `"DATASTEP"` ; texte « blank » = 8 espaces | `wr.c:511, 485, 507-510` | non |
| Premier bloc de texte | `{0, 0, 0, 20}` (20 = UTF-8) | `wr.c:484` | oui (commentaire en ligne) |
| Page « COMP » (fichier compressé) | type `0x9000` ; valeurs à `phs-6`, `phs-4`, `phs-2`, `page_size-7`, `-6`, `-2` | `wr.c:1036-1046` | oui pour le principe (`wr.c:1023`), non pour chaque valeur |
| Pointeur terminal de sous-en-tête | longueur 0, compression = 1 (TRUNC) | `wr.c:1003-1004` | oui |
| Manquants | `.` = octet 5 à `0xFE` ; `_` = `0xFF` ; `A`-`Z` = `0xFD`…`0xE4` ; octets 6-7 = `0xFF` | `wr.c:1360-1389` | non |

### 3.3 Champs ignorés par les implémentations

Tous marqués **`inconnu`** (leur rôle n'est établi par aucune source lue) :

- **En-tête** : octets 33-36, 38, 40-63, 72-83 et 124-155 (« mystery » de ReadStat, `sas.h:10-21`) ; 8 octets sautés après le nombre de pages (`sas.c:309-316`) ; zone à partir de l'offset 288 (checksum de mot de passe, numéro de page, doublon du timestamp), sautée par ReadStat (`sas.c:366`) ; troisième horodatage (PDF 328+a1+a2), non lu par Parso, pandas, `sas7bdat` Python ; octet 39 (plateforme) lu par `sas7bdat` Python seulement (`PY:1395-1400`) et par aucune autre implémentation.
- **Sous-en-têtes** : *Subheader Counts* (`F-FFFFFC00`) et *Column List* (`FEFFFFFF`) ignorés par ReadStat (`rd.c:706-717`), Parso (corps vides, `SFP:1516-1517, 1727-1729`), pandas (« unknown purpose », `P7:740-742, 845-847`) et `sas7bdat` Python (« Not sure what to do here yet », `PY:944, 1182`) ; PDF : « The purpose of this subheader is not clear » (p.13) ; signatures `0xFFFFFFF8` (« locale? »), `0xFFFFFFF9`, `0xFFFFFFFA` (« unknown ») et `0xFFFFFFFB` (`sas.h:98-99`, PDF p.10 « unknown signature #1/#2/#3 »).
- **Row Size** : la plupart des champs, hors longueur de ligne, nombre de lignes, lignes supprimées, lignes sur page mix, références du libellé et de la compression (`rd.c:245-313`).
- **Pages** : numéro de séquence de page (4 premiers octets) ; page « comp » `0x9000` (vue une fois dans le PDF p.7, ignorée par Parso, pandas, `sas7bdat` Python, rejetée par R) ; les 4 derniers octets d'un pointeur de sous-en-tête (« uncertain », PDF p.7).
- **Colonnes** : informats (aucun lecteur ne les restitue, sauf copie du format par ReadStat) ; NCFL1/NCFL2 (nombre de sous-en-têtes Format&Label par page) lus comme somme de contrôle par pandas et `sas7bdat` Python seulement.
- **Catalogue** : certains offsets documentés avec doute (« might be 0x10, not sure », `bcat.c:200`).
- **Écrits par ReadStat, lus par aucun lecteur lu** : octets 47 et 71 d'encodage, checksum de mot de passe, masque de numéro de page.
- **Dans le PDF** : la page ToDo (p.18) liste BINARY, encodage, chiffrement, fichiers de plus de 2,1 milliards de lignes, valeurs manquantes, « page sequence signature », pages amd, Column List, « header bytes by SAS_host ».

---

## 4. Liste des manques

Un manque est une information absente, contradictoire ou non vérifiée dans les sources publiques. Chacun est rattaché à **un seul** changement parmi 02 `specify-sas7bdat-read-format`, 03 `specify-sas7bdat-compression`, 04 `specify-sas-formats-encodings`, 05 `specify-sas7bdat-writing`. Impact : `bloquant` (le driver ne peut pas fonctionner correctement sans), `dégradé` (fonctionnement partiel ou risqué), `confort`. **Méthode par défaut : analyse différentielle** de fichiers produits par SAS (corpus de `setup-sas-oracle-corpus`) ; la décompilation n'est retenue que lorsque la variation contrôlée des entrées ne suffit pas, avec la justification.

### 4.1 Rattachés à 02 `specify-sas7bdat-read-format`

| # | Manque | Impact | Méthode |
|---|---|---|---|
| M01 | Rôle des octets d'en-tête non expliqués (33-36, 38, 40-63, 72-83, 124-155) et des 8 octets après le nombre de pages | dégradé | Analyse différentielle : paires de fichiers ne différant que par un paramètre (plateforme, nom, encodage, taille de page) |
| M02 | Horodatages : création, modification, deux champs « diff » de décalage UTC, troisième horodatage, effet de `a1` sur les offsets | confort | Analyse différentielle (créations successives, `TIMEZONE=`) |
| M03 | Types de pages et flags : 128, 384, 640, flag `0x0080`, masque `0xFF00`, `META2` 0x4000, page `AMD` (position, rôle), contradictions D12 | bloquant | Analyse différentielle sur fichiers avec suppressions, grosses métadonnées et pages amd ; décompilation du parcours de pages seulement si les fichiers ne lèvent pas l'ambiguïté |
| M04 | Rôle des sous-en-têtes *Subheader Counts* et *Column List*, des signatures `F8`/`F9`/`FA`/`FB`, des champs de Row Size non lus, du flag de nom de colonne (D17), des octets D8-D9 | dégradé | Analyse différentielle (variation du nombre de colonnes, de la longueur des noms) |
| M05 | Disposition des signatures 64 bits big-endian et variante `FFFFFBFE` (D22) | dégradé | Analyse différentielle sur fichiers 64 bits big-endian (corpus externe : un seul oracle SAS non big-endian sur l'hôte, voir 4.5) |
| M06 | Bitmap de lignes supprimées : emplacement, ordre des bits, lien avec le champ « unused bytes » (D13), compteur de lignes supprimées (D11) | dégradé | Analyse différentielle (suppressions contrôlées, si l'oracle permet de supprimer des lignes) |
| M07 | Formule d'alignement du début des lignes sur pages *mix* (D14) | bloquant | Analyse différentielle (tables variant le nombre et la taille des sous-en-têtes) |
| M08 | Numériques de largeur 1-2 octets (D15) ; troncature 3-8 octets | dégradé | Analyse différentielle (`LENGTH` variable ; 2 octets possible sous z/OS seulement d'après la doc : cas probablement hors d'atteinte) |
| M09 | Codage exact des manquants spéciaux dans les données (D16) | dégradé | Analyse différentielle (toutes les valeurs `.`, `._`, `.A`–`.Z`) |
| M10 | Limites réelles : fichiers > 2 Go, > 2^31 ou > 2^32 lignes, nombre de colonnes, taille de page max | dégradé | Analyse différentielle sur fichiers volumineux ; décompilation des contrôles de validité d'ouverture si la génération de gros fichiers n'est pas possible dans l'environnement |
| M11 | Champs `host`, `version`, `os_vendor`, `os_name`, `release` de fin d'en-tête et leur sémantique ; octet 39 (plateforme) | confort | Analyse différentielle (fichiers de plusieurs plateformes : corpus externe) |

### 4.2 Rattachés à 03 `specify-sas7bdat-compression`

| # | Manque | Impact | Méthode |
|---|---|---|---|
| M12 | Facteur de la commande RLE `0x40` (×16 ou ×256) et commandes `0x10`, `0x20`, `0x30`, `0x50` (D5-D7) | bloquant | Analyse différentielle : tables conçues pour produire de longues suites de chaque octet ; décompilation de `sasyzcrl` si des opcodes ne sont pas produits |
| M13 | Détection de la compression et granularité (par ligne ? par page ?), pointeur type 1, rôle de la page `COMP` `0x9000`, trois méthodes en conflit (D10) | bloquant | Analyse différentielle (`COMPRESS=NO|CHAR|BINARY`, tables à une page ou à plusieurs) |
| M14 | Format exact du RDC (SASYZCR2) : fenêtre, seuils, codage des flags ; les trois implémentations concordent mais le PDF ne le documente pas et aucun test RDC dédié n'est certain dans ReadStat | dégradé | Analyse différentielle avec vecteurs (ligne brute, ligne compressée) ; décompilation de `sasyzcr2` si les vecteurs ne couvrent pas les cas limites |
| M15 | Règles de décision du compresseur de SAS : seuils d'avantage, repli sur ligne brute, surcoût de 12/24 octets par observation | dégradé | Décompilation du chemin de compression : la variation d'entrées ne révèle pas les seuils de décision (justification) ; les sorties du driver doivent seulement être décodables par SAS |

### 4.3 Rattachés à 04 `specify-sas-formats-encodings`

| # | Manque | Impact | Méthode |
|---|---|---|---|
| M16 | Table code d'encodage → nom : désaccords D1-D4, codes absents (52-54, 71, 73, 76, 119, 128, 137, 169, 172, 228-229), octets 47/70/71 | bloquant pour les fichiers non UTF-8 | Corpus externe (Parso : ~70 encodages) + `tknlsenc.so` ; l'oracle `sas_small` ne produit que de l'UTF-8 : décompilation justifiée pour la table et les cas non reproductibles |
| M17 | Comportement face aux octets invalides et à la troncature d'un caractère multi-octets en fin de champ | confort | Analyse différentielle (valeurs tronquées dans des variables de longueur contrainte) |
| M18 | Classification des formats natifs (date, datetime, heure, numérique, décimal, caractère), liste exhaustive des noms de formats natifs (« plusieurs centaines », non établie), formats NLS | bloquant | Décompilation de la table de `tkefmt*` (aucune liste unique publique) ; algorithmes d'affichage vérifiés par analyse différentielle (`PROC PRINT`) |
| M19 | Stockage du nom de format et d'informat dans le descripteur (largeur du champ de nom, informats non restitués par aucun lecteur, mode « legacy ») (D18) | dégradé | Analyse différentielle (noms de formats de 1 à 32 caractères) |
| M20 | Structure du `.sas7bcat` : en-tête, répertoire d'entrées, entrées `FORMAT/FORMATC/INFMT/INFMTC`, `PICTURE`, plages, encodage des entrées ; seuls ReadStat et trois lecteurs récents le décodent, sans spécification | dégradé | Analyse différentielle si `PROC FORMAT` fonctionne dans l'oracle ; sinon catalogues du corpus externe |
| M21 | Plage de dates valide (4000 contre 19 900, D20) et anomalie des années 4000/8000 | confort | Analyse différentielle (`PROC PRINT` aux bornes) |

### 4.4 Rattachés à 05 `specify-sas7bdat-writing`

| # | Manque | Impact | Méthode |
|---|---|---|---|
| M22 | Rôle et caractère obligatoire de chaque constante écrite par ReadStat (3.2) : lesquelles SAS vérifie, lesquelles il ignore | bloquant | Analyse différentielle par mutation d'un fichier valide (SAS relit-il ?), puis décompilation des chemins de validation à l'ouverture pour les rejets inexpliqués |
| M23 | Aucune preuve qu'un fichier écrit par ReadStat, `statfmt` ou `goreadstat` soit relu par SAS sans erreur (D21) | bloquant | Boucle de validation par SAS (`PROC CONTENTS`, `PROC PRINT`, log) |
| M24 | Calcul des champs dérivés : checksum de mot de passe, numéro de page, taille de page et d'en-tête, nombres de blocs, localisations des sous-en-têtes, ordre des sous-en-têtes | bloquant | Analyse différentielle ; compléter par mutation |
| M25 | Écriture non couverte par aucun écrivain public : BINARY (RDC), endianness autre que l'hôte, 32 bits, encodage non UTF-8, noms de table, pages > 16 MiB, valeur 0 ligne | dégradé | Analyse différentielle ; validation par SAS |
| M26 | Ajout de lignes à un fichier existant, index `.sas7bndx`, contraintes d'intégrité, piste d'audit `.sas7baud`, attributs étendus `.sas7bxat`, chiffrement (`ENCRYPT=YES|AES|AES2` : aucune source publique) | confort | Documenter seulement ce qui est observable ; le chiffrement exige SAS/SECURE, disponibilité dans l'oracle `inconnue` |

### 4.5 Manques qui exigent des fichiers de plateformes particulières (à signaler à `setup-sas-oracle-corpus`)

Sources de fichiers externes repérées (licences à 5.3) :

- **Big-endian et 32 bits** : `kshedden/datareader` (`test_files/data/test1..21`, produits par SAS : win32, win64, lin64, aix32 BE, aix64 BE × `compress=no|char|binary`), pandas (3 fichiers 32 BE, 3 fichiers 64 BE), Parso (2 fichiers 64 BE), `polars_readstat` (licence des fichiers `inconnue`).
- **RDC** : `datareader` (7), pandas (6), ReadStat (4 fichiers `compression_type_*`), `pyreadstat` (1).
- **Encodages** : Parso (~70 codes), pandas (codes 0/20/28/29/40/61/62).
- **Catalogues, manquants spéciaux** : `pyreadstat` (`missing_test`, catalogues Windows/Linux), ReadStat `resources/`, `polars_readstat` `haven/`.
- **Fichiers adverses** (colonnes déclarées en excès, `row_length` géant) : `sas7bdat-parser-rs`.
- **Aucun fichier public repéré** pour : fichier chiffré, index `.sas7bndx`.
- **Point d'attention matériel** : `sas_small` est un ELF x86-64 Linux ; l'hôte de travail est un Mac. L'émulation (Rosetta ou QEMU) peut être lente ou instable : à traiter par `setup-sas-oracle-corpus`.

---

## 5. Contraintes de licence

> Constat de fait sur des licences lues dans les sources, pas un avis juridique. La décision finale appartient au référent licence / juridique de NULL (voir 5.4).

### 5.1 Règle de lecture par source

Règles : **lecture libre** = le code peut être lu en détail pendant la conception (la licence est permissive) ; une reprise de code ou de texte impose de conserver l'avis de copyright ; **documentation seule** = lecture limitée à la documentation et aux comportements observables, pas de lecture détaillée du code ; **non lu** = la source n'est pas utilisée. Une licence `inconnue` est traitée comme copyleft.

| Source | Licence | Règle de lecture | Effet sur un driver sous licence permissive |
|---|---|---|---|
| ReadStat (I1), `haven` (I2) | MIT | Lecture libre. **Exception** : ne pas lire `src/sas/ieee.c` (routines XPORT d'origine SAS TS-140, licence non précisée) | Aucun obstacle, avis MIT à conserver en cas de reprise |
| `pyreadstat` (I3) | Apache-2.0 (+ MIT) | Lecture libre | Compatible ; Apache-2.0 impose avis et mention des modifications |
| Parso ≥ 2.0.8 (I4) | Apache-2.0 | Lecture libre. **Ne pas lire les versions ≤ 2.0.7** (GPL v3) | Compatible. Éviter toute confusion de version dans un dépôt cloné |
| pandas (I5) | BSD-3-Clause (+ MIT) | Lecture libre | Compatible |
| `sas7bdat` Python (I6) | MIT | Lecture libre. Lien de filiation avec Parso/pandas (constantes de même nom) | Compatible. Prudence supply chain : version 2.3.0 publiée après 7 ans de silence |
| Paquet R `sas7bdat` (I8), dépôt GitHub | GPL-2.0-or-later (déclaré, sans fichier `LICENSE`) | **Documentation seule** : lire la vignette (faits sur le format), **pas le code R** | Risque de contamination copyleft si le code R était reproduit ; en lecture seule de faits, pas de dérivation |
| PDF de Shotwell (I7) | CC BY-SA 3.0 | Lecture libre de la description du format (faits) ; **ne pas recopier le texte, les tableaux ni les figures** dans la spécification ou le code | La clause SA s'appliquerait à toute reproduction : la spécification doit être réécrite, pas recopiée. La licence du PDF diffère de celle du paquet qui l'embarque |
| `FredHutch/sas7bdat` (I9, écrivain Java) | MIT | Lecture libre | Compatible |
| `FredHutch/sas7bdat-specification` (I9) | `inconnu` | **Documentation seule** (non lue à ce stade) ; ne pas recopier | Traitée comme non réutilisable |
| Crate `sas7bdat` (I10) | MIT | Lecture libre. Dépôt à un seul mainteneur : ne pas en faire une dépendance critique sans évaluation | Compatible |
| `polars-readstat-rs` (I11) | Apache-2.0 (fichier) alors que le registre dit MIT | Lecture libre sous Apache-2.0 ; **re-vérifier à la prochaine release**. Code largement produit par assistants IA (revendiqué) : à valider contre l'oracle, pas à prendre comme source de vérité | Compatible sous Apache-2.0 |
| `readstat-rs` (I12) | MIT (ReadStat vendorisé MIT) | Lecture libre | Compatible |
| `sasquatch` Rust (I13) | MIT (fichier) / « MIT or Apache-2.0 » (métadonnées) | **Documentation seule** tant que la divergence n'est pas levée | Statut ambigu |
| `SAS-reader` (I14), `quick-sas7bdat`, `rust-sas-reader`, `readstat-go` (I15, I20) | `inconnu` (aucune licence) | **Non lu** | Interdit en l'état |
| `statfmt` (I16) | MIT (+ avis MIT de ReadStat) | Lecture libre | Compatible |
| `datareader` Go (I17) | BSD-3-Clause | Lecture libre | Compatible |
| `sas7bdat-js` / npm `sas7bdat` (I18) | `Apache-2.0` déclarée sans fichier `LICENSE` | **Documentation seule** (traité comme `inconnu`) | Statut incertain |
| `js-stream-sas7bdat` (I19) | MIT | Binding de ReadStat : ne rien apporter de plus que ReadStat | Compatible |
| `goreadstat` (I20) | licence composite (pyreadstat Apache-2.0 + ReadStat MIT) | Documentation seule (binding de ReadStat) | Statut ambigu |
| `nbbrd/sasquatch` Java (I21) | EUPL-1.2 | **Documentation seule** | Copyleft : pas de lecture détaillée du code |
| `richokelly/sas7bdat` (C#), `cpp-sas7bdat` (C++), `SASLib.jl` | Apache-2.0, Apache-2.0, MIT | Lecture libre | Compatible |
| Extensions DuckDB (I22), paquets npm `readstat-js`, `sas7bdat.js` (I23) | `inconnu` | **Non lu** | Interdit en l'état |
| Documentation SAS officielle (1.4) | Droits réservés SAS ; Terms of Use S17 | Lecture pour en tirer des **faits publics** ; ne pas reproduire de passages ; voir 5.4 pour la clause de S17 | Voir 5.4 |
| Papiers XPORT (S12, S13) | Copyright SAS ; décrits comme non propriétaires ; code C « as is » sans licence open source explicite | Lecture des faits du format de transport ; **ne pas reprendre le code C** | Hors périmètre sas7bdat |

### 5.2 Licence recommandée pour le driver

**Recommandation : double licence `MIT OR Apache-2.0`** (convention de l'écosystème Rust), à défaut Apache-2.0 seule.

Justification :
- **Exposition en R et en Python.** Les deux licences sont acceptées par CRAN et PyPI. Apache-2.0 est incompatible avec la GPL v2 stricte, alors que MIT l'est : le double choix laisse à chaque utilisateur la licence compatible avec ses autres dépendances.
- **Intégration Arrow, DataFusion, Arrow Flight, Dremio.** Cet écosystème est en licence Apache-2.0 : Apache-2.0 est naturellement compatible, et sa clause de brevets protège les utilisateurs en entreprise.
- **Aucun obstacle de dérivation** si la règle de lecture de 5.1 est respectée : aucune source classée « lecture libre » n'est sous copyleft.
- **Cohérence avec les voisins existants** : ReadStat (MIT), Parso (Apache-2.0), pandas (BSD-3-Clause), `sas7bdat` Rust (MIT), `polars-readstat-rs` (Apache-2.0).

Hypothèse de travail : cette recommandation suppose que NULL autorise la publication du driver en logiciel libre. Si la propriété intellectuelle du code revient à NULL et que celle-ci en décide autrement, la licence est à reprendre. La confirmation définitive est laissée à `design-rust-driver` (06) sans modifier les specs ni les tâches de ce changement.

### 5.3 Licence des jeux de fichiers de test

| Jeu | Licence lue | Redistribution dans le dépôt du driver |
|---|---|---|
| `datareader` `test_files/data` (21 fichiers produits par SAS) | BSD-3-Clause (dépôt) | possible avec attribution |
| Parso `src/test/resources` (134 fichiers) | Apache-2.0 (dépôt) | possible avec attribution |
| pandas `pandas/tests/io/sas/data` (30 fichiers) | BSD-3-Clause (dépôt) | possible avec attribution |
| `pyreadstat` `test_data`, ReadStat `resources` | Apache-2.0 (+ MIT), MIT | possible avec attribution |
| `sas7bdat-parser-rs` (fixtures) | attributions dans `THIRD_PARTY_NOTICES.md` | possible avec attribution |
| `polars_readstat` `tests/sas/data` (445), `sas7bdat-js` `test/data` (202) | licence **des fichiers `inconnue`** (provenance non documentée : `data_poe`, `AHS2013`, `sas_to_csv`, `data_reikoch`, `data_big5`) | **non** : usage local d'analyse seulement |

### 5.4 Points à confirmer en interne chez NULL

1. **Conditions d'utilisation de sas.com (S17, révisées 2018-10-18).** Constat, paraphrasé : l'usage des contenus du site est autorisé à titre personnel ou non commercial avec mention de la source ; modification, usage commercial et miroir exigent un accord écrit ; le texte contient une clause interdisant d'utiliser les contenus pour développer ou utiliser une offre concurrente d'une offre SAS, avec dommages-intérêts forfaitaires. La portée exacte de la clause est à lire sur la page. Elle vise le contenu du site, pas les binaires de `src/`, mais elle est à faire examiner avant d'utiliser la documentation SAS comme source de la spécification du driver.
2. **Licence SAS de `src/`** (SAS 9.4 TS1M7, titulaire NULL) : confirmation interne (référent licence SAS / juridique) que l'analyse à des fins d'interopérabilité est couverte, avant toute décompilation (changements 02 à 05). Ce point est inscrit comme blocage ouvert dans `rea_output/STATE.md`.
3. **Cadre d'interopérabilité** (directive 2009/24/CE, art. 6) : l'exception ne couvre que les informations qui ne sont pas déjà accessibles. Ce document délimite le connu public (sections 1 à 3) pour que la liste des manques (section 4) borne la décompilation.
4. **Autorisation de publication** du driver en logiciel libre (licence recommandée en 5.2).
