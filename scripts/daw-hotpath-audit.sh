#!/usr/bin/env bash
set -euo pipefail

DB="${DB:-.tokensave/tokensave.db}"
OUT="${OUT:-target/daw-perf/hotpath-audit.md}"

if [[ ! -f "$DB" ]]; then
  echo "TokenSave database not found at $DB" >&2
  exit 1
fi

mkdir -p "$(dirname "$OUT")"

cat > "$OUT" <<'HEADER'
# DAW Hot-Path Audit

This report is generated from TokenSave process-node ranges and a conservative
text scan for operations that are risky in audio callback paths. Findings are
candidates for review, not automatic bugs.

HEADER

sqlite3 -separator $'\t' "$DB" \
  "select file_path,start_line,end_line,qualified_name from nodes
   where file_path like 'crates/sotf-plugins/%'
     and file_path not like '%/tests/%'
     and file_path not like '%/tests.rs'
     and file_path not like '%/bin/%'
     and kind in ('method','function')
     and name in ('process','process_in_place','process_block')
   order by file_path,start_line" |
while IFS=$'\t' read -r file start end name; do
  [[ -f "$file" ]] || continue
  matches="$(
    sed -n "${start},${end}p" "$file" |
      nl -ba -v "$start" |
      grep -E 'Vec::new|Vec::with_capacity|vec!|HashMap|HashSet|\.lock\(|RwLock|Mutex|\.clone\(|Box::new|Arc::new|format!|println!|eprintln!|collect::<|\.collect\(|resize\(|reserve\(|push\(' || true
  )"
  if [[ -n "$matches" ]]; then
    {
      echo "## $name"
      echo
      echo "\`$file:$start\`"
      echo
      echo '```text'
      echo "$matches"
      echo '```'
      echo
    } >> "$OUT"
  fi
done

echo "Hot-path audit: $OUT" >&2
