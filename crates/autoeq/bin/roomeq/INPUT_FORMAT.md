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
  "speakers": { ... },
  "crossovers": { ... },
  "target_curve": "...",
  "group_delay": [ ... ],
  "optimizer": { ... }
}
```

### Top-Level Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `version` | string | No | `"1.1.0"` | Configuration version (semantic versioning) |
| `speakers` | object | **Yes** | - | Map of channel names to speaker configurations |
| `crossovers` | object | No | - | Crossover configurations referenced by multi-driver speakers |
| `target_curve` | string or path | No | - | Target frequency response curve |
| `group_delay` | array | No | - | Group delay optimization configurations |
| `optimizer` | object | No | defaults | Optimization parameters |

---

## Speakers Configuration

The `speakers` field is a map where keys are channel names (e.g., `"left"`, `"right"`, `"center"`, `"lfe"`) and values are speaker configurations.

RoomEQ supports four speaker types:
1. **Single** - A single speaker measurement
2. **Group** - Multi-driver speaker with crossover optimization
3. **MultiSub** - Multiple subwoofers with gain/delay optimization
4. **DBA** - Double Bass Array with front/rear optimization

### Measurement References

Measurements can be specified in three ways:

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

For optimizing front and rear bass arrays with phase cancellation.

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
| `rear` | array | **Yes** | Measurements for the rear array (will be phase-inverted) |

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

## Group Delay Configuration

Optimizes time alignment between subwoofers and main speakers in the crossover region. This minimizes group delay variation in the combined response, resulting in better transient response and smoother frequency transitions.

```json
{
  "group_delay": [
    {
      "subwoofer": "sub",
      "speakers": ["left", "right"],
      "min_freq": 30.0,
      "max_freq": 120.0
    }
  ]
}
```

**GroupDelayConfig Fields:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `subwoofer` | string | **Yes** | - | Channel name of the subwoofer to use as reference |
| `speakers` | array | **Yes** | - | Array of speaker channel names to align with the subwoofer |
| `min_freq` | number (Hz) | No | `30.0` | Minimum frequency for optimization |
| `max_freq` | number (Hz) | No | `120.0` | Maximum frequency for optimization |

The optimizer searches for the optimal delay (±30ms range) that minimizes group delay variation in the specified frequency range. Positive delays are applied to speakers. If the optimal delay is negative (meaning the subwoofer should be delayed), a warning is shown since this may affect other speaker alignments.

---

## Optimizer Configuration

Controls the optimization algorithm and constraints.

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
    "peq_model": "pk"
  }
}
```

**OptimizerConfig Fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `mode` | string | `"iir"` | Optimization mode: `"iir"`, `"fir"`, or `"mixed"` |
| `fir` | object | - | FIR configuration (when mode is `"fir"` or `"mixed"`) |
| `loss_type` | string | `"flat"` | Loss function: `"flat"` or `"score"` |
| `algorithm` | string | `"cobyla"` | Optimization algorithm |
| `num_filters` | integer | `10` | Number of PEQ filters per channel |
| `min_q` | number | `0.5` | Minimum Q factor |
| `max_q` | number | `10.0` | Maximum Q factor |
| `min_db` | number | `-12.0` | Minimum gain in dB |
| `max_db` | number | `12.0` | Maximum gain in dB |
| `min_freq` | number (Hz) | `20.0` | Minimum frequency |
| `max_freq` | number (Hz) | `20000.0` | Maximum frequency |
| `max_iter` | integer | `10000` | Maximum optimization iterations |
| `peq_model` | string | `"pk"` | PEQ model type |

### FIR Configuration

When `mode` is `"fir"` or `"mixed"`:

```json
{
  "optimizer": {
    "mode": "fir",
    "fir": {
      "taps": 4096,
      "phase": "kirkeby"
    }
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `taps` | integer | `4096` | Number of FIR filter taps |
| `phase` | string | `"kirkeby"` | Phase type: `"linear"` or `"kirkeby"` |

### Optimization Algorithms

| Algorithm | Description |
|-----------|-------------|
| `cobyla` | COBYLA (Constrained Optimization BY Linear Approximations) |
| `nlopt:cobyla` | NLopt COBYLA variant |
| `autoeq:de` | Differential Evolution (global optimizer) |
| `nlopt:isres` | Improved Stochastic Ranking Evolution Strategy |

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
| `phase` | number (degrees) | No | Phase response |

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
  "speakers": {
    "left": {
      "path": "measurements/left_speaker.csv"
    },
    "right": {
      "path": "measurements/right_speaker.csv"
    },
    "lfe": {
      "path": "measurements/subwoofer.csv"
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

### Example 7: With Averaging Multiple Measurement Positions

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

### Example 8: FIR Mode Optimization

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
      "phase": "linear"
    },
    "algorithm": "cobyla",
    "max_iter": 5000,
    "min_freq": 20.0,
    "max_freq": 20000.0
  }
}
```

### Example 9: Target Curve Matching

```json
{
  "speakers": {
    "left": "measurements/left.csv",
    "right": "measurements/right.csv"
  },
  "target_curve": "targets/harman_curve.csv",
  "optimizer": {
    "loss_type": "flat",
    "num_filters": 12,
    "algorithm": "cobyla",
    "max_iter": 10000
  }
}
```

### Example 10: Group Delay Alignment (2.1 System)

```json
{
  "speakers": {
    "sub": "measurements/subwoofer.csv",
    "left": "measurements/left_speaker.csv",
    "right": "measurements/right_speaker.csv"
  },
  "group_delay": [
    {
      "subwoofer": "sub",
      "speakers": ["left", "right"],
      "min_freq": 40.0,
      "max_freq": 100.0
    }
  ],
  "optimizer": {
    "num_filters": 10,
    "algorithm": "cobyla",
    "max_iter": 10000
  }
}
```
