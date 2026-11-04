#!/usr/bin/env python3
"""Benchmark local d'Arrow Flight (J7, critère 4) : débit de `DoGet` sur localhost, 1 puis N endpoints en parallèle, face à la
lecture directe du même fichier par `uniformat_io.sas`. Aucun seuil annoncé avant mesure.

Usage : python benches/bench_flight.py <uniformat-sas-flight-server> <dossier de travail> [lignes=2000000] [repetitions=5]
Le fichier mesuré est écrit par le driver (`write_sas`) dans le dossier de travail.
"""
import json
import os
import statistics
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor

import pyarrow as pa
import pyarrow.flight as fl

import uniformat_io.sas as sas


def main(binary, workdir, rows=2_000_000, reps=5):
    os.makedirs(workdir, exist_ok=True)
    path = os.path.join(workdir, "bench.sas7bdat")
    t = pa.table({
        "id": pa.array(range(rows), pa.float64()),
        "x": pa.array([i * 0.25 for i in range(rows)]),
        "y": pa.array([(i % 1000) / 7 for i in range(rows)]),
        "s": pa.array(["ligne %d" % (i % 997) for i in range(rows)]),
    })
    sas.write_sas(path, t)
    size = os.path.getsize(path)
    proc = subprocess.Popen([binary, workdir, "--addr", "127.0.0.1:0"], stdout=subprocess.PIPE, text=True)
    addr = proc.stdout.readline().split()[1]
    client = fl.connect("grpc://" + addr)

    def info(parts):
        d = fl.FlightDescriptor.for_command(json.dumps({"path": "bench.sas7bdat", "partitions": parts}).encode())
        return client.get_flight_info(d)

    def read_flight(parts):
        i = info(parts)

        def one(ep):
            return fl.connect("grpc://" + addr).do_get(ep.ticket).read_all()

        with ThreadPoolExecutor(max_workers=len(i.endpoints)) as ex:
            return pa.concat_tables(list(ex.map(one, i.endpoints)))

    out = []
    for label, fn in (("lecture directe (uniformat_io.sas)", lambda: sas.read_sas_table(path)),
                      ("Flight, 1 endpoint", lambda: read_flight(1)),
                      ("Flight, 4 endpoints en parallèle", lambda: read_flight(4)),
                      ("Flight, 8 endpoints en parallèle", lambda: read_flight(8))):
        fn()  # chauffe
        times = []
        for _ in range(reps):
            t0 = time.perf_counter()
            tab = fn()
            times.append(time.perf_counter() - t0)
        best, med = min(times), statistics.median(times)
        out.append((label, tab.num_rows, tab.nbytes, best, med))
    t0 = time.perf_counter()
    for _ in range(50):
        info(4)
    schema_ms = (time.perf_counter() - t0) / 50 * 1000
    proc.terminate()
    proc.wait(timeout=10)
    print(f"fichier : {rows} lignes, {size / 1e6:.1f} Mo ; GetFlightInfo (schéma + 4 endpoints) : {schema_ms:.2f} ms")
    print(f"{'lecture':38} {'meilleur (s)':>13} {'médiane (s)':>12} {'M lignes/s':>11} {'Mo Arrow/s':>11}")
    for label, n, nb, best, med in out:
        print(f"{label:38} {best:13.3f} {med:12.3f} {n / best / 1e6:11.2f} {nb / best / 1e6:11.0f}")


if __name__ == "__main__":
    a = sys.argv
    main(a[1], a[2], int(a[3]) if len(a) > 3 else 2_000_000, int(a[4]) if len(a) > 4 else 5)
