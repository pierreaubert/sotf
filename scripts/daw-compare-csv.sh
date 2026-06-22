#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -ne 2 ]]; then
  echo "usage: $0 BASELINE.csv CANDIDATE.csv" >&2
  exit 2
fi

BASELINE="$1"
CANDIDATE="$2"

awk -F, '
NR == FNR {
  if (FNR == 1) {
    for (i = 1; i <= NF; i++) bidx[$i] = i
    next
  }
  key = $bidx["scenario"] "," $bidx["mode"] "," $bidx["chain"] "," $bidx["tracks"] "," $bidx["plugins_per_track"]
  bp99[key] = $(bidx["p99_us"]) + 0
  bp999[key] = $(bidx["p999_us"]) + 0
  bmax[key] = $(bidx["max_us"]) + 0
  bmiss[key] = $(bidx["deadline_misses"]) + 0
  next
}
FNR == 1 {
  for (i = 1; i <= NF; i++) cidx[$i] = i
  print "scenario,mode,chain,tracks,plugins_per_track,p99_delta_pct,p999_delta_pct,max_delta_pct,deadline_miss_delta"
  next
}
{
  key = $cidx["scenario"] "," $cidx["mode"] "," $cidx["chain"] "," $cidx["tracks"] "," $cidx["plugins_per_track"]
  if (!(key in bp99)) next
  cp99 = $(cidx["p99_us"]) + 0
  cp999 = $(cidx["p999_us"]) + 0
  cmax = $(cidx["max_us"]) + 0
  cmiss = $(cidx["deadline_misses"]) + 0
  split(key, parts, ",")
  printf "%s,%s,%s,%s,%s,%.2f,%.2f,%.2f,%d\n",
    parts[1], parts[2], parts[3], parts[4], parts[5],
    pct(cp99, bp99[key]), pct(cp999, bp999[key]), pct(cmax, bmax[key]), cmiss - bmiss[key]
}
function pct(candidate, baseline) {
  if (baseline == 0) {
    return candidate == 0 ? 0 : 999999
  }
  return ((candidate - baseline) / baseline) * 100.0
}
' "$BASELINE" "$CANDIDATE"
