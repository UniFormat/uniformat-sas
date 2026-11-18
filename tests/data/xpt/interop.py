"""Independent XPT checks: pip install pandas pyreadstat; python interop.py [reference.xpt].
The optional reference output is produced by ReadStat, never added to the repository.
"""
from pathlib import Path
import sys
import pandas as pd
import pyreadstat

fixture = Path(__file__).with_name("v5-synthetic.xpt")
expected = pd.DataFrame({"VALUE": [1.0, -1.0, 0.5, 100.0, float("nan")],
                         "TEXT": ["one", "moins", "été", "cent", "missing"]})
actual = pd.read_sas(fixture, format="xport", encoding="utf-8")
pd.testing.assert_frame_equal(actual, expected)
actual, metadata = pyreadstat.read_xport(str(fixture), encoding="UTF-8")
pd.testing.assert_frame_equal(actual, expected)
assert metadata.column_labels == ["Number", "Text"]
assert metadata.table_name == "DATASET"
# Regression: an empty final character field must not consume a numeric row.
blank_tail = fixture.with_name("v5-blank-tail.xpt")
blank_expected = expected.copy()
blank_expected.loc[4, "TEXT"] = ""
actual, _ = pyreadstat.read_xport(str(blank_tail), encoding="UTF-8")
pd.testing.assert_frame_equal(actual, blank_expected)
if len(sys.argv) > 1:
    pyreadstat.write_xport(expected, sys.argv[1], file_format_version=5,
                          table_name="REF", column_labels=["Number", "Text"])
print(f"pandas {pd.__version__} and pyreadstat {pyreadstat.__version__}: schema, values, labels OK")
