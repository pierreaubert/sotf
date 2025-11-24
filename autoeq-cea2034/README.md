<!-- markdownlint-disable-file MD013 -->

# AutoEQ CEA2034 Scoring

This crate implements CEA2034-based preference scoring algorithms for loudspeaker measurements, based on research by Harman, Olive, and others.

## Features

- **CEA2034 Metrics Computation**: Calculate standard metrics from spinorama measurements
- **Preference Score Calculation**: Harman/Olive preference rating algorithm
- **Curve Analysis**: Slope, smoothness, and spectral analysis tools
- **PIR Computation**: Predicted In-Room response calculation from CEA2034 data
- **Octave Band Processing**: Frequency-weighted analysis and filtering

## CEA2034 Standard

The CEA2034 standard defines a set of measurements for evaluating loudspeaker performance:

- **On-axis (0°)**: Direct response
- **Listening Window**: Average of on-axis and early reflections (±10°, ±15°, ±20°, ±25°, ±30°)
- **Early Reflections**: Floor, ceiling, front/rear wall, and side wall reflections
- **Sound Power**: Total acoustic power output
- **Directivity Index (DI)**: Ratio of on-axis to sound power

## Two examples of CEA2034 / Spinorama

![Example 1 of Spinorama](https://www.spinorama.org/speakers/Audiovector%20M1%20Super/Misc/misc-ageve/CEA2034.webp)
![Example 2 of Spinorama](https://www.spinorama.org/speakers/MoFi%20SourcePoint%20V10%20Master%20Edition/ErinsAudioCorner/eac/CEA2034.webp)

## Preference Score Algorithm

The preference score is based on research showing correlation between measured responses and listener preference:

```rust
use autoeq_cea2034::{compute_cea2034_metrics, Curve};
use ndarray::Array1;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let frequencies = Array1::from(vec![20.0, 25.0, 31.5, /* ... */ 20000.0]);

    // Example spinorama data - would typically come from measurements
    let mut spinorama_data = HashMap::new();
    spinorama_data.insert("On Axis".to_string(), Curve {
        freq: frequencies.clone(),
        spl: Array1::from(vec![-2.1, -1.8, -1.2, /* ... */ -10.5]),
    });
    spinorama_data.insert("Listening Window".to_string(), Curve {
        freq: frequencies.clone(),
        spl: Array1::from(vec![-2.0, -1.7, -1.1, /* ... */ -10.3]),
    });
    spinorama_data.insert("Sound Power".to_string(), Curve {
        freq: frequencies.clone(),
        spl: Array1::from(vec![-2.5, -2.0, -1.5, /* ... */ -11.0]),
    });
    spinorama_data.insert("Estimated In-Room Response".to_string(), Curve {
        freq: frequencies.clone(),
        spl: Array1::from(vec![-2.2, -1.9, -1.3, /* ... */ -10.7]),
    });

    let equalized_response = Array1::from(vec![0.5, 0.3, 0.1, /* ... */ -1.0]);

    let metrics = compute_cea2034_metrics(
        &frequencies,
        &spinorama_data,
        Some(&equalized_response)
    ).await?;

    println!("Preference Score: {:.2}", metrics.pref_score);
    Ok(())
}
```

## Scoring Components

### Listening Window (LW) Score

- Flatness and smoothness of the listening window response
- Deviation from target curve
- Frequency-weighted penalties

### Predicted In-Room (PIR) Score

- Computed from listening window, early reflections, and sound power
- Represents typical in-room response
- Critical for overall preference rating

### Bass Extension

- Low-frequency response evaluation
- Extension and smoothness below 100 Hz

### Directivity Analysis

- Consistency of off-axis response
- Smoothness of directivity index
- Early reflection characteristics

## Usage

```rust,no_run
use autoeq_cea2034::{score, octave_intervals, compute_pir_from_lw_er_sp};
use ndarray::Array1;

// Example frequency and response data
let frequencies = Array1::from(vec![20.0, 25.0, 31.5, /* ... */ 20000.0]);
let on_axis = Array1::from(vec![-2.1, -1.8, -1.2, /* ... */ -10.5]);
let listening_window = Array1::from(vec![-2.0, -1.7, -1.1, /* ... */ -10.3]);
let sound_power = Array1::from(vec![-2.5, -2.0, -1.5, /* ... */ -11.0]);
let pir_response = Array1::from(vec![-2.2, -1.9, -1.3, /* ... */ -10.7]);

// Compute octave band intervals for analysis
let intervals = octave_intervals(2, &frequencies);

// Compute preference score for the frequency response
let preference_metrics = score(
    &frequencies,
    &intervals,
    &on_axis,
    &listening_window,
    &sound_power,
    &pir_response
);

println!("Preference Score: {:.2}", preference_metrics.pref_score);

// Compute PIR from CEA2034 measurements
let lw_curve = Array1::from(vec![-2.0, -1.7, -1.1, /* ... */ -10.3]);
let er_curve = Array1::from(vec![-2.3, -2.1, -1.6, /* ... */ -10.8]);
let sp_curve = Array1::from(vec![-2.5, -2.0, -1.5, /* ... */ -11.0]);
let computed_pir = compute_pir_from_lw_er_sp(&lw_curve, &er_curve, &sp_curve);
```

## Integration

This crate is part of the AutoEQ ecosystem:

- Used by `autoeq` for optimization target scoring
- Provides objective functions for `autoeq-de` optimization
- Integrates with measurement data from Spinorama.org API

## Research Background

Based on published research:

- Olive, S. E., & Toole, F. E. (1989). "The detection of reflections in typical rooms"
- Olive, S. E. (2004). "A method for training listeners and selecting program material"
- Harman International patent applications on preference scoring algorithms

## License

GPL-3.0-or-later
