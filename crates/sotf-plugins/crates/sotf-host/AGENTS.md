# sotf-host

Core traits, host, and shared utilities for SOTF audio plugins.

## Architecture

Central infrastructure crate that all `sotf-plugin-*` crates depend on. Provides the plugin trait system, DAG-based host, parameter system, analyzers, and shared DSP utilities (re-exported from math-dsp and math-iir-fir).

### Core Plugin System
- `plugin.rs` — `Plugin` trait (variable I/O channels) and `ParametricInPlacePlugin` trait (same I/O channels, adapted via `ParametricInPlacePluginAdapter`)
- `host.rs` — `DawHost` / `Host`: DAG-based plugin routing with parallel processing, topological sorting, cycle detection
- `error.rs` — Plugin error types

### Parameter System
- `parameters.rs` — `Parameter`, `ParameterId`, `ParameterValue` (Float, Int, Bool, Choice)
- `param_specs.rs` — Centralized parameter defaults/ranges for all plugins
- `param_registry.rs` — Parameter registry
- `param_bridge.rs` — Parameter bridging between systems
- `plugin_params.rs` — Plugin parameter helpers
- `automation.rs` — Parameter automation

### Analyzers
- `analyzer.rs` — `AnalyzerData`, `LoudnessData`, `SpectrumData`
- `analyzer_spectrum.rs` — `SpectrumAnalyzerPlugin`: FFT-based spectrum analysis
- `analyzer_loudness_monitor.rs` — `LoudnessMonitorPlugin`: EBU R128 loudness measurement

### Shared Infrastructure
- `auto_gain.rs` — `AutoGain`: automatic gain normalization
- `lufs_target.rs` — `LufsTarget`: LUFS-based target level
- `sofa.rs` — SOFA HRTF file parsing
- `speaker_config.rs` — Speaker layout definitions
- `vbap.rs` — Vector Base Amplitude Panning
- `oversampling.rs` — Oversampling support
- `layout_solver.rs` — Channel layout solving
- `plugin_layout.rs` — Plugin layout definitions
- `render_plan.rs` — Render plan for plugin chains
- `serialization.rs` — Preset management
- `custom_views.rs` — Custom UI view types
- `test_utils.rs` — Signal generators, allocation tracking, performance profilers (behind `qa`/`test`/`debug_assertions`)

### Re-exports from math-dsp
ADAA, auto makeup, channel linking, DC blocker, delta monitor, detector, dynamics core, envelope, envelope follower, lookahead, SIMD, smoothing, STFT, true peak detection.

### Re-exports from math-iir-fir
FIR crossover, LR4 crossover.

## Key Public API

- `Plugin` trait: `process(&mut self, input: &[f32], output: &mut [f32], context: &ProcessContext)` (`plugin.rs`)
- `ParametricInPlacePlugin` trait: `process_in_place(&mut self, buffer: &mut [f32], context: &ProcessContext)` (`plugin.rs`)
- `DawHost`: DAG-based host with `add_node()`, `add_edge()`, `build()`, `process()` (`host.rs`)
- `PluginFactoryFn`: function signature for plugin factories (`lib.rs`)

## Features

- `qa` — QA benchmark utilities, `qa-host` binary, test helpers

## Testing

```bash
cargo test -p sotf-host --lib
cargo check -p sotf-host && cargo clippy -p sotf-host
```

## Important Notes

- DSP plugins need params in 3 places: `rebuild_cached_parameters` + `set_parameter` + `get_parameter`. Missing `cached_parameters` causes silent rejection.
- STFT plugins must return `context.num_frames` (not actual draining count) to prevent ring buffer underruns
- Channel count can change between plugins (e.g., upmixer: 2ch → 5ch)
- Audio buffers are flat interleaved: `[ch0_frame0, ch1_frame0, ch0_frame1, ch1_frame1, ...]`
- Analyzers pass audio through unmodified while extracting measurements via `get_data()`
