#!/usr/bin/env bash
# Rejoue le match de démonstration, le corpus (en local et contre notre serveur) et la suite de tests.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release
TARGET=${CARGO_TARGET_DIR:-target}
BIN=$TARGET/release/palestre
OUT=$TARGET/demo
mkdir -p "$OUT"

echo "== Assemblage des agents"
"$BIN" asm agents/idler.asm -o "$OUT/idler.pbc"
"$BIN" asm agents/harvester.asm -o "$OUT/harvester.pbc"

echo "== Match de démonstration (récolteur contre inerte, graine 1)"
"$BIN" match "$OUT/harvester.pbc" "$OUT/idler.pbc" --seed 1 --log "$OUT/match.json"

echo "== Corpus public, VM en mémoire"
"$BIN" corpus corpus

echo "== Corpus public, contre palestre exec"
"$BIN" exec --listen 127.0.0.1:0 > "$OUT/exec.log" 2>&1 &
SERVER=$!
trap 'kill "$SERVER" 2>/dev/null || true' EXIT
for _ in $(seq 50); do grep -q 'écoute sur' "$OUT/exec.log" && break; sleep 0.1; done
ADDR=$(awk '/écoute sur/ { print $NF; exit }' "$OUT/exec.log")
if [ -z "$ADDR" ]; then
    echo "le serveur palestre exec n'a pas démarré" >&2
    exit 1
fi
"$BIN" corpus corpus --server "$ADDR"

echo "== Suite de tests"
cargo test --workspace --release

echo "== Tout est vert."
