# uniformat-sas-flight

Serveur Arrow Flight (lecture seule) sur un dossier de fichiers `.sas7bdat`.

```bash
uniformat-sas-flight-server /donnees --addr 127.0.0.1:50051 [--partitions 8] [--token secret]
```

```python
import json, pyarrow.flight as fl
c = fl.connect("grpc://127.0.0.1:50051")
info = c.get_flight_info(fl.FlightDescriptor.for_command(json.dumps(
    {"path": "sous/dossier/t.sas7bdat", "partitions": 4, "columns": ["x"], "missing": "sidecar"}).encode()))
info.schema                                   # schéma Arrow, aucune page de données lue
tables = [c.do_get(ep.ticket).read_all() for ep in info.endpoints]   # à lire en parallèle
```

Descripteur `PATH` (`["sous/dossier/t.sas7bdat"]`) ou `CMD` JSON : `path`, `partitions`, `columns`, `limit` (un seul endpoint),
`missing` (`null` | `sidecar`), `dates` (`native` | `float`), `encoding`, `trailing_blanks`, `plan`, `batch_rows`.
Une table compressée n'a qu'un endpoint. `list_flights` énumère les fichiers ; l'action `stats` rend les octets lus.

Sécurité : les chemins sont confinés au dossier racine et les tickets revalidés ; le jeton porteur circule en clair (pas de
TLS) : placer le service derrière un proxy TLS. Service **non publié** (décision NULL, ADR 0007).
