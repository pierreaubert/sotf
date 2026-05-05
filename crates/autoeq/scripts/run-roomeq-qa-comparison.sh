#!/usr/bin/env bash
# Run roomeq multi-case comparison on real QA recordings and generate
# a single comparison HTML covering the standard processing-mode/loss
# matrix plus, optionally, automatic optimizer and group-delay
# optimisation variants.
#
# The standard suite runs all 8 combinations (4 processing modes × 2
# loss functions):
#
#     iir              fir              hybrid              mixed_phase
#     iir_epa          fir_epa          hybrid_epa          mixed_phase_epa
#
# The auto suite exercises automatic filter-count, Q-bound, and gain-bound
# selection on IIR/mixed-phase paths:
#
#     iir_auto_filters
#     iir_auto_bounds
#     iir_auto_all
#     mixed_phase_auto_all
#
# The GD suite adds production-wired group-delay variants:
#
#     iir_gd_safety_gate
#     iir_gd_delay_only
#     iir_gd_fixed_allpass
#     iir_gd_adaptive_allpass
#     fir_gd_phase_linear
#     mixed_phase_gd
#
# These are useful even when a recording has no coherence / independent
# sweep data: the output metadata records whether GD ran, was applied, or
# downgraded to a safer advisory path.
#
# For each recording this generates one comparison report:
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
#   ./scripts/run-roomeq-qa-comparison.sh [--suite standard|auto|gd|all] [recording_name]
#
# If recording_name is omitted, runs on all recordings found in the QA data dir.
#
# Examples:
#   ./scripts/run-roomeq-qa-comparison.sh                      # all recordings, all cases
#   ./scripts/run-roomeq-qa-comparison.sh --suite standard     # original 8-mode matrix
#   ./scripts/run-roomeq-qa-comparison.sh --suite auto 2.0_d3v # automatic optimizer cases
#   ./scripts/run-roomeq-qa-comparison.sh --suite gd 2.0_t7v   # GD cases for one recording

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
cd "${PROJECT_ROOT}"

QA_DATA_DIR="crates/autoeq/data_tests/roomeq/measured"
OUTPUT_BASE="data_generated/roomeq_qa_comparison"
PYTHON="${PROJECT_ROOT}/venv/bin/python"
if [ ! -x "${PYTHON}" ]; then
    PYTHON="python3"
fi

SUITE="all"
RECORDING_FILTER=""

usage() {
    sed -n '2,/^$/p' "$0" | sed '$d' | sed 's/^# \{0,1\}//'
}

while [ $# -gt 0 ]; do
    case "$1" in
        --suite)
            if [ $# -lt 2 ]; then
                echo "Error: --suite requires one of: standard, auto, gd, all" >&2
                exit 1
            fi
            SUITE="$2"
            shift 2
            ;;
        --standard)
            SUITE="standard"
            shift
            ;;
        --auto)
            SUITE="auto"
            shift
            ;;
        --gd)
            SUITE="gd"
            shift
            ;;
        --all)
            SUITE="all"
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        -*)
            echo "Error: unknown option: $1" >&2
            usage
            exit 1
            ;;
        *)
            if [ -n "$RECORDING_FILTER" ]; then
                echo "Error: multiple recording filters supplied: ${RECORDING_FILTER} and $1" >&2
                exit 1
            fi
            RECORDING_FILTER="$1"
            shift
            ;;
    esac
done

case "$SUITE" in
    standard|auto|gd|all) ;;
    *)
        echo "Error: invalid suite '${SUITE}' (expected: standard, auto, gd, all)" >&2
        exit 1
        ;;
esac

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

cat > "${TMPDIR_MODES}/iir_auto_filters.json" <<'EOF'
{
    "optimizer": {
        "max_iter": 50000,
        "population": 300,
        "strategy": "lshade",
        "processing_mode": "low_latency",
        "loss_type": "flat",
        "auto_optimizer": {
            "enabled": true,
            "filter_count": true,
            "q_bounds": false,
            "gain_bounds": false,
            "min_filters": 2,
            "max_filters": 12
        }
    }
}
EOF

cat > "${TMPDIR_MODES}/iir_auto_bounds.json" <<'EOF'
{
    "optimizer": {
        "max_iter": 50000,
        "population": 300,
        "strategy": "lshade",
        "processing_mode": "low_latency",
        "loss_type": "flat",
        "auto_optimizer": {
            "enabled": true,
            "filter_count": false,
            "q_bounds": true,
            "gain_bounds": true,
            "min_filters": 2,
            "max_filters": 12
        }
    }
}
EOF

cat > "${TMPDIR_MODES}/iir_auto_all.json" <<'EOF'
{
    "optimizer": {
        "max_iter": 50000,
        "population": 300,
        "strategy": "lshade",
        "processing_mode": "low_latency",
        "loss_type": "flat",
        "auto_optimizer": {
            "enabled": true,
            "filter_count": true,
            "q_bounds": true,
            "gain_bounds": true,
            "min_filters": 2,
            "max_filters": 12
        }
    }
}
EOF

cat > "${TMPDIR_MODES}/mixed_phase_auto_all.json" <<'EOF'
{
    "optimizer": {
        "max_iter": 50000,
        "population": 300,
        "strategy": "lshade",
        "processing_mode": "mixed_phase",
        "loss_type": "flat",
        "mixed_phase": {
            "max_fir_length_ms": 10.0,
            "pre_ringing_threshold_db": -30.0,
            "min_spatial_depth": 0.5,
            "phase_smoothing_octaves": 0.167
        },
        "auto_optimizer": {
            "enabled": true,
            "filter_count": true,
            "q_bounds": true,
            "gain_bounds": true,
            "min_filters": 2,
            "max_filters": 12
        }
    }
}
EOF

cat > "${TMPDIR_MODES}/iir_gd_safety_gate.json" <<'EOF'
{
    "optimizer": {
        "max_iter": 50000,
        "population": 300,
        "strategy": "lshade",
        "processing_mode": "low_latency",
        "loss_type": "flat",
        "group_delay": {
            "enabled": true,
            "max_delay_ms": 25.0,
            "ap_per_channel": 2,
            "optimize_polarity": true,
            "adaptive_allpass": false,
            "min_improvement_db": 0.0,
            "max_iter": 2000,
            "popsize": 20
        }
    }
}
EOF

cat > "${TMPDIR_MODES}/iir_gd_delay_only.json" <<'EOF'
{
    "optimizer": {
        "max_iter": 50000,
        "population": 300,
        "strategy": "lshade",
        "processing_mode": "low_latency",
        "loss_type": "flat",
        "group_delay": {
            "enabled": true,
            "max_delay_ms": 25.0,
            "ap_per_channel": 0,
            "optimize_polarity": false,
            "adaptive_allpass": false,
            "min_improvement_db": 0.0,
            "max_iter": 2000,
            "popsize": 20
        }
    }
}
EOF

cat > "${TMPDIR_MODES}/iir_gd_fixed_allpass.json" <<'EOF'
{
    "optimizer": {
        "max_iter": 50000,
        "population": 300,
        "strategy": "lshade",
        "processing_mode": "low_latency",
        "loss_type": "flat",
        "group_delay": {
            "enabled": true,
            "max_delay_ms": 25.0,
            "ap_per_channel": 1,
            "optimize_polarity": true,
            "adaptive_allpass": false,
            "min_improvement_db": 0.0,
            "max_iter": 2000,
            "popsize": 20
        }
    }
}
EOF

cat > "${TMPDIR_MODES}/iir_gd_adaptive_allpass.json" <<'EOF'
{
    "optimizer": {
        "max_iter": 50000,
        "population": 300,
        "strategy": "lshade",
        "processing_mode": "low_latency",
        "loss_type": "flat",
        "group_delay": {
            "enabled": true,
            "max_delay_ms": 25.0,
            "ap_per_channel": 1,
            "optimize_polarity": false,
            "adaptive_allpass": true,
            "min_improvement_db": 0.0,
            "max_iter": 2000,
            "popsize": 20
        }
    }
}
EOF

cat > "${TMPDIR_MODES}/fir_gd_phase_linear.json" <<'EOF'
{
    "optimizer": {
        "max_iter": 50000,
        "population": 300,
        "strategy": "lshade",
        "processing_mode": "phase_linear",
        "loss_type": "flat",
        "max_freq": 1500.0,
        "fir": {
            "taps": 4096,
            "phase": "linear",
            "correct_excess_phase": false,
            "phase_smoothing": 0.167
        },
        "group_delay": {
            "enabled": true,
            "max_delay_ms": 25.0,
            "ap_per_channel": 0,
            "optimize_polarity": false,
            "adaptive_allpass": false,
            "min_improvement_db": 0.0,
            "max_iter": 2000,
            "popsize": 20
        }
    }
}
EOF

cat > "${TMPDIR_MODES}/mixed_phase_gd.json" <<'EOF'
{
    "optimizer": {
        "max_iter": 50000,
        "population": 300,
        "strategy": "lshade",
        "processing_mode": "mixed_phase",
        "loss_type": "flat",
        "mixed_phase": {
            "max_fir_length_ms": 10.0,
            "pre_ringing_threshold_db": -30.0,
            "min_spatial_depth": 0.5,
            "phase_smoothing_octaves": 0.167
        },
        "group_delay": {
            "enabled": true,
            "max_delay_ms": 25.0,
            "ap_per_channel": 2,
            "optimize_polarity": true,
            "adaptive_allpass": false,
            "min_improvement_db": 0.0,
            "max_iter": 2000,
            "popsize": 20
        }
    }
}
EOF

# Standard modes are interleaved so each processing mode's `flat` and `_epa`
# variants sit next to each other in the comparison's plot legends and
# subplot grid. This makes the loss-type effect easy to spot at a
# glance per processing mode.
STANDARD_MODES=(
    iir         iir_epa
    fir         fir_epa
    hybrid      hybrid_epa
    mixed_phase mixed_phase_epa
)

AUTO_MODES=(
    iir_auto_filters
    iir_auto_bounds
    iir_auto_all
    mixed_phase_auto_all
)

GD_MODES=(
    iir_gd_safety_gate
    iir_gd_delay_only
    iir_gd_fixed_allpass
    iir_gd_adaptive_allpass
    fir_gd_phase_linear
    mixed_phase_gd
)

case "$SUITE" in
    standard)
        MODES=("${STANDARD_MODES[@]}")
        ;;
    auto)
        MODES=("${AUTO_MODES[@]}")
        ;;
    gd)
        MODES=("${GD_MODES[@]}")
        ;;
    all)
        MODES=("${STANDARD_MODES[@]}" "${AUTO_MODES[@]}" "${GD_MODES[@]}")
        ;;
esac

echo "=== Suite: ${SUITE} (${#MODES[@]} case(s): ${MODES[*]}) ==="

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
    # script — the combined comparison subsumes it.
    rm -f "${OUTPUT_DIR}/loss_types.html"

    HTML_OUTPUT="${OUTPUT_DIR}/comparison.html"
    echo ""
    echo "=== Generating ${#JSONS[@]}-case comparison report ==="
    "${PYTHON}" "${SCRIPT_DIR}/display-roomeq.py" --compare "${JSONS[@]}" -o "${HTML_OUTPUT}"
    echo "  Report: ${HTML_OUTPUT}"

    # Open in browser (macOS)
    if command -v open &>/dev/null; then
        open "${HTML_OUTPUT}"
    fi
done

echo ""
echo "=== All done ==="
