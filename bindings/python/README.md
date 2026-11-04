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
