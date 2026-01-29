# Audio Corrections for RoomEQ

This document describes the advanced audio correction features available in RoomEQ for optimizing room acoustics in two scenarios:

- **Scenario A (WITH Subwoofers)**: Phase alignment and multi-seat variance minimization
- **Scenario B (WITHOUT Subwoofers)**: Schroeder split, excursion protection, and target tilt

## Table of Contents

1. [Target Curve Tilt](#target-curve-tilt)
2. [Excursion Protection](#excursion-protection)
3. [Schroeder Frequency Split](#schroeder-frequency-split)
4. [Phase Alignment](#phase-alignment)
5. [Multi-Seat Optimization](#multi-seat-optimization)

---

## Target Curve Tilt

### Overview

Instead of optimizing to a flat target, many listeners prefer a gently downward-sloping target curve. Research by Harman International shows that a **-0.8 dB/octave** tilt is psychoacoustically preferred for in-room listening.

### Configuration

```json
{
  "optimizer": {
    "target_tilt": {
      "tilt_type": "harman",
      "slope_db_per_octave": -0.8,
      "reference_freq": 1000,
      "bass_shelf_db": 0,
      "bass_shelf_freq": 200
    }
  }
}
```

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `tilt_type` | string | `"flat"` | Target type: `"flat"`, `"harman"`, or `"custom"` |
| `slope_db_per_octave` | number | -0.8 | Slope in dB/octave (negative = downward tilt). Used when `tilt_type` is `"custom"` |
| `reference_freq` | number | 1000 | Frequency where tilt equals 0 dB (Hz) |
| `bass_shelf_db` | number | 0 | Additional bass shelf boost (dB) |
| `bass_shelf_freq` | number | 200 | Bass shelf transition frequency (Hz) |

### How It Works

The target curve is computed as:
```
target_db(f) = slope * log2(f / reference_freq) + bass_shelf(f)
```

Where `bass_shelf(f)` applies a smooth 2nd-order shelf transition below `bass_shelf_freq`.

### Example: Harman with Bass Boost

```json
{
  "optimizer": {
    "target_tilt": {
      "tilt_type": "harman",
      "bass_shelf_db": 3,
      "bass_shelf_freq": 200
    }
  }
}
```

This creates a -0.8 dB/octave tilt with +3 dB bass shelf below 200 Hz.

---

## Excursion Protection

### Overview

Bookshelf speakers and small drivers have limited bass extension. Attempting to boost bass below the speaker's F3 point (-3dB frequency) can cause:
- Excessive driver excursion
- Increased distortion
- Potential damage

Excursion protection automatically detects the F3 rolloff and generates a highpass filter to prevent dangerous over-boost.

### Configuration

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

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `enabled` | boolean | false | Enable excursion protection |
| `auto_detect_f3` | boolean | true | Auto-detect F3 from measurement |
| `manual_f3_hz` | number | - | Manual F3 override (Hz) when auto-detect is false |
| `filter_order` | integer | 4 | HPF order: 2=12dB/oct, 4=24dB/oct |
| `filter_type` | string | `"linkwitzriley"` | Filter type: `"linkwitzriley"` or `"butterworth"` |
| `margin_octaves` | number | 0.25 | Safety margin below F3 for HPF placement |

### F3 Detection Algorithm

1. Smooth the measurement curve (1/3 octave)
2. Find reference level at 100-200 Hz
3. Search downward for -3dB point
4. Place HPF at `F3 * 2^(-margin_octaves)`

### Example: Manual F3

```json
{
  "optimizer": {
    "excursion_protection": {
      "enabled": true,
      "auto_detect_f3": false,
      "manual_f3_hz": 55,
      "filter_order": 4
    }
  }
}
```

---

## Schroeder Frequency Split

### Overview

The **Schroeder frequency** marks the transition between modal (low frequency) and statistical (high frequency) behavior in a room. Below this frequency, room modes dominate and require high-Q narrow filters for correction. Above this frequency, broad tonal adjustments are more appropriate.

Typical Schroeder frequencies:
- Small room (15 m³): ~400 Hz
- Medium room (40 m³): ~250 Hz
- Large room (100 m³): ~160 Hz

### Configuration

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

### Parameters

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

### Room Dimensions (Auto-Calculate Schroeder)

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

---

## Phase Alignment

### Overview

When integrating a subwoofer with main speakers, proper time/phase alignment in the crossover region is critical. Misalignment causes:
- Cancellation dips at crossover
- Reduced bass output
- Poor transient response

Phase alignment optimizes the delay and polarity to maximize energy sum in the crossover region.

### Configuration

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

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `enabled` | boolean | true | Enable phase alignment |
| `min_freq` | number | 60 | Minimum optimization frequency (Hz) |
| `max_freq` | number | 100 | Maximum optimization frequency (Hz) |
| `optimize_polarity` | boolean | true | Test both normal and inverted polarity |
| `max_delay_ms` | number | 30 | Maximum delay search range (ms) |

### Algorithm

1. **Grid search**: Test delays from -max_delay to +max_delay (0.5ms steps)
2. **For each candidate**: Compute combined response `|H_sub + H_speaker * e^(-jωτ) * polarity|`
3. **Integrate energy** in [min_freq, max_freq] band
4. **Fine search**: Refine around best result with 0.1ms steps
5. **Output**: Optimal delay and polarity for maximum energy sum

### Requirements

**Phase data required**: Both subwoofer and speaker measurements must include phase data (export from REW with phase, or measure with calibrated mic).

---

## Multi-Seat Optimization

### Overview

In rooms with multiple listening positions, optimizing for one seat often degrades others. Multi-seat optimization finds subwoofer gain/delay settings that minimize variance across all seats.

This implements MSO (Multi-Subwoofer Optimizer) logic.

### Configuration

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

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `enabled` | boolean | false | Enable multi-seat optimization |
| `strategy` | string | `"minimize_variance"` | Optimization strategy |
| `primary_seat` | integer | 0 | Primary seat index (0-based) |
| `max_deviation_db` | number | 6 | Max deviation at secondary seats (dB) |

### Strategies

| Strategy | Description |
|----------|-------------|
| `minimize_variance` | Minimize standard deviation of SPL across all seats |
| `primary_with_constraints` | Optimize primary seat, constrain others within max_deviation |
| `average` | Optimize for flattest average response across seats |

### Measurement Setup

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

### Algorithm

1. Load measurements for each sub at each seat
2. For each gain/delay candidate combination:
   - Compute combined response at each seat
   - Calculate standard deviation of SPL across seats
3. Find parameters that minimize variance

---

## Complete Example

### Scenario A: System with Subwoofers

```json
{
  "speakers": {
    "left": "measurements/left.csv",
    "right": "measurements/right.csv",
    "sub": "measurements/subwoofer.csv"
  },
  "optimizer": {
    "algorithm": "autoeq:de",
    "num_filters": 10,
    "refine": true,

    "target_tilt": {
      "tilt_type": "harman",
      "bass_shelf_db": 2
    },

    "phase_alignment": {
      "enabled": true,
      "min_freq": 60,
      "max_freq": 100,
      "optimize_polarity": true,
      "max_delay_ms": 30
    }
  },
  "group_delay": [
    {
      "subwoofer": "sub",
      "speakers": ["left", "right"],
      "min_freq": 30,
      "max_freq": 120
    }
  ]
}
```

### Scenario B: Bookshelf Speakers without Subwoofer

```json
{
  "speakers": {
    "left": "measurements/left_bookshelf.csv",
    "right": "measurements/right_bookshelf.csv"
  },
  "optimizer": {
    "algorithm": "autoeq:de",
    "num_filters": 12,
    "refine": true,

    "target_tilt": {
      "tilt_type": "harman"
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

---

## Optimization Flow

When multiple features are enabled, the optimization follows this order:

```
1. Load measurement(s)
2. Build target curve (with tilt if configured)
3. [IF excursion_protection] Detect F3, generate protection HPF
4. [IF has_subwoofer && phase_alignment] Optimize delay/polarity for energy max
5. [IF multi_seat] Optimize sub gains/delays for variance minimization
6. [IF schroeder_split] Two-pass EQ (low-Q high freq, high-Q low freq)
   [ELSE] Standard EQ optimization
7. Combine all filters into DSP chain
```

---

## API Reference

The features are also available programmatically:

```rust
use autoeq::roomeq::{
    // Target Tilt
    build_target_curve_with_tilt,
    build_harman_target_curve,
    TargetTiltConfig, TiltType,

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

See the module documentation for detailed API usage.
