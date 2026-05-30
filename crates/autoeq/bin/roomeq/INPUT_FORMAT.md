# RoomEQ Input Configuration Format

This document describes the JSON input format for the `roomeq` command-line tool.

**JSON Schema:** [`input_schema.json`](./input_schema.json)

To validate your configuration against the schema:
```bash
# Using ajv-cli
npx ajv validate -s input_schema.json -d your_config.json

# Using check-jsonschema
check-jsonschema --schemafile input_schema.json your_config.json
```

## Root Structure

```json
{
  "version": "3.0.0",
  "system": { ... },
  "speakers": { ... },
  "crossovers": { ... },
  "target_curve": "...",
  "optimizer": { ... },
  "recording_config": { ... },
  "ctc": { ... }
}
```

### Top-Level Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `version` | string | No | `"3.0.0"` | Configuration version (semantic versioning) |
| `system` | object | No | - | System topology and logical channel mapping |
| `speakers` | object | **Yes** | - | Map of channel names to speaker configurations |
| `crossovers` | object | No | - | Crossover configurations referenced by multi-driver speakers |
| `target_curve` | string | No | - | Target frequency response curve |
| `optimizer` | object | No | defaults | Optimization parameters |
| `recording_config` | object | No | - | Recording configuration (device settings, signal parameters used during capture) |
| `ctc` | object | No | - | Cross-talk cancellation / binaural-aware correction using measured two-ear IRs or HRTF/SOFA data |

---

## Cross-talk Cancellation (CTC)

The optional `ctc` block exports a `recommended_xtc_matrix.json` artifact that can be loaded by the XTC plugin with `source_mode: "roomeq_recommended"`.

```json
{
  "ctc": {
    "enabled": true,
    "matrix_source": "measured",
    "measurements": {
      "speakers": ["L", "R"],
      "mics": ["left_ear", "right_ear"],
      "head_positions": [
        { "id": "primary", "x": 0.0, "y": 0.0, "z": 0.0, "yaw_deg": 0.0 }
      ],
      "files": [
        { "head_position": "primary", "speaker": "L", "ir": "irs/L_primary.wav" },
        { "head_position": "primary", "speaker": "R", "ir": "irs/R_primary.wav" }
      ]
    },
    "window": {
      "window_type": "ctc_direct",
      "start_ms": 0.0,
      "length_ms": 6.0,
      "fade_ms": 1.0
    },
    "regularization": { "beta_db": -30.0, "beta_lf_db": -20.0, "beta_hf_db": -40.0, "max_gain_db": 12.0 },
    "robustness": "average",
    "include_room_eq_dsp": true,
    "fir_taps": 4096
  }
}
```

Measured CTC input expects one processed, deconvolved, loopback-aligned stereo IR WAV per speaker and head position. Channel 1 is left ear, channel 2 is right ear.

Raw sweep CTC input can deconvolve and align takes inside roomEQ. Use `matrix_source: "raw_sweep"`, provide the emitted sweep as `reference_sweep`, and provide a two-channel ear recording plus a loopback WAV for each speaker and head position:

```json
{
  "ctc": {
    "enabled": true,
    "matrix_source": "raw_sweep",
    "reference_sweep": "sweeps/reference.wav",
    "sweep_duration_s": 15.0,
    "sweep_start_hz": 20.0,
    "sweep_end_hz": 24000.0,
    "measurements": {
      "speakers": ["L", "R"],
      "mics": ["left_ear", "right_ear"],
      "files": [
        {
          "head_position": "primary",
          "speaker": "L",
          "raw_sweep": "takes/L_primary_ears.wav",
          "loopback": "takes/L_primary_loopback.wav"
        },
        {
          "head_position": "primary",
          "speaker": "R",
          "raw_sweep": "takes/R_primary_ears.wav",
          "loopback": "takes/R_primary_loopback.wav"
        }
      ]
    },
    "window": {
      "window_type": "fdw",
      "fdw_cycles": 8.0,
      "fdw_min_ms": 3.0,
      "fdw_max_ms": 200.0
    },
    "regularization": { "beta_db": -30.0, "beta_lf_db": -20.0, "beta_hf_db": -40.0, "max_gain_db": 12.0 },
    "robustness": "minimax",
    "include_room_eq_dsp": true,
    "minimax_iterations": 8,
    "fir_taps": 4096
  }
}
```

`include_room_eq_dsp` defaults to `true`. When CTC runs from the full RoomEQ pipeline, it folds each exported per-channel gain/EQ/delay response into the acoustic matrix before solving so the recommended XTC matrix matches runtime order: global XTC first, then channel correction. Set it to `false` only when the artifact will be used without the exported RoomEQ channel chains.

`matrix_source: "hrtf_database"` uses `ctc.hrtf.hrtf_file` plus per-speaker azimuth/elevation/distance entries instead. The generated artifact includes FIR taps, latency, condition number, mean/worst reconstruction error, delivered-response metrics, crosstalk residual, and electrical sum-gain/headroom metrics.

---

## System Configuration

The `system` section decouples logical channel roles (e.g., "L", "R", "LFE") from physical measurement files. This allows for explicit topology definitions and automatic subwoofer alignment strategies.

```json
{
  "system": {
    "model": "stereo",
    "speakers": {
      "L": "left_meas",
      "R": "right_meas",
      "LFE": "sub_meas"
    },
    "subwoofers": {
      "config": "single",
      "crossover": "bass_xover",
      "sub_meas": "L"
    }
  },
  "crossovers": {
    "bass_xover": {
      "type": "LR24",
      "frequency": 80.0
    }
  }
}
```

### System Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `model` | string | No | `"custom"` | Topology model: `"stereo"`, `"home_cinema"`, `"custom"` |
| `speakers` | map | **Yes** | - | Map of Logical Role → Measurement Key. The key must exist in the root `speakers` object. |
| `subwoofers` | object | No | - | Subwoofer configuration and alignment mapping |

### Subwoofers Configuration

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `config` | string | No | `"single"` | Strategy: `"single"`, `"mso"`, `"dba"` |
| `crossover` | string | No | - | Reference to a crossover definition in the `crossovers` map |
| `*` | string | - | - | Any other key is treated as: `Subwoofer Measurement Key` → `Main Speaker Logical Role` (for alignment) |

---

## Speakers Configuration

The `speakers` field is a map where keys are channel names (e.g., `"left"`, `"right"`, `"center"`, `"lfe"`) and values are speaker configurations.

RoomEQ supports five speaker types:
1. **Single** - A single speaker measurement
2. **Group** - Multi-driver speaker with crossover optimization
3. **MultiSub** - Multiple subwoofers with gain/delay optimization
4. **DBA** - Double Bass Array with front/rear optimization
5. **Cardioid** - Gradient Cardioid configuration (2 subs)

### Measurement References

Measurements can be specified in several ways:

**1. Simple path string:**
```json
"left": "measurements/left.csv"
```

**2. Object with path:**
```json
"left": {
  "path": "measurements/left.csv"
}
```

**3. Named measurement:**
```json
"left": {
  "path": "measurements/left.csv",
  "name": "Left Main Speaker"
}
```

**4. Measurement with speaker model name:**
```json
"left": {
  "path": "measurements/left.csv",
  "speaker_name": "KEF R3"
}
```

**5. Multiple measurements (averaged):**
```json
"left": {
  "measurements": [
    "measurements/left_pos1.csv",
    "measurements/left_pos2.csv",
    "measurements/left_pos3.csv"
  ]
}
```

**6. Inline measurement data (no external file):**
```json
"left": {
  "frequencies": [20, 50, 100, 200, 500, 1000, 5000, 10000, 20000],
  "magnitude_db": [60, 72, 78, 80, 82, 80, 79, 75, 68],
  "phase_deg": [45, 30, 15, 5, -10, -30, -60, -90, -120]
}
```

### Single Speaker

The simplest configuration for a single speaker measurement.

```json
{
  "speakers": {
    "left": "measurements/left.csv",
    "right": "measurements/right.csv"
  }
}
```

Or with explicit path objects:

```json
{
  "speakers": {
    "left": {
      "path": "measurements/left.csv",
      "name": "Left Speaker"
    },
    "right": {
      "path": "measurements/right.csv",
      "name": "Right Speaker"
    }
  }
}
```

### Multi-Driver Speaker (Group)

For speakers with multiple drivers (woofer, midrange, tweeter) requiring crossover optimization.

> **Note:** For accurate crossover optimization, measurements should include phase data. The optimizer uses complex summation (vector sum) to model interference between drivers at crossover frequencies. Without phase data, the optimizer assumes 0° phase.

```json
{
  "speakers": {
    "left": {
      "name": "Left 2-Way Speaker",
      "speaker_name": "KEF R3",
      "measurements": [
        "measurements/left_woofer.csv",
        "measurements/left_tweeter.csv"
      ],
      "crossover": "main_crossover"
    }
  },
  "crossovers": {
    "main_crossover": {
      "type": "LR24"
    }
  }
}
```

**SpeakerGroup Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | **Yes** | Descriptive name for the speaker |
| `speaker_name` | string | No | Speaker model name (e.g., "KEF R3") |
| `measurements` | array | **Yes** | Array of measurement sources (order: lowest to highest frequency driver) |
| `crossover` | string | No | Key referencing a crossover in the `crossovers` map |

### Multiple Subwoofers (MultiSub)

For optimizing multiple subwoofers with individual gain and delay adjustments.

> **Note:** For accurate optimization, measurements **must** include phase data. The optimizer uses complex summation to model constructive/destructive interference between subwoofers.

```json
{
  "speakers": {
    "lfe": {
      "name": "Quad Subwoofers",
      "subwoofers": [
        "measurements/sub_front_left.csv",
        "measurements/sub_front_right.csv",
        "measurements/sub_rear_left.csv",
        "measurements/sub_rear_right.csv"
      ]
    }
  }
}
```

With all-pass optimization:
```json
{
  "speakers": {
    "lfe": {
      "name": "Quad Subwoofers",
      "subwoofers": [ ... ],
      "allpass_optimization": true
    }
  }
}
```

With production multi-seat MSO, each subwoofer entry can be a multi-measurement
source whose `measurements` array is ordered by seat. Every subwoofer must have
the same number of seat measurements, and each measurement must include phase:

```json
{
  "speakers": {
    "lfe": {
      "name": "Dual Subwoofers",
      "subwoofers": [
        {
          "measurements": [
            "measurements/sub1_seat1.csv",
            "measurements/sub1_seat2.csv",
            "measurements/sub1_seat3.csv"
          ]
        },
        {
          "measurements": [
            "measurements/sub2_seat1.csv",
            "measurements/sub2_seat2.csv",
            "measurements/sub2_seat3.csv"
          ]
        }
      ]
    }
  },
  "optimizer": {
    "multi_seat": {
      "enabled": true,
      "strategy": "modal_basis",
      "optimize_polarity": true,
      "allpass_filters_per_sub": 1,
      "per_sub_peq": true,
      "global_eq": true
    }
  }
}
```

**MultiSubGroup Fields:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `name` | string | **Yes** | - | Name of the subwoofer group |
| `speaker_name` | string | No | - | Speaker model name |
| `subwoofers` | array | **Yes** | - | Array of measurement sources for each subwoofer |
| `allpass_optimization` | boolean | No | `false` | Enable per-sub all-pass filter optimization (gain + delay + all-pass biquad) |

### Double Bass Array (DBA)

For optimizing front and rear bass arrays with phase cancellation. The rear array is automatically phase-inverted (180°).

> **Note:** For accurate DBA optimization, measurements **must** include phase data.

```json
{
  "speakers": {
    "lfe": {
      "name": "Double Bass Array",
      "front": [
        "measurements/front_sub1.csv",
        "measurements/front_sub2.csv"
      ],
      "rear": [
        "measurements/rear_sub1.csv",
        "measurements/rear_sub2.csv"
      ]
    }
  }
}
```

**DBAConfig Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | **Yes** | Name of the DBA system |
| `speaker_name` | string | No | Speaker model name |
| `front` | array | **Yes** | Measurements for the front array |
| `rear` | array | **Yes** | Measurements for the rear array (will be phase-inverted by adding 180°) |

### Gradient Cardioid (2 Subs)

For optimizing a pair of subwoofers in a gradient cardioid configuration (e.g., stacked front/back) to reduce rear radiation. Delay is calculated from the physical separation.

```json
{
  "speakers": {
    "lfe": {
      "name": "Cardioid Stack",
      "front": "measurements/sub_front.csv",
      "rear": "measurements/sub_rear.csv",
      "separation_meters": 0.5
    }
  }
}
```

**CardioidConfig Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | **Yes** | Name of the cardioid system |
| `speaker_name` | string | No | Speaker model name |
| `front` | source | **Yes** | Measurement for the front (primary) subwoofer |
| `rear` | source | **Yes** | Measurement for the rear (cancellation) subwoofer |
| `separation_meters` | number | **Yes** | Physical separation distance between acoustic centers (meters) |

---

## Crossovers Configuration

Defines crossover types and frequencies for multi-driver speakers.

```json
{
  "crossovers": {
    "2way_lr24": {
      "type": "LR24",
      "frequency": 2500
    },
    "2way_linear_phase": {
      "type": "LinearPhase",
      "frequency": 2500
    },
    "3way_auto": {
      "type": "LR24",
      "frequency_range": [200, 4000]
    }
  }
}
```

**CrossoverConfig Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type` | string | **Yes** | Crossover type (see below) |
| `frequency` | number (Hz) | No | Fixed crossover frequency (for 2-way speakers) |
| `frequencies` | array (Hz) | No | Fixed crossover frequencies (for 3-way+, e.g., `[500, 3000]`) |
| `frequency_range` | [min, max] | No | Frequency range for automatic optimization |

**Supported Crossover Types:**

| Type | Alias | Description |
|------|-------|-------------|
| `LR24` | `LR4` | Linkwitz-Riley 24 dB/oct (4th order) |
| `LR48` | `LR8` | Linkwitz-Riley 48 dB/oct (8th order) |
| `Butterworth12` | `BW12` | Butterworth 12 dB/oct (2nd order) |
| `Butterworth24` | `BW24` | Butterworth 24 dB/oct (4th order) |
| `LinearPhase` | `FIR`, `LPFIR` | FIR complementary low/high crossover with constant group delay and no phase rotation |

---

## Target Curve Configuration

Optional target frequency response to match.

**Predefined target:**
```json
{
  "target_curve": "flat"
}
```

**Custom CSV file:**
```json
{
  "target_curve": "targets/harman_curve.csv"
}
```

Predefined options: `"flat"`, `"harman"`

---

## Optimizer Configuration

Controls the optimization algorithm, constraints, and advanced features.

```json
{
  "optimizer": {
    "mode": "iir",
    "loss_type": "flat",
    "algorithm": "autoeq:cmaes",
    "num_filters": 7,
    "min_q": 0.5,
    "max_q": 6.0,
    "min_db": -12.0,
    "max_db": 4.0,
    "min_freq": 20.0,
    "max_freq": 1600.0,
    "max_iter": 50000,
    "peq_model": "pk",
    "refine": true,
    "local_algo": "cobyla",
    "psychoacoustic": true,
    "psychoacoustic_smoothing": {
      "low_freq_n": 48,
      "high_freq_n": 6,
      "low_freq": 100.0,
      "high_freq": 1000.0
    },
    "asymmetric_loss": true,
    "asymmetric_loss_config": {
      "peak_weight": 2.0,
      "dip_weight": 1.0,
      "bass_peak_weight": 5.0,
      "bass_dip_weight": 1.0,
      "transition_freq": 300.0
    },
    "perceptual_policy": {
      "preset": "music",
      "apply_defaults": true,
      "override_existing": false
    },
    "audibility_deadband": {
      "enabled": true,
      "bass_db": 0.25,
      "mid_db": 0.75,
      "treble_db": 1.0,
      "disable_below_schroeder": true
    },
    "high_frequency_correction": {
      "enabled": true,
      "start_hz": 1600.0,
      "extra_deadband_db": 0.75,
      "smoothing_n": 3,
      "max_q": 2.0
    },
    "early_late_correction": {
      "enabled": true,
      "direct_window_ms": 5.0,
      "early_window_ms": 30.0,
      "late_window_ms": 120.0
    },
    "validation_bundle": {
      "enabled": true,
      "abx": true,
      "mushra": true,
      "target_lufs": -23.0
    }
  }
}
```

**OptimizerConfig Fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `mode` | string | `"iir"` | Optimization mode: `"iir"`, `"fir"`, `"mixed"`, or `"mixed_phase"` |
| `processing_mode` | string | `"low_latency"` | V2 processing mode: `"low_latency"`, `"phase_linear"`, `"hybrid"`, `"mixed_phase"`, `"warped_iir"`, or `"kautz_modal"` |
| `fir` | object | - | FIR configuration (when mode is `"fir"` or `"mixed"`) |
| `mixed_config` | object | - | Mixed mode configuration for frequency-based crossover |
| `mixed_phase` | object | - | Mixed-phase correction config (when processing_mode is `"mixed_phase"`) |
| `loss_type` | string | `"flat"` | Loss function: `"flat"`, `"score"`, or `"epa"` (see [Loss Types](#loss-types) and [EPA Configuration](#epa-configuration)) |
| `epa_config` | object | - | Tuning knobs for the EPA psychoacoustic loss (consulted for any `loss_type`; required only when you want to override defaults). See [EPA Configuration](#epa-configuration) |
| `algorithm` | string | `"autoeq:cmaes"` | Optimization algorithm |
| `num_filters` | integer | `7` | Number of PEQ filters per channel |
| `min_q` | number | `0.5` | Minimum Q factor |
| `max_q` | number | `6.0` | Maximum Q factor |
| `min_db` | number | `-12.0` | Minimum gain in dB |
| `max_db` | number | `4.0` | Maximum gain in dB |
| `min_freq` | number (Hz) | `20.0` | Minimum frequency |
| `max_freq` | number (Hz) | `1600.0` | Maximum frequency |
| `max_iter` | integer | `50000` | Maximum optimization iterations |
| `population` | integer | `50` | Population size for population-based optimizers |
| `peq_model` | string | `"pk"` | PEQ model type |
| `seed` | integer | - | Random seed for reproducible results |
| `refine` | boolean | `true` | Enable hybrid two-stage optimization (DE global + COBYLA local) |
| `local_algo` | string | `"cobyla"` | Local optimizer for refinement stage (when `refine=true`) |
| `psychoacoustic` | boolean | `true` | Enable psychoacoustic variable smoothing before optimization |
| `psychoacoustic_smoothing` | object | - | Optional variable smoothing config. Defaults preserve `1/48` octave below 100 Hz through `1/6` octave above 1 kHz |
| `asymmetric_loss` | boolean | `true` | Penalize peaks 2x more than dips (psychoacoustically correct) |
| `asymmetric_loss_config` | object | - | Optional peak/dip weighting config. Defaults preserve peak `2.0`, dip `1.0`, bass peak `5.0`, bass dip `1.0`, transition `300 Hz` |
| `perceptual_policy` | object | - | Product preset (`reference`, `music`, `cinema`, `night`, `speech`) that fills coherent defaults for target, smoothing, loss, robustness, early-cue, and validation settings |
| `audibility_deadband` | object | - | JND-style residual deadband applied after smoothing so sub-threshold errors do not consume filters |
| `high_frequency_correction` | object | - | Safer opt-in high-frequency behavior: stronger smoothing/deadband and default-Q capping above `start_hz` |
| `early_late_correction` | object | - | Direct/early/late correction-energy report windows for FIR and mixed-phase safety advisories |
| `validation_bundle` | object | - | Emit `roomeq_validation_bundle.json` with loudness-matched ABX/MUSHRA descriptors and perceptual regression summaries |
| `tolerance` | number | `1e-5` | Optimization convergence tolerance (relative) |
| `atolerance` | number | `1e-5` | Optimization convergence tolerance (absolute) |
| `allow_delay` | boolean | - | Allow inter-speaker delay optimization. Default: false for IIR, true for FIR/mixed. |
| `target_response` | object | - | Unified target response configuration (shape, preference shelves, broadband pre-correction) |
| `excursion_protection` | object | - | Excursion protection for bookshelf speakers |
| `schroeder_split` | object | - | Different Q constraints above/below Schroeder frequency |
| `auto_optimizer` | object | - | Opt-in automatic filter count, Q bound, and gain bound selection |
| `smoothness_penalty` | object | - | TV²-style log-frequency curvature penalty on the correction curve |
| `phase_alignment` | object | - | Phase alignment for subwoofer integration |
| `multi_seat` | object | - | Multi-seat variance optimization |
| `vog` | object | - | Voice of God (timbre matching) |
| `multi_measurement` | object | - | Multi-measurement optimization strategy |
| `decomposed_correction` | object | - | Decomposed correction |
| `sub_config` | object | - | Subwoofer-specific optimizer overrides (num_filters, max_db, Q range) |
| `channel_matching` | object | - | Inter-channel consistency correction (post-hoc PEQ matching) |

### Optimization Algorithms

| Algorithm | Description |
|-----------|-------------|
| `autoeq:cmaes` | CMA-ES (default global optimizer) |
| `autoeq:de` | Differential Evolution |
| `cobyla` | COBYLA (Constrained Optimization BY Linear Approximations) |
| `de` | Bare DE alias |
| `nlopt:cobyla` | NLopt COBYLA variant |
| `nlopt:isres` | Improved Stochastic Ranking Evolution Strategy |
| `mh:firefly` | Firefly Algorithm |
| `mh:pso` | Particle Swarm Optimization |

### Local Algorithms (for `refine`)

| Algorithm | Description |
|-----------|-------------|
| `cobyla` | COBYLA (default) |
| `bobyqa` | Bound Optimization BY Quadratic Approximations |
| `sbplx` | Subplex method |

### Loss Types

| Type | Description |
|------|-------------|
| `flat` | Optimize for flat frequency response. Internally uses an ERB-weighted + band-weighted MSE (70/30 blend) for better perceptual relevance than a plain MSE. |
| `score` | Optimize for Harman/Olive preference score (bass boost + flat PIR). |
| `epa` | EPA (Evaluation / Potency / Activity) psychoacoustic loss. Combines an ERB-weighted flatness term with sharpness (vs. target), roughness (spectral beating), and loudness-balance penalties derived from Zwicker's models. Tuning knobs live in [`epa_config`](#epa-configuration). |

### EPA Configuration

EPA tuning knobs. Every field is optional and serde-defaulted, so omitting
`epa_config` entirely gives sensible defaults.

```json
{
  "optimizer": {
    "loss_type": "epa",
    "epa_config": {
      "listening_level_phon": 75.0,
      "target_sharpness": 1.2,
      "max_roughness": 0.5,
      "evaluation_weight": 0.6,
      "potency_weight": 0.2,
      "activity_weight": 0.2,
      "flatness_erb_weight": 1.0,
      "flatness_band_weight": 0.0,
      "temporal_masking": {
        "enabled": true,
        "weight": 0.15,
        "profile": "mixed",
        "ir_enabled": true,
        "ir_weight": 0.05,
        "pre_mask_ms": 3.0,
        "post_mask_ms": 120.0,
        "pre_ringing_weight": 2.0,
        "post_ringing_weight": 1.0,
        "ir_audibility_threshold_db": -45.0
      },
      "flatness_band_weights": {
        "bass_min": 20.0,
        "bass_max": 200.0,
        "mid_min": 200.0,
        "mid_max": 4000.0,
        "treble_min": 4000.0,
        "treble_max": 20000.0,
        "bass_weight": 2.0,
        "mid_weight": 1.0,
        "treble_weight": 0.8
      }
    }
  }
}
```

**EpaConfig fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `listening_level_phon` | number | `75.0` | Presentation level in phon. Also used to denormalize level-relative measurement curves (~0 dB mean) back to absolute dB SPL before running the Zwicker loudness model. |
| `target_sharpness` | number | `1.2` | Target Zwicker sharpness in acum. `1.0` ≈ broadband noise character; higher = brighter. |
| `max_roughness` | number | `0.5` | Roughness threshold above which the penalty kicks in (spectral beating between close modes). |
| `evaluation_weight` | number | `0.6` | Weight of the Evaluation (quality) dimension in the composite score. |
| `potency_weight` | number | `0.2` | Weight of the Potency (energy) dimension. |
| `activity_weight` | number | `0.2` | Weight of the Activity (temporal complexity) dimension. |
| `flatness_erb_weight` | number | `1.0` | ERB-weighted blend for the flatness term. Default pure ERB because EPA already has band-sensitive sharpness/roughness/loudness-balance terms. |
| `flatness_band_weight` | number | `0.0` | Band-weighted blend for the flatness term. Increase to add an explicit bass/mid/treble bias on top of the ERB flatness. |
| `flatness_band_weights` | object | see `FrequencyBandWeights` defaults | Per-band frequency ranges and weights. Only consulted when `flatness_band_weight > 0`. |
| `temporal_masking` | object | see below | Modal and FIR impulse-response temporal-masking controls. The modal path uses detected room-mode Q/prominence data inside the EPA optimizer loss; the FIR path analyzes generated FIR impulse responses for pre/post ringing under pre- and post-masking windows. |

**TemporalMaskingConfig fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | boolean | `true` | Enable modal temporal masking when detected room-mode data is available. |
| `weight` | number | `0.15` | Weight for the modal ringing penalty in the EPA optimizer loss. |
| `profile` | string | `"mixed"` | Programme material profile: `"transient"`, `"mixed"`, or `"sustained"`. Transient material applies the strongest temporal penalty. |
| `ir_enabled` | boolean | `true` | Enable true FIR impulse-response temporal masking analysis when FIR coefficients are available. |
| `ir_weight` | number | `0.05` | Weight used for the scalar FIR temporal masking penalty reported in output metrics. |
| `pre_mask_ms` | number | `3.0` | Pre-masking window before the main impulse. Pre-ringing inside this window is partially masked; earlier pre-ringing is fully audible. |
| `post_mask_ms` | number | `120.0` | Post-masking window after the main impulse. Post-ringing grows more audible as it decays beyond this window. |
| `pre_ringing_weight` | number | `2.0` | Relative weight for audible pre-ringing, which is usually more objectionable than post-ringing. |
| `post_ringing_weight` | number | `1.0` | Relative weight for audible post-ringing. |
| `ir_audibility_threshold_db` | number | `-45.0` | Audibility floor for weighted FIR pre/post ringing energy, in dB relative to the main impulse peak. |

**FrequencyBandWeights fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `bass_min` / `bass_max` | number (Hz) | `20.0` / `200.0` | Bass band bounds. |
| `mid_min` / `mid_max` | number (Hz) | `200.0` / `4000.0` | Midrange band bounds. |
| `treble_min` / `treble_max` | number (Hz) | `4000.0` / `20000.0` | Treble band bounds. |
| `bass_weight` | number | `2.0` | Weight applied to the bass RMS contribution. |
| `mid_weight` | number | `1.0` | Weight applied to the midrange RMS contribution. |
| `treble_weight` | number | `0.8` | Weight applied to the treble RMS contribution. |

The EPA metrics (`pre`/`post` per channel) are always emitted in the output
JSON under `metadata.epa_per_channel`, with a whole-system BS.1770-style
aggregate under `metadata.epa_multichannel`, regardless of which `loss_type`
is selected — EPA is usable as a diagnostic even when the optimizer is
minimizing a different objective. See [`OUTPUT_FORMAT.md`](./OUTPUT_FORMAT.md)
for the EPA score fields.

### PEQ Models

| Model | Description |
|-------|-------------|
| `pk` | Peaking EQ only |
| `ls-pk-hs` | Low shelf + Peaking + High shelf |
| `free` | Unconstrained filter types |

---

## FIR Configuration

When `mode` is `"fir"` or `"mixed"`, a WAV file is generated per channel (e.g., `left_fir.wav`) and referenced in the output JSON via a convolution plugin.

```json
{
  "optimizer": {
    "mode": "fir",
    "fir": {
      "taps": 4096,
      "phase": "kirkeby",
      "correct_excess_phase": false,
      "phase_smoothing": 0.167
    }
  }
}
```

With pre-ringing suppression:
```json
{
  "optimizer": {
    "mode": "fir",
    "fir": {
      "taps": 4096,
      "phase": "kirkeby",
      "pre_ringing": {
        "threshold_db": -30.0,
        "max_time_s": 0.005
      }
    }
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `taps` | integer | `4096` | Number of FIR filter taps (64-65536) |
| `phase` | string | `"kirkeby"` | Phase type: `"linear"` (symmetric FIR), `"minimum"` (minimum-phase FIR), or `"kirkeby"` (magnitude limits) |
| `correct_excess_phase` | boolean | `false` | Correct excess phase (kirkeby only). Requires clean phase measurements. |
| `phase_smoothing` | number | `0.167` | Phase smoothing width in octaves (0 = disabled). Applied via group delay smoothing when excess phase correction is enabled. |
| `pre_ringing` | object | - | Pre-ringing suppression configuration |

**PreRinging Fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `threshold_db` | number | `-30.0` | Maximum pre-ringing level in dB relative to main tap |
| `max_time_s` | number | `0.005` | Maximum pre-ringing time in seconds |

---

## Mixed Mode Configuration

When `mode` is `"mixed"` and `mixed_config` is provided, the optimizer uses different filter types for different frequency bands separated by a crossover.

```json
{
  "optimizer": {
    "mode": "mixed",
    "mixed_config": {
      "crossover_freq": 300.0,
      "crossover_type": "LR24",
      "fir_band": "low"
    },
    "fir": {
      "taps": 4096,
      "phase": "kirkeby"
    }
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `crossover_freq` | number (Hz) | `300.0` | Crossover frequency dividing IIR and FIR bands |
| `crossover_type` | string | `"LR24"` | Crossover filter type: `"LR24"`, `"LR48"`, `"LR4"`, `"LR8"` |
| `fir_band` | string | `"low"` | Which band uses FIR: `"low"` or `"high"`. FIR is typically better for low frequencies. |

---

## Mixed-Phase Correction Configuration

When `processing_mode` is `"mixed_phase"`, decomposes the measurement into minimum-phase (corrected by IIR) and excess phase (corrected by short FIR). Requires phase data.

```json
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
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_fir_length_ms` | number | `10.0` | Maximum FIR length in ms for excess phase correction |
| `pre_ringing_threshold_db` | number | `-30.0` | Pre-ringing threshold in dB |
| `min_spatial_depth` | number | `0.5` | Minimum spatial correction depth (0.0-1.0) |
| `phase_smoothing_octaves` | number | `0.167` | Phase smoothing width in octaves (1/6 octave) |

---

## Target Response Configuration

`target_response` is the unified entry point for target shaping. It
bundles the target curve shape, the user-preference shelves that layer
on top of it, and the broadband pre-correction toggle (see the
[Broadband Pre-correction](#broadband-pre-correction) section below).
The Harman-style tilt (-0.8 dB/octave referenced at 1 kHz) is
psychoacoustically preferred for in-room listening.

```json
{
  "optimizer": {
    "target_response": {
      "shape": "harman"
    }
  }
}
```

With a custom slope, bass preference shelf, and broadband pre-correction:
```json
{
  "optimizer": {
    "target_response": {
      "shape": "custom",
      "slope_db_per_octave": -1.0,
      "reference_freq": 1000,
      "preference": {
        "bass_shelf_db": 3.0,
        "bass_shelf_freq": 150,
        "treble_shelf_db": 0.0,
        "treble_shelf_freq": 8000
      },
      "broadband_precorrection": true
    }
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `shape` | string | `"flat"` | Target shape: `"flat"`, `"harman"` (-0.8 dB/oct at 1 kHz), `"custom"`, `"file"` (load CSV from `curve_path`), `"from_measurement"` (auto-derive slope from input curve) |
| `slope_db_per_octave` | number | `-0.8` | Slope in dB/octave (negative = downward tilt). Used when `shape == "custom"`. Ignored for `"flat"`, `"harman"`, `"file"`, `"from_measurement"`. |
| `reference_freq` | number (Hz) | `1000` | Frequency where the target slope passes through 0 dB |
| `curve_path` | string (path) | - | Path to a target-curve CSV (required when `shape == "file"`) |
| `preference.bass_shelf_db` | number (dB) | `0.0` | Bass shelf preference layered on top of the target shape |
| `preference.bass_shelf_freq` | number (Hz) | `200` | Bass shelf transition frequency |
| `preference.treble_shelf_db` | number (dB) | `0.0` | Treble shelf preference layered on top of the target shape |
| `preference.treble_shelf_freq` | number (Hz) | `8000` | Treble shelf transition frequency |
| `broadband_precorrection` | boolean | `false` | Enable a preliminary broadband shelf + gain fit before fine-grained PEQ optimization. Useful when `min_freq`/`max_freq` restrict the main pass and overall tonal balance would otherwise drift. |

**Auto-derive from measurement** (`"from_measurement"`): the optimizer fits
a least-squares line through the input measurement curve (200 Hz – 10 kHz
by default) and uses the resulting slope as the target. This preserves
the speaker's natural broadband character while correcting room anomalies.

```json
{
  "optimizer": {
    "target_response": {
      "shape": "from_measurement"
    }
  }
}
```

**Load from file** (`"file"`): load a target curve directly from a CSV on
disk. `curve_path` must point at a two-column (frequency, magnitude) CSV.

```json
{
  "optimizer": {
    "target_response": {
      "shape": "file",
      "curve_path": "targets/harman-in-room.csv"
    }
  }
}
```

---

## Excursion Protection Configuration

Detects the speaker's F3 rolloff and generates a highpass filter to prevent dangerous over-boost of bass frequencies. Recommended for bookshelf speakers.

```json
{
  "optimizer": {
    "excursion_protection": {
      "enabled": true,
      "auto_detect_f3": true,
      "f3_reference_min_hz": 100.0,
      "f3_reference_max_hz": 200.0,
      "filter_order": 4,
      "filter_type": "linkwitzriley",
      "margin_octaves": 0.25
    }
  }
}
```

With manual F3 override:
```json
{
  "optimizer": {
    "excursion_protection": {
      "enabled": true,
      "auto_detect_f3": false,
      "manual_f3_hz": 60,
      "filter_order": 4
    }
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | boolean | `false` | Enable excursion protection |
| `auto_detect_f3` | boolean | `true` | Auto-detect F3 from measurement |
| `manual_f3_hz` | number (Hz) | - | Manual F3 override (used if `auto_detect_f3` is false) |
| `f3_reference_min_hz` | number (Hz) | `100.0` | Lower bound of the auto-detection reference band |
| `f3_reference_max_hz` | number (Hz) | `200.0` | Upper bound of the auto-detection reference band |
| `filter_order` | integer | `4` | Filter order: `2` (12 dB/oct), `4` (24 dB/oct), `6` (36 dB/oct), `8` (48 dB/oct) |
| `filter_type` | string | `"linkwitzriley"` | Highpass filter type: `"linkwitzriley"` or `"butterworth"` |
| `margin_octaves` | number | `0.25` | Safety margin in octaves below F3 for HPF placement |

---

## Schroeder Split Configuration

Applies different Q constraints below and above the Schroeder frequency:
- **Below**: high-Q narrow filters to address room modes
- **Above**: low-Q broad filters for gentle tone control

```json
{
  "optimizer": {
    "schroeder_split": {
      "enabled": true,
      "schroeder_freq": 300,
      "low_freq_config": {
        "max_q": 10.0,
        "min_q": 0.5,
        "allow_boost": false
      },
      "high_freq_config": {
        "max_q": 1.0,
        "shelving_only": false
      }
    }
  }
}
```

With automatic Schroeder frequency from room dimensions:
```json
{
  "optimizer": {
    "schroeder_split": {
      "enabled": true,
      "room_dimensions": {
        "length": 6.0,
        "width": 4.5,
        "height": 2.8
      }
    }
  }
}
```

**SchroederSplitConfig Fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | boolean | `false` | Enable Schroeder split optimization |
| `schroeder_freq` | number (Hz) | `300` | Schroeder frequency (typical: 200-500 Hz for domestic rooms) |
| `room_dimensions` | object | - | Room dimensions for automatic Schroeder frequency calculation |
| `low_freq_config` | object | - | Low frequency filter configuration (below Schroeder) |
| `high_freq_config` | object | - | High frequency filter configuration (above Schroeder) |

**RoomDimensions Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `length` | number (m) | **Yes** | Room length in meters |
| `width` | number (m) | **Yes** | Room width in meters |
| `height` | number (m) | **Yes** | Room height in meters |

**LowFreqFilterConfig Fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_q` | number | `10.0` | Maximum Q factor for low frequency filters |
| `min_q` | number | `0.5` | Minimum Q factor |
| `allow_boost` | boolean | `false` | Allow boost (`true`) or cuts only (`false`). Cuts-only is recommended for room modes. |
| `max_db` | number | - | Maximum boost/cut in dB for below-Schroeder filters. Room modes can be 15+ dB. When set, allows wider range than the global `max_db`. Omit to use global `max_db`. |

**HighFreqFilterConfig Fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_q` | number | `1.0` | Maximum Q factor for high frequency filters |
| `shelving_only` | boolean | `false` | Use shelving filters only (no parametric peaks) |

---

## Auto Optimizer Configuration

Automatically resolves the maximum PEQ filter count plus Q and gain bounds from
the measured response, detected F3, Schroeder frequency, target tilt, and
subwoofer/main-channel role. The final number of active filters can still be
lower because RoomEQ's adaptive optimizer and backward elimination prune
low-value filters.

```json
{
  "optimizer": {
    "auto_optimizer": {
      "enabled": true,
      "max_filters": 12
    }
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | boolean | `false` | Enable automatic optimizer parameter selection |
| `filter_count` | boolean | `true` | Automatically choose the maximum PEQ filter count |
| `q_bounds` | boolean | `true` | Automatically choose `min_q`, `max_q`, and Schroeder low/high Q bounds |
| `gain_bounds` | boolean | `true` | Automatically choose `min_db`, `max_db`, below-F3/below-Schroeder boost envelope, and Schroeder low-frequency gain range |
| `min_filters` | integer | `1` | Lower clamp for automatic filter-count selection |
| `max_filters` | integer | `12` | Upper clamp for automatic filter-count selection |

---

## Smoothness Penalty Configuration

Adds a second-difference regularizer on the correction curve in log-frequency.
This discourages narrow opposite-cancel wiggles while preserving broad tilts
and shelves.

```json
{
  "optimizer": {
    "smoothness_penalty": {
      "tv2_weight": 0.05,
      "schroeder_hz": 300.0,
      "modal_weight_scale": 0.1,
      "exponent": 1.0
    }
  }
}
```

If `schroeder_hz` is omitted, RoomEQ derives it from `schroeder_split` when
available (including room-dimension auto mode); otherwise it remains unset.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `tv2_weight` | number | `0.0` | Penalty weight. `0` disables smoothness regularization. Suggested start: `0.05`. |
| `schroeder_hz` | number (Hz) | - | Optional cutoff for modal-region relaxation. |
| `modal_weight_scale` | number | `0.1` | Multiplier below `schroeder_hz` (`0` fully exempts modal region). |
| `exponent` | number | `1.0` | Per-bin exponent: `1.0` (TV²-like sparse curvature), `2.0` (L2 smoothing). |

---

## Phase Alignment Configuration

Optimizes delay and polarity to maximize energy sum in the crossover region between subwoofer and main speakers.

```json
{
  "optimizer": {
    "phase_alignment": {
      "enabled": true,
      "min_freq": 60,
      "max_freq": 100,
      "optimize_polarity": true,
      "max_delay_ms": 3.0
    }
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | boolean | `true` | Enable phase alignment optimization |
| `min_freq` | number (Hz) | `60` | Minimum frequency for optimization |
| `max_freq` | number (Hz) | `100` | Maximum frequency for optimization |
| `optimize_polarity` | boolean | `true` | Optimize polarity (normal vs inverted) |
| `max_delay_ms` | number (ms) | `3.0` | Maximum delay in milliseconds |

---

## Multi-Seat Configuration

Optimizes subwoofer delay, gain, optional polarity, optional all-pass, and
optional PEQ across multiple listening positions. In production multi-sub
workflows this is enabled by providing each subwoofer as a multi-measurement
source in the `subwoofers` array.

```json
{
  "optimizer": {
    "multi_seat": {
      "enabled": true,
      "strategy": "minimize_variance"
    }
  }
}
```

With primary seat constraints:
```json
{
  "optimizer": {
    "multi_seat": {
      "enabled": true,
      "strategy": "primary_with_constraints",
      "primary_seat": 0,
      "max_deviation_db": 6
    }
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | boolean | `false` | Enable multi-seat optimization |
| `strategy` | string | `"minimize_variance"` | Strategy: `"minimize_variance"`, `"primary_with_constraints"`, `"average"`, `"modal_basis"` |
| `primary_seat` | integer | `0` | Index of primary seat (0-based, used with `primary_with_constraints`) |
| `max_deviation_db` | number (dB) | `6` | Maximum allowed deviation at non-primary seats |
| `optimize_polarity` | boolean | `false` | Search normal/inverted polarity per subwoofer |
| `allpass_filters_per_sub` | integer | `0` | Number of per-sub all-pass filters to optimize in the MSO pass |
| `per_sub_peq` | boolean | `true` | Optimize per-sub PEQ from that sub's seat measurements before MSO |
| `global_eq` | boolean | `true` | Optimize shared EQ on the post-MSO combined response across seats |
| `all_channel_enabled` | boolean | `true` | Enable derived multi-seat correction for non-sub home-cinema channels |
| `all_channel_strategy` | string | `"spatial_robustness"` | Multi-measurement PEQ strategy used for derived all-channel correction and multi-seat sub PEQ |
| `seat_weights` | array | - | Optional per-seat weights |
| `primary_seat_weight` | number | `2.0` | Weight multiplier for the primary seat with `primary_with_constraints` |

`modal_basis` uses complex subwoofer transfer functions to extract dominant
seat-to-seat modal patterns, then optimizes sub gain, delay, polarity, and
configured all-pass filters against that modal basis. It requires phase data
for every sub/seat measurement.

### Continuous Listening-Area Strategy

Setting `multi_seat.strategy = "continuous_area"` switches MSO from a discrete
seats array to a continuous probability density π(p) over positions. Calibration
measurements at K seats are spatially interpolated (IDW on log-magnitude with
shortest-arc phase) at each of Q quadrature points, and the per-quadrature
flatness loss is scalarised via expected value, worst-case, or CVaR.

```json
"multi_seat": {
  "enabled": true,
  "strategy": "continuous_area",
  "continuous_area": {
    "dimensions": 2,
    "bounds": [[0.0, 1.5], [0.0, 0.6]],
    "seat_positions": [
      [0.25, 0.30],
      [0.75, 0.30],
      [0.25, 0.50],
      [0.75, 0.50]
    ],
    "prior":        { "kind": "uniform" },
    "quadrature":   { "kind": "sobol", "num_points": 64, "seed": 42 },
    "scalarisation":{ "kind": "expected_value" },
    "idw_power": 2.0
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `dimensions` | integer | - | 1, 2, or 3 |
| `bounds` | array of `[lo, hi]` pairs | - | Axis-aligned bounding box; length must equal `dimensions` |
| `seat_positions` | array of arrays | - | Spatial coordinates of each calibration seat; outer length = #seats, inner length = `dimensions` |
| `prior.kind` | string | `"uniform"` | `"uniform"` or `"gaussian"` (Gaussian also takes `mean`, `cov_diag`, `truncation_sigmas`) |
| `quadrature.kind` | string | `"sobol"` | `"sobol"`, `"latin_hypercube"`, or `"gauss_legendre"` |
| `scalarisation.kind` | string | `"expected_value"` | `"expected_value"`, `"worst_case"`, or `"cvar"` |
| `idw_power` | number | `2.0` | IDW power exponent for the spatial interpolator |

---

## Broadband Pre-correction

Using `min_freq` / `max_freq` limits the optimization range, which can
leave spectral imbalances outside that band. Broadband pre-correction
solves this with a preliminary alignment pass:

1. Analyzes the full 20 Hz – 20 kHz spectrum.
2. Fits Low Shelf (200 Hz), High Shelf (4 kHz), and Gain filters to
   match the target curve.
3. Applies this correction *before* the fine-grained PEQ optimization.

This ensures the overall tonal balance is correct even when the main
optimizer focuses only on modal correction below 1 kHz. It is controlled
by the `broadband_precorrection` boolean inside
[`target_response`](#target-response-configuration):

```json
{
  "optimizer": {
    "target_response": {
      "shape": "harman",
      "broadband_precorrection": true
    }
  }
}
```

---

## Voice of God (Timbre Matching)

Matches the tonal character of all speakers to a reference channel.

```json
{
  "optimizer": {
    "vog": {
      "enabled": true,
      "reference_channel": "Center"
    }
  }
}
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `enabled` | boolean | No | `false` | Enable Voice of God optimization |
| `reference_channel` | string | **Yes** | - | Reference channel name (e.g., "Center" or "Left") |

---

## Multi-Measurement Optimization

When a speaker has multiple measurements (different listening positions), controls how they are combined during optimization.

```json
{
  "optimizer": {
    "multi_measurement": {
      "strategy": "spatial_robustness",
      "spatial_robustness": {
        "variance_threshold_db": 3.0,
        "transition_width_db": 2.0,
        "min_correction_depth": 0.1,
        "mask_smoothing_octaves": 0.167
      }
    }
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `strategy` | string | `"average"` | Strategy: `"average"`, `"weighted_sum"`, `"minimax"`, `"variance_penalized"`, `"spatial_robustness"`, `"minimax_uncertainty"` |
| `weights` | array | - | Weights for `weighted_sum` (normalized internally). Equal if omitted. |
| `variance_lambda` | number | `1.0` | Lambda for `variance_penalized`. Higher = more consistent. |
| `spatial_robustness` | object | - | Configuration for `spatial_robustness` strategy |
| `bootstrap_uncertainty` | object | - | Configuration for `minimax_uncertainty` strategy |

**Multi-Measurement Strategies:**

| Strategy | Description |
|----------|-------------|
| `average` | RMS-average curves, optimize on average (default) |
| `weighted_sum` | loss = sum(w_i * loss_i) - weighted sum of per-measurement losses |
| `minimax` | loss = max(loss_i) - optimize worst case across all measurements |
| `variance_penalized` | loss = mean(loss_i) + lambda * var(loss_i) - balance quality + consistency |
| `spatial_robustness` | RMS-average + correction depth mask based on spatial variance |
| `minimax_uncertainty` | Case-bootstrap the input curves, then optimise worst-case (or CVaR) loss across the resampled bank — robust to measurement noise / mic jitter |

**SpatialRobustness Fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `variance_threshold_db` | number | `3.0` | Variance threshold (dB) below which full correction is allowed |
| `transition_width_db` | number | `2.0` | Transition width (dB) for sigmoid blending |
| `min_correction_depth` | number | `0.1` | Minimum correction depth (0.0-1.0) |
| `mask_smoothing_octaves` | number | `0.167` | Smoothing width in octaves for the correction depth mask |

**BootstrapUncertainty Fields** (used when `strategy == "minimax_uncertainty"`):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `num_resamples` | integer | `400` | Number of case-bootstrap resamples B over the input curves |
| `alpha` | number | `0.10` | Two-sided confidence level (only affects diagnostic plots; the optimizer uses all B resamples) |
| `seed` | integer | `0xC0FFEE` | PRNG seed for reproducibility |
| `scalarisation` | string | `"worst_case"` | How to combine the B losses: `"worst_case"` (max) or `"cvar"` (mean of worst α-tail) |
| `cvar_alpha` | number | `0.20` | Tail fraction for CVaR (only when `scalarisation == "cvar"`) |

---

## Decomposed Correction

Applies frequency-dependent correction weights based on acoustic decomposition. Room modes get aggressive correction, steady-state response gets gentle correction, early reflections get reduced correction.

```json
{
  "optimizer": {
    "decomposed_correction": {
      "schroeder_freq": 200,
      "room_dimensions": {
        "length": 4.0,
        "width": 3.0,
        "height": 2.5
      },
      "min_mode_q": 3.0,
      "min_mode_prominence_db": 3.0,
      "mode_correction_weight": 1.0,
      "early_reflection_weight": 0.3,
      "steady_state_weight": 0.5,
      "fdw_enabled": true,
      "fdw_cycles": 8.0,
      "fdw_min_window_ms": 3.0,
      "fdw_max_window_ms": 500.0,
      "fdw_smoothing_octaves": 0.041666666666666664
    }
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `schroeder_freq` | number (Hz) | `200` | Fallback Schroeder frequency when no IR or no `room_dimensions`. Below: modal, above: statistical. |
| `room_dimensions` | object | - | Optional L × W × H in metres. When both this and `ssir_wav_path` are set, the optimizer measures RT60 from the recorded impulse response (Schroeder backward integration) and computes the Schroeder frequency as `2000 · √(RT60 / V)`, overriding the fallback `schroeder_freq` above. |
| `min_mode_q` | number | `3.0` | Minimum Q to qualify as a room mode |
| `min_mode_prominence_db` | number | `3.0` | Minimum prominence (dB) for mode detection |
| `mode_correction_weight` | number | `1.0` | Correction weight for room modes (0.0-1.0) |
| `early_reflection_weight` | number | `0.3` | Correction weight for early reflections (0.0-1.0) |
| `steady_state_weight` | number | `0.5` | Correction weight for steady-state above Schroeder (0.0-1.0) |
| `fdw_enabled` | boolean | `true` | Enable Frequency-Dependent Windowing when `ssir_wav_path` provides an impulse response |
| `fdw_cycles` | number | `8.0` | FDW window length in cycles before min/max clamping |
| `fdw_min_window_ms` | number | `3.0` | Minimum FDW window length |
| `fdw_max_window_ms` | number | `500.0` | Maximum FDW window length |
| `fdw_smoothing_octaves` | number | `0.041666666666666664` | FDW smoothing width (1/24 octave by default) |

---

## Subwoofer Optimizer Configuration

Subwoofers deal with dense room modes that can be 15+ dB. They need more filters and wider dB range than main speakers. When `sub_config` is set, these values override the global optimizer parameters for channels identified as subwoofers (via `system.subwoofers` mapping or name-based detection for "lfe"/"sub*" channels).

```json
{
  "optimizer": {
    "sub_config": {
      "num_filters": 10,
      "max_db": 18.0,
      "min_db": -18.0,
      "min_q": 0.5,
      "max_q": 10.0
    }
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `num_filters` | integer | `10` | Number of PEQ filters for subwoofer channels |
| `max_db` | number (dB) | `18.0` | Maximum boost (room gain can be 15+ dB at resonances) |
| `min_db` | number (dB) | `-18.0` | Maximum cut |
| `min_q` | number | `0.5` | Minimum Q factor |
| `max_q` | number | `10.0` | Maximum Q factor (higher Q for narrow room modes) |

---

## Channel Matching Configuration

After independent per-channel EQ optimization, channels may have residual SPL differences at specific frequencies. The channel matching post-processing pass adds a small number of parametric filters per channel to reduce the deviation from the group average.

```json
{
  "optimizer": {
    "channel_matching": {
      "enabled": true,
      "threshold_db": 1.5,
      "max_filters": 3
    }
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | boolean | `true` | Enable inter-channel matching correction |
| `threshold_db` | number (dB) | `1.5` | Midrange ICD RMS threshold below which no correction is applied |
| `max_filters` | integer | `3` | Maximum additional PEQ filters per channel for matching |

The matching filters are labeled `"channel_matching"` in the output DSP chain and target the largest per-frequency deviations from the group average. The inter-channel deviation (ICD) metric is reported in the output metadata.

---

## Measurement CSV Format

Measurement files should be CSV with these columns:

```csv
freq,spl,phase
20,75.0,45.2
50,78.0,30.1
100,80.0,15.5
...
20000,60.0,-90.3
```

| Column | Type | Required | Description |
|--------|------|----------|-------------|
| `freq` | number (Hz) | **Yes** | Frequency |
| `spl` | number (dB) | **Yes** | Sound Pressure Level |
| `phase` | number (degrees) | No | Phase response (recommended for subwoofer and multi-driver configs) |

---

## Complete Examples

### Example 1: Simple Stereo System

```json
{
  "speakers": {
    "left": "measurements/left.csv",
    "right": "measurements/right.csv"
  },
  "optimizer": {
    "num_filters": 7,
    "algorithm": "autoeq:cmaes",
    "max_iter": 50000,
    "min_freq": 20.0,
    "max_freq": 1600.0
  }
}
```

### Example 2: 2.1 System with Subwoofer

```json
{
  "version": "1.3.0",
  "system": {
    "model": "stereo",
    "speakers": {
      "L": "left",
      "R": "right",
      "LFE": "sub"
    },
    "subwoofers": {
      "config": "single",
      "crossover": "bass_xover",
      "sub": "L"
    }
  },
  "speakers": {
    "left": "measurements/left_speaker.csv",
    "right": "measurements/right_speaker.csv",
    "sub": "measurements/subwoofer.csv"
  },
  "crossovers": {
    "bass_xover": {
      "type": "LR24",
      "frequency": 80.0
    }
  },
  "optimizer": {
    "num_filters": 7,
    "algorithm": "autoeq:cmaes",
    "max_iter": 50000,
    "min_freq": 20.0,
    "max_freq": 1600.0
  }
}
```

### Example 3: 2-Way Active Speaker

```json
{
  "speakers": {
    "left": {
      "name": "Left 2-Way",
      "speaker_name": "KEF R3",
      "measurements": [
        "measurements/left_woofer.csv",
        "measurements/left_tweeter.csv"
      ],
      "crossover": "main_xo"
    },
    "right": {
      "name": "Right 2-Way",
      "speaker_name": "KEF R3",
      "measurements": [
        "measurements/right_woofer.csv",
        "measurements/right_tweeter.csv"
      ],
      "crossover": "main_xo"
    }
  },
  "crossovers": {
    "main_xo": {
      "type": "LR24",
      "frequency_range": [1500, 3500]
    }
  },
  "optimizer": {
    "num_filters": 7,
    "max_iter": 50000,
    "psychoacoustic": true,
    "asymmetric_loss": true
  }
}
```

### Example 4: Multi-Sub System with Spatial Robustness

```json
{
  "speakers": {
    "left": "measurements/left.csv",
    "right": "measurements/right.csv",
    "lfe": {
      "name": "Dual Subs",
      "subwoofers": [
        "measurements/sub_front.csv",
        "measurements/sub_rear.csv"
      ],
      "allpass_optimization": true
    }
  },
  "optimizer": {
    "num_filters": 7,
    "multi_measurement": {
      "strategy": "spatial_robustness",
      "spatial_robustness": {
        "variance_threshold_db": 3.0,
        "min_correction_depth": 0.1
      }
    },
    "decomposed_correction": {
      "schroeder_freq": 200,
      "mode_correction_weight": 1.0,
      "steady_state_weight": 0.5
    }
  }
}
```
