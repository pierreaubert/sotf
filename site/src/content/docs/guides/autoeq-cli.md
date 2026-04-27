---
title: "AutoEQ CLI"
description: "Use the autoeq command-line tool to optimize EQ filters for speakers and headphones."
---

The `autoeq` binary optimizes parametric EQ filters to flatten a speaker or headphone's
frequency response. It can fetch measurements directly from spinorama.org or read local
CSV files.

## Quick Examples

Optimize a speaker from spinorama.org (7 filters, COBYLA):

```bash
autoeq --speaker "KEF R3" --version asr --measurement CEA2034
```

Optimize headphone from a local CSV with 5 filters and a custom target curve:

```bash
autoeq --curve headphone.csv --target harman-over-ear-2018.csv --loss headphone-score -n 5
```

Use Differential Evolution for a global search:

```bash
autoeq --speaker "Genelec 8030C" --version asr --measurement CEA2034 --algo autoeq:de -n 7
```

## Input Sources

### From spinorama.org (API)

Use `--speaker`, `--version`, and `--measurement` together:

```bash
autoeq --speaker "Yamaha NS-10M" --version asr --measurement CEA2034
```

For CEA2034 measurements, optionally choose which curve to optimize against
(default: `Listening Window`):

```bash
autoeq --speaker "JBL M2" --version asr --measurement CEA2034 --curve-name "On Axis"
```

### From a CSV File

Provide a frequency-response file with `--curve`. Columns: `frequency,spl`:

```bash
autoeq --curve my-speaker.csv
```

Add a custom target curve with `--target` (same format):

```bash
autoeq --curve my-speaker.csv --target target-curve.csv
```

## Core Parameters

| Flag | Default | Description |
|------|---------|-------------|
| `-n` / `--num-filters` | `7` | Number of IIR biquad filters |
| `--min-db` | `1.0` | Minimum filter gain (dB) |
| `--max-db` | `3.0` | Maximum filter gain (dB) |
| `--min-q` | `1.0` | Minimum Q factor |
| `--max-q` | `3.0` | Maximum Q factor |
| `--min-freq` | `60` | Minimum filter frequency (Hz) |
| `--max-freq` | `16000` | Maximum filter frequency (Hz) |
| `--sample-rate` | `48000` | Output filter sample rate (Hz) |

## Loss Functions

Select with `--loss`:

| Value | Use case |
|-------|----------|
| `speaker-flat` *(default)* | Flatten speaker response to 0 dB |
| `speaker-score` | Optimize Harman/Olive preference score (adds bass boost) |
| `headphone-flat` | Flatten headphone response toward a target |
| `headphone-score` | Target Harman headphone curve |
| `drivers-flat` | Multi-driver loudspeaker crossover optimization |

## Optimization Algorithms

Select with `--algo`. Use `--algo-list` to see all options.

| Value | Type | Notes |
|-------|------|-------|
| `nlopt:cobyla` *(default)* | Local | Fast, reliable for ≤10 filters |
| `autoeq:de` | Global (DE) | More thorough; use with `--strategy` |
| `nlopt:isres` | Global | Slower but explores wide parameter space |
| `nlopt:nelder-mead` | Local | Good for fine-tuning |

For Differential Evolution, configure the search strategy with `--strategy`
(default: `currenttobest1bin`). List available strategies with `--strategy-list`.

## PEQ Filter Models

Control filter structure with `--peq-model`. Default: `pk` (all peak filters).

| Model | Structure |
|-------|-----------|
| `pk` | All peak/bell filters |
| `ls-pk-hs` | Lowshelf + peaks + highshelf |
| `hp-pk-lp` | Highpass + peaks + lowpass |
| `free` | Any filter type per band |

List all models with `--peq-model-list`.

## Presets

Named presets set all optimizer parameters at once. Individual flags override:

| Preset | Description |
|--------|-------------|
| `quick` | Fast result, fewer evaluations |
| `balanced` | Good quality/speed tradeoff |
| `max-quality` | Maximum optimization effort |
| `score` | Optimized for speaker preference score |

```bash
autoeq --speaker "KEF R3" --version asr --measurement CEA2034 --preset balanced
```

## Output

By default, autoeq prints optimized PEQ filters to stdout in a human-readable format.

Save a plot of the result (requires `plotly_static` feature):

```bash
autoeq --speaker "KEF R3" --version asr --measurement CEA2034 --output result.png
```

## Reproducible Results

Set `--seed` for deterministic optimization:

```bash
autoeq --curve speaker.csv --algo autoeq:de --seed 42
```
