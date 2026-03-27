# AutoEQ

Automatic equalization for speakers, headphones, and rooms.

## Crates

| Crate | Lib Name | Description |
|---|---|---|
| [autoeq](autoeq/) | `autoeq` | Core optimization library and CLI tools for speaker, headphone, and room EQ |
| [autoeq-env](autoeq-env/) | `autoeq_env` | Shared environment utilities (paths, constants) |

## Binaries

| Binary | Description |
|---|---|
| `autoeq` | Main CLI -- optimize PEQ filters for speakers/headphones |
| `roomeq` | Multi-channel room EQ optimizer |
| `autoeq-download-speakers` | Download spinorama.org speaker database |
| `benchmark-autoeq-speaker` | QA benchmarking on reference speakers |
| `roomeq-fuzzer` | Fuzz testing for roomeq configurations |
| `roomeq-qa-quality` | Convergence quality tests |
| `roomeq-qa-coverage` | Configuration coverage tests |
| `roomeq-qa-features` | Feature-specific QA tests |
| `roomeq-qa-synthetic` | Synthetic data validation |
| `convert-recording` | WAV recording format converter |

## Build & Test

```bash
# Build all release binaries
just prod-autoeq

# Run tests
cargo test -p autoeq -p autoeq-env --lib

# QA suites
just qa-autoeq       # Speaker optimization benchmarks
just qa-roomeq       # Room EQ configuration tests

# Benchmarks
just bench-autoeq
```

## Dependency Graph

```
autoeq-env  (independent, minimal)
    |
    v
autoeq  (depends on autoeq-env, math-iir-fir, math-dsp, math-optimisation)
```
