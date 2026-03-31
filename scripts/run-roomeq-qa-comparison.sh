#!/usr/bin/env bash
# Run roomeq multi-mode comparison on real QA recordings and generate visual reports.
#
# For each recording in roomeq_qa_data/*/recordings.json, runs roomeq in all 4
# processing modes (iir, fir, hybrid, mixed_phase) and generates a comparison HTML.
#
# Usage:
#   ./scripts/run-roomeq-qa-comparison.sh [recording_name]
#
# If recording_name is omitted, runs on all recordings found in the QA data dir.
#
# Examples:
#   ./scripts/run-roomeq-qa-comparison.sh              # all recordings
#   ./scripts/run-roomeq-qa-comparison.sh 2.0_8361a    # single recording

set -euo pipefail

QA_DATA_DIR="crates/autoeq/bin/roomeq_qa_data"
OUTPUT_BASE="data_generated/roomeq_qa_comparison"
ROOMEQ="cargo run --bin roomeq --release --"

RECORDING_FILTER="${1:-}"

# Build roomeq first (avoids repeated compilation)
echo "=== Building roomeq (release) ==="
cargo build --bin roomeq --release

ROOMEQ="./target/release/roomeq"

# Collect recordings
RECORDINGS=()
for dir in "${QA_DATA_DIR}"/*/; do
    config="${dir}recordings.json"
    if [ ! -f "$config" ]; then
        continue
    fi
    name="$(basename "$dir")"
    if [ -n "$RECORDING_FILTER" ] && [ "$name" != "$RECORDING_FILTER" ]; then
        continue
    fi
    RECORDINGS+=("$name")
done

if [ ${#RECORDINGS[@]} -eq 0 ]; then
    echo "Error: No recordings found"
    [ -n "$RECORDING_FILTER" ] && echo "  (filter: ${RECORDING_FILTER})"
    exit 1
fi

echo "=== Found ${#RECORDINGS[@]} recording(s): ${RECORDINGS[*]} ==="

# Mode override configs (written to temp files)
TMPDIR_MODES="$(mktemp -d)"
trap 'rm -rf "${TMPDIR_MODES}"' EXIT

cat > "${TMPDIR_MODES}/iir.json" <<'EOF'
{
    "optimizer": {
        "processing_mode": "low_latency"
    }
}
EOF

cat > "${TMPDIR_MODES}/fir.json" <<'EOF'
{
    "optimizer": {
        "processing_mode": "phase_linear",
        "max_freq": 1500.0,
        "fir": {
            "taps": 4096,
            "phase": "kirkeby",
            "correct_excess_phase": false,
            "phase_smoothing": 0.167
        },
        "phase_correction": {
            "max_fir_length_ms": 42.0,
            "pre_ringing_threshold_db": -30.0,
            "min_spatial_depth": 0.5,
            "phase_smoothing_octaves": 0.167
        }
    }
}
EOF

cat > "${TMPDIR_MODES}/hybrid.json" <<'EOF'
{
    "optimizer": {
        "processing_mode": "hybrid",
        "max_freq": 1500.0,
        "fir": {
            "taps": 2048,
            "phase": "kirkeby",
            "correct_excess_phase": false,
            "phase_smoothing": 0.167
        },
        "phase_correction": {
            "max_fir_length_ms": 10.0,
            "pre_ringing_threshold_db": -30.0,
            "min_spatial_depth": 0.5,
            "phase_smoothing_octaves": 0.167
        }
    }
}
EOF

cat > "${TMPDIR_MODES}/mixed_phase.json" <<'EOF'
{
    "optimizer": {
        "processing_mode": "mixed_phase",
        "mixed_phase": {
            "max_fir_length_ms": 10.0,
            "pre_ringing_threshold_db": -30.0,
            "min_spatial_depth": 0.5,
            "phase_smoothing_octaves": 0.167
        }
    }
}
EOF

MODES=(iir fir hybrid mixed_phase)

for recording in "${RECORDINGS[@]}"; do
    echo ""
    echo "========================================"
    echo "=== Recording: ${recording} ==="
    echo "========================================"

    CONFIG="${QA_DATA_DIR}/${recording}/recordings.json"
    OUTPUT_DIR="${OUTPUT_BASE}/${recording}"
    mkdir -p "${OUTPUT_DIR}"

    JSONS=()

    for mode in "${MODES[@]}"; do
        MODE_DIR="${OUTPUT_DIR}/${mode}"
        mkdir -p "${MODE_DIR}"
        OUTPUT_JSON="${MODE_DIR}/${mode}.json"
        OVERRIDE="${TMPDIR_MODES}/${mode}.json"

        echo ""
        echo "--- Mode: ${mode} ---"
        if ${ROOMEQ} -c "${CONFIG}" -o "${OUTPUT_JSON}" --override-config "${OVERRIDE}"; then
            JSONS+=("${OUTPUT_JSON}")
            echo "  Output: ${OUTPUT_JSON}"
        else
            echo "  FAILED (skipping)"
        fi
    done

    if [ ${#JSONS[@]} -lt 2 ]; then
        echo "Warning: Need at least 2 JSON files for comparison. Found ${#JSONS[@]}. Skipping report."
        continue
    fi

    # Generate comparison HTML
    HTML_OUTPUT="${OUTPUT_DIR}/comparison.html"
    echo ""
    echo "=== Generating comparison report ==="
    ./venv/bin/python scripts/display-roomeq.py --compare "${JSONS[@]}" -o "${HTML_OUTPUT}"

    echo "  Report: ${HTML_OUTPUT}"

    # Open in browser (macOS)
    if command -v open &>/dev/null; then
        open "${HTML_OUTPUT}"
    fi
done

echo ""
echo "=== All done ==="
