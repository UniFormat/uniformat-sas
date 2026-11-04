#!/usr/bin/env python3
"""Mesures de référence (docs/architecture.md §6.5) : pyreadstat et pandas sur les mêmes fichiers que
crates/uniformat-sas-arrow/examples/bench.rs. Même format de sortie TSV.

Usage : <venv>/bin/python benches/bench_py.py <corpus> [répétitions]
"""
import os
import statistics
import sys
import time
import warnings

import pandas
import pyreadstat

warnings.simplefilter("ignore")
ROOT = sys.argv[1]
REPS = int(sys.argv[2]) if len(sys.argv) > 2 else 20
FILES = [
    "files/taille/taille_n50000_no.sas7bdat",
    "files/taille/taille_n50000_char.sas7bdat",
    "files/taille/taille_n50000_binary.sas7bdat",
    "files/base/base_n3000_mix_no.sas7bdat",
    "files/taille/taille_wide300_no.sas7bdat",
    "files/taille/taille_wide5000_no.sas7bdat",
    "files/compression/compression_pat4_char.sas7bdat",
    "files/compression/compression_pat2_binary.sas7bdat",
]


def bench(case, name, size, fn):
    rows = fn()
    times = []
    for _ in range(REPS):
        t = time.perf_counter()
        n = fn()
        times.append(time.perf_counter() - t)
        assert n == rows
    med = statistics.median(times)
    print(f"{case}\t{name}\t{rows}\t{size}\t{med * 1e3:.2f}\t{size / 1048576 / med:.1f}\t{rows / med:.0f}")


print("cas\tfichier\tlignes\toctets\tmediane_ms\tMio_s\tlignes_s")
for rel in FILES:
    path = os.path.join(ROOT, rel)
    if not os.path.exists(path):
        continue
    name = os.path.basename(path)
    size = os.path.getsize(path)
    _, meta = pyreadstat.read_sas7bdat(path)
    cols = meta.column_names
    enc = meta.file_encoding or "latin1"
    bench("pyreadstat_complet", name, size, lambda: len(pyreadstat.read_sas7bdat(path)[0]))
    bench("pyreadstat_dict", name, size,
          lambda: len(next(iter(pyreadstat.read_sas7bdat(path, output_format="dict")[0].values()))))
    if len(cols) >= 2:
        bench("pyreadstat_projection2", name, size,
              lambda: len(pyreadstat.read_sas7bdat(path, usecols=cols[:2])[0]))
    bench("pyreadstat_limit1000", name, size,
          lambda: len(pyreadstat.read_sas7bdat(path, row_limit=1000)[0]))
    try:
        bench("pandas_complet", name, size, lambda: len(pandas.read_sas(path, format="sas7bdat", encoding=enc)))
    except Exception as e:  # pandas ne lit pas tous les fichiers
        print(f"pandas_complet\t{name}\terreur\t{size}\t-\t-\t- ({type(e).__name__})")
