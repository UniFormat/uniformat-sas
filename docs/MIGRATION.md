# Migration from the former `SAS_DRIVERS` project

This repository is the SAS module extracted from the former single-project workspace. Names changed as follows.

| Former | Now |
|---|---|
| crate `sas7bdat` | `uniformat-sas` |
| crates `sas7bdat-io`, `-arrow`, `-datafusion`, `-flight` | `uniformat-sas-io`, `-arrow`, `-datafusion`, `-flight` |
| `ByteSource`, `FileSource`, `SliceSource`, `CountingSource`, `AtomicFile` (in `sas7bdat-io`) | `uniformat-core` (re-exported by `uniformat-sas-io`) |
| crate `sas7bdat-py`, module `sas7bdat_rs` | `bindings/python`, PyPI `uniformat-sas`, import `uniformat_io.sas` |
| crate `sas7bdat-r`, R package `sas7bdatrs` | `bindings/r`, R package `uniformatsas` |
| crate `sas7bdat-cli`, binary `sas7bdat` | `uniformat-cli`, binary `uniformat` |
| crate `sas7bdat-testkit` | `uniformat-conformance/crates/uniformat-testkit` |
| binary `sas7bdat-flight-server` | `uniformat-sas-flight-server` |
| `SAS7BDAT_CORPUS`, `SAS7BDAT_FLIGHT_TOKEN`, `SAS7BDAT_FLIGHT_BIN` | `UNIFORMAT_CORPUS` (old name still read), `UNIFORMAT_FLIGHT_TOKEN`, `UNIFORMAT_FLIGHT_BIN` |
| `rea_output/*.md` | `docs/spec/*.md` |
| `rea_output/tools/` | `uniformat-conformance/sas/tools/` (and `tests/data/compression_vectors`) |

Unchanged on purpose: the file format name and extension `sas7bdat`, the Arrow metadata keys `sas.*`, the exception names
(`SasError`, …), the R function names (`read_sas7bdat`, `write_sas7bdat`) and the Python function names (`read_sas_table`,
`write_sas`, …). Source comments and the documents under `docs/` are still in French; translation is a separate task.
