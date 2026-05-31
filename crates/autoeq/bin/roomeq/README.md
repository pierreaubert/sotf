# RoomEQ - Multi-channel Room Equalization Optimizer

`roomeq` is a command-line tool for optimizing multi-channel speaker systems. It analyzes frequency response measurements and generates optimal DSP chains (EQ, crossovers, gains) for each channel.

## Features

- **Single speaker optimization**: Optimize EQ for individual speakers
- **Multi-driver crossover optimization**: Optimize crossovers for multi-driver speakers (woofer + tweeter, etc.)
- **Group delay alignment**: Optimize time alignment between subwoofers and main speakers
- **Multiple optimization algorithms**: Support for COBYLA, Differential Evolution, and other optimizers
- **AudioEngine-compatible output**: Generates JSON DSP chains compatible with the AudioEngine plugin system
- **Target curve tilt**: Harman-style tilted target curves with optional bass shelf
- **Excursion protection**: Automatic F3 detection and highpass filter generation
- **Schroeder frequency split**: Separate EQ strategies for modal and statistical room behavior
- **Phase alignment**: Subwoofer/speaker phase and polarity optimization
- **Multi-seat optimization**: Minimize response variance across multiple listening positions

## Usage

```bash
cargo run --bin roomeq -- --config <config.json> --output <output.json> [OPTIONS]
```

### Options

- `--config <CONFIG>`: Path to room configuration JSON file (required)
- `--output <OUTPUT>`: Path to output DSP chain JSON file (required)
- `--sample-rate <RATE>`: Sample rate for filter design (default: 48000 Hz)
- `--verbose`: Enable verbose output
- `--help`: Print help information

## Configuration File Format

### Simple Stereo System

```json
{
  "speakers": {
    "left": "measurements/left_speaker.csv",
    "right": "measurements/right_speaker.csv"
  },
  "optimizer": {
    "num_filters": 10,
    "algorithm": "nlopt:cobyla",
    "max_iter": 5000,
    "min_freq": 20.0,
    "max_freq": 20000.0,
    "min_q": 0.5,
    "max_q": 10.0,
    "min_db": -12.0,
    "max_db": 12.0,
    "loss_type": "flat"
  }
}
```

### 2.1 System with Explicit Topology (v2.1)

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
      "crossover": "bass_xo",
      "sub_meas": "L"
    }
  },
  "crossovers": {
    "bass_xo": {
      "type": "LR24",
      "frequency": 80.0
    }
  },
  "speakers": {
    "left_meas": "measurements/left.csv",
    "right_meas": "measurements/right.csv",
    "sub_meas": "measurements/sub.csv"
  },
  "optimizer": {
    "num_filters": 10,
    "algorithm": "cobyla"
  }
}
```

### Multi-driver Speaker (2-way)

```json
{
  "speakers": {
    "left": {
      "name": "Left Speaker (2-way)",
      "measurements": [
        "measurements/left_woofer.csv",
        "measurements/left_tweeter.csv"
      ],
      "crossover": "default_lr24"
    }
  },
  "crossovers": {
    "default_lr24": {
      "type": "LR24"
    }
  },
  "optimizer": {
    "num_filters": 10,
    "algorithm": "nlopt:cobyla",
    "max_iter": 5000,
    "min_freq": 100.0,
    "max_freq": 10000.0,
    "min_q": 0.5,
    "max_q": 10.0,
    "min_db": -12.0,
    "max_db": 12.0,
    "loss_type": "flat"
  }
}
```

### Measurement CSV Format

Measurement CSV files should have the following columns:
- `freq`: Frequency in Hz
- `spl`: Sound pressure level in dB

Example:
```csv
freq,spl
20,75.0
50,78.0
100,80.0
200,82.0
...
```

## Optimizer Configuration

### Algorithms

- `autoeq:cmaes`: CMA-ES (default global optimizer)
- `autoeq:de`: Differential Evolution
- `autoeq:cobyla`: COBYLA (Constrained Optimization BY Linear Approximations)
- `autoeq:isres`: Improved Stochastic Ranking Evolution Strategy
- Other AutoEQ and metaheuristics algorithms supported by autoeq

### Loss Types

- `flat`: Optimize for flat frequency response
- `score`: Optimize for Harman/Olive score (bass boost + flat PIR)
- `epa`: Psychoacoustic EPA loss with configurable penalty weights

When `psychoacoustic` is enabled, `psychoacoustic_smoothing` can override the default variable smoothing curve (`1/48` octave below 100 Hz through `1/6` octave above 1 kHz). When `asymmetric_loss` is enabled, `asymmetric_loss_config` can override peak/dip and bass peak/dip weights without changing the default behavior for existing configs.

`perceptual_policy` can fill coherent defaults for `reference`, `music`, `cinema`, `night`, and `speech` use cases. The policy layer maps existing knobs rather than replacing them: target response, EPA/asymmetric weighting, psychoacoustic smoothing, spatial/bootstrap robustness, audibility deadband, high-frequency guardrails, FIR direct/early/late advisories, and validation bundle descriptors remain individually configurable.

### Crossover Types

- `LR24` or `LR4`: Linkwitz-Riley 24 dB/oct (4th order)
- `LR48` or `LR8`: Linkwitz-Riley 48 dB/oct (8th order)
- `Butterworth12` or `BW12`: Butterworth 12 dB/oct (2nd order)
- `Butterworth24` or `BW24`: Butterworth 24 dB/oct (4th order)
- `LinearPhase`, `FIR`, or `LPFIR`: complementary FIR crossover with constant group delay and no crossover-point phase rotation

## Advanced Audio Corrections

RoomEQ provides advanced audio correction features for optimizing room acoustics in two scenarios:

- **Scenario A (WITH Subwoofers)**: Phase alignment and multi-seat variance minimization
- **Scenario B (WITHOUT Subwoofers)**: Schroeder split, excursion protection, and target response shaping

### Target Response

Instead of optimizing to a flat target, many listeners prefer a gently downward-sloping target curve. Research by Harman International shows that a **-0.8 dB/octave** tilt is psychoacoustically preferred for in-room listening. Target shaping is configured via the unified `target_response` object, which covers the base shape, optional user-preference shelves layered on top, and the broadband pre-correction toggle.

```json
{
  "optimizer": {
    "target_response": {
      "shape": "harman",
      "slope_db_per_octave": -0.8,
      "reference_freq": 1000,
      "preference": {
        "bass_shelf_db": 0,
        "bass_shelf_freq": 200,
        "treble_shelf_db": 0,
        "treble_shelf_freq": 8000
      },
      "broadband_precorrection": false
    }
  }
}
```

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `shape` | string | `"flat"` | Target shape: `"flat"`, `"harman"`, `"custom"`, `"file"`, `"from_measurement"` |
| `slope_db_per_octave` | number | -0.8 | Slope in dB/octave (negative = downward tilt). Used when `shape == "custom"` |
| `reference_freq` | number | 1000 | Frequency where the target slope passes through 0 dB (Hz) |
| `curve_path` | string (path) | - | CSV target file path (used when `shape == "file"`) |
| `preference.bass_shelf_db` | number | 0 | Bass shelf preference layered on top of the target shape (dB) |
| `preference.bass_shelf_freq` | number | 200 | Bass shelf transition frequency (Hz) |
| `preference.treble_shelf_db` | number | 0 | Treble shelf preference layered on top of the target shape (dB) |
| `preference.treble_shelf_freq` | number | 8000 | Treble shelf transition frequency (Hz) |
| `broadband_precorrection` | boolean | false | Run a preliminary broadband shelf + gain fit before the fine-grained PEQ pass |

The target curve is computed as:
```
target_db(f) = slope * log2(f / reference_freq)
             + preference_bass_shelf(f)
             + preference_treble_shelf(f)
```

Where the preference shelf terms apply smooth 2nd-order shelf transitions around `bass_shelf_freq` and `treble_shelf_freq`.

**Example: Harman with Bass Boost**

```json
{
  "optimizer": {
    "target_response": {
      "shape": "harman",
      "preference": {
        "bass_shelf_db": 3,
        "bass_shelf_freq": 200
      }
    }
  }
}
```

### Excursion Protection

Bookshelf speakers and small drivers have limited bass extension. Attempting to boost bass below the speaker's F3 point (-3dB frequency) can cause excessive driver excursion, increased distortion, and potential damage.

Excursion protection automatically detects the F3 rolloff and generates a highpass filter to prevent dangerous over-boost.

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

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `enabled` | boolean | false | Enable excursion protection |
| `auto_detect_f3` | boolean | true | Auto-detect F3 from measurement |
| `manual_f3_hz` | number | - | Manual F3 override (Hz) when auto-detect is false |
| `filter_order` | integer | 4 | HPF order: 2=12dB/oct, 4=24dB/oct |
| `filter_type` | string | `"linkwitzriley"` | Filter type: `"linkwitzriley"` or `"butterworth"` |
| `margin_octaves` | number | 0.25 | Safety margin below F3 for HPF placement |

**F3 Detection Algorithm:**
1. Smooth the measurement curve (1/3 octave)
2. Find reference level at 100-200 Hz
3. Search downward for -3dB point
4. Place HPF at `F3 * 2^(-margin_octaves)`

### Schroeder Frequency Split

The **Schroeder frequency** marks the transition between modal (low frequency) and statistical (high frequency) behavior in a room. Below this frequency, room modes dominate and require high-Q narrow filters for correction. Above this frequency, broad tonal adjustments are more appropriate.

Typical Schroeder frequencies:
- Small room (15 m³): ~400 Hz
- Medium room (40 m³): ~250 Hz
- Large room (100 m³): ~160 Hz

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

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `enabled` | boolean | false | Enable Schroeder split |
| `schroeder_freq` | number | 300 | Schroeder frequency (Hz) |
| `room_dimensions` | object | - | Optional room dimensions for auto-calculation |
| `low_freq_config.max_q` | number | 10.0 | Max Q for low-freq filters |
| `low_freq_config.min_q` | number | 0.5 | Min Q for low-freq filters |
| `low_freq_config.allow_boost` | boolean | false | Allow boosts (not recommended) |
| `high_freq_config.max_q` | number | 1.0 | Max Q for high-freq filters |
| `high_freq_config.shelving_only` | boolean | false | Use only shelving filters |

**Auto-Calculate Schroeder from Room Dimensions:**

```json
{
  "optimizer": {
    "schroeder_split": {
      "enabled": true,
      "room_dimensions": {
        "length": 5.0,
        "width": 4.0,
        "height": 2.5
      }
    }
  }
}
```

The Schroeder frequency is calculated as: `fs ≈ 11885 / √V` where V is room volume in m³.

### Phase Alignment

When integrating a subwoofer with main speakers, proper time/phase alignment in the crossover region is critical. Misalignment causes cancellation dips at crossover, reduced bass output, and poor transient response.

Phase alignment optimizes the delay and polarity to maximize energy sum in the crossover region.

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

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `enabled` | boolean | true | Enable phase alignment |
| `min_freq` | number | 60 | Minimum optimization frequency (Hz) |
| `max_freq` | number | 100 | Maximum optimization frequency (Hz) |
| `optimize_polarity` | boolean | true | Test both normal and inverted polarity |
| `max_delay_ms` | number | 30 | Maximum delay search range (ms) |

**Algorithm:**
1. **Grid search**: Test delays from -max_delay to +max_delay (0.5ms steps)
2. **For each candidate**: Compute combined response `|H_sub + H_speaker * e^(-jωτ) * polarity|`
3. **Integrate energy** in [min_freq, max_freq] band
4. **Fine search**: Refine around best result with 0.1ms steps
5. **Output**: Optimal delay and polarity for maximum energy sum

**Note:** Both subwoofer and speaker measurements must include phase data (export from REW with phase, or measure with calibrated mic).

### Multi-Seat Optimization

In rooms with multiple listening positions, optimizing for one seat often degrades others. Multi-seat optimization finds subwoofer gain/delay settings that minimize variance across all seats.

```json
{
  "optimizer": {
    "multi_seat": {
      "enabled": true,
      "strategy": "minimize_variance",
      "primary_seat": 0,
      "max_deviation_db": 6
    }
  }
}
```

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `enabled` | boolean | false | Enable multi-seat optimization |
| `strategy` | string | `"minimize_variance"` | Optimization strategy |
| `primary_seat` | integer | 0 | Primary seat index (0-based) |
| `max_deviation_db` | number | 6 | Max deviation at secondary seats (dB) |

**Strategies:**

| Strategy | Description |
|----------|-------------|
| `minimize_variance` | Minimize standard deviation of SPL across all seats |
| `primary_with_constraints` | Optimize primary seat, constrain others within max_deviation |
| `average` | Optimize for flattest average response across seats |
| `modal_basis` | Complex modal-basis SFM: extracts dominant seat modes from per-sub/per-seat transfer functions and optimizes sub gain/delay/polarity/all-pass controls |

**Measurement Setup:**

For multi-seat optimization, you need measurements of each subwoofer at each seat position:

```json
{
  "speakers": {
    "subs": {
      "name": "Multi-seat Subwoofers",
      "subwoofers": [
        ["sub1_seat1.csv", "sub1_seat2.csv", "sub1_seat3.csv"],
        ["sub2_seat1.csv", "sub2_seat2.csv", "sub2_seat3.csv"]
      ]
    }
  }
}
```

### Complete Configuration Examples

**Scenario A: System with Subwoofers**

```json
{
  "speakers": {
    "left": "measurements/left.csv",
    "right": "measurements/right.csv",
    "sub": "measurements/subwoofer.csv"
  },
  "optimizer": {
    "algorithm": "autoeq:cmaes",
    "num_filters": 10,
    "refine": true,

    "target_response": {
      "shape": "harman",
      "preference": {
        "bass_shelf_db": 2
      }
    },

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

**Scenario B: Bookshelf Speakers without Subwoofer**

```json
{
  "speakers": {
    "left": "measurements/left_bookshelf.csv",
    "right": "measurements/right_bookshelf.csv"
  },
  "optimizer": {
    "algorithm": "autoeq:cmaes",
    "num_filters": 12,
    "refine": true,

    "target_response": {
      "shape": "harman"
    },

    "excursion_protection": {
      "enabled": true,
      "auto_detect_f3": true,
      "filter_order": 4,
      "margin_octaves": 0.25
    },

    "schroeder_split": {
      "enabled": true,
      "schroeder_freq": 300,
      "low_freq_config": {
        "max_q": 10,
        "allow_boost": false
      },
      "high_freq_config": {
        "max_q": 1.0
      }
    }
  }
}
```

### Optimization Flow

When multiple features are enabled, the optimization follows this order:

```
1. Load measurement(s)
2. Build target curve from `target_response` (shape + preference shelves)
3. [IF excursion_protection] Detect F3, generate protection HPF
4. [IF has_subwoofer && phase_alignment] Optimize delay/polarity for energy max
5. [IF multi_seat] Optimize sub gains/delays for variance minimization
6. [IF schroeder_split] Two-pass EQ (low-Q high freq, high-Q low freq)
   [ELSE] Standard EQ optimization
7. Combine all filters into DSP chain
```

### API Reference

The features are also available programmatically:

```rust
use autoeq::roomeq::{
    // Target Response
    build_complete_target_curve,
    TargetResponseConfig, TargetShape, UserPreference,

    // Excursion Protection
    detect_f3, generate_excursion_protection,
    ExcursionProtectionConfig, ExcursionProtectionResult,

    // Phase Alignment
    optimize_phase_alignment,
    PhaseAlignmentConfig, PhaseAlignmentResult,

    // Multi-Seat
    optimize_multiseat,
    MultiSeatMeasurements, MultiSeatConfig, MultiSeatOptimizationResult,
};
```

## Output Format

The output is a JSON file containing DSP chains for each channel:

```json
{
  "channels": {
    "left": {
      "channel": "left",
      "plugins": [
        {
          "plugin_type": "gain",
          "parameters": {
            "gain_db": -2.5
          }
        },
        {
          "plugin_type": "eq",
          "parameters": {
            "filters": [
              {
                "filter_type": "peak",
                "freq": 1000.0,
                "q": 1.5,
                "db_gain": 3.0
              }
            ]
          }
        }
      ]
    }
  },
  "metadata": {
    "pre_score": 0.0,
    "post_score": 0.0,
    "algorithm": "nlopt:cobyla",
    "iterations": 5000,
    "timestamp": "2025-01-15T12:00:00Z"
  }
}
```

This output can be loaded directly into the AudioEngine plugin system.

## Examples

See the `tests/data/roomeq/` directory for example configurations:
- `test_config_stereo.json`: Simple stereo system
- `test_config_multidriver.json`: Multi-driver speaker with crossover

## Documentation

Detailed format documentation with examples:
- [`INPUT_FORMAT.md`](./INPUT_FORMAT.md): Complete input configuration format
- [`OUTPUT_FORMAT.md`](./OUTPUT_FORMAT.md): Complete DSP chain output format

JSON Schemas for validation:
- [`input_schema.json`](./input_schema.json): Input configuration schema
- [`output_schema.json`](./output_schema.json): Output DSP chain schema

## Testing

Run the integration tests:
```bash
cargo test -p autoeq --test roomeq_integration_test
```

Run the unit tests:
```bash
cargo test -p autoeq --bin roomeq
```

## Architecture

The roomeq binary uses autoeq's proven optimization infrastructure:

1. **Load measurements**: Read frequency response curves from CSV files
2. **Crossover optimization** (for multi-driver speakers):
   - Uses `autoeq::loss::DriversFlat` loss function
   - Optimizes driver gains and crossover frequencies
   - Computes combined frequency response
3. **EQ optimization**:
   - Uses `autoeq::workflow::setup_objective_data` for single measurements
   - Uses `autoeq::optim::optimize_filters` for optimization
   - Converts parameters to Biquad filters via `autoeq::x2peq::x2peq`
4. **Output DSP chain**: Generate AudioEngine PluginConfig JSON

## Implementation Details

- **Modules**:
  - `types.rs`: Configuration and output data structures
  - `load.rs`: CSV measurement loading
  - `eq_optim.rs`: EQ optimization using autoeq workflow
  - `crossover_optim.rs`: Multi-driver crossover optimization
  - `output.rs`: DSP chain JSON generation

- **Dependencies**:
  - `autoeq`: Core optimization algorithms (includes `cea2034` module for curve data structures)
  - `math-iir-fir`: Biquad filter implementation
