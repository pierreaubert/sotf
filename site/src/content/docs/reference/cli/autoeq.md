---
title: autoeq
description: Speaker and headphone EQ optimization CLI — full flag reference.
---

Optimizes parametric EQ filters to flatten a speaker or headphone frequency response.
See the [AutoEQ CLI guide](/guides/autoeq-cli/) for task-oriented examples.

## Synopsis

```
autoeq [OPTIONS]
```

## Flags

### Input

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `-c` / `--curve` | path | — | Input frequency response CSV (`frequency,spl`) |
| `-t` / `--target` | path | — | Target curve CSV (default: flat 0 dB) |
| `--speaker` | string | — | Speaker name for spinorama.org API lookup |
| `--version` | string | — | Measurement version (e.g., `asr`) |
| `--measurement` | string | — | Measurement type (e.g., `CEA2034`) |
| `--curve-name` | string | `Listening Window` | Curve within a CEA2034 measurement |
| `--driver1..4` | path | — | Driver CSVs for multi-driver mode (`--loss drivers-flat`) |

### Filter Constraints

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `-n` / `--num-filters` | int | `7` | Number of IIR biquad filters |
| `--min-db` | float | `1.0` | Minimum filter gain (dB) |
| `--max-db` | float | `3.0` | Maximum filter gain (dB) |
| `--min-q` | float | `1.0` | Minimum Q factor |
| `--max-q` | float | `3.0` | Maximum Q factor |
| `--min-freq` | float | `60` | Minimum filter frequency (Hz) |
| `--max-freq` | float | `16000` | Maximum filter frequency (Hz) |
| `-s` / `--sample-rate` | float | `48000` | Output filter sample rate (Hz) |
| `--peq-model` | enum | `pk` | Filter structure model (see below) |
| `--min-spacing-oct` | float | `0.2` | Minimum spacing between filters (octaves) |
| `--spacing-weight` | float | `20.0` | Penalty weight for filter crowding |

### Optimization

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--algo` | string | `nlopt:cobyla` | Optimizer (use `--algo-list` to list) |
| `--preset` | string | — | Named preset: `quick`, `balanced`, `max-quality`, `score` |
| `--loss` | enum | `speaker-flat` | Loss function (see below) |
| `--maxeval` | int | `2000` | Maximum optimizer evaluations |
| `--population` | int | `300` | Population size (for population-based algos) |
| `--refine` | bool | `false` | Local refinement pass after global search |
| `--local-algo` | string | `cobyla` | Local optimizer for refinement |
| `--tolerance` | float | `1e-3` | Relative convergence tolerance |
| `--atolerance` | float | `1e-4` | Absolute convergence tolerance |
| `--seed` | int | — | Fixed random seed (reproducible results) |
| `--no-parallel` | flag | — | Disable parallel evaluation |
| `--parallel-threads` | int | `0` (all) | Thread count for parallel evaluation |

### DE-Specific

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--strategy` | string | `currenttobest1bin` | DE mutation strategy (use `--strategy-list`) |
| `--recombination` | float | `0.9` | Crossover probability |
| `--adaptive-weight-f` | float | `0.9` | Adaptive F weight |
| `--adaptive-weight-cr` | float | `0.9` | Adaptive CR weight |

### Smoothing

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--smooth` | bool | `true` | Smooth the inverted target curve |
| `--smooth-n` | int | `2` | Smoothing level (1/N octave) |

### Multi-Driver Crossover

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--crossover-type` | string | `linkwitzriley4` | Crossover filter type for `drivers-flat` mode |

### Output

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `-o` / `--output` | path | — | Save a result plot PNG |
| `--qa` | float? | — | QA mode: suppress output, print one-line summary |

### Discovery

| Flag | Description |
|------|-------------|
| `--algo-list` | Print all available optimization algorithms and exit |
| `--strategy-list` | Print all available DE strategies and exit |
| `--peq-model-list` | Print all available PEQ models and exit |

## Loss Functions (`--loss`)

| Value | Description |
|-------|-------------|
| `speaker-flat` | ERB-weighted MSE toward flat (default) |
| `speaker-score` | Harman/Olive preference score (bass boost + PIR flatness) |
| `headphone-flat` | Flatten headphone response toward target |
| `headphone-score` | Harman headphone curve |
| `drivers-flat` | Multi-driver crossover optimization |

## PEQ Models (`--peq-model`)

| Value | Structure |
|-------|-----------|
| `pk` *(default)* | All peak/bell filters |
| `hp-pk` | Highpass + peaks |
| `hp-pk-lp` | Highpass + peaks + lowpass |
| `ls-pk` | Low shelf + peaks |
| `ls-pk-hs` | Low shelf + peaks + high shelf |
| `free-pk-free` | Any + peaks + any |
| `free` | All filters unconstrained |

## Environment

`RUST_LOG` controls log verbosity: `error`, `warn` *(default)*, `info`, `debug`.
