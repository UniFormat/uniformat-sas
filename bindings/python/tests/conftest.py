"""Corpus de référence : les tests d'intégration sont sautés sans ``UNIFORMAT_CORPUS`` (comme les tests Rust)."""

from __future__ import annotations

import os
from pathlib import Path

import pytest


def _root() -> Path | None:
    env = os.environ.get("UNIFORMAT_CORPUS") or os.environ.get("SAS7BDAT_CORPUS")
    if not env:
        return None
    p = Path(env)
    return p if p.is_dir() else None


@pytest.fixture(scope="session")
def corpus() -> Path:
    root = _root()
    if root is None:
        pytest.skip("UNIFORMAT_CORPUS absent : test sauté")
    return root


def corpus_files(root: Path):
    """Les 232 fichiers de ``files/`` et ``external/`` (même découpage que ``uniformat-testkit``) et leur CSV."""
    out = []
    for top in ("files", "external"):
        base = root / top
        if not base.is_dir():
            continue
        for path in sorted(base.rglob("*.sas7bdat")):
            csv = path.with_suffix(".csv")
            out.append((path, csv if csv.is_file() else None))
    # les fichiers publics ``datareader/testN`` partagent le CSV de leur jeu de données (EXTERNAL.tsv, colonne note)
    tsv = root / "external" / "EXTERNAL.tsv"
    if tsv.is_file():
        shared = {}
        for line in tsv.read_text(encoding="utf-8").splitlines()[1:]:
            cols = line.split("\t")
            if len(cols) > 8:
                for part in cols[8].split("(")[1:]:
                    name = part.split(")")[0]
                    if name.endswith(".csv"):
                        shared[root / cols[0]] = name
        out = [
            (p, (p.with_name(shared[p]) if p in shared and p.with_name(shared[p]).is_file() else None) if p in shared else c)
            for p, c in out
        ]
    return out
