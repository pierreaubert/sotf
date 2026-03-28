# autoeq (lib: `autoeq`, version: 0.4.17)

Core automatic equalization system for speakers, headphones, and rooms.

## Optimization Workflow

1. **Data Input** (`read/`): Load measurements from spinorama.org API or CSV files
2. **CEA2034 Scoring** (`cea2034.rs`): Compute preference scores (NBD, LFX, SM PIR)
3. **Loss Functions** (`loss.rs`): Evaluate EQ quality against targets
4. **Optimization** (`optim*.rs`): Find optimal filter parameters
5. **Output** (`x2peq.rs`): Convert solution to parametric EQ filters (uses math-iir-fir::Biquad)

## Loss Functions

- `speaker-flat` -- Minimize deviation from flat response
- `speaker-score` -- Optimize Harman/Olive score (bass boost + PIR flatness)
- `headphone-flat` -- Minimize deviation from headphone target
- `headphone-score` -- Target Harman headphone curve
- `drivers-flat` -- Multi-driver flat response
- `multi-sub-flat` -- Multi-subwoofer flat response

## Optimizer Backends

- **DE** (`optim_de.rs`): Differential Evolution (global)
- **NLOPT** (`optim_nlopt.rs`, feature-gated): ISRES, AGS, ORIGDIRECT, COBYLA, Nelder-Mead
- **MetaHeuristics** (`optim_mh.rs`): PSO and other nature-inspired algorithms

## Module Layout

| Module | Description |
|---|---|
| `cea2034.rs` | CEA2034/Spinorama metrics and preference scoring |
| `cli.rs` | Shared CLI argument definitions |
| `loss.rs` | Loss function implementations |
| `loss/` | Enhanced weights, bass boost, phase-aware loss |
| `workflow.rs` | High-level optimization workflows (speaker, headphone) |
| `optim.rs` | Objective functions and optimizer interfaces |
| `optim_de.rs` | Differential Evolution integration |
| `optim_nlopt.rs` | NLOPT wrapper (feature-gated) |
| `optim_mh.rs` | MetaHeuristics wrapper |
| `optim_callback.rs` | Progress callback utilities |
| `read/` | Spinorama API client, CSV parsing, smoothing, interpolation, normalization |
| `roomeq/` | Multi-channel room optimization (types, workflows, export, spectral alignment, group delay, phase, multi-seat, multi-sub, DBA, FIR, excursion protection) |
| `plot/` | Visualization (spinorama plots, filter plots, driver plots, result plots) |
| `constraints/` | Frequency spacing, ceiling, min gain, crossover monotonicity |
| `x2peq.rs` | Solution vector to PEQ conversion |
| `param_utils.rs` | PEQ model parameter utilities |
| `response.rs` | Frequency response utilities |
| `signal.rs` | Signal processing utilities |
| `fir.rs` | FIR filter design and optimization |
| `init_sobol.rs` | Sobol sequence initialization |
| `initial_guess.rs` | Smart initial guess generation |
| `error.rs` | Error types (`AutoeqError`) |

## PEQ Filter Models

- `pk` -- Peak only
- `hp-pk` -- Highpass + peaks
- `hp-pk-lp` -- Highpass + peaks + lowpass
- `ls-pk-hs` -- Lowshelf + peaks + highshelf
- `free` -- Any filter type

## Key CLI Parameters

- `-n`: Number of PEQ filters
- `--min-q`, `--max-q`: Q factor bounds
- `--min-db`, `--max-db`: Gain bounds
- `--min-freq`, `--max-freq`: Frequency range
- `--algo`: Optimizer selection (e.g., `autoeq:de`, `cobyla`)
- `--strategy`: DE mutation strategy (e.g., `currenttobest1bin`)
- `--loss`: Loss function selection
- `--model`: PEQ filter model

## Features

- `nlopt` (default) -- NLOPT optimization algorithms
- `plotly_static` -- Static plot generation

## Binaries

- `autoeq` -- Main optimization CLI
- `benchmark-autoeq-speaker` -- QA benchmarking (parallel, CSV output)
- `autoeq-download-speakers` -- Spinorama database downloader
- `roomeq` -- Room EQ optimizer
- `roomeq-fuzzer` -- Room EQ fuzzing tool
- `roomeq-qa-quality` -- Convergence quality tests
- `roomeq-qa-coverage` -- Configuration coverage tests
- `roomeq-qa-features` -- Feature-specific QA tests
- `roomeq-qa-synthetic` -- Synthetic data validation
- `convert-recording` -- Recording format converter

## Examples

```bash
cargo run --release --example cea2034_score -p autoeq
cargo run --release --example headphone_loss_demo -p autoeq
cargo run --release --example headphone_loss_validation -p autoeq
```

## Testing

```bash
cargo test -p autoeq --lib
cargo check -p autoeq && cargo clippy -p autoeq
```

## QA

```bash
just qa-autoeq       # AutoEQ speaker benchmarks
just qa-roomeq       # RoomEQ configuration tests
just qa-roomeq-quick # Fast RoomEQ run
```
