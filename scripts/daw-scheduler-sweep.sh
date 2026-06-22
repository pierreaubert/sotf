#!/usr/bin/env bash
set -euo pipefail

BLOCKS="${BLOCKS:-256}"
WARMUP_BLOCKS="${WARMUP_BLOCKS:-64}"
SAMPLE_RATE="${SAMPLE_RATE:-48000}"
OUT_DIR="${OUT_DIR:-target/daw-perf/scheduler-sweep-$(date -u +%Y%m%dT%H%M%SZ)}"
TRACK_SET="${TRACK_SET:-16 32 64 128}"
PLUGIN_SET="${PLUGIN_SET:-1 4 8 16}"
BLOCK_SIZE_SET="${BLOCK_SIZE_SET:-64 128 256}"
CHAIN_SET="${CHAIN_SET:-gain eq mute-solo mixed heavy}"
MODE="${MODE:-both}"

mkdir -p "$OUT_DIR"
CSV="$OUT_DIR/scheduler-sweep.csv"
: > "$CSV"

echo "Writing DAW scheduler sweep to $OUT_DIR" >&2

first=1
for block_size in $BLOCK_SIZE_SET; do
  for tracks in $TRACK_SET; do
    for plugins in $PLUGIN_SET; do
      for chain in $CHAIN_SET; do
        tmp="$OUT_DIR/${chain}-${block_size}-${tracks}-${plugins}.csv"
        cargo run --release -p sotf-plugins --bin daw-scale-stress -- \
          --chain "$chain" \
          --mode "$MODE" \
          --block-size "$block_size" \
          --sample-rate "$SAMPLE_RATE" \
          --tracks "$tracks" \
          --plugins "$plugins" \
          --blocks "$BLOCKS" \
          --warmup-blocks "$WARMUP_BLOCKS" > "$tmp"
        if [[ "$first" -eq 1 ]]; then
          cat "$tmp" >> "$CSV"
          first=0
        else
          tail -n +2 "$tmp" >> "$CSV"
        fi
      done
    done
  done
done

"$(dirname "$0")/daw-rank-csv.sh" "$CSV" > "$OUT_DIR/ranking.csv"
echo "CSV: $CSV" >&2
echo "Ranking: $OUT_DIR/ranking.csv" >&2
