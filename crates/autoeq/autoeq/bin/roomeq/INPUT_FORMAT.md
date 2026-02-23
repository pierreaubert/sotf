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
  "version": "1.1.0",
  "system": { ... },
  "speakers": { ... },
  "crossovers": { ... },
  "target_curve": "...",
  "optimizer": { ... }
}
```

### Top-Level Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `version` | string | No | `"1.1.0"` | Configuration version (semantic versioning) |
| `system` | object | No | - | System topology and logical channel mapping |
| `speakers` | object | **Yes** | - | Map of channel names to speaker configurations |
| `crossovers` | object | No | - | Crossover configurations referenced by multi-driver speakers |
| `target_curve` | string | No | - | Target frequency response curve |
| `optimizer` | object | No | defaults | Optimization parameters |

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

Measurements can be specified in four ways:

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

**4. Multiple measurements (averaged):**
```json
"left": [
  "measurements/left_pos1.csv",
  "measurements/left_pos2.csv",
  "measurements/left_pos3.csv"
]
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

**MultiSubGroup Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | **Yes** | Name of the subwoofer group |
| `subwoofers` | array | **Yes** | Array of measurement sources for each subwoofer |

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
    "algorithm": "cobyla",
    "num_filters": 10,
    "min_q": 0.5,
    "max_q": 10.0,
    "min_db": -12.0,
    "max_db": 12.0,
    "min_freq": 20.0,
    "max_freq": 20000.0,
    "max_iter": 10000,
    "peq_model": "pk",
    "refine": true,
    "local_algo": "cobyla",
    "psychoacoustic": true,
    "asymmetric_loss": true
  }
}
```

**OptimizerConfig Fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `mode` | string | `"iir"` | Optimization mode: `"iir"`, `"fir"`, or `"mixed"` |
| `fir` | object | - | FIR configuration (when mode is `"fir"` or `"mixed"`) |
| `loss_type` | string | `"flat"` | Loss function: `"flat"` or `"score"` |
| `algorithm` | string | `"cobyla"` | Global optimization algorithm |
| `num_filters` | integer | `10` | Number of PEQ filters per channel |
| `min_q` | number | `0.5` | Minimum Q factor |
| `max_q` | number | `10.0` | Maximum Q factor |
| `min_db` | number | `-12.0` | Minimum gain in dB |
| `max_db` | number | `12.0` | Maximum gain in dB |
| `min_freq` | number (Hz) | `20.0` | Minimum frequency |
| `max_freq` | number (Hz) | `20000.0` | Maximum frequency |
| `max_iter` | integer | `10000` | Maximum optimization iterations |
| `peq_model` | string | `"pk"` | PEQ model type |
| `refine` | boolean | `true` | Enable hybrid two-stage optimization (DE global + COBYLA local) |
| `local_algo` | string | `"cobyla"` | Local optimizer for refinement stage (when `refine=true`) |
| `psychoacoustic` | boolean | `true` | Enable psychoacoustic variable smoothing before optimization |
| `asymmetric_loss` | boolean | `true` | Penalize peaks 2× more than dips (psychoacoustically correct) |
| `target_tilt` | object | - | Target curve tilt configuration |
| `excursion_protection` | object | - | Excursion protection for bookshelf speakers |
| `schroeder_split` | object | - | Different Q constraints above/below Schroeder frequency |
| `phase_alignment` | object | - | Phase alignment for subwoofer integration |
| `multi_seat` | object | - | Multi-seat variance optimization |
| `broadband_target_matching` | object | - | Preliminary broadband shelf alignment |

### Optimization Algorithms

| Algorithm | Description |
|-----------|-------------|
| `cobyla` | COBYLA (Constrained Optimization BY Linear Approximations) |
| `nlopt:cobyla` | NLopt COBYLA variant |
| `autoeq:de` | Differential Evolution (global optimizer) |
| `nlopt:isres` | Improved Stochastic Ranking Evolution Strategy |

### Local Algorithms (for `refine`)

| Algorithm | Description |
|-----------|-------------|
| `cobyla` | COBYLA (default) |
| `bobyqa` | Bound Optimization BY Quadratic Approximations |
| `sbplx` | Subplex method |

### Loss Types

| Type | Description |
|------|-------------|
| `flat` | Optimize for flat frequency response |
| `score` | Optimize for Harman/Olive score (bass boost + flat PIR) |

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

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `taps` | integer | `4096` | Number of FIR filter taps (64–65536) |
| `phase` | string | `"kirkeby"` | Phase type: `"linear"` (symmetric FIR) or `"kirkeby"` (magnitude limits) |
| `correct_excess_phase` | boolean | `false` | Correct excess phase (kirkeby only). Requires clean phase measurements. |
| `phase_smoothing` | number | `0.167` | Phase smoothing width in octaves (0 = disabled). Applied via group delay smoothing when excess phase correction is enabled. |

---

## Target Tilt Configuration

Applies a frequency-dependent tilt to the target curve. The Harman-style tilt (-0.8 dB/octave) is psychoacoustically preferred for in-room listening.

```json
{
  "optimizer": {
    "target_tilt": {
      "tilt_type": "harman"
    }
  }
}
```

With custom tilt and bass shelf:
```json
{
  "optimizer": {
    "target_tilt": {
      "tilt_type": "custom",
      "slope_db_per_octave": -1.0,
      "reference_freq": 1000,
      "bass_shelf_db": 3.0,
      "bass_shelf_freq": 150
    }
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `tilt_type` | string | `"flat"` | Tilt type: `"flat"` (no tilt), `"harman"` (-0.8 dB/oct), `"custom"` |
| `slope_db_per_octave` | number | `-0.8` | Slope in dB/octave (negative = downward tilt). Used with `"custom"`. |
| `reference_freq` | number (Hz) | `1000` | Reference frequency where tilt equals 0 dB |
| `bass_shelf_db` | number (dB) | `0.0` | Bass shelf boost in dB (applied below `bass_shelf_freq`) |
| `bass_shelf_freq` | number (Hz) | `200` | Bass shelf frequency |

---

## Excursion Protection Configuration

Detects the speaker's F3 rolloff and generates a highpass filter to prevent dangerous over-boost of bass frequencies. Recommended for bookshelf speakers.

```json
{
  "optimizer": {
    "excursion_protection": {
      "enabled": true,
      "auto_detect_f3": true,
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
| `schroeder_freq` | number (Hz) | `300` | Schroeder frequency (typical: 200–500 Hz for domestic rooms) |
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

**HighFreqFilterConfig Fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_q` | number | `1.0` | Maximum Q factor for high frequency filters |
| `shelving_only` | boolean | `false` | Use shelving filters only (no parametric peaks) |

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
      "max_delay_ms": 30
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
| `max_delay_ms` | number (ms) | `30` | Maximum delay in milliseconds |

---

## Multi-Seat Configuration

Optimizes subwoofer delays and gains to minimize response variance across multiple listening positions.

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
| `strategy` | string | `"minimize_variance"` | Strategy: `"minimize_variance"`, `"primary_with_constraints"`, `"average"` |
| `primary_seat` | integer | `0` | Index of primary seat (0-based, used with `primary_with_constraints`) |
| `max_deviation_db` | number (dB) | `6` | Maximum allowed deviation at non-primary seats |

---

## Broadband Target Matching

Using `min_freq` / `max_freq` limits the optimization range, which can leave spectral imbalances outside that band. Broadband Target Matching solves this with a preliminary alignment pass:

1. Analyzes the full 20 Hz–20 kHz spectrum.
2. Fits Low Shelf (200 Hz), High Shelf (4 kHz), and Gain filters to match the target curve.
3. Applies this correction *before* the fine-grained PEQ optimization.

This ensures the overall tonal balance is correct even when the main optimizer focuses only on modal correction below 1 kHz.

```json
{
  "optimizer": {
    "broadband_target_matching": {
      "enabled": true
    }
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | boolean | `true` | Enable broadband target matching |

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
    "num_filters": 10,
    "algorithm": "cobyla",
    "max_iter": 5000,
    "min_freq": 20.0,
    "max_freq": 20000.0
  }
}
```

### Example 2: 2.1 System with Subwoofer

```json
{
  "version": "1.2.0",
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
    "num_filters": 10,
    "algorithm": "cobyla",
    "max_iter": 10000,
    "min_freq": 20.0,
    "max_freq": 20000.0
  }
}
```

### Example 3: 2-Way Active Speaker

```json
{
  "speakers": {
    "left": {
      "name": "Left 2-Way",
      "measurements": [
        "measurements/left_woofer.csv",
        "measurements/left_tweeter.csv"
      ],
      "crossover": "lr24_xover"
    },
    "right": {
      "name": "Right 2-Way",
      "measurements": [
        "measurements/right_woofer.csv",
        "measurements/right_tweeter.csv"
      ],
      "crossover": "lr24_xover"
    }
  },
  "crossovers": {
    "lr24_xover": {
      "type": "LR24",
      "frequency_range": [1500, 3500]
    }
  },
  "optimizer": {
    "num_filters": 12,
    "algorithm": "cobyla",
    "max_iter": 15000,
    "min_freq": 50.0,
    "max_freq": 20000.0
  }
}
```

### Example 4: 3-Way Speaker with Fixed Crossovers

```json
{
  "speakers": {
    "left": {
      "name": "Left 3-Way Tower",
      "measurements": [
        {"path": "measurements/left_woofer.csv", "name": "woofer"},
        {"path": "measurements/left_midrange.csv", "name": "midrange"},
        {"path": "measurements/left_tweeter.csv", "name": "tweeter"}
      ],
      "crossover": "3way_fixed"
    }
  },
  "crossovers": {
    "3way_fixed": {
      "type": "LR48",
      "frequencies": [500, 3000]
    }
  },
  "optimizer": {
    "num_filters": 15,
    "algorithm": "autoeq:de",
    "max_iter": 20000
  }
}
```

### Example 5: Home Theater with Multiple Subwoofers

```json
{
  "speakers": {
    "left": "measurements/left_main.csv",
    "center": "measurements/center.csv",
    "right": "measurements/right_main.csv",
    "surround_left": "measurements/surround_left.csv",
    "surround_right": "measurements/surround_right.csv",
    "lfe": {
      "name": "Quad Subwoofers",
      "subwoofers": [
        "measurements/sub_fl.csv",
        "measurements/sub_fr.csv",
        "measurements/sub_rl.csv",
        "measurements/sub_rr.csv"
      ]
    }
  },
  "optimizer": {
    "num_filters": 10,
    "algorithm": "cobyla",
    "max_iter": 10000,
    "min_freq": 20.0,
    "max_freq": 20000.0,
    "min_db": -15.0,
    "max_db": 15.0
  }
}
```

### Example 6: Double Bass Array

```json
{
  "speakers": {
    "left": "measurements/left.csv",
    "right": "measurements/right.csv",
    "lfe": {
      "name": "DBA System",
      "front": [
        "measurements/front_sub_left.csv",
        "measurements/front_sub_right.csv"
      ],
      "rear": [
        "measurements/rear_sub_left.csv",
        "measurements/rear_sub_right.csv"
      ]
    }
  },
  "optimizer": {
    "num_filters": 8,
    "algorithm": "cobyla",
    "max_iter": 15000,
    "min_freq": 20.0,
    "max_freq": 200.0
  }
}
```

### Example 7: Multiple Measurement Positions (Averaged)

```json
{
  "speakers": {
    "left": [
      "measurements/left_pos1.csv",
      "measurements/left_pos2.csv",
      "measurements/left_pos3.csv"
    ],
    "right": [
      "measurements/right_pos1.csv",
      "measurements/right_pos2.csv",
      "measurements/right_pos3.csv"
    ]
  },
  "optimizer": {
    "num_filters": 10,
    "algorithm": "cobyla",
    "max_iter": 10000
  }
}
```

### Example 8: FIR Mode with Excess Phase Correction

```json
{
  "speakers": {
    "left": "measurements/left.csv",
    "right": "measurements/right.csv"
  },
  "optimizer": {
    "mode": "fir",
    "fir": {
      "taps": 8192,
      "phase": "kirkeby",
      "correct_excess_phase": true,
      "phase_smoothing": 0.167
    },
    "algorithm": "cobyla",
    "max_iter": 5000,
    "min_freq": 20.0,
    "max_freq": 20000.0
  }
}
```

### Example 9: Target Curve with Harman Tilt

```json
{
  "speakers": {
    "left": "measurements/left.csv",
    "right": "measurements/right.csv"
  },
  "target_curve": "flat",
  "optimizer": {
    "loss_type": "flat",
    "num_filters": 12,
    "algorithm": "cobyla",
    "max_iter": 10000,
    "target_tilt": {
      "tilt_type": "harman"
    }
  }
}
```

### Example 10: Bookshelf Speakers with Excursion Protection

```json
{
  "speakers": {
    "left": "measurements/left_bookshelf.csv",
    "right": "measurements/right_bookshelf.csv"
  },
  "optimizer": {
    "num_filters": 10,
    "algorithm": "cobyla",
    "max_iter": 10000,
    "excursion_protection": {
      "enabled": true,
      "auto_detect_f3": true,
      "filter_order": 4,
      "filter_type": "linkwitzriley"
    }
  }
}
```

### Example 11: Room Mode Correction with Schroeder Split

```json
{
  "speakers": {
    "left": "measurements/left.csv",
    "right": "measurements/right.csv"
  },
  "optimizer": {
    "num_filters": 15,
    "algorithm": "autoeq:de",
    "refine": true,
    "local_algo": "cobyla",
    "psychoacoustic": true,
    "asymmetric_loss": true,
    "schroeder_split": {
      "enabled": true,
      "room_dimensions": {
        "length": 6.5,
        "width": 4.2,
        "height": 2.7
      },
      "low_freq_config": {
        "max_q": 12.0,
        "allow_boost": false
      },
      "high_freq_config": {
        "max_q": 1.0
      }
    }
  }
}
```

### Example 12: Subwoofer Phase Alignment

```json
{
  "speakers": {
    "left": "measurements/left.csv",
    "right": "measurements/right.csv",
    "sub": "measurements/sub.csv"
  },
  "optimizer": {
    "num_filters": 10,
    "algorithm": "cobyla",
    "max_iter": 10000,
    "phase_alignment": {
      "enabled": true,
      "min_freq": 40,
      "max_freq": 120,
      "optimize_polarity": true,
      "max_delay_ms": 30
    }
  }
}
```

### Example 13: Multi-Seat Home Theater Optimization

```json
{
  "speakers": {
    "left": "measurements/left.csv",
    "right": "measurements/right.csv",
    "sub": {
      "name": "Dual Subs",
      "subwoofers": [
        "measurements/sub_left.csv",
        "measurements/sub_right.csv"
      ]
    }
  },
  "optimizer": {
    "num_filters": 10,
    "algorithm": "autoeq:de",
    "refine": true,
    "multi_seat": {
      "enabled": true,
      "strategy": "primary_with_constraints",
      "primary_seat": 0,
      "max_deviation_db": 6
    }
  }
}
```

### Example 14: Full-Featured Room Correction

```json
{
  "version": "1.2.0",
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
    "left": "measurements/left.csv",
    "right": "measurements/right.csv",
    "sub": "measurements/sub.csv"
  },
  "crossovers": {
    "bass_xover": {
      "type": "LR24",
      "frequency": 80.0
    }
  },
  "target_curve": "flat",
  "optimizer": {
    "num_filters": 15,
    "algorithm": "autoeq:de",
    "refine": true,
    "local_algo": "cobyla",
    "psychoacoustic": true,
    "asymmetric_loss": true,
    "min_freq": 20.0,
    "max_freq": 20000.0,
    "target_tilt": {
      "tilt_type": "harman"
    },
    "schroeder_split": {
      "enabled": true,
      "schroeder_freq": 300,
      "low_freq_config": {
        "max_q": 10.0,
        "allow_boost": false
      },
      "high_freq_config": {
        "max_q": 1.0
      }
    },
    "phase_alignment": {
      "enabled": true,
      "min_freq": 60,
      "max_freq": 100
    },
    "broadband_target_matching": {
      "enabled": true
    }
  }
}
```
