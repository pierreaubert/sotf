# sotf-plugins (lib: `sotf_plugins`)

Comprehensive audio plugin system — facade crate re-exporting `sotf-host` infrastructure and all `sotf-plugin-*` crates.

## Plugin Architecture

Workspace of individual plugin crates under `crates/`:
- **`sotf-host`**: Core infrastructure — `Plugin`/`ParametricInPlacePlugin` traits, `DawHost` (DAG-based host), parameter system, analyzers, SIMD, smoothing, SOFA/HRTF, speaker configs, STFT common, test utilities
- **`sotf-plugin-*`**: Individual plugin crates (30+), each self-contained

Two core traits:
- `Plugin` (`sotf-host/plugin.rs`): Variable input/output channel counts — `process(&mut self, input: &[f32], output: &mut [f32], context: &ProcessContext) -> PluginResult<()>`
- `ParametricInPlacePlugin` (`sotf-host/plugin.rs`): Same in/out channel count — `process_in_place(&mut self, buffer: &mut [f32], context: &ProcessContext)`, auto-wrapped into `Plugin` via `ParametricInPlacePluginAdapter`
- `DawHost` (`sotf-host/host.rs`): DAG-based plugin routing with parallel processing

Audio buffers are flat interleaved: `[ch0_frame0, ch1_frame0, ch0_frame1, ch1_frame1, ...]`

Plugins are constructed directly by consumer code (no factory pattern). `lib.rs` re-exports all public plugin types.

## Processing Plugins (transform audio)

| Plugin | Crate | Description |
|--------|-------|-------------|
| EQ | `sotf-plugin-eq` | Parametric EQ with biquad filters (uses math-iir-fir::Biquad) |
| Gain | `sotf-plugin-gain` | Volume control with smoothing |
| Compressor | `sotf-plugin-compressor` | Dynamic range compression |
| Expander | `sotf-plugin-expander` | Dynamic range expansion |
| Gate | `sotf-plugin-gate` | Noise gate |
| Limiter | `sotf-plugin-limiter` | Peak limiter |
| Delay | `sotf-plugin-delay` | Audio delay with feedback |
| Convolution | `sotf-plugin-convolution` | FFT-based convolution for IR processing |
| Crossover | `sotf-plugin-crossover` | Frequency band splitting (Linkwitz-Riley) |
| Matrix | `sotf-plugin-matrix` | Channel matrix mixing with gain smoothing |
| Channel Mute/Solo | `sotf-plugin-channel-mute-solo` | Per-channel mute/solo/dim with fade smoothing |
| Resampler | `sotf-plugin-resampler` | Sample rate conversion |
| Upmixer | `sotf-plugin-upmixer` | Stereo to surround via FFT spatial processing + VBAP |
| Downmix | `sotf-plugin-downmix` | Surround to stereo downmixing |
| Binaural | `sotf-plugin-binaural` | HRTF-based binaural rendering |
| XTC | `sotf-plugin-xtc` | Crosstalk cancellation |
| PND | `sotf-plugin-pnd` | Perceptual Noise Diffusion |
| Crossfeed | `sotf-plugin-crossfeed` | Headphone crossfeed |
| Loudness Comp | `sotf-plugin-loudness-compensation` | Equal-loudness contour compensation |
| Fletcher-Munson | `sotf-plugin-fletcher-munson` | Fletcher-Munson equal-loudness correction |
| Mono to Stereo | `sotf-plugin-mono-to-stereo` | Mono to stereo conversion |
| Multiband Comp | `sotf-plugin-multiband-compressor` | Multiband dynamic range compression (2-5 bands) |
| Multiband Exp | `sotf-plugin-multiband-expander` | Multiband dynamic range expansion (2-5 bands) |
| Denoiser | `sotf-plugin-denoiser` | Audio denoising (MCRA/Wiener) |
| AB Compare | `sotf-plugin-ab-compare` | A/B comparison plugin |
| Band Split | `sotf-plugin-band-split` | Split signal into frequency bands |
| Band Merge | `sotf-plugin-band-merge` | Merge frequency bands back together |
| AEC | `sotf-plugin-aec` | Acoustic echo cancellation |
| Beamformer | `sotf-plugin-beamformer` | Microphone beamforming |
| Auto Gain | `sotf-host/auto_gain.rs` | Automatic gain normalization |

## Analyzer Plugins (extract data, do not modify audio)

| Plugin | Location | Description |
|--------|----------|-------------|
| Spectrum | `sotf-host/analyzer_spectrum.rs` | FFT-based spectrum analysis |
| Loudness Monitor | `sotf-host/analyzer_loudness_monitor.rs` | EBU R128 loudness measurement |

## Infrastructure (in `sotf-host`)

- `host.rs` — DawHost: DAG-based plugin routing with parallel processing
- `plugin.rs` — Core Plugin/ParametricInPlacePlugin traits
- `parameters.rs` — Parameter system (Float, Int, Bool, Choice)
- `param_specs.rs` — Centralized parameter defaults/ranges for all plugins
- `param_registry.rs` — Parameter registry
- `automation.rs` — Parameter automation
- `smoothing.rs` — One-pole parameter smoother for click-free transitions
- `auto_gain.rs` — Automatic gain normalization
- `serialization.rs` — Preset management
- `speaker_config.rs` — Channel layout definitions
- `sofa.rs` — HRTF data loading (SOFA format)
- `simd.rs` — SIMD optimizations
- `stft_common.rs` — Shared STFT utilities for FFT-based plugins
- `layout_solver.rs` — Channel layout solving
- `plugin_layout.rs` — Plugin layout definitions
- `test_utils.rs` — Signal generators, allocation tracking, performance profilers
- macOS HAL Input/Output plugins behind `hal` feature

## Features

- `hal` — macOS HAL plugins
- `onnx` — ONNX runtime for ML-based plugins (upmixer)
- `qa` — QA benchmark utilities and test helpers

## Testing

```bash
cargo test -p sotf-plugins --lib
cargo check -p sotf-plugins && cargo clippy -p sotf-plugins
```

## Benchmarks

```bash
cargo bench -p sotf-plugins --bench all-plugins-benchmark
cargo bench -p sotf-plugins --bench allocation-benchmark
cargo bench -p sotf-plugins --bench plugin-benchmark
```

## Binaries

- `plugin-fuzzer` — Stress test plugins with random input

## Important Notes

- Channel count can change between plugins (e.g., upmixer: 2ch → 5ch)
- Plugins are constructed directly by consumer code (no factory pattern)
- The DAG host in `sotf-host/host.rs` supports parallel processing of independent plugin chains
- DSP plugins need params in 3 places: `rebuild_cached_parameters` + `set_parameter` + `get_parameter`. Missing `cached_parameters` causes silent rejection.
- STFT plugins must return `context.num_frames` (not actual draining count) to prevent ring buffer underruns
