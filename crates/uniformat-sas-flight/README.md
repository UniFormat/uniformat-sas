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

## Limites et déploiement

Le service refuse les projections répétées et applique des budgets serveur : 4 requêtes simultanées, 1024 colonnes, 8 Mio par lot et 30 secondes par requête. Les payloads sont limités à 64 Kio et le listing est borné. Les limites peuvent être configurées avec `ServiceLimits` côté Rust, jamais via un ticket client.

Une adresse hors loopback exige `--trusted-proxy`. Ce drapeau suppose un réseau privé et un frontal TLS authentifié qui limite aussi la durée des connexions/flux ; il n'active pas TLS dans le serveur. Préférer la variable `UNIFORMAT_FLIGHT_TOKEN` aux arguments de commande et conserver la racine des fichiers en lecture seule pour les utilisateurs non fiables. Voir [les garanties et limites détaillées](../../docs/security.md).
