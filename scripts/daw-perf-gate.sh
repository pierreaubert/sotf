#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -ne 1 ]]; then
  echo "usage: $0 stress.csv" >&2
  exit 2
fi

CSV="$1"
MAX_P99_REALTIME="${MAX_P99_REALTIME:-0.85}"
MAX_DEADLINE_MISSES="${MAX_DEADLINE_MISSES:-0}"

awk -F, -v max_p99="$MAX_P99_REALTIME" -v max_misses="$MAX_DEADLINE_MISSES" '
NR == 1 {
  for (i = 1; i <= NF; i++) idx[$i] = i
  next
}
{
  p99 = $(idx["p99_realtime_factor"]) + 0
  misses = $(idx["deadline_misses"]) + 0
  if (p99 > max_p99 || misses > max_misses) {
    printf "DAW perf gate failed: scenario=%s mode=%s chain=%s tracks=%s plugins=%s p99_rt=%.3f misses=%d\n",
      $(idx["scenario"]), $(idx["mode"]), $(idx["chain"]), $(idx["tracks"]), $(idx["plugins_per_track"]), p99, misses > "/dev/stderr"
    failed = 1
  }
}
END {
  exit failed ? 1 : 0
}
' "$CSV"
