# ADR 0001 — Découpage en crates et dépendances autorisées dans le cœur

- Statut : proposée (2026-10-30), à valider au jalon 0 de `docs/roadmap.md`
- Spécification liée : `10` §6, `15` §2, `20` §4, `30` §1 et §6 (le cœur les implémente toutes)

## Contexte

Le format se décrit entièrement sur des tampons d'octets : l'en-tête fait `HL` octets, une page `P` octets à l'offset `HL + k × P`, une ligne `row_length` octets (`10` §1, §3.3). Rien dans la spécification n'impose d'I/O. En revanche les consommateurs sont hétérogènes : Arrow/DataFusion (async, `object_store`), Python et R (pas de runtime async souhaitable), Flight (tonic/tokio). Le parseur est exposé à des fichiers non fiables (S3, Flight) et doit être fuzzable (tâche 4.4). Le critère de la spécification d'OpenSpec impose que les adaptateurs dépendent du cœur et non l'inverse.

## Options étudiées

| Option | Description | Pour | Contre |
|---|---|---|---|
| A. Crate unique avec features | `sas7bdat` avec features `arrow`, `datafusion`, `python`, `r`, `async` | un seul paquet à publier ; démarrage simple | les features Cargo s'additionnent (unification) : un utilisateur d'`arrow` tire l'async ; le fuzzing et l'audit du parseur portent sur un graphe de dépendances large ; PyO3/extendr imposent `crate-type = ["cdylib"]` incompatible avec une bibliothèque réutilisable ; difficile de garantir « sans I/O » |
| B. Cœur sans I/O + adaptateurs (retenue) | `sas7bdat` (octets seulement), `uniformat-sas-io`, `-arrow`, `-datafusion`, `-py`, `-r`, `-flight`, `-cli` | frontière vérifiable (liste de dépendances du cœur) ; le cœur se fuzze et se teste sans disque ni réseau ; l'async reste dans les adaptateurs ; R n'a pas besoin d'Arrow | plusieurs paquets à versionner ensemble ; discipline de workspace |
| C. Cœur avec I/O synchrone intégré (`Read + Seek` dans le cœur) | pas de crate `-io` | un crate de moins | le cœur dépend de `std::io` pour son API ; le mapping mmap/`object_store` (plages) reste à greffer ; l'API `Read + Seek` pousse à une lecture séquentielle contraire au besoin de plages |

## Décision

**Option B.** Workspace Cargo avec huit crates : `sas7bdat`, `uniformat-sas-io`, `uniformat-sas-arrow`, `uniformat-sas-datafusion`, `uniformat-sas-py`, `uniformat-sas-r`, `uniformat-sas-flight` (jalon 7), `uniformat-cli`. Une crate `-io` s'ajoute à la liste du prompt pour porter l'abstraction `ByteSource` (plages) partagée par Arrow et R (R ne passe pas par Arrow).

**Dépendances autorisées dans le cœur `sas7bdat`** (liste fermée) :

| Dépendance | Raison |
|---|---|
| `thiserror` | erreurs typées (ADR 0007) |
| `encoding_rs` | décodage/encodage des 127 codes d'encodage supportés (`30` §6.1) ; pure Rust, sans I/O ; réserves ISO-8859-1 / US-ASCII de `30` §6.1 prises en compte dans l'ADR 0003 |
| `memchr` | recherche d'octets (littéraux `SASYZCRL`/`SASYZCR2`, blancs finaux) |
| `proptest` | dev-dependency seulement |

**Interdits dans le cœur** : `std::fs`, `std::net`, `std::thread`, `tokio`, `futures`, `memmap2`, `arrow*`, `datafusion`, `pyo3`, `extendr-*`, `rayon`, `bytemuck`/toute crate exigeant `unsafe` côté notre code, `serde` (les types de métadonnées sérialisables le sont par les adaptateurs). Vérification : un test de CI (`cargo tree -p uniformat-sas --edges normal`) compare l'arbre à cette liste ; `#![forbid(unsafe_code)]` dans le cœur.

**Graphe** : `-cli`, `-datafusion`, `-py`, `-flight` → `-arrow` → `-io` → `sas7bdat` ; `-r` → `-io`. Aucune flèche ne part du cœur.

**Versionnement** : tous les crates partagent la version du workspace (`0.x` tant que la spécification contient des `inconnu` non levés) ; les crates `-py` et `-r` ne sont pas publiées sur crates.io (publiées sur PyPI et CRAN/r-universe).

## Conséquences

- (+) Le fuzzing du parseur et du décompresseur ne dépend d'aucun runtime.
- (+) Une application Rust sans Arrow (outil de contrôle, convertisseur) dépend de `sas7bdat` + `uniformat-sas-io` seulement.
- (+) La lecture sur `object_store` (S3/HTTP) ne demande aucune modification du cœur : l'adaptateur obtient des plages d'octets (ADR 0002) et appelle `PageDecoder`.
- (−) Chaque évolution de l'API du cœur doit être répercutée dans quatre adaptateurs ; un `CHANGELOG` commun et des tests de compilation du workspace complet en CI sont nécessaires.
- (−) `encoding_rs` ne couvre pas ISO-8859-1 ni US-ASCII stricts (`30` §6.1) : le cœur expose les octets bruts (`RowRef::bytes`) pour que l'appelant impose son décodage, et applique la politique de l'ADR 0003.
- Point ouvert : publier `uniformat-sas-io` et `sas7bdat` sous un seul nom si la communauté préfère ; sans effet sur les frontières.
