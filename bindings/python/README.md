# uniformat-sas (Python)

Read and write SAS `.sas7bdat` files with the UniFormat Rust driver: Arrow streams (PyCapsule interface), pandas,
polars, schema and metadata without reading data pages, special missing values (`.`, `._`, `.A`–`.Z`) as a sidecar
column, plain and compressed (RLE / RDC) tables.

```python
import uniformat_io.sas as sas

sas.schema("t.sas7bdat")                       # pyarrow.Schema, no data page is read
sas.metadata("t.sas7bdat")                     # table, columns, timestamps, compression
tbl = sas.read_sas_table("t.sas7bdat", columns=["x", "y"], limit=1000)
df = sas.read_sas_pandas("t.sas7bdat", missing="sidecar")        # df.attrs["sas_missing"], df.attrs["sas"]
pl = sas.read_sas_polars("t.sas7bdat", missing="sidecar", missing_as="categorical")
for batch in sas.read_sas("big.sas7bdat", batch_rows=65536): ...  # stream of pyarrow.RecordBatch
sas.write_sas("out.sas7bdat", tbl, compression="char", formats={"d": "DATE9."}, column_labels={"x": "Amount"})
```

Through the common API (`uniformat-core`), the format is found by extension:

```python
import uniformat_io.core as uf
uf.read("t.sas7bdat"); uf.convert("t.sas7bdat", "copy.sas7bdat", write_options={"compression": "binary"})
```

Errors: `SasError` (a `uniformat_io.core.UniFormatError`) with `SasFormatError`, `SasUnsupportedError`, `SasWriteError`;
`OSError` for I/O. Warnings: `SasWarning`.

Status: **not published** (private repository until the licensing decision is made). Local paths only. Build:
`maturin develop --release`. The `uniformat-core` package must be installed (it provides `uniformat_io.core`).

## SAS Transport v5

`read_xpt(path, **options)` returns an Arrow stream; `read_xpt_table` collects it
within `max_table_bytes` (256 MiB by default). `xpt_schema` reads the first member's
headers only. Select another dataset with `member="NAME"` or `member_index=1`
(zero-based); selection requires a sequential scan.

`write_xpt(path, data, **options)` accepts Arrow, pandas and polars and writes
atomically. Known character widths permit streaming; otherwise inference is
bounded by `max_buffer_bytes` (256 MiB). `lengths`, `column_labels`, `formats`
and `informats` override field metadata. Encoding defaults to strict UTF-8;
`latin1` and `windows-1252` must be explicit. Use `missing="sidecar"` to preserve
all special SAS missing codes during conversions. Header dates are deterministic
unless explicitly supplied as `ddMMMyy:hh:mm:ss` strings.
