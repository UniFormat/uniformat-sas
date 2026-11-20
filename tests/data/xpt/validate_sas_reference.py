"""Validate an external SAS-produced corpus without running its supplied programs.

Usage: python tests/data/xpt/validate_sas_reference.py /path/sas-xpt-v5-reference
Requires the installed uniformat-sas Python binding. The corpus stays outside git.
"""
from __future__ import annotations

import csv
from decimal import Decimal
import hashlib
import json
from pathlib import Path
import struct
import sys
import tempfile

import pyarrow as pa
from uniformat_io.sas import read_xpt_table, write_xpt


def csv_rows(path):
    with path.open(encoding="utf-8", newline="") as stream:
        return list(csv.DictReader(stream))


def verify_manifest(root):
    count = 0
    for line in (root / "SHA256SUMS").read_text(encoding="utf-8").splitlines():
        digest, name = line.split(maxsplit=1)
        path = (root / name.lstrip("*")).resolve()
        if root not in path.parents:
            raise ValueError("manifest path outside corpus")
        if hashlib.sha256(path.read_bytes()).hexdigest() != digest:
            raise ValueError(f"checksum mismatch: {name}")
        count += 1
    return count


def check(condition, context):
    if not condition:
        raise AssertionError(context)


def missing_code(value):
    return 1 if value == "." else 2 if value == "._" else ord(value[1]) - ord("A") + 3


def matches_cell(table, name, index, reference, encoding):
    value = table[name][index].as_py()
    if name + "__hex16" in reference:
        expected = reference[name + "__hex16"]
        code = table[name + "__missing"][index].as_py()
        if expected.startswith("."):
            return value is None and code == missing_code(expected)
        return value is not None and code == 0 and struct.pack(">d", value).hex().upper() == expected
    # Compare BOTH the session bytes and the independently transcoded CSV text.
    return (value == reference[name] and
            value.encode(encoding) == bytes.fromhex(reference[name + "__hex"]))


def check_schema(table, descriptors, member):
    names = [d["NAME"] for d in descriptors]
    actual = [f.name for f in table.schema if not f.name.endswith("__missing")]
    check(actual == names, f"{member['MEMNAME']}: column order/names")
    check(table.schema.metadata[b"sas.table_name"].decode() == member["MEMNAME"], "member name")
    check(table.schema.metadata[b"sas.table_label"].decode() == member["MEMLABEL"], "member label")
    for d in descriptors:
        field = table.schema.field(d["NAME"])
        check(pa.types.is_float64(field.type) if d["TYPE"] == "1" else pa.types.is_string(field.type),
              f"{d['NAME']}: storage type")
        metadata = {k.decode(): v.decode() for k, v in field.metadata.items()}
        for key, ref_key, default in [
            ("sas.length", "LENGTH", "0"), ("sas.label", "LABEL", ""),
            ("sas.format", "FORMAT", ""), ("sas.format_width", "FORMATL", "0"),
            ("sas.format_decimals", "FORMATD", "0"), ("sas.informat", "INFORMAT", ""),
            ("sas.informat_width", "INFORML", "0"), ("sas.informat_decimals", "INFORMD", "0"),
        ]:
            check(metadata.get(key, default) == d[ref_key], f"{d['NAME']}: {key}")


def validate_native_dates(root):
    table = read_xpt_table(root / "xpt/dates-v5.xpt", time="duration", missing="sidecar")
    rows = csv_rows(root / "reference/reread/dates-v5.DATES.obs.csv")
    checked = 0
    for name, offset, factor, target in [
        ("DATE", 3653, 1, pa.int32()),
        ("DATETIME", 315619200, 1000000, pa.int64()),
        ("TIME", 0, 1000000, pa.int64()),
    ]:
        values = table[name].cast(target).to_pylist()
        for i, row in enumerate(rows):
            raw = row[name + "__hex16"]
            if raw.startswith("."):
                check(values[i] is None, f"{name}: missing temporal value")
            else:
                number = Decimal.from_float(struct.unpack(">d", bytes.fromhex(raw))[0])
                check(values[i] == int((number - offset) * factor), f"{name}: native temporal value {i}")
            checked += 1
    return checked


def validate(root):
    root = Path(root).resolve()
    hashes = verify_manifest(root)
    descriptors = csv_rows(root / "checks/xpt_namestr.csv")
    results = []
    cells = 0
    numeric = 0
    missing = 0
    tags = set()
    known_source_changes = {
        "blank-col12-v5", "blank-col12-aligned-v5", "limits-truncation-v5", "numeric-range-v5"
    }
    with tempfile.TemporaryDirectory(prefix="uniformat-xpt-validation-") as directory:
        for path in sorted((root / "xpt").glob("*.xpt")):
            # This SAS LATIN1 session writes 0x80 for EUR. Windows-1252 explicitly
            # reproduces its CSV text; true ISO Latin-1 is checked separately below.
            encoding = "windows-1252" if path.stem in ("enc-latin1-v5", "enc-wlatin1-v5") else "utf-8"
            members = csv_rows(root / "reference/reread" / f"{path.stem}.members.csv")
            for index, member in enumerate(members):
                name = member["MEMNAME"]
                table = read_xpt_table(path, member_index=index, encoding=encoding,
                                       dates="float", missing="sidecar", trailing_blanks="keep", batch_rows=3)
                expected = csv_rows(root / "reference/reread" / f"{path.stem}.{name}.obs.csv")
                columns = [d for d in descriptors if d["FILE"] == path.name and d["MEMBER"] == name]
                check(len(columns) == int(member["NVAR"]), f"{path.name}: descriptor count")
                check(table.num_rows == len(expected) == int(member["NOBS"]), f"{path.name}: row count")
                check_schema(table, columns, member)
                for i, row in enumerate(expected):
                    for column in columns:
                        col = column["NAME"]
                        check(matches_cell(table, col, i, row, encoding), f"{path.name}/{name}/{i + 1}/{col}: SAS value mismatch")
                        cells += 1
                        if column["TYPE"] == "1":
                            code = table[col + "__missing"][i].as_py()
                            numeric += code == 0
                            missing += code != 0
                            tags.add(code)
                source = csv_rows(root / "reference/source" / f"{path.stem}.{name}.obs.csv")
                differences = abs(len(source) - table.num_rows)
                for i, row in enumerate(source[:table.num_rows]):
                    for column in columns:
                        col = column["NAME"]
                        source_name = next(k for k in row if k.upper() == col.upper())
                        mapped = {col + k[len(source_name):]: v for k, v in row.items()
                                  if k == source_name or k.startswith(source_name + "__")}
                        differences += not matches_cell(table, col, i, mapped, encoding)
                check(not differences or path.stem in known_source_changes,
                      f"{path.name}: unexpected divergence from source WORK table")
                destination = Path(directory) / f"{path.stem}-{name}.xpt"
                write_xpt(destination, table, encoding=encoding)
                back = read_xpt_table(destination, encoding=encoding, dates="float",
                                      missing="sidecar", trailing_blanks="keep")
                check(table.equals(back, check_metadata=True), f"{path.name}/{name}: Arrow round trip")
                results.append({"file": path.name, "member": name, "rows": table.num_rows,
                                "columns": len(columns), "source_differences": differences})
    check(len(results) == 15, "incomplete reference corpus: expected 15 members")
    check(tags == set(range(29)), "missing coverage for a SAS missing code")
    # Preserve the user-approved distinction between ISO-8859-1 and Windows-1252.
    latin = root / "xpt/enc-latin1-v5.xpt"
    iso = read_xpt_table(latin, encoding="latin1")
    windows = read_xpt_table(latin, encoding="windows-1252")
    check(iso["TEXT"][1].as_py() == "\u0080", "true Latin-1 0x80")
    check(windows["TEXT"][1].as_py() == "€", "Windows-1252 0x80")
    return {"status": "passed", "manifest_files": hashes,
            "files": len({r["file"] for r in results}), "members": len(results),
            "rows": sum(r["rows"] for r in results), "cells": cells,
            "numeric_values_bit_exact": numeric, "missing_cells": missing, "missing_codes": sorted(tags),
            "native_temporal_cells": validate_native_dates(root),
            "encoding_note": "SAS LATIN1 0x80 uses explicit windows-1252 for CSV agreement; ISO Latin-1 remains distinct",
            "results": results}


if __name__ == "__main__":
    print(json.dumps(validate(sys.argv[1]), ensure_ascii=False, indent=2))
