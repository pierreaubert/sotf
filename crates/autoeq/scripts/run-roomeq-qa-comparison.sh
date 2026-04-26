#!/usr/bin/env bash
# Run roomeq multi-mode comparison on real QA recordings and generate
# a single comparison HTML covering every (processing_mode × loss_type)
# combination supported on roomeq recordings.
#
# For each recording in roomeq_qa_data/*/recordings.json this runs all
# 8 combinations (4 processing modes × 2 loss functions):
#
#     iir              fir              hybrid              mixed_phase
#     iir_epa          fir_epa          hybrid_epa          mixed_phase_epa
#
# and generates one comparison report:
#
#     <recording>/comparison.html
#
# The plain (non-`_epa`) variants minimize the default `flat` ERB+band
# weighted loss; the `_epa` variants minimize the EPA psychoacoustic
# composite (flatness + Zwicker sharpness / roughness / loudness
# balance). The `score` loss is *not* included because it requires
# CEA2034 speaker data, which the roomeq QA recordings do not provide.
#
# The comparison report's per-channel EPA score table makes the
# perceptual outcome of every combination directly comparable, even
# though the absolute pre/post loss numbers across `flat` vs `epa` runs
# live on different scales (the report flags this in red automatically).
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

# Mode override configs (written to temp files). Each pair shares its
# processing-mode-specific knobs but the `_epa` variant adds
# `loss_type: "epa"` so it minimizes the EPA composite instead of the
# default ERB+band weighted flat loss.
TMPDIR_MODES="$(mktemp -d)"
trap 'rm -rf "${TMPDIR_MODES}"' EXIT

cat > "${TMPDIR_MODES}/iir.json" <<'EOF'
{
    "optimizer": {
        "max_iter": 50000,
        "population": 300,
        "strategy": "lshade",
        "processing_mode": "low_latency"
    }
}
EOF

cat > "${TMPDIR_MODES}/iir_epa.json" <<'EOF'
{
    "optimizer": {
        "max_iter": 50000,
        "population": 300,
        "strategy": "lshade",
        "processing_mode": "low_latency",
        "loss_type": "epa"
    }
}
EOF

cat > "${TMPDIR_MODES}/fir.json" <<'EOF'
{
    "optimizer": {
        "max_iter": 50000,
        "population": 300,
        "strategy": "lshade",
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

cat > "${TMPDIR_MODES}/fir_epa.json" <<'EOF'
{
    "optimizer": {
        "max_iter": 50000,
        "population": 300,
        "strategy": "lshade",
        "processing_mode": "phase_linear",
        "loss_type": "epa",
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
        "max_iter": 50000,
        "population": 300,
        "strategy": "lshade",
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

cat > "${TMPDIR_MODES}/hybrid_epa.json" <<'EOF'
{
    "optimizer": {
        "max_iter": 50000,
        "population": 300,
        "strategy": "lshade",
        "processing_mode": "hybrid",
        "loss_type": "epa",
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
        "max_iter": 50000,
        "population": 300,
        "strategy": "lshade",
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

cat > "${TMPDIR_MODES}/mixed_phase_epa.json" <<'EOF'
{
    "optimizer": {
        "max_iter": 50000,
        "population": 300,
        "strategy": "lshade",
        "processing_mode": "mixed_phase",
        "loss_type": "epa",
        "mixed_phase": {
            "max_fir_length_ms": 10.0,
            "pre_ringing_threshold_db": -30.0,
            "min_spatial_depth": 0.5,
            "phase_smoothing_octaves": 0.167
        }
    }
}
EOF

# Modes are interleaved so each processing mode's `flat` and `_epa`
# variants sit next to each other in the comparison's plot legends and
# subplot grid. This makes the loss-type effect easy to spot at a
# glance per processing mode.
MODES=(
    iir         iir_epa
    fir         fir_epa
    hybrid      hybrid_epa
    mixed_phase mixed_phase_epa
)

for recording in "${RECORDINGS[@]}"; do
    echo ""
    echo "========================================"
    echo "=== Recording: ${recording} ==="
    echo "========================================"

    CONFIG="${QA_DATA_DIR}/${recording}/recordings.json"
    OUTPUT_DIR="${OUTPUT_BASE}/${recording}"
    mkdir -p "${OUTPUT_DIR}"

    for mode in "${MODES[@]}"; do
        MODE_DIR="${OUTPUT_DIR}/${mode}"
        mkdir -p "${MODE_DIR}"
        OUTPUT_JSON="${MODE_DIR}/${mode}.json"
        OVERRIDE="${TMPDIR_MODES}/${mode}.json"

        echo ""
        echo "--- Mode: ${mode} ---"
        if ${ROOMEQ} -c "${CONFIG}" -o "${OUTPUT_JSON}" --override-config "${OVERRIDE}"; then
            echo "  Output: ${OUTPUT_JSON}"
        else
            echo "  FAILED (skipping)"
            rm -f "${OUTPUT_JSON}"
        fi
    done

    # Collect JSONs that actually landed on disk, preserving MODES order
    # so the report renders combinations in a predictable layout.
    JSONS=()
    for mode in "${MODES[@]}"; do
        json="${OUTPUT_DIR}/${mode}/${mode}.json"
        if [ -f "$json" ]; then
            JSONS+=("$json")
        fi
    done

    if [ ${#JSONS[@]} -lt 2 ]; then
        echo "Warning: Need at least 2 JSON files for comparison. Found ${#JSONS[@]}. Skipping report."
        continue
    fi

    # Drop any stale loss_types.html from earlier versions of this
    # script — the 8-mode comparison subsumes it.
    rm -f "${OUTPUT_DIR}/loss_types.html"

    HTML_OUTPUT="${OUTPUT_DIR}/comparison.html"
    echo ""
    echo "=== Generating 8-mode comparison report ==="
    ./venv/bin/python scripts/display-roomeq.py --compare "${JSONS[@]}" -o "${HTML_OUTPUT}"
    echo "  Report: ${HTML_OUTPUT}"

    # Open in browser (macOS)
    if command -v open &>/dev/null; then
        open "${HTML_OUTPUT}"
    fi
done

echo ""
echo "=== All done ==="
