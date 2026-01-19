<!-- markdownlint-disable-file MD013 -->

# AutoEQ: Automatic Equalization for Speakers, Headphones, and Rooms

## Introduction

AutoEQ is a Rust CLI toolkit for computing parametric EQ corrections. It uses global optimization algorithms to find optimal IIR filter parameters that minimize deviation from a target response or maximize perceptual preference scores.

**Note:** A graphical desktop application is available in a separate repository: [SotF](https://github.com/pierreaubert/sotf)

## Capabilities

### Supported Use Cases

- **Speaker EQ:** Optimize parametric EQ for loudspeakers using CEA2034/Spinorama measurements from [spinorama.org](https://spinorama.org)
- **Headphone EQ:** Generate EQ corrections for headphones targeting Harman curves or custom targets
- **Multi-Channel Systems:** Optimize stereo, 2.1, and multi-driver configurations with crossover management
- **Room Correction:** Multi-subwoofer alignment and Double Bass Array (DBA) optimization

### Optimization Algorithms

Three algorithm libraries are available:

| Library | Algorithms | Constraint Support |
|---------|------------|-------------------|
| **NLopt** | ISRES, COBYLA, SLSQP, BOBYQA, DIRECT, StoGO, etc. | Nonlinear constraints |
| **Metaheuristics** | DE, PSO, RGA, TLBO, Firefly | Penalty-based |
| **AutoEQ Custom** | Adaptive Differential Evolution | Nonlinear constraints |

### Loss Functions

- `speaker-flat`: Minimize deviation from target curve (near-field listening)
- `speaker-score`: Maximize Harman/Olive preference score (far-field listening)
- `headphone-flat`: Flatten headphone response to target
- `headphone-score`: Optimize headphone preference score
- `drivers-flat`: Multi-driver crossover optimization
- `multi-sub-flat`: Multi-subwoofer array optimization

### PEQ Filter Models

- `pk`: All peak/bell filters (default)
- `hp-pk`: Highpass + peak filters
- `hp-pk-lp`: Highpass + peaks + lowpass
- `ls-pk-hs`: Low shelf + peaks + high shelf
- `free`: All filters can be any type

---

## AutoEQ CLI

The `autoeq` binary optimizes EQ for individual speakers or headphones.

### Basic Usage

```bash
# From spinorama.org API data
cargo run --bin autoeq --release -- \
  --speaker="JBL M2" --version eac --measurement CEA2034 \
  --algo nlopt:cobyla -n 7

# From local CSV file (format: frequency,spl)
cargo run --bin autoeq --release -- \
  --curve measurements.csv --target harman.csv \
  --algo autoeq:de -n 5
```

### Finding Speakers and Measurements

```bash
# List all speakers
curl http://api.spinorama.org/v1/speakers

# Get versions for a speaker
curl "http://api.spinorama.org/v1/speakers/JBL%20M2/versions"

# Get measurements for a speaker/version
curl "http://api.spinorama.org/v1/speakers/JBL%20M2/versions/eac/measurements"
```

### Key Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `-n, --num-filters` | 7 | Number of IIR filters |
| `--algo` | nlopt:cobyla | Optimization algorithm |
| `--loss` | speaker-flat | Loss function |
| `--peq-model` | pk | Filter structure model |
| `--min-freq` / `--max-freq` | 60 / 16000 | Frequency range for filters |
| `--min-q` / `--max-q` | 1 / 3 | Q factor limits |
| `--min-db` / `--max-db` | 1 / 3 | Gain limits (dB) |
| `--maxeval` | 2000 | Maximum optimizer evaluations |
| `--refine` | false | Run local refinement after global optimization |

### Algorithm Selection

```bash
# List all available algorithms
cargo run --bin autoeq --release -- --algo-list

# Recommended: global search + local refinement
cargo run --bin autoeq --release -- \
  --algo nlopt:isres --refine --local-algo cobyla \
  --speaker="KEF R3" --version asr --measurement CEA2034
```

### Differential Evolution Options

When using `autoeq:de`, additional parameters control the optimizer:

```bash
# List available strategies
cargo run --bin autoeq --release -- --strategy-list

# Use adaptive strategy
cargo run --bin autoeq --release -- \
  --algo autoeq:de --strategy adaptivebin \
  --adaptive-weight-f 0.8 --adaptive-weight-cr 0.7 \
  --speaker="KEF R3" --version asr --measurement CEA2034
```

| Parameter | Default | Description |
|-----------|---------|-------------|
| `--strategy` | currenttobest1bin | DE mutation strategy |
| `--population` | 300 | Population size |
| `--tolerance` | 0.001 | Relative convergence tolerance |
| `--atolerance` | 0.0001 | Absolute convergence tolerance |
| `--recombination` | 0.9 | Crossover probability |
| `--seed` | random | Random seed for reproducibility |

### Headphone Example

```bash
cargo run --bin autoeq --release -- \
  --curve headphone_measurement.csv \
  --target harman-over-ear-2018.csv \
  --loss headphone-score \
  --algo mh:rga -n 5 --maxeval 20000 \
  --min-freq 20 --max-freq 10000 --peq-model hp-pk-lp
```

---

## RoomEQ CLI

The `roomeq` binary optimizes multi-channel speaker systems with JSON configuration.

### Basic Usage

```bash
cargo run --bin roomeq --release -- --config room_config.json --output dsp_chain.json
```

### Features

- **Stereo Optimization:** Independent EQ for left/right channels
- **2.1 Systems:** Bass management with crossover optimization
- **Multi-Driver Speakers:** Active crossover optimization for multi-way systems
- **Multi-Subwoofer Arrays:** Gain/delay alignment to minimize seat-to-seat variation
- **Double Bass Array (DBA):** Front/rear array optimization for room mode cancellation

### Configuration File Format

**Stereo system:**

```json
{
  "speakers": {
    "left": { "path": "measurements/left.csv" },
    "right": { "path": "measurements/right.csv" }
  },
  "optimizer": {
    "loss_type": "flat",
    "algorithm": "cobyla",
    "num_filters": 10,
    "min_q": 0.5, "max_q": 10.0,
    "min_db": -12.0, "max_db": 12.0,
    "min_freq": 20.0, "max_freq": 20000.0,
    "max_iter": 10000
  }
}
```

**2.1 system with bass management:**

```json
{
  "speakers": {
    "left": { "path": "measurements/left.csv" },
    "right": { "path": "measurements/right.csv" },
    "lfe": { "path": "measurements/subwoofer.csv" }
  },
  "crossovers": {
    "bass_management": {
      "type": "LR24",
      "frequency_range": [60, 100]
    }
  },
  "optimizer": {
    "loss_type": "flat",
    "algorithm": "cobyla",
    "num_filters": 10,
    "min_q": 0.5, "max_q": 10.0,
    "min_db": -12.0, "max_db": 12.0,
    "min_freq": 20.0, "max_freq": 20000.0,
    "max_iter": 10000
  }
}
```

### Output Schema

```bash
cargo run --bin roomeq --release -- --schema
```

---

## Development

### Prerequisites

Install [rustup](https://rustup.rs/) and [just](https://github.com/casey/just):

```bash
cargo install just
```

### Build Commands

```bash
just                  # List available commands
just prod             # Build all release binaries
just prod-autoeq      # Build autoeq only
just prod-roomeq      # Build roomeq only
just dev              # Build debug binaries
```

### Testing

Tests require the `AUTOEQ_DIR` environment variable:

```bash
export AUTOEQ_DIR=$(pwd)

# Run all tests
just test             # cargo check + cargo test --workspace --lib

# Run tests with nextest (faster)
just ntest

# Run specific test
AUTOEQ_DIR=$(pwd) cargo test --lib test_name

# Run tests for a specific crate
AUTOEQ_DIR=$(pwd) cargo test -p autoeq --lib
AUTOEQ_DIR=$(pwd) cargo test -p autoeq-cea2034 --lib
```

### Fuzzing

Fuzz targets are in `autoeq/fuzz/fuzz_targets/`:

- `autoeq_config.rs`: Fuzzes configuration/CSV parsing
- `autoeq_csv.rs`: Fuzzes CSV input handling

To run fuzzing (requires nightly Rust and cargo-fuzz):

```bash
cargo install cargo-fuzz
cd autoeq
cargo +nightly fuzz run autoeq_csv
```

### Quality Assurance

The QA suite runs optimization scenarios with regression thresholds:

```bash
just qa
```

This executes predefined scenarios testing:

- Speaker optimization (flat and score loss)
- Headphone optimization (multiple algorithms)
- Various PEQ models and algorithm combinations

Each scenario has a `--qa <threshold>` flag that fails if the final loss exceeds the threshold.

Individual QA targets:

```bash
just qa-ascilab-6b           # Speaker with score loss
just qa-jbl-m2-flat          # Speaker with flat loss
just qa-jbl-m2-score         # Speaker with score loss
just qa-beyerdynamic-dt1990pro  # Headphone tests
just qa-edifierw830nb        # Multiple algorithm comparison
```

### Benchmarking

```bash
# Download speaker data from spinorama.org
just download

# Run algorithm benchmarks
just bench-autoeq-speaker

# Run convergence benchmarks
just bench-convergence
```

### Code Quality

```bash
just fmt              # Format code
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets
```

---

## Contributing

- Open an issue on [GitHub](https://github.com/pierreaubert/autoeq)
- Send a PR
