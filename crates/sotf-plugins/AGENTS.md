# sotf-plugins (lib: `sotf_plugins`)

Facade crate re-exporting `sotf-host` infrastructure and all `sotf-plugin-*` crates.

## Architecture

- **`sotf-host`** (`crates/sotf-host/`): Core infrastructure -- `Plugin`/`InPlacePlugin` traits, `DawHost` (DAG host), parameter system, analyzers, SIMD, smoothing, SOFA/HRTF, speaker configs, STFT
- **`sotf-plugin-*`** (`crates/sotf-plugin-*/`): 30+ individual plugin crates, each self-contained

Two core traits in `sotf-host/plugin.rs`:
- `Plugin`: Variable input/output channel counts -- `process(&mut self, input: &[f32], output: &mut [f32], context: &ProcessContext)`
- `InPlacePlugin`: Same in/out channels -- `process_in_place(&mut self, buffer: &mut [f32], context: &ProcessContext)`, auto-wrapped via `InPlacePluginAdapter`

Audio buffers: flat interleaved `[ch0_f0, ch1_f0, ch0_f1, ch1_f1, ...]`

Plugins are constructed directly by consumer code (no factory pattern). `lib.rs` re-exports all public plugin types.

## Key Infrastructure (in `sotf-host`)

- `host.rs` -- DawHost: DAG-based plugin routing with parallel processing
- `plugin.rs` -- Core traits
- `parameters.rs` -- Parameter system (Float, Int, Bool, Choice)
- `param_specs.rs` -- Centralized parameter defaults/ranges
- `smoothing.rs` -- One-pole parameter smoother
- `stft_common.rs` -- Shared STFT utilities for FFT-based plugins
- `test_utils.rs` -- Signal generators, allocation tracking, performance profilers

## Features

- `hal` -- macOS HAL plugins
- `onnx` -- ONNX runtime for ML-based plugins (upmixer)
- `qa` -- QA benchmark utilities

## Testing

```bash
cargo test -p sotf-plugins --lib
cargo check -p sotf-plugins && cargo clippy -p sotf-plugins
```

## Important Notes

- DSP plugins need params in 3 places: `rebuild_cached_parameters` + `set_parameter` + `get_parameter`. Missing `cached_parameters` causes silent rejection.
- STFT plugins must return `context.num_frames` (not actual draining count) to prevent ring buffer underruns
- Channel count can change between plugins (e.g., upmixer: 2ch -> 5ch)
- See CLAUDE.md in this directory for the full plugin catalog
