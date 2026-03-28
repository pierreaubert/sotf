#!/usr/bin/env bash
# Run roomeq multi-mode comparison test and generate visual comparison report.
#
# Usage:
#   ./scripts/run-roomeq-comparison.sh [scenario]
#
# Default scenario: small_stereo_2_0

set -euo pipefail

SCENARIO="${1:-small_stereo_2_0}"
OUTPUT_DIR="data_generated/roomeq_comparison/${SCENARIO}"

echo "=== Running multi-mode comparison for: ${SCENARIO} ==="

# Run the comparison test (generates JSON outputs for all modes)
cargo test -p autoeq "test_multimode_comparison_${SCENARIO}" --release -- --nocapture

# Find the generated JSONs
JSONS=()
for mode in iir fir hybrid mixed_phase; do
    JSON="${OUTPUT_DIR}/${mode}/${mode}.json"
    if [ -f "$JSON" ]; then
        JSONS+=("$JSON")
        echo "  Found: ${JSON}"
    fi
done

if [ ${#JSONS[@]} -lt 2 ]; then
    echo "Error: Need at least 2 JSON files for comparison. Found ${#JSONS[@]}."
    exit 1
fi

# Generate comparison HTML
HTML_OUTPUT="${OUTPUT_DIR}/comparison.html"
echo "=== Generating comparison report ==="
./venv/bin/python scripts/display-roomeq.py --compare "${JSONS[@]}" -o "${HTML_OUTPUT}"

echo ""
echo "=== Done ==="
echo "Open: ${HTML_OUTPUT}"

# Open in browser (macOS)
if command -v open &>/dev/null; then
    open "${HTML_OUTPUT}"
fi
