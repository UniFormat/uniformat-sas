# Mesures de lecture — premiers chiffres (J4)

> Mesures du 2026-11-04. **Ce sont des premiers chiffres sur de petits fichiers du corpus, pas un banc de performance.** Les fichiers font de 0,2 à 1,3 Mio, restent dans le cache du système, et la lecture complète d'un fichier prend moins de 30 ms ; ils ne disent rien du comportement sur des fichiers de plusieurs Gio. Aucun seuil n'est annoncé.

## Méthode

- Machine : Apple M1, 8 cœurs, 16 Gio ; macOS (Darwin 25.2) ; Rust 1.99.1 (profil `release`, `lto = "thin"`) ; Python 3.9 avec `pyreadstat` 1.3.7 et `pandas` 3.1.0 (environnement isolé, hors dépôt).
- Pilote Rust : `cargo run --release -p uniformat-sas-arrow --example bench -- corpus 30` (30 répétitions, médiane, après un tour d'échauffement). Pilote Python : `benches/bench_py.py corpus 20`.
- Le driver produit des `RecordBatch` Arrow en mémoire ; `pyreadstat` et `pandas` produisent des `DataFrame` pandas. **Le travail n'est pas le même** : la comparaison dit combien coûte « lire le fichier jusqu'à un objet exploitable » dans chaque outil, pas l'écart entre deux décodeurs à fonctionnalité égale. Le mode `output_format="dict"` de `pyreadstat` (sans pandas) donne 26 ms sur `taille_n50000_no`, soit le même ordre que `DataFrame`.
- « driver complet » est mesuré dans un fil dédié : sur le fil principal de macOS la même lecture coûte 2,5 ms au lieu de 1,6 ms (effet de l'allocateur, non élucidé) ; le chiffre du fil dédié est celui d'un utilisateur multi-fils. La colonne « 4 fils » lit quatre groupes de pages en parallèle (plan rapide).

## Lecture complète

| fichier | lignes | driver complet (fil dédié) | driver 4 fils | pyreadstat | pandas | rapport driver/pyreadstat |
|---|---:|---:|---:|---:|---:|---:|
| `taille_n50000_no.sas7bdat` | 50000 | 1.60 ms (31.17 M lignes/s) | 0.50 ms (100.03 M lignes/s) | 28.03 ms (1.78 M lignes/s) | 27.01 ms (1.85 M lignes/s) | 18× |
| `taille_n50000_char.sas7bdat` | 50000 | 1.56 ms (32.01 M lignes/s) | 0.50 ms (99.35 M lignes/s) | 28.52 ms (1.75 M lignes/s) | 28.49 ms (1.76 M lignes/s) | 18× |
| `taille_n50000_binary.sas7bdat` | 50000 | 1.56 ms (32.03 M lignes/s) | 0.50 ms (99.32 M lignes/s) | 27.55 ms (1.82 M lignes/s) | 26.02 ms (1.92 M lignes/s) | 18× |
| `base_n3000_mix_no.sas7bdat` | 3000 | 0.22 ms (13.76 M lignes/s) | 0.18 ms (16.94 M lignes/s) | 17.27 ms (0.17 M lignes/s) | 19.12 ms (0.16 M lignes/s) | 78× |
| `taille_wide300_no.sas7bdat` | 3 | 0.49 ms (0.01 M lignes/s) | 0.43 ms (0.01 M lignes/s) | 21.31 ms (0.00 M lignes/s) | 24.89 ms (0.00 M lignes/s) | 43× |
| `taille_wide5000_no.sas7bdat` | 3 | 6.70 ms (0.00 M lignes/s) | 16.82 ms (0.00 M lignes/s) | 115.91 ms (0.00 M lignes/s) | 124.94 ms (0.00 M lignes/s) | 17× |
| `compression_pat4_char.sas7bdat` | 200 | 0.14 ms (1.44 M lignes/s) | — | 15.64 ms (0.01 M lignes/s) | 18.59 ms (0.01 M lignes/s) | 112× |
| `compression_pat2_binary.sas7bdat` | 200 | 0.10 ms (1.98 M lignes/s) | — | 16.47 ms (0.01 M lignes/s) | 18.87 ms (0.01 M lignes/s) | 165× |

Rapport = durée `pyreadstat` / durée du driver (un fil). Pour `taille_wide5000`, la durée du driver est dominée par la création de 5 001 constructeurs de colonnes pour 3 lignes ; la lecture parallèle y est plus lente que la séquentielle (surcoût de 4 assembleurs de 5 001 colonnes).

## Autres cas (driver, un fil dédié sauf mention)

| cas | `taille_n50000_no` | `base_n3000_mix_no` |
|---|---:|---:|
| projection de 2 colonnes | 1,30 ms (38,4 M lignes/s) | 0,14 ms |
| `LIMIT 1000` | 0,08 ms | 0,08 ms |
| 1 partition (`with_group`) | 1,56 ms | — |
| 4 partitions | 0,50 ms (99,6 M lignes/s) | 0,18 ms |
| 8 partitions | 0,54 ms | — |

`pyreadstat` : projection de 2 colonnes 24,5 ms, `LIMIT 1000` (`row_limit`) 16,7 ms sur `taille_n50000_no` — soit ≈ 19× et ≈ 200× plus lent que le driver. `pyreadstat` a un coût fixe d'environ 15 ms par appel (même pour 200 lignes), qui explique les rapports élevés sur les petits fichiers.

## Ce que ces mesures ne couvrent pas

- Pas de fichier de plus de 1,3 Mio : les débits de plusieurs Gio/s sur 4 fils viennent du cache et de fichiers qui tiennent en quelques pages.
- Les fichiers compressés du corpus ont 200 lignes : le débit RLE/RDC n'est pas mesuré à l'échelle.
- Pas de mesure `object_store` distant (S3), pas de DataFusion (SQL), pas de `criterion` (statistiques, comparaison entre versions) : `docs/architecture.md` §6.5 décrit le banc cible.
- `pandas.read_sas` n'a pas été mesuré en mode `chunksize` ni avec l'accélérateur C optionnel.
- Objectif de J4 (« au moins égal à `pyreadstat` en lecture complète sur le même corpus ») : **atteint** sur tous les fichiers mesurés, avec les réserves ci-dessus.

## Arrow Flight en local (J7, 2026-11-04)

Script : `benches/bench_flight.py`. Table de 2 000 000 lignes (trois `double`, un texte de 11 à 13 caractères) écrite par le driver (80,4 Mo), serveur `uniformat-sas-flight-server` (release) et clients `pyarrow.flight` sur localhost, une machine (macOS arm64), meilleur de 5 répétitions après une lecture de chauffe. « Mo Arrow/s » = taille mémoire de la table Arrow obtenue.

| lecture | meilleur (s) | M lignes/s | Mo Arrow/s |
|---|---|---|---|
| lecture directe (`uniformat_io.sas.read_sas_table`) | 0,096 | 20,7 | 765 |
| Flight, 1 endpoint | 0,100 | 20,0 | 737 |
| Flight, 4 endpoints en parallèle | 0,065 | 30,9 | 1 142 |
| Flight, 8 endpoints en parallèle | 0,063 | 31,8 | 1 174 |

`GetFlightInfo` (schéma et 4 endpoints) : 0,24 ms. Réserves : localhost (pas de réseau), un seul client, fichier en cache, un seul fichier, pas de TLS ; le surcoût du transport Flight en un endpoint est de l'ordre de 4 % sur ce fichier. Aucun seuil n'avait été annoncé.

