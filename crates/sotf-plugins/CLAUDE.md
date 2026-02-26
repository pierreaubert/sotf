# plugins (lib: `sotf_plugins`)

Comprehensive audio plugin system with processing plugins, analyzers, and DAG-based routing.

## Plugin Architecture

Two core traits:
- `Plugin` (`plugin.rs`): Variable input/output channel counts — `process(&mut self, input: &[f32], output: &mut [f32], context: &ProcessContext) -> Result<usize, String>`
- `InPlacePlugin` (`plugin.rs`): Same in/out channel count — `process_in_place(&mut self, buffer: &mut [f32], context: &ProcessContext)`, auto-wrapped into `Plugin` via `InPlacePluginAdapter`
- `DawHost` (`host.rs`): DAG-based plugin routing with parallel processing

Audio buffers are flat interleaved: `[ch0_frame0, ch1_frame0, ch0_frame1, ch1_frame1, ...]`

Plugins are constructed directly by consumer code (no factory pattern). `lib.rs` re-exports all public plugin types.

## Processing Plugins (transform audio)

| Plugin | File | Description |
|--------|------|-------------|
| EQ | `plugin_eq.rs` | Parametric EQ with biquad filters (uses math-audio-iir-fir::Biquad) |
| Gain | `plugin_gain.rs` | Simple volume control with smoothing |
| Compressor | `plugin_compressor.rs` | Dynamic range compression |
| Expander | `plugin_expander.rs` | Dynamic range expansion |
| Gate | `plugin_gate.rs` | Noise gate |
| Limiter | `plugin_limiter.rs` | Peak limiter |
| Delay | `plugin_delay.rs` | Audio delay with feedback |
| Convolution | `plugin_convolution.rs` | FFT-based convolution for IR processing |
| Crossover | `plugin_crossover.rs` | Frequency band splitting (Linkwitz-Riley) |
| Matrix | `plugin_matrix.rs` | Channel matrix mixing with gain smoothing |
| Channel Mute/Solo | `plugin_channel_mute_solo.rs` | Per-channel mute/solo/dim with fade smoothing |
| Resampler | `plugin_resampler.rs` | Sample rate conversion |
| Upmixer | `plugin_upmixer/` | Stereo to 5.0 surround via FFT spatial processing |
| Binaural | `plugin_binaural/` | HRTF-based binaural rendering |
| XTC | `plugin_xtc/` | Crosstalk cancellation |
| PND | `plugin_pnd/` | Perceptual Noise Diffusion |
| Loudness Comp | `plugin_loudness_compensation.rs` | Equal-loudness contour compensation |
| Fletcher-Munson | `plugin_fletcher_munson.rs` | Fletcher-Munson equal-loudness correction |
| Multiband Compressor | `plugin_multiband_compressor.rs` | Multiband dynamic range compression (2-5 bands) |
| Multiband Expander | `plugin_multiband_expander.rs` | Multiband dynamic range expansion (2-5 bands) |
| Denoiser | `plugin_denoiser/` | Audio denoising (MCRA/Wiener) |
| AB Compare | `plugin_ab_compare.rs` | A/B comparison plugin |
| Band Split | `plugin_band_split.rs` | Split signal into frequency bands |
| Band Merge | `plugin_band_merge.rs` | Merge frequency bands back together |
| Auto Gain | `auto_gain.rs` | Automatic gain normalization |

## Analyzer Plugins (extract data, do not modify audio)

| Plugin | File | Description |
|--------|------|-------------|
| Spectrum | `analyzer_spectrum.rs` | FFT-based spectrum analysis |
| Loudness Monitor | `analyzer_loudness_monitor.rs` | EBU R128 loudness measurement |

## Module Layout

- `plugin_*.rs` - Individual plugin implementations
- `host.rs` - DAG host routing (large file ~58KB)
- `analyzer_*.rs` - Analyzer plugins
- `plugin_binaural/`, `plugin_upmixer/`, `plugin_pnd/`, `plugin_xtc/`, `plugin_denoiser/` - Complex plugins in subdirectories
- `speaker_config.rs` - Channel layout configuration
- `sofa.rs` - HRTF data loading (SOFA format)
- `simd.rs` - SIMD optimizations
- `parameters.rs`, `automation.rs` - Parameter system
- `param_specs.rs` - Centralized parameter defaults/ranges
- `smoothing.rs` - One-pole parameter smoother for click-free transitions
- `serialization.rs` - Preset management
- `lib.rs` - Public API exports (all plugin types re-exported)
- macOS HAL Input/Output plugins behind `hal` feature

## Features

- `hal` - macOS HAL plugins
- `sofa_support` (default) - HRTF support via netCDF

## Testing

```bash
cargo test -p plugins --no-default-features --lib
cargo check -p plugins --no-default-features && cargo clippy -p plugins --no-default-features
```

## Benchmarks

```bash
cargo bench -p plugins -- binaural-decoder
cargo bench -p plugins -- upmixer
cargo bench -p plugins -- compressor
cargo bench -p plugins -- all-plugins
```

## Binaries

- `plugin-fuzzer` - Stress test plugins with random input

## Important Notes

- Channel count can change between plugins (e.g., upmixer: 2ch → 5ch)
- Plugin configs are JSON-based, created from `PluginConfig { plugin_type, parameters }`
- The DAG host in `host.rs` supports parallel processing of independent plugin chains
- Use `--no-default-features` for build/test to avoid `hdf5-metno-sys` build issues
