#!/usr/bin/env bash
# Critère de fin de J0 (docs/roadmap.md) : l'arbre de dépendances du cœur est égal à la liste fermée de l'ADR 0001.
#  1. dépendances directes de production == thiserror, encoding_rs, memchr ;
#  2. aucune crate interdite (ADR 0001) dans tout l'arbre de production ;
#  3. graphe des crates du workspace : aucune flèche du cœur vers un adaptateur.
set -euo pipefail
cd "$(dirname "$0")/.."

ALLOWED="encoding_rs memchr thiserror"
FORBIDDEN='^(tokio|futures|futures-[a-z]+|memmap2|arrow[a-z0-9_-]*|datafusion[a-z0-9_-]*|pyo3[a-z0-9_-]*|extendr-[a-z0-9_-]+|rayon|bytemuck|serde[a-z0-9_-]*|object_store|uniformat-[a-z-]+)$'

direct=$(cargo tree -p uniformat-sas --edges normal --depth 1 --prefix none --charset ascii \
  | tail -n +2 | awk '{print $1}' | sort -u | tr '\n' ' ' | sed 's/ $//')
expected=$(echo "$ALLOWED" | tr ' ' '\n' | sort -u | tr '\n' ' ' | sed 's/ $//')
if [ "$direct" != "$expected" ]; then
  echo "ECHEC : dépendances directes du cœur = [$direct], attendu (ADR 0001) = [$expected]" >&2
  exit 1
fi

all=$(cargo tree -p uniformat-sas --edges normal --prefix none --charset ascii | awk '{print $1}' | sort -u)
# the codec itself is the root of the tree; every other UniFormat crate (including uniformat-core) is forbidden below it
all=$(echo "$all" | grep -vx 'uniformat-sas' || true)
bad=$(echo "$all" | grep -E "$FORBIDDEN" || true)
if [ -n "$bad" ]; then
  echo "ECHEC : crates interdites dans l'arbre du cœur :" >&2
  echo "$bad" >&2
  exit 1
fi

echo "OK : cœur uniformat-sas = {$direct}, $(echo "$all" | wc -l | tr -d ' ') crates au total, aucune interdite"
