"""SAS Transport v5/v8, with explicit encodings and bounded materialization."""
from __future__ import annotations
import os
from . import _native, _pa, _prepare_table, metadata_from_schema


def _options(options):
    return {key: str(value).lower() if isinstance(value, bool) else str(value)
            for key, value in options.items() if value is not None}


def read_xpt(path, *, columns=None, limit=None, **options):
    """Return an Arrow stream. UTF-8 is strict by default; encoding is explicit.

    ``member`` selects a dataset name; ``member_index`` is zero-based. Common
    Arrow options include missing='sidecar', dates='float', batch_rows,
    max_batch_bytes, invalid_bytes='binary', and trailing_blanks='keep'.
    """
    reader = _native.read_xpt(os.fspath(path), _options(options),
                              None if columns is None else list(columns), limit)
    return _pa().RecordBatchReader.from_stream(reader)


def read_xpt_table(path, *, max_table_bytes=256 * 1024 * 1024, **options):
    """Collect an XPT stream into an Arrow table within a retained-buffer budget."""
    if max_table_bytes <= 0:
        raise ValueError("max_table_bytes must be positive")
    reader = read_xpt(path, **options)
    batches, retained = [], 0
    try:
        for batch in reader:
            retained += batch.get_total_buffer_size() + 256
            if retained > max_table_bytes:
                raise MemoryError("XPT table exceeds max_table_bytes; use read_xpt for streaming")
            batches.append(batch)
        return _pa().Table.from_batches(batches, schema=reader.schema)
    finally:
        reader.close()


def xpt_schema(path, **options):
    """Read the first member's schema without reading observations.

    Selecting a later member requires an explicit sequential index scan.
    """
    return _pa().schema(_native.xpt_schema(os.fspath(path), _options(options)))


def write_xpt(path, data, *, column_labels=None, formats=None, informats=None,
              lengths=None, missing_suffix="__missing", **options):
    """Write XPORT v5/v8 atomically, rejecting truncation and unrepresentable values.

    Accepts Arrow, pandas, or polars. Text widths are measured in encoded bytes
    when omitted, within max_buffer_bytes (256 MiB). Header dates default to
    01JAN00:00:00:00. Options: version (5 by default, or 8), encoding, table_name, table_label,
    created, modified, row_count (an exact count for v8 streaming inputs).
    Materialized v8 tables automatically declare their number of observations.
    Streaming inputs with widths in their schema are consumed without collection.
    """
    if not isinstance(data, _pa().RecordBatchReader) or any(
            x is not None for x in (column_labels, formats, informats, lengths)):
        data = _prepare_table(data, column_labels, formats, informats, lengths, missing_suffix)
    if str(options.get("version", 5)) == "8" and isinstance(data, _pa().Table):
        options.setdefault("row_count", data.num_rows)
    rows = _native.write_xpt(os.fspath(path), data,
                             _options(dict(options, missing_suffix=missing_suffix)))
    return {"rows": rows, "warnings": []}


def _metadata(path, **options):
    return metadata_from_schema(xpt_schema(path, **options))


from uniformat_io.core import Format
XPT_FORMAT = Format(name="xpt", extensions=("xpt",), read_table=read_xpt_table,
                    write_table=write_xpt, schema=xpt_schema, metadata=_metadata,
                    description="SAS Transport v5/v8 (.xpt)")
