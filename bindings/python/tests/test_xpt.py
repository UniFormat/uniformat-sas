"""XPORT API, independent reader compatibility and bounded failure cases."""
import datetime
import pyarrow as pa
import pytest
from uniformat_io.sas import read_xpt, read_xpt_table, write_xpt, xpt_schema


def table():
    return pa.table({"VALUE": pa.array([1.5] + [None]*28, type=pa.float64()),
                     "VALUE__missing": pa.array(range(29), type=pa.uint8()),
                     "TEXT": ["été"]*29,
                     "DAY": [datetime.date(1960, 1, 1)]*29})


def test_roundtrip_missing_dates_and_metadata(tmp_path):
    path = tmp_path / "test.xpt"
    assert write_xpt(path, table(), column_labels={"VALUE": "Measured value"})["rows"] == 29
    back = read_xpt_table(path, missing="sidecar", batch_rows=3)
    assert back["VALUE__missing"].to_pylist() == list(range(29))
    assert back["TEXT"].to_pylist() == ["été"]*29
    assert back["DAY"].to_pylist() == [datetime.date(1960, 1, 1)]*29
    assert xpt_schema(path).field("VALUE").metadata[b"sas.label"] == b"Measured value"
    copy = tmp_path / "copy.xpt"
    write_xpt(copy, read_xpt(path, missing="sidecar"))
    assert path.read_bytes() == copy.read_bytes()


def test_projection_limit_encodings_and_budgets(tmp_path):
    path = tmp_path / "latin1.xpt"
    write_xpt(path, pa.table({"N": [1], "TEXT": ["\x80été"]}), encoding="latin1")
    assert read_xpt_table(path, encoding="latin1")["TEXT"].to_pylist() == ["\x80été"]
    assert read_xpt_table(path, encoding="windows-1252")["TEXT"].to_pylist() == ["€été"]
    assert read_xpt_table(path, columns=["N"], limit=1).num_rows == 1
    with pytest.raises(Exception, match="UTF-8"):
        read_xpt_table(path)
    with pytest.raises(MemoryError):
        read_xpt_table(path, encoding="latin1", max_table_bytes=1)
    with pytest.raises(Exception):
        read_xpt_table(path, columns=["absent"])


def test_atomic_failure_and_unknown_options(tmp_path):
    path = tmp_path / "existing.xpt"
    path.write_bytes(b"previous")
    with pytest.raises(Exception):
        write_xpt(path, pa.table({"VALUE": [1.0, float("inf")]}))
    assert path.read_bytes() == b"previous"
    with pytest.raises(ValueError):
        write_xpt(path, table(), version=7)
    with pytest.raises(ValueError):
        read_xpt(path, invalid_bytes="replace")
    with pytest.raises(ValueError):
        read_xpt(path, member="DATASET", member_index=0)


def test_core_discovers_xpt_and_converts(tmp_path):
    from uniformat_io.sas import XPT_FORMAT
    path = tmp_path / "input.xpt"
    XPT_FORMAT.write_table(path, pa.table({"N": [1.25, None], "TEXT": ["one", "missing"]}))
    assert XPT_FORMAT.read_table(path).num_rows == 2
    assert XPT_FORMAT.schema(path).names == ["N", "TEXT"]


def test_independent_pyreadstat_reader(tmp_path):
    pyreadstat = pytest.importorskip("pyreadstat")
    path = tmp_path / "interop.xpt"
    write_xpt(path, pa.table({"VALUE": [1.0, -1.0, 0.5, 100.0], "TEXT": ["one", "minus", "half", "cent"]}))
    values, meta = pyreadstat.read_xport(path)
    assert values["VALUE"].tolist() == [1.0, -1.0, 0.5, 100.0]
    assert meta.column_names == ["VALUE", "TEXT"]
