"""Optional conformance against externally supplied SAS XPORT references."""
import json
import os
from pathlib import Path
import subprocess
import sys

import pytest


def test_sas_produced_xpt_reference():
    root = os.environ.get("UNIFORMAT_XPT_SAS_CORPUS")
    if not root:
        pytest.skip("set UNIFORMAT_XPT_SAS_CORPUS to the external SAS reference directory")
    script = Path(__file__).resolve().parents[3] / "tests/data/xpt/validate_sas_reference.py"
    result = subprocess.run([sys.executable, str(script), root], text=True, capture_output=True)
    assert result.returncode == 0, result.stdout + result.stderr
    report = json.loads(result.stdout)
    assert (report["files"], report["members"], report["rows"]) == (14, 15, 116)
    assert report["missing_codes"] == list(range(29))
