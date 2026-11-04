"""Resource regressions use generated tables and always run without an external corpus."""
import pyarrow as pa
import pytest
import uniformat_io.sas as sas


def test_complete_table_budget_and_streaming_alternative(tmp_path):
    path = tmp_path / "synthetic.sas7bdat"
    table = pa.table({"x": pa.array(range(10_000), type=pa.float64())})
    sas.write_sas(path, table, page_length=4096)
    with pytest.raises(MemoryError, match="max_table_bytes"):
        sas.read_sas_table(path, max_table_bytes=32_768)
    assert sas.read_sas_table(path, limit=7, max_table_bytes=32_768).num_rows == 7
    assert sum(batch.num_rows for batch in sas.read_sas(path)) == 10_000
    with pytest.raises(ValueError, match="max_table_bytes"):
        sas.read_sas_table(path, max_table_bytes=0)


def test_forged_row_count_does_not_force_preallocation(tmp_path):
    path = tmp_path / "forged.sas7bdat"
    sas.write_sas(path, pa.table({"x": [1.0]}))
    data = bytearray(path.read_bytes())
    assert data[32] == 0x33
    offset = data.index(bytes([0xF7]) * 4) + 48
    assert int.from_bytes(data[offset:offset + 8], "little") == 1
    data[offset:offset + 8] = ((1 << 64) - 1).to_bytes(8, "little")
    path.write_bytes(data)
    assert sas.read_sas_table(path, limit=1).column("x").to_pylist() == [1.0]
