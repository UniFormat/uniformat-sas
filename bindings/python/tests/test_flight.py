"""J7 : un client `pyarrow.flight` obtient le schéma sans transfert de données et lit les endpoints en parallèle.

Lance le serveur `uniformat-sas-flight-server` (variable `UNIFORMAT_FLIGHT_BIN`, binaire construit par `cargo build -p
uniformat-sas-flight`) sur le corpus ; sauté sans binaire ou sans corpus.
"""

from __future__ import annotations

import json
import os
import subprocess
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import pytest

flight = pytest.importorskip("pyarrow.flight")
import pyarrow as pa  # noqa: E402

import uniformat_io.sas as sas  # noqa: E402


@pytest.fixture(scope="module")
def server(corpus):
    binary = os.environ.get("UNIFORMAT_FLIGHT_BIN")
    if not binary or not Path(binary).is_file():
        pytest.skip("UNIFORMAT_FLIGHT_BIN absent : test sauté")
    proc = subprocess.Popen([binary, str(corpus / "files"), "--addr", "127.0.0.1:0"], stdout=subprocess.PIPE, text=True)
    line = proc.stdout.readline().strip()
    assert line.startswith("listening "), line
    addr = line.split()[1]
    client = flight.connect("grpc://" + addr)
    yield client, corpus / "files", addr
    client.close()
    proc.terminate()
    proc.wait(timeout=10)


def command(path, **kw):
    return flight.FlightDescriptor.for_command(json.dumps({"path": path, **kw}).encode())


def stats(client):
    (r,) = list(client.do_action(flight.Action("stats", b"")))
    return json.loads(r.body.to_pybytes())


def test_schema_without_data_then_parallel_endpoints_equal_the_direct_read(server):
    client, root, addr = server
    for rel, parts in (("taille/taille_n50000_no.sas7bdat", 4), ("compression/compression_pat2_char.sas7bdat", 4)):
        before = stats(client)
        info = client.get_flight_info(command(rel, partitions=parts))
        after = stats(client)
        assert after["data_bytes"] == before["data_bytes"], "aucun octet de données pour le schéma"
        size = (root / rel).stat().st_size
        assert after["info_bytes"] - before["info_bytes"] < size, "le schéma ne lit pas tout le fichier"
        direct = sas.read_sas_table(root / rel)
        assert info.schema.names == direct.schema.names
        assert [f.type for f in info.schema] == [f.type for f in direct.schema]
        assert info.total_records == direct.num_rows
        expected = 1 if "compression" in rel else parts
        assert len(info.endpoints) == expected

        def read(ep):
            c = flight.connect("grpc://" + addr)
            return c.do_get(ep.ticket).read_all()

        with ThreadPoolExecutor(max_workers=len(info.endpoints)) as ex:
            pieces = list(ex.map(read, info.endpoints))
        got = pa.concat_tables(pieces)
        assert got.num_rows == direct.num_rows
        for name in direct.schema.names:
            assert got.column(name).to_pylist() == direct.column(name).to_pylist(), (rel, name)


def test_projection_limit_missing_codes_and_errors(server):
    client, _, _ = server
    info = client.get_flight_info(command("taille/taille_n50000_no.sas7bdat", limit=7, columns=["i"]))
    t = client.do_get(info.endpoints[0].ticket).read_all()
    assert t.num_rows == 7 and t.schema.names == ["i"]
    info = client.get_flight_info(command("valeurs/valeurs_manquants_no.sas7bdat", missing="sidecar", partitions=1))
    t = client.do_get(info.endpoints[0].ticket).read_all()
    assert t.column("x__missing").to_pylist() == list(range(1, 29))
    with pytest.raises((flight.FlightError, pa.ArrowException)):
        client.get_flight_info(command("../MANIFEST.sas7bdat"))
    with pytest.raises((flight.FlightError, pa.ArrowException)):
        client.do_get(flight.Ticket(b"{}")).read_all()
    listing = list(client.list_flights())
    assert len(listing) >= 190 and all(len(i.endpoints) == 0 for i in listing)
