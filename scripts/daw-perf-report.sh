#!/usr/bin/env bash
set -euo pipefail

MODE="${MODE:-both}"
BLOCK_SIZE="${BLOCK_SIZE:-128}"
SAMPLE_RATE="${SAMPLE_RATE:-48000}"
BLOCKS="${BLOCKS:-1024}"
WARMUP_BLOCKS="${WARMUP_BLOCKS:-128}"
TRACKS="${TRACKS:-}"
PLUGINS="${PLUGINS:-}"
OUT_DIR="${OUT_DIR:-target/daw-perf/$(date -u +%Y%m%dT%H%M%SZ)}"

mkdir -p "$OUT_DIR"

CSV="$OUT_DIR/stress.csv"
RANKING="$OUT_DIR/ranking.csv"
META="$OUT_DIR/meta.txt"
: > "$CSV"

{
  echo "date_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "mode=$MODE"
  echo "block_size=$BLOCK_SIZE"
  echo "sample_rate=$SAMPLE_RATE"
  echo "blocks=$BLOCKS"
  echo "warmup_blocks=$WARMUP_BLOCKS"
  echo "tracks=${TRACKS:-default}"
  echo "plugins=${PLUGINS:-default}"
  git rev-parse --show-toplevel >/dev/null 2>&1 && echo "git_commit=$(git rev-parse HEAD)"
  rustc --version
  cargo --version
} > "$META"

echo "Writing DAW stress report to $OUT_DIR" >&2

first=1
for chain in gain eq mixed heavy; do
  args=(
    run --release -p sotf-plugins --bin daw-scale-stress --
    --chain "$chain"
    --mode "$MODE"
    --block-size "$BLOCK_SIZE"
    --sample-rate "$SAMPLE_RATE"
    --blocks "$BLOCKS"
    --warmup-blocks "$WARMUP_BLOCKS"
  )
  if [[ -n "$TRACKS" ]]; then
    args+=(--tracks "$TRACKS")
  fi
  if [[ -n "$PLUGINS" ]]; then
    args+=(--plugins "$PLUGINS")
  fi

  tmp="$OUT_DIR/${chain}.csv"
  cargo "${args[@]}" > "$tmp"
  if [[ "$first" -eq 1 ]]; then
    cat "$tmp" >> "$CSV"
    first=0
  else
    tail -n +2 "$tmp" >> "$CSV"
  fi
done

"$(dirname "$0")/daw-rank-csv.sh" "$CSV" > "$RANKING"

echo "CSV: $CSV" >&2
echo "Ranking: $RANKING" >&2
echo "Metadata: $META" >&2
