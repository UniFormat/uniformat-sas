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
| `crates/uniformat-sas-io` | `uniformat-sas-io` | `SasFile`, page plans, `SasWriter`; XPORT `XptFile` / `XptWriter` over `uniformat-core` byte sources |
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

## XPORT v5/v8 (issue #1)

The first XPORT development milestone provides `uniformat_sas::xpt` (IBM numbers,
NAMESTR metadata, row codec) and `uniformat_sas_io::{XptFile, XptWriter}`. The writer
is transactional and deterministic. Special missing values are preserved. UTF-8 is
strict by default; Latin-1 and Windows-1252 are explicit options.

```rust,no_run
use uniformat_sas::xpt::Row;
use uniformat_sas_io::{FileSource, XptFile, XptOpenOptions};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let file = XptFile::open(FileSource::open("study.xpt")?, XptOpenOptions::default())?;
let columns = file.metadata().columns(); // first schema; no observations read
let members = file.members()?;           // explicit sequential indexing pass
for member in &members {
    for bytes in file.rows(member) {
        let bytes = bytes?;
        let row = Row::new(&bytes, member.metadata())?;
        // row.number(column_index)? / row.text(column_index, trim_trailing)?
    }
}
# Ok(())
# }
```

See the [specification](docs/spec/xpt.md), [ADR](docs/adr/0008-xport.md) and
[synthetic fixture / interoperability instructions](tests/data/xpt/README.md).
Without a positive v8 observation count, trailing all-blank short rows and literal
member-marker pairs are ambiguous: writes that cannot round-trip are rejected.
Listing members scans uncounted data; counted v8 members use checked extents.

The Arrow adapter now exposes `XptBatchReader`, `XptArrowWriter`, `xpt_schema`
and `write_xpt_batches` in `uniformat_sas_arrow::xpt`. `register()` adds both
`sas7bdat` and `xpt`, so the CLI recognizes `.xpt` automatically:

```sh
uniformat convert study.xpt copy.sas7bdat --read-opt missing=sidecar
uniformat convert copy.sas7bdat study-copy.xpt --read-opt missing=sidecar
```

```python
from uniformat_io.sas import read_xpt_table, write_xpt
study = read_xpt_table("study.xpt", missing="sidecar")
write_xpt("copy.xpt", study)
```

R exports `read_xpt()` / `write_xpt()` with haven-compatible tagged NA.
Text widths are inferred within a 256 MiB buffer budget; known widths permit
streaming writes. Python table reads have a separate 256 MiB retained-buffer
budget. Numeric, boolean and integer values map to SAS numbers; native temporal
types carry SAS formats. Keep sidecars when converting special missing values.

SAS-produced v5 references have now been validated: 14 files, 15 members,
116 observations, all 28 missing codes, and exact round trips through Arrow.
The [validation report](docs/xpt-sas-validation.md) documents the IBM upper-bound
fix and the explicit Windows-1252 option needed for the observed SAS LATIN1
Euro byte. Licensed reference files remain outside git.

A reproducible throughput probe is available with
`cargo run --release -p uniformat-sas-arrow --example xpt_benchmark -- 100000`.
XPORT v8 is detected automatically on input. To write it, use `version=8` in
Python/R, `WriteOptions { version: Version::V8, ..Default::default() }` in Rust,
or `--write-opt version=8` in the CLI. V5 remains the default; long metadata is
never silently truncated or promoted. V8 supports 32-byte variable/dataset names,
256-byte variable labels, long formats/informats and 32767-byte character cells.
Dataset labels remain limited to 40 bytes. See the [v8 specification](docs/spec/xpt-v8.md).

Materialized Python tables and R data frames declare their row count in v8.
For Rust or Python streams, supply `row_count` to preserve trailing blank rows;
the writer checks this exact count before commit. Without it, streaming remains
bounded and follows the legacy ambiguity policy. Independent ReadStat/haven tests
cover v8; a new execution in SAS v8 has not been performed.

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
