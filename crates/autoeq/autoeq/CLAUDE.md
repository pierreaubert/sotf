# autoeq (lib: `autoeq`, version: 0.3.13)

Core automatic equalization system for speakers, headphones, and rooms.

## Optimization Workflow

1. **Data Input** (`read/`): Load measurements from spinorama.org API or CSV files
2. **Signal Processing** (`signal.rs`): Smoothing, interpolation, frequency domain operations
3. **Loss Functions** (`loss.rs`): Evaluate EQ quality against targets
4. **Optimization** (`optim*.rs`): Find optimal filter parameters
5. **Output** (`x2peq.rs`): Convert solution to parametric EQ filters (uses math-iir-fir::Biquad)

## Loss Functions

- `speaker-flat` - Minimize deviation from flat response
- `speaker-score` - Optimize Harman/Olive score (bass boost + PIR flatness)
- `headphone-score` - Target Harman headphone curve

## Optimizer Backends

- **DE** (`optim_de.rs`): Differential Evolution (global)
- **NLOPT** (`optim_nlopt.rs`): ISRES, AGS, ORIGDIRECT, COBYLA, Nelder-Mead
- **MetaHeuristics** (`optim_mh.rs`): PSO and other nature-inspired algorithms

## Module Layout

- `cli.rs` - Shared CLI argument definitions (~33KB)
- `loss.rs` - Loss function implementations (~51KB)
- `workflow.rs` - High-level optimization workflows (~64KB)
- `optim.rs`, `optim_de.rs`, `optim_nlopt.rs`, `optim_mh.rs` - Optimizer implementations
- `read/` - Spinorama API client and CSV parsing
- `roomeq/` - Multi-channel room optimization
- `plot/` - Visualization
- `constraints/` - Optimization constraints (frequency spacing, Q limits, dB bounds)
- `x2peq.rs` - Solution to PEQ conversion
- `signal.rs` - Signal processing utilities

## Key CLI Parameters

- `-n`: Number of PEQ filters
- `--min-q`, `--max-q`: Q factor bounds
- `--min-db`, `--max-db`: Gain bounds
- `--min-freq`, `--max-freq`: Frequency range
- `--algo`: Optimizer selection (e.g., `autoeq:de`, `cobyla`)
- `--strategy`: DE mutation strategy (e.g., `currenttobest1bin`)

## Features

- `nlopt` (default) - NLOPT optimization algorithms
- `plotly_static` - Static plot generation

## Binaries

- `autoeq` - Main optimization CLI
- `benchmark-autoeq-speaker` - QA benchmarking
- `autoeq-download-speakers` - Spinorama database downloader
- `roomeq` - Room EQ optimizer
- `roomeq-fuzzer` - Room EQ fuzzing tool
- `convert-recording` - Recording format converter

## Testing

```bash
cargo test -p autoeq --lib
cargo check -p autoeq && cargo clippy -p autoeq
```

## QA

```bash
just qa  # Runs optimization benchmarks on reference speakers
```
