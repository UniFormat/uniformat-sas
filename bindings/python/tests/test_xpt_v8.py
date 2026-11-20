"""V8 public APIs, independent interop and explicit downgrade failures."""
import pyarrow as pa
import pytest
from uniformat_io.sas import read_xpt, read_xpt_table, write_xpt, xpt_schema


@pytest.mark.parametrize("extended", [False, True])
def test_long_metadata_roundtrip_and_independent_reader(tmp_path, extended):
    path = tmp_path / "long.xpt"
    label = "a" * 39 + "é" * 80  # the historical 40-byte prefix splits UTF-8
    table = pa.table({"same_prefix_first": [1.25, None, -2.5],
                      "same_prefix_second": ["x" * 250, "été", "end"]})
    formats = {"same_prefix_first": "LONG_FORMAT_NAME12.2"} if extended else None
    labels = {"same_prefix_first": label, "same_prefix_second": "b" * 256}
    write_xpt(path, table, version=8, table_name="LONG_DATASET_NAME",
              column_labels=labels, formats=formats)
    schema = xpt_schema(path)
    assert schema.metadata[b"xpt.version"] == b"8"
    assert schema.metadata[b"sas.table_name"] == b"LONG_DATASET_NAME"
    assert schema.field(0).metadata[b"sas.label"].decode() == label
    back = read_xpt_table(path, batch_rows=1)
    assert back.to_pydict() == table.to_pydict()
    copy = tmp_path / "copy.xpt"
    write_xpt(copy, back, version=8)
    assert path.read_bytes() == copy.read_bytes()
    with pytest.raises(Exception):
        write_xpt(tmp_path / "v5.xpt", back)  # never silently promote or truncate
    pyreadstat = pytest.importorskip("pyreadstat")
    values, meta = pyreadstat.read_xport(path)
    assert meta.column_names == table.column_names
    assert meta.column_names_to_labels == labels
    assert values["same_prefix_second"].tolist() == table.column(1).to_pylist()
    if extended:
        assert meta.original_variable_types["same_prefix_first"] == "LONG_FORMAT_NAME12.2"


@pytest.mark.parametrize("extended", [False, True])
def test_independent_v8_writer(tmp_path, extended):
    pyreadstat = pytest.importorskip("pyreadstat")
    pd = pytest.importorskip("pandas")
    path = tmp_path / "reference.xpt"
    frame = pd.DataFrame({"long_numeric_name": [1.0, -1.0, .5],
                          "long_character_name": ["été", "x" * 250, "tail"]})
    labels = {c: "label " + "é" * 60 for c in frame.columns}
    # All extended entries carry a long format: old ReadStat writers mishandle
    # mixed LABELV9 entries. Our own writer's mixed case is tested separately.
    formats = {"long_numeric_name": "LONG_FORMAT12.2",
               "long_character_name": "$LONG_FORMAT250."} if extended else {}
    pyreadstat.write_xport(frame, str(path), file_format_version=8,
                          table_name="LONG_DATASET_NAME", column_labels=labels,
                          variable_format=formats)
    decoded = read_xpt_table(path)
    assert decoded.to_pydict() == frame.to_dict(orient="list")
    for name, label in labels.items():
        assert decoded.schema.field(name).metadata[b"sas.label"].decode() == label


def test_counted_blanks_streams_and_atomic_count_mismatch(tmp_path):
    path = tmp_path / "blank.xpt"
    table = pa.table({"long_blank_name": ["x", "", ""]})
    write_xpt(path, table, version=8)
    assert read_xpt_table(path).to_pydict() == table.to_pydict()
    before = path.read_bytes()
    for count in [1, 4]:
        with pytest.raises(Exception, match="count"):
            write_xpt(path, table, version=8, row_count=count)
        assert path.read_bytes() == before
    stream = read_xpt(path)
    with pytest.raises(Exception, match="blank"):
        write_xpt(tmp_path / "ambiguous.xpt", stream, version=8)
    write_xpt(tmp_path / "counted.xpt", read_xpt(path), version=8, row_count=3)
    assert read_xpt_table(tmp_path / "counted.xpt").num_rows == 3


def test_all_missing_codes_survive_v8(tmp_path):
    path = tmp_path / "missing.xpt"
    table = pa.table({"long_numeric_name": pa.array([1.25] + [None]*28, type=pa.float64()),
                      "long_numeric_name__missing": pa.array(range(29), type=pa.uint8())})
    write_xpt(path, table, version=8)
    assert read_xpt_table(path, missing="sidecar").to_pydict() == table.to_pydict()
