# RoomEQ - Multi-channel Room Equalization Optimizer

`roomeq` is a command-line tool for optimizing multi-channel speaker systems. It analyzes frequency response measurements and generates optimal DSP chains (EQ, crossovers, gains) for each channel.

## Features

- **Single speaker optimization**: Optimize EQ for individual speakers
- **Multi-driver crossover optimization**: Optimize crossovers for multi-driver speakers (woofer + tweeter, etc.)
- **Group delay alignment**: Optimize time alignment between subwoofers and main speakers
- **Multiple optimization algorithms**: Support for COBYLA, Differential Evolution, and other optimizers
- **AudioEngine-compatible output**: Generates JSON DSP chains compatible with the AudioEngine plugin system

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

- `nlopt:cobyla`: COBYLA (Constrained Optimization BY Linear Approximations)
- `autoeq:de`: Differential Evolution (global optimizer)
- `nlopt:isres`: Improved Stochastic Ranking Evolution Strategy
- Other NLopt and metaheuristics algorithms supported by autoeq

### Loss Types

- `flat`: Optimize for flat frequency response
- `score`: Optimize for Harman/Olive score (bass boost + flat PIR)

### Crossover Types

- `LR24` or `LR4`: Linkwitz-Riley 24 dB/oct (4th order)
- `LR48` or `LR8`: Linkwitz-Riley 48 dB/oct (8th order)
- `Butterworth12` or `BW12`: Butterworth 12 dB/oct (2nd order)
- `Butterworth24` or `BW24`: Butterworth 24 dB/oct (4th order)

### Group Delay Optimization

Group delay optimization aligns the phase/timing between subwoofers and main speakers in the crossover region. This minimizes group delay variation in the combined response, resulting in better transient response and smoother frequency transitions.

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
      "min_freq": 30.0,
      "max_freq": 120.0
    }
  ],
  "optimizer": {
    "num_filters": 10
  }
}
```

Configuration options:
- `subwoofer`: Channel name of the subwoofer to use as reference
- `speakers`: Array of speaker channel names to align with the subwoofer
- `min_freq`: Minimum frequency for optimization (default: 30 Hz)
- `max_freq`: Maximum frequency for optimization (default: 120 Hz)

The optimizer searches for the optimal delay (±30ms range) that minimizes group delay variation in the specified frequency range. Positive delays are applied to speakers; negative delays indicate the subwoofer should be delayed (a warning is shown in this case).

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
  - `group_delay_optim.rs`: Group delay alignment between subwoofer and speakers
  - `output.rs`: DSP chain JSON generation

- **Dependencies**:
  - `autoeq`: Core optimization algorithms
  - `autoeq-iir`: Biquad filter implementation
  - `autoeq-cea2034`: Curve data structures
