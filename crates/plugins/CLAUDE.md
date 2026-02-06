# plugins (lib: `sotf_plugins`)

Comprehensive audio plugin system with processing plugins, analyzers, and DAG-based routing.

## Plugin Architecture

Two core traits:
- `Plugin` (`plugin.rs`): Variable input/output channel counts — `process(&mut self, input: &[&[f32]], output: &mut [&mut [f32]])`
- `InPlacePlugin` (`plugin.rs`): Same in/out channel count — auto-wrapped into `Plugin`
- `DawHost` (`host.rs`): DAG-based plugin routing with parallel processing

Plugins are instantiated from `PluginConfig` JSON via the factory in `mod.rs`.

## Processing Plugins (transform audio)

| Plugin | File | Description |
|--------|------|-------------|
| EQ | `plugin_eq.rs` | Parametric EQ with biquad filters (uses math-iir-fir::Biquad) |
| Gain | `plugin_gain.rs` | Simple volume control |
| Compressor | `plugin_compressor.rs` | Dynamic range compression |
| Gate | `plugin_gate.rs` | Noise gate |
| Limiter | `plugin_limiter.rs` | Peak limiter |
| Delay | `plugin_delay.rs` | Audio delay |
| Convolution | `plugin_convolution.rs` | FIR convolution processing |
| Crossover | `plugin_crossover.rs` | Frequency band splitting |
| Matrix | `plugin_matrix.rs` | Channel matrix mixing |
| Resampler | `plugin_resampler.rs` | Sample rate conversion |
| Upmixer | `plugin_upmixer/` | Stereo to 5.0 surround via FFT spatial processing |
| Binaural | `plugin_binaural/` | HRTF-based binaural rendering |
| XTC | `plugin_xtc/` | Crosstalk cancellation |
| PND | `plugin_pnd/` | Perceptual Noise Diffusion |
| Loudness Comp | `plugin_loudness_compensation.rs` | Equal-loudness contour compensation |
| Multiband Dyn | `plugin_multiband_dynamics.rs` | Multiband dynamics processing |
| Denoiser | `plugin_denoiser/` | Audio denoising |

## Analyzer Plugins (extract data, do not modify audio)

| Plugin | File | Description |
|--------|------|-------------|
| Spectrum | `analyzer_spectrum.rs` | FFT-based spectrum analysis |
| Loudness Monitor | `analyzer_loudness_monitor.rs` | EBU R128 loudness measurement |

## Special Plugins

- AB Compare, Band Split/Merge, Channel Mute/Solo, Fletcher-Munson
- macOS HAL Input/Output (behind `hal` feature)

## Module Layout

- `plugin_*.rs` - Individual plugin implementations
- `host.rs` - DAG host routing (large file ~58KB)
- `analyzer_*.rs` - Analyzer plugins
- `plugin_binaural/`, `plugin_upmixer/`, `plugin_pnd/`, `plugin_xtc/`, `plugin_denoiser/` - Complex plugins in subdirectories
- `speaker_config.rs` - Channel layout configuration
- `sofa.rs` - HRTF data loading (SOFA format)
- `simd.rs` - SIMD optimizations
- `parameters.rs`, `automation.rs` - Parameter system
- `serialization.rs` - Preset management
- `mod.rs` - Plugin factory (creates plugins from PluginConfig)

## Features

- `hal` - macOS HAL plugins
- `sofa_support` (default) - HRTF support via netCDF

## Testing

```bash
cargo test -p plugins --lib
cargo check -p plugins && cargo clippy -p plugins
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
