#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -lt 2 || "$#" -gt 3 ]]; then
  echo "usage: $0 baseline.csv candidate.csv [relative_tolerance]" >&2
  exit 2
fi

BASELINE="$1"
CANDIDATE="$2"
RELATIVE_TOLERANCE="${3:-0.05}"

[[ -f "$BASELINE" ]] || { echo "baseline CSV not found: $BASELINE" >&2; exit 2; }
[[ -f "$CANDIDATE" ]] || { echo "candidate CSV not found: $CANDIDATE" >&2; exit 2; }

awk -F, -v tolerance="$RELATIVE_TOLERANCE" '
NR == FNR {
  if (FNR == 1) {
    for (i = 1; i <= NF; i++) idx[$i] = i
    for (required in required_columns) {
      if (!(required in idx)) {
        print "missing required column in baseline: " required > "/dev/stderr"
        failed = 1
      }
    }
    next
  }
  baseline[$(idx["scenario"]) "|" $(idx["mode"]) "|" \
    $(idx["chain"]) "|" $(idx["tracks"]) "|" \
    $(idx["plugins_per_track"])] = $0
  next
}

FNR == 1 {
  for (i = 1; i <= NF; i++) candidate_idx[$i] = i
  for (required in required_columns) {
    if (!(required in candidate_idx)) {
      print "missing required column in candidate: " required > "/dev/stderr"
      failed = 1
    }
  }
  next
}

{
  current_key = $(candidate_idx["scenario"]) "|" \
    $(candidate_idx["mode"]) "|" $(candidate_idx["chain"]) "|" \
    $(candidate_idx["tracks"]) "|" $(candidate_idx["plugins_per_track"])
  if (!(current_key in baseline)) {
    print "missing baseline scenario: " current_key > "/dev/stderr"
    failed = 1
    next
  }

  split(baseline[current_key], previous, FS)
  previous_p99 = previous[idx["p99_realtime_factor"]] + 0
  current_p99 = $candidate_idx["p99_realtime_factor"] + 0
  if (current_p99 > previous_p99 * (1 + tolerance)) {
    printf "performance regression: %s p99 %.4f -> %.4f\n", \
      current_key, previous_p99, current_p99 > "/dev/stderr"
    failed = 1
  }

  previous_misses = previous[idx["deadline_misses"]] + 0
  current_misses = $candidate_idx["deadline_misses"] + 0
  if (current_misses > previous_misses) {
    printf "deadline regression: %s misses %d -> %d\n", \
      current_key, previous_misses, current_misses > "/dev/stderr"
    failed = 1
  }
}

BEGIN {
  required_columns["scenario"] = 1
  required_columns["mode"] = 1
  required_columns["chain"] = 1
  required_columns["tracks"] = 1
  required_columns["plugins_per_track"] = 1
  required_columns["p99_realtime_factor"] = 1
  required_columns["deadline_misses"] = 1
  if (tolerance < 0) {
    print "relative tolerance must be non-negative" > "/dev/stderr"
    exit 2
  }
}

END {
  exit failed ? 1 : 0
}
' "$BASELINE" "$CANDIDATE"
