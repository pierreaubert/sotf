#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -ne 1 ]]; then
  echo "usage: $0 stress.csv" >&2
  exit 2
fi

CSV="$1"

awk -F, '
NR == 1 {
  for (i = 1; i <= NF; i++) idx[$i] = i
  print "rank_key,scenario,mode,chain,tracks,plugins_per_track,total_plugins,p99_us,p999_us,max_us,deadline_misses,p99_realtime_factor,max_realtime_factor"
  next
}
{
  print $(idx["p99_realtime_factor"]) "," \
    $(idx["scenario"]) "," \
    $(idx["mode"]) "," \
    $(idx["chain"]) "," \
    $(idx["tracks"]) "," \
    $(idx["plugins_per_track"]) "," \
    $(idx["total_plugins"]) "," \
    $(idx["p99_us"]) "," \
    $(idx["p999_us"]) "," \
    $(idx["max_us"]) "," \
    $(idx["deadline_misses"]) "," \
    $(idx["p99_realtime_factor"]) "," \
    $(idx["max_realtime_factor"])
}
' "$CSV" | {
  IFS= read -r header
  echo "$header"
  sort -t, -k1,1nr
}
