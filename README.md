# uniformat-sas

Read and write SAS `.sas7bdat` data sets from Rust, Python and R, as a format module of the UniFormat family.
It depends only on [`uniformat-core`](https://github.com/UniFormat/uniformat-core).

- plain and compressed tables (`COMPRESS=CHAR` RLE, `COMPRESS=BINARY` RDC), 32/64-bit, both byte orders (read), 64-bit
  little endian (write);
- metadata (labels, formats, informats, encodings, timestamps) and the schema **without reading data pages**;
- special missing values (`.`, `._`, `.A`–`.Z`) as a sidecar column;
- Arrow `RecordBatch` streams, parallel page-range plans, DataFusion SQL, Arrow Flight service;
- transactional writing (temporary file renamed on commit).

## Layout

| Path | Crate / package | Role |
|---|---|---|
| `crates/uniformat-sas` | `uniformat-sas` | codec on byte buffers, no I/O, three dependencies (`thiserror`, `encoding_rs`, `memchr`) |
| `crates/uniformat-sas-io` | `uniformat-sas-io` | `SasFile`, page plans, `SasWriter` over `uniformat-core` byte sources |
| `crates/uniformat-sas-arrow` | `uniformat-sas-arrow` | Arrow schema, batches, writer; implements `uniformat_core::Format` |
| `crates/uniformat-sas-datafusion` | `uniformat-sas-datafusion` | DataFusion `TableProvider` |
| `crates/uniformat-sas-flight` | `uniformat-sas-flight` | Arrow Flight server (`uniformat-sas-flight-server`) |
| `bindings/python` | PyPI `uniformat-sas`, import `uniformat_io.sas` | PyO3 / maturin |
| `bindings/r` | R package `uniformatsas` | extendr |
| `docs/` | | architecture, ADRs, roadmap, benchmarks, format specification (`docs/spec`, French) |
| `tests/data/` | | small reference tables used by tests and by `ci/gen_tables.py` |
| `fuzz/`, `ci/`, `benches/` | | fuzz targets (never run yet), dependency and table checks, benchmarks |

## Quick start

```python
import uniformat_io.sas as sas
tbl = sas.read_sas_table("data.sas7bdat", missing="sidecar")
sas.write_sas("copy.sas7bdat", tbl, compression="binary")
```

```bash
uniformat convert data.sas7bdat copy.sas7bdat --write-opt compression=char   # uniformat-cli
```

## Development

Repositories are expected side by side (`../uniformat-core`, `../uniformat-conformance`) because of Rust path
dependencies.

```bash
cargo test --workspace --exclude uniformat-sas-py            # unit tests
UNIFORMAT_CORPUS=/path/to/corpus cargo test --workspace --exclude uniformat-sas-py -- --ignored   # optional corpus tests
ci/check_core_deps.sh && python3 -I ci/gen_tables.py --check
cd bindings/python && maturin develop --release && pytest      # Python
R CMD INSTALL bindings/r/rpkg                                   # R
```

The reference corpus produced with a licensed SAS installation is **not** part of any repository; tests that need it are
skipped when `UNIFORMAT_CORPUS` is not set (`UNIFORMAT_CORPUS` also accepts the former name `SAS7BDAT_CORPUS`). The public
files of `uniformat-conformance/data` can be used instead.

## Status and licensing

0.1, **packages not yet published to registries**. Source repositories are public. License `MIT OR Apache-2.0`, provisional.
`docs/spec` and `tests/data` document format analysis and provenance; consult those notes when redistributing reference material.

## Sécurité

Les limites de ressources, les tests synthétiques obligatoires, les conditions d'exposition de Flight et les commandes d'audit sont décrits dans [docs/security.md](docs/security.md). Les tests utilisant le corpus externe sont désormais explicitement ignorés par défaut ; ils se lancent avec `UNIFORMAT_CORPUS=...` et `--ignored`.
