## Why

Une part importante des `.sas7bdat` produits en pratique est compressée (`COMPRESS=CHAR` ou `COMPRESS=BINARY`). Sans spécification complète de ces deux compressions, **en décompression comme en compression**, le driver Rust ne pourrait ni lire ces fichiers ni en écrire que SAS accepte. Les descriptions publiques (ReadStat, Parso, pandas, PDF de Shotwell) ne sont que des hypothèses : il faut les confirmer ou les infirmer sur le corpus et, pour les seuls manques, sur les modules SAS `sasyzcrl` / `sasyzcr2`.

## What Changes

- Vérifier, avec preuve (chaîne dans le fichier, symbole, appel), les liens supposés `COMPRESS=CHAR` ↔ `SASYZCRL` ↔ `src/sasexe/sasyzcrl` ↔ RLE et `COMPRESS=BINARY` ↔ `SASYZCR2` ↔ `src/sasexe/sasyzcr2` ↔ RDC (Ross Data Compression).
- Établir, avec le corpus `NO` / `CHAR` / `BINARY`, ce qui est compressé (ligne, page), comment une ligne compressée est repérée (pointeurs de sous-en-têtes, indicateurs) et le traitement des lignes que la compression n'améliore pas.
- Décrire chaque algorithme de décompression sous forme de table d'opcodes (RLE : chaque commande ; RDC : groupes de contrôle et références arrière), en ne décompilant que les opcodes que les fichiers n'expliquent pas.
- Décrire les règles de décision du chemin d'écriture (seuils de longueur, fenêtre RDC, choix ligne brute / compressée) en séparant ce qui est **obligatoire** pour que SAS relise de ce qui est un **choix libre**.
- Produire des vecteurs de test (paires ligne brute / ligne compressée extraites du corpus + cas limites) sous `rea_output/tools/compression_vectors/`.
- Étendre `rea_output/tools/ref_reader.py` avec un décompresseur écrit depuis la spec et écrire un compresseur de référence `rea_output/tools/ref_compress.py`.
- Livrable principal : `rea_output/15_compression.md` ; mise à jour de `rea_output/60_inconnues.md`, `rea_output/evidence/` et `rea_output/STATE.md`.
- Hors périmètre : écrivain `.sas7bdat` complet et vérification que SAS relit les lignes produites par `ref_compress.py` (reportées à `specify-sas7bdat-writing`), formats et encodages (`specify-sas-formats-encodings`), conception et code Rust (`design-rust-driver`), toute modification de `src/`.

## Capabilities

### New Capabilities
- `sas7bdat-compression` : spécification vérifiée des compressions de lignes `CHAR` (RLE) et `BINARY` (RDC) des `.sas7bdat` — détection, opcodes de décompression, règles de compression (obligatoire / libre), vecteurs de test et outils de référence Python.

### Modified Capabilities
<!-- Aucune : aucune spec existante sous openspec/specs/. -->

## Impact

- **Dépendances** : `specify-sas7bdat-read-format` (02) doit être terminé — `rea_output/10_format_sas7bdat.md`, `ref_reader.py` ; `setup-sas-oracle-corpus` (01) fournit l'oracle SAS et `corpus/` (variantes `NO` / `CHAR` / `BINARY`, `corpus/MANIFEST.md`) ; `survey-sas7bdat-prior-art` (00) délimite ce qui est déjà publié.
- **Parallélisme** : indépendant de `specify-sas-formats-encodings` (04), peut tourner en parallèle (session séparée).
- **Débloque** : `specify-sas7bdat-writing` (05, écriture de fichiers compressés et vérification SAS de `ref_compress.py`) et `design-rust-driver` (06, qui ne lira que `rea_output/15_compression.md`, jamais `evidence/`).
- **Outils** : skill `/reverse-engineer-anything` (MCP `rea`) sur `src/sasexe/sasyzcrl` et `src/sasexe/sasyzcr2`, en lecture seule.
- **Licence** : analyse à des fins d'interopérabilité sur un SAS sous licence Apache ; confirmation interne à consigner dans `rea_output/STATE.md` avant toute décompilation.
