#!/usr/bin/env python3
"""Génère les tables embarquées du cœur à partir de la spécification (jalon J3).

Usage : python3 -I ci/gen_tables.py [--check]
  sans option : réécrit crates/uniformat-sas/src/format_table.rs et encoding_table.rs ;
  --check     : échoue (code 1) si les fichiers présents diffèrent de ce que la spécification produit.

Sources : docs/spec/30_formats_encodages.md (sections 1, 1.1 et 6.1) et
tests/data/encodages_natifs.tsv (section 6.3). Ne pas éditer les fichiers générés.
"""
import csv
import os
import re
import sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
SPEC = os.path.join(ROOT, "docs", "spec", "30_formats_encodages.md")
NATIFS = os.path.join(ROOT, "tests", "data", "encodages_natifs.tsv")
OUT_FORMATS = os.path.join(ROOT, "crates", "uniformat-sas", "src", "format_table.rs")
OUT_ENCODINGS = os.path.join(ROOT, "crates", "uniformat-sas", "src", "encoding_table.rs")


def cells(line):
    return [c.strip() for c in line.strip().strip("|").split("|")]


def logical_class(category):
    c = category.lower()
    if c.startswith("numérique ou texte") or c.startswith("texte ("):
        return "ByStorage"
    if c.startswith("datetime"):
        return "DateTime"
    if c.startswith("date"):
        return "Date"
    if c.startswith("heure"):
        return "Time"
    if c.startswith("numérique"):
        return "Number"
    if c.startswith("caractère"):
        return "Text"
    raise SystemExit(f"catégorie inconnue : {category}")


def format_entries():
    mode = None
    out = {}
    for line in open(SPEC, encoding="utf-8").read().splitlines():
        if line.startswith("## 1. Table"):
            mode = "1"
        elif line.startswith("### 1.1"):
            mode = "11"
        elif line.startswith("## 2."):
            mode = None
        if mode and line.startswith("| `"):
            c = cells(line)
            name = c[0].strip("`").upper()
            sense = c[1]
            category = c[2] if mode == "1" else c[3]
            if sense not in ("format", "informat"):
                raise SystemExit(f"sens inconnu : {line[:80]}")
            key = (name, sense)
            cls = logical_class(category)
            if key in out and out[key] != cls:
                raise SystemExit(f"classes contradictoires pour {key}")
            out[key] = cls
    return sorted(out.items(), key=lambda kv: (kv[0][0], kv[0][1]))


def encoding_entries():
    whatwg = {}
    for line in open(SPEC, encoding="utf-8").read().splitlines():
        if re.match(r"\| \d+ \|", line):
            c = cells(line)
            m = re.match(r"`([^`]+)`", c[5])
            whatwg[int(c[0])] = m.group(1) if m else None
    rows = list(csv.DictReader(open(NATIFS, encoding="utf-8"), delimiter="\t"))
    out = []
    for r in rows:
        if not r["nom_sas"]:
            continue
        code = int(r["code"])
        blank = int(r["octet_blanc"], 16)
        out.append((code, r["nom_sas"], r["nom_iconv_std"], whatwg.get(code), blank))
    # Emplacements sans nom dans le binaire mais présents en 6.1 (code 0 : « encodage non déclaré », repli windows-1252).
    by_code = {int(r["code"]): r for r in rows}
    for code in sorted(set(whatwg) - {e[0] for e in out}):
        r = by_code.get(code)
        if r is None:
            raise SystemExit(f"code de 6.1 absent du binaire : {code}")
        out.append((code, "", "", whatwg[code], int(r["octet_blanc"], 16)))
    return sorted(out), whatwg


def rust_str(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def render_formats():
    ents = format_entries()
    lines = [
        "// @generated par ci/gen_tables.py d'après docs/spec/30_formats_encodages.md (sections 1 et 1.1) : ne pas éditer.",
        "//! Table de classification des formats et informats natifs (`30` §1 et §1.1).",
        "",
        "use crate::formats::{Class, Sense};",
        "",
        "/// Entrées triées par (nom, sens) pour la recherche dichotomique.",
        "#[rustfmt::skip]",
        "pub(crate) const FORMAT_TABLE: &[(&str, Sense, Class)] = &[",
    ]
    for (name, sense), cls in ents:
        s = "Format" if sense == "format" else "Informat"
        lines.append(f"    ({rust_str(name)}, Sense::{s}, Class::{cls}),")
    lines.append("];")
    return "\n".join(lines) + "\n"


def render_encodings():
    ents, _ = encoding_entries()
    lines = [
        "// @generated par ci/gen_tables.py d'après docs/spec/30_formats_encodages.md (6.1) et encodages_natifs.tsv (6.3) : ne pas éditer.",
        "//! Table native des encodages SAS indexée par le code de l'octet 70 (`30` §6).",
        "",
        "use crate::encoding::EncodingInfo;",
        "",
        "/// Codes nommés, triés par code. `whatwg` : label `encoding_rs` quand il existe un équivalent.",
        "#[rustfmt::skip]",
        "pub(crate) const ENCODING_TABLE: &[EncodingInfo] = &[",
    ]
    for code, sas, iconv, whatwg, blank in ents:
        w = f"Some({rust_str(whatwg)})" if whatwg else "None"
        lines.append(
            f"    EncodingInfo {{ code: {code}, sas_name: {rust_str(sas)}, iconv_name: {rust_str(iconv)}, whatwg: {w}, blank: {blank:#04x} }},"
        )
    lines.append("];")
    return "\n".join(lines) + "\n"


def main(argv):
    targets = [(OUT_FORMATS, render_formats()), (OUT_ENCODINGS, render_encodings())]
    bad = 0
    for path, text in targets:
        if "--check" in argv:
            current = open(path, encoding="utf-8").read() if os.path.exists(path) else None
            if current != text:
                print(f"ECART : {os.path.relpath(path, ROOT)} n'est pas à jour (lancer python3 -I ci/gen_tables.py)")
                bad += 1
        else:
            open(path, "w", encoding="utf-8").write(text)
            print(f"écrit {os.path.relpath(path, ROOT)} ({text.count(chr(10))} lignes)")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
