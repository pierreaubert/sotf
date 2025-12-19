# SOTF Audio Plugins

A comprehensive audio plugin system for real-time audio processing, featuring a flexible architecture that supports both linear plugin chains and complex DAG-based routing topologies.

## Overview

This crate provides:

- **Core Plugin System**: Traits and interfaces for building audio processors
- **Processing Plugins**: EQ, compressor, limiter, gate, gain, delay, convolution, upmixer, and more
- **Analyzer Plugins**: Spectrum analyzer, loudness monitor (EBU R128)
- **Plugin Host**: DAG-based host supporting parallel processing with thread-safe execution
- **FFI Layer**: C interface for integration with Audio Units and other native hosts
- **macOS Audio Units**: Swift/Objective-C wrapper for system-wide audio processing

## Architecture

### Core Traits

The plugin system is built on two main traits:

#### `Plugin` Trait

For plugins that may change the channel count (e.g., upmixers, downmixers):

```rust
pub trait Plugin: Send {
    fn info(&self) -> PluginInfo;
    fn input_channels(&self) -> usize;
    fn output_channels(&self) -> usize;
    fn parameters(&self) -> Vec<Parameter>;
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()>;
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue>;
    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()>;
    fn reset(&mut self);
    fn process(&mut self, input: &[f32], output: &mut [f32], context: &ProcessContext) -> PluginResult<()>;
    fn latency_samples(&self) -> usize;
    fn get_data(&self) -> Option<Arc<dyn Any + Send + Sync>>; // For analyzers
}
```

#### `InPlacePlugin` Trait

For plugins that process audio in-place (same input/output channel count):

```rust
pub trait InPlacePlugin: Send {
    fn info(&self) -> PluginInfo;
    fn channels(&self) -> usize;
    fn process_in_place(&mut self, buffer: &mut [f32], context: &ProcessContext) -> PluginResult<()>;
    // ... parameter methods ...
}
```

Use `InPlacePluginAdapter` to wrap an `InPlacePlugin` as a `Plugin`.

### Audio Format

All audio data uses **interleaved f32** format:
- Stereo: `[L0, R0, L1, R1, L2, R2, ...]`
- 5.1 surround: `[FL0, FR0, C0, LFE0, SL0, SR0, FL1, FR1, ...]`

Sample values are typically in the range -1.0 to +1.0.

## Plugin Categories

### Processing Plugins

| Plugin | Description | Channels | Key Parameters |
|--------|-------------|----------|----------------|
| `GainPlugin` | Volume control | N → N | `gain_db` |
| `EqPlugin` | Parametric EQ with biquad filters | N → N | `filters[]`, `channel_filters[][]` |
| `CompressorPlugin` | Dynamic range compression | N → N | `threshold`, `ratio`, `attack`, `release`, `knee` |
| `LimiterPlugin` | Peak limiting | N → N | `threshold`, `release`, `lookahead` |
| `GatePlugin` | Noise gate | N → N | `threshold`, `attack`, `hold`, `release` |
| `DelayPlugin` | Multi-channel delay | N → N | `delay_ms[]`, `feedback`, `mix` |
| `CrossoverPlugin` | Frequency band splitting | N → N×bands | `frequencies[]` |
| `MatrixPlugin` | Channel routing/mixing | N → M | `matrix[][]` |
| `ResamplerPlugin` | Sample rate conversion | N → N | `target_rate` |
| `ConvolutionPlugin` | IR-based convolution | N → N | `ir_path`, `wet_dry` |
| `LoudnessCompensationPlugin` | Equal-loudness contour | N → N | `reference_level`, `current_level` |
| `UpmixerPlugin` | Stereo to surround upmixing | 2 → M | `speaker_config`, `gain_*`, `lfe_cutoff` |
| `BinauralDecoderPlugin` | Surround to binaural | M → 2 | `sofa_file`, `externalization` |
| `ChannelMuteSoloPlugin` | Channel mute/solo control | N → N | `mute[]`, `solo[]` |

### Analyzer Plugins

| Plugin | Description | Output Data |
|--------|-------------|-------------|
| `SpectrumAnalyzerPlugin` | FFT-based spectrum analysis | `SpectrumInfo { frequencies, magnitudes, peak }` |
| `LoudnessMonitorPlugin` | EBU R128 loudness measurement | `LoudnessInfo { momentary, short_term, integrated, range }` |

Analyzers pass audio through unmodified while extracting measurements accessible via `get_data()`.

### macOS-Specific Plugins

| Plugin | Description | Feature Flag |
|--------|-------------|--------------|
| `HalInputPlugin` | CoreAudio HAL input capture | `hal` |
| `HalOutputPlugin` | CoreAudio HAL output routing | `hal` |

## Plugin Host

### DawHost (PluginHost)

The main host implementation supports two modes:

#### Chain Mode (Linear)

Compatible with simple PluginHost API:

```rust
use sotf_plugins::{PluginHost, GainPlugin, InPlacePluginAdapter, Host};

let mut host = PluginHost::new(2, 48000); // 2 channels, 48kHz

// Add plugins - they process in sequence
host.add_plugin(Box::new(InPlacePluginAdapter::new(GainPlugin::new(2, -6.0))))?;
host.add_plugin(Box::new(InPlacePluginAdapter::new(GainPlugin::new(2, -3.0))))?;

// Process audio
let input = vec![1.0; 1024]; // 512 frames, 2 channels
let mut output = vec![0.0; 1024];
host.process(&input, &mut output)?;
```

#### Graph Mode (DAG)

For complex routing topologies with parallel processing:

```rust
use sotf_plugins::{DawHost, GraphEdge, GainPlugin, InPlacePluginAdapter};

let mut host = DawHost::new(2, 48000);

// Create a diamond pattern:
//       -> gain2 ->
// gain1             gain4
//       -> gain3 ->

let node1 = host.add_node("input".into(), Box::new(plugin1))?;
let node2 = host.add_node("branch_a".into(), Box::new(plugin2))?;
let node3 = host.add_node("branch_b".into(), Box::new(plugin3))?;
let node4 = host.add_node("merge".into(), Box::new(plugin4))?;

host.add_edge(GraphEdge::new(node1, node2))?;
host.add_edge(GraphEdge::new(node1, node3))?;
host.add_edge(GraphEdge::new(node2, node4))?;
host.add_edge(GraphEdge::new(node3, node4))?;

host.build()?;
host.process(&input, &mut output)?;
```

Features:
- **Automatic cycle detection**: Prevents feedback loops
- **Topological sorting**: Ensures correct processing order
- **Stage-based parallel execution**: Nodes without dependencies run concurrently
- **Thread-safe plugins**: `Arc<Mutex<>>` wrapping with scoped threads
- **Channel mapping**: Custom routing between nodes via `GraphEdge::with_channels()`

## Individual Plugin Documentation

### EqPlugin

Parametric EQ supporting both single-chain and per-channel modes:

```rust
use sotf_plugins::{EqPlugin, EqPluginParams, BiquadFilterConfig};
use autoeq_iir::Biquad;

// Mode 1: Single chain for all channels
let filters = vec![
    Biquad::peak(1000.0, 1.5, 3.0, 48000.0),  // 1kHz, Q=1.5, +3dB
    Biquad::highshelf(8000.0, 0.7, -2.0, 48000.0),
];
let eq = EqPlugin::new(2, filters);

// Mode 2: Per-channel EQ
let channel_filters = vec![
    vec![Biquad::peak(100.0, 1.0, -6.0, 48000.0)],  // Left channel
    vec![Biquad::peak(100.0, 1.0, -3.0, 48000.0)],  // Right channel
];
let eq = EqPlugin::new_per_channel(2, channel_filters)?;

// From JSON params
let params = EqPluginParams {
    filters: vec![
        BiquadFilterConfig { filter_type: "peak".into(), freq: 1000.0, q: 1.5, db_gain: 3.0 },
    ],
    channel_filters: None,
};
```

Filter types: `peak`, `lowshelf`, `highshelf`, `lowpass`, `highpass`, `bandpass`, `notch`

### CompressorPlugin

Full-featured dynamics compressor:

```rust
use sotf_plugins::{CompressorPlugin, CompressorPluginParams};

let params = CompressorPluginParams {
    threshold_db: -20.0,      // Compression starts at -20dB
    ratio: 4.0,               // 4:1 compression ratio
    attack_ms: 10.0,          // 10ms attack time
    release_ms: 100.0,        // 100ms release time
    knee_db: 6.0,             // 6dB soft knee
    makeup_gain_db: 0.0,      // Manual makeup gain
    mix: 1.0,                 // 100% wet (parallel compression: use < 1.0)
    auto_makeup: true,        // Auto-calculate makeup gain
    link_channels: true,      // Stereo linking
    sidechain_hpf_hz: 80.0,   // HPF on detector to reduce pumping
};

let compressor = CompressorPlugin::new(2, params);

// Get gain reduction data for metering
if let Some(data) = compressor.get_data() {
    let comp_data = data.downcast_ref::<CompressorData>().unwrap();
    println!("GR: {:?} dB", comp_data.gain_reduction_db);
}
```

### UpmixerPlugin

FFT-based stereo to surround upmixer with VBAP panning:

```rust
use sotf_plugins::{UpmixerPlugin, UpmixerPluginParams};

let params = UpmixerPluginParams {
    speaker_config: "5.1".into(),      // Target speaker layout
    gain_front_direct: 1.0,            // Front direct sound level
    gain_front_ambient: 0.7,           // Front ambient level
    gain_rear_ambient: 0.8,            // Rear ambient level
    lfe_cutoff_hz: 120.0,              // LFE crossover frequency
    stereo_width: 0.5,                 // 0=wide, 1=narrow
    height_gain: 0.5,                  // Height channel level (if applicable)
    enable_hr_processing: true,        // High-resolution direct path
    // ... more parameters
};

let upmixer = UpmixerPlugin::new(params)?;
```

Supported configurations: `2.0`, `5.0`, `5.1`, `7.1`, `5.1.2`, `5.1.4`, `7.1.2`, `7.1.4`, `9.1.4`, `9.1.6`

### BinauralDecoderPlugin

Multichannel to binaural rendering using SOFA HRTFs:

```rust
use sotf_plugins::{BinauralDecoderPlugin, BinauralDecoderParams};

let params = BinauralDecoderParams {
    sofa_file: Some("path/to/hrtf.sofa".into()),
    fft_size: 2048,
    enable_optimization: true,  // Sum-before-IFFT optimization
    externalization: 0.5,       // Early reflection simulation
    room_model: RoomModel::Small,
};

let decoder = BinauralDecoderPlugin::new(6, params)?; // 5.1 input
```

### SpectrumAnalyzerPlugin

Real-time FFT spectrum analysis:

```rust
use sotf_plugins::{SpectrumAnalyzerPlugin, SpectrumConfig};

let config = SpectrumConfig {
    num_bins: 30,           // Number of frequency bands
    min_freq: 20.0,         // Minimum frequency (Hz)
    max_freq: 20000.0,      // Maximum frequency (Hz)
    smoothing: 0.7,         // EMA smoothing factor
};

let analyzer = SpectrumAnalyzerPlugin::new(2, 48000, config)?;

// After processing, get spectrum data
if let Some(data) = analyzer.get_data() {
    let spectrum = data.downcast_ref::<SpectrumInfo>().unwrap();
    for (freq, mag) in spectrum.frequencies.iter().zip(&spectrum.magnitudes) {
        println!("{:.1} Hz: {:.1} dB", freq, mag);
    }
}
```

### LoudnessMonitorPlugin

EBU R128 loudness measurement:

```rust
use sotf_plugins::LoudnessMonitorPlugin;

let monitor = LoudnessMonitorPlugin::new(2, 48000)?;

// After processing
if let Some(data) = monitor.get_data() {
    let loudness = data.downcast_ref::<LoudnessInfo>().unwrap();
    println!("Momentary: {:.1} LUFS", loudness.momentary);
    println!("Short-term: {:.1} LUFS", loudness.short_term);
    println!("Integrated: {:.1} LUFS", loudness.integrated);
    println!("Loudness Range: {:.1} LU", loudness.range);
}
```

## Directory Structure

```
sotf-audio-plugins/
├── Cargo.toml              # Crate manifest
├── README.md               # This file
├── src/
│   ├── lib.rs              # Public API exports
│   ├── plugin.rs           # Core Plugin/InPlacePlugin traits
│   ├── host.rs             # DawHost implementation
│   ├── parameters.rs       # Parameter system
│   ├── param_specs.rs      # Parameter specifications/constants
│   ├── analyzer.rs         # Analyzer traits and types
│   │
│   ├── plugin_eq.rs        # Parametric EQ
│   ├── plugin_gain.rs      # Gain/volume
│   ├── plugin_compressor.rs # Dynamics compressor
│   ├── plugin_limiter.rs   # Peak limiter
│   ├── plugin_gate.rs      # Noise gate
│   ├── plugin_delay.rs     # Multi-channel delay
│   ├── plugin_crossover.rs # Frequency crossover
│   ├── plugin_matrix.rs    # Channel matrix routing
│   ├── plugin_resampler.rs # Sample rate conversion
│   ├── plugin_convolution.rs # IR convolution
│   ├── plugin_loudness_compensation.rs # Equal-loudness curves
│   ├── plugin_channel_mute_solo.rs # Mute/solo control
│   │
│   ├── plugin_upmixer/     # Stereo to surround upmixer
│   │   ├── mod.rs          # Main plugin implementation
│   │   ├── config.rs       # Configuration types
│   │   ├── fft.rs          # FFT processing
│   │   ├── panning.rs      # VBAP panning
│   │   ├── bass.rs         # LFE management
│   │   ├── decorrelation.rs # Ambient decorrelation
│   │   ├── height.rs       # Height channel processing
│   │   └── ...
│   │
│   ├── plugin_binaural/    # Multichannel to binaural decoder
│   │   ├── mod.rs          # Main plugin
│   │   ├── hrtf.rs         # HRTF processing
│   │   ├── filter.rs       # Convolution filters
│   │   ├── room.rs         # Room simulation
│   │   └── ...
│   │
│   ├── analyzer_spectrum.rs     # Spectrum analyzer
│   ├── analyzer_loudness_monitor.rs # EBU R128 loudness
│   │
│   ├── plugin_hal_input.rs  # macOS HAL input (feature: hal)
│   ├── plugin_hal_output.rs # macOS HAL output (feature: hal)
│   │
│   ├── speaker_config.rs   # Speaker layout definitions
│   ├── sofa.rs             # SOFA file parsing
│   └── simd.rs             # SIMD optimizations
│
├── bin/
│   └── plugin_fuzzer.rs    # Fuzz testing tool
│
├── tests/
│   ├── test_plugins.rs         # Basic plugin tests
│   ├── test_eq_plugin.rs       # EQ plugin tests
│   ├── test_dynamics_plugins.rs # Compressor/limiter/gate tests
│   ├── test_analyzer_plugins.rs # Analyzer tests
│   ├── test_binaural_decoder.rs # Binaural tests
│   ├── test_upmixer_integration.rs # Upmixer tests
│   ├── test_loudness_compensation.rs
│   └── test_resampler_plugin.rs
│
├── benches/
│   ├── README.md                    # Benchmark documentation
│   ├── binaural-decoder-benchmark.rs
│   ├── upmixer-benchmark.rs
│   └── compressor-benchmark.rs
│
├── src-ffi/                 # C FFI for native hosts
│   ├── Cargo.toml
│   ├── src/
│   │   └── lib.rs           # FFI exports
│   ├── sotf_audio_plugin_ffi.h # Generated C header
│   └── cbindgen.toml
│
└── src-au/                  # macOS Audio Unit wrapper
    ├── README.md
    ├── QUICKSTART.md
    ├── SETUP_GUIDE.md
    ├── EQAudioUnit/         # Swift AU implementation
    └── SOTFAudioUnits.xcodeproj
```

## Testing

### Unit Tests

```bash
# Run all tests
cargo test -p sotf-audio-plugins

# Run specific test module
cargo test -p sotf-audio-plugins test_eq_plugin

# Run with output
cargo test -p sotf-audio-plugins -- --nocapture
```

### Integration Tests

```bash
cargo test -p sotf-audio-plugins --test test_upmixer_integration
cargo test -p sotf-audio-plugins --test binaural_decoder_integration
```

### Fuzz Testing

The plugin fuzzer tests plugins with random parameter combinations to detect:
- NaN/Inf values
- Extreme amplitudes
- DC offset
- Excessive clipping

```bash
# Run fuzzer
cargo run -p sotf-audio-plugins --bin plugin-fuzzer -- \
    --file audio.wav \
    --plugin compressor \
    --iterations 1000 \
    --seed 42
```

Supported plugins: `gain`, `eq`, `compressor`, `limiter`, `gate`, `delay`, `loudness`, `crossover`, `upmixer`

## Benchmarks

Performance benchmarks using Criterion:

```bash
# Run all benchmarks
cargo bench -p sotf-audio-plugins

# Run specific benchmark
cargo bench -p sotf-audio-plugins --bench upmixer-benchmark

# Run with specific test group
cargo bench -p sotf-audio-plugins --bench binaural-decoder-benchmark -- binaural_atmos_7_1_4

# Quick run (no statistical analysis)
cargo bench -p sotf-audio-plugins -- --quick
```

### Benchmark Groups

**Binaural Decoder:**
- `binaural_process_channels` - Scaling with channel count
- `binaural_fft_sizes` - FFT size impact
- `binaural_optimization` - Optimized vs standard path
- `binaural_atmos_7_1_4` - Real-world 12-channel workload

**Upmixer:**
- `upmixer_5_1_block_sizes` - Buffer size scaling
- `upmixer_configs` - Speaker configuration comparison
- `upmixer_fft_sizes` - FFT size trade-offs

### Performance Targets

For real-time audio at 48kHz with 512-sample buffers (10.67ms of audio):
- **Target**: < 2ms processing time (5x real-time margin)
- **Maximum**: < 10ms (1x real-time)

## FFI Integration

### C Header

The FFI layer exposes plugins via a C-compatible interface:

```c
#include "sotf_audio_plugin_ffi.h"

// Create and configure a plugin
SotfPluginHandle* plugin = sotf_create_eq_plugin(2, 48000);
sotf_plugin_set_parameter_float(plugin, "gain_db", -6.0f);

// Process audio
sotf_plugin_process(plugin, input, output, num_frames);

// Cleanup
sotf_destroy_plugin(plugin);
```

### macOS Audio Units

The `src-au/` directory contains a Swift wrapper for building system-wide Audio Units:

```bash
# Build Audio Unit
cd src-au
xcodebuild -project SOTFAudioUnits.xcodeproj -scheme EQAudioUnit -configuration Release

# Install to user Audio Units folder
cp -r build/Release/EQAudioUnit.appex ~/Library/Audio/Plug-Ins/Components/

# Validate
auval -v aufx EQau SOTF
```

See `src-au/README.md` and `src-au/SETUP_GUIDE.md` for detailed instructions.

## Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `hal` | Enable macOS CoreAudio HAL plugins | Yes |
| `no-hal` | Build without HAL dependencies | No |

```bash
# Build without HAL
cargo build -p sotf-audio-plugins --no-default-features --features no-hal
```

## Creating Custom Plugins

### Basic InPlacePlugin

```rust
use sotf_plugins::{InPlacePlugin, PluginInfo, ProcessContext, Parameter, ParameterId, ParameterValue};

pub struct MyPlugin {
    channels: usize,
    gain: f32,
}

impl InPlacePlugin for MyPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "My Plugin".into(),
            version: "1.0.0".into(),
            author: "Your Name".into(),
            description: "Does something cool".into(),
        }
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![Parameter::new_float("gain", "Gain", 1.0, 0.0, 2.0)]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> Result<(), String> {
        match id.as_str() {
            "gain" => {
                if let ParameterValue::Float(v) = value {
                    self.gain = v;
                    Ok(())
                } else {
                    Err("Expected float".into())
                }
            }
            _ => Err("Unknown parameter".into()),
        }
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        match id.as_str() {
            "gain" => Some(ParameterValue::Float(self.gain)),
            _ => None,
        }
    }

    fn process_in_place(&mut self, buffer: &mut [f32], _context: &ProcessContext) -> Result<(), String> {
        for sample in buffer.iter_mut() {
            *sample *= self.gain;
        }
        Ok(())
    }
}
```

### Channel-Changing Plugin

```rust
use sotf_plugins::{Plugin, PluginInfo, ProcessContext, Parameter, ParameterId, ParameterValue};

pub struct StereoToMonoPlugin;

impl Plugin for StereoToMonoPlugin {
    fn info(&self) -> PluginInfo { /* ... */ }
    fn input_channels(&self) -> usize { 2 }
    fn output_channels(&self) -> usize { 1 }
    fn parameters(&self) -> Vec<Parameter> { vec![] }
    fn set_parameter(&mut self, _: ParameterId, _: ParameterValue) -> Result<(), String> {
        Err("No parameters".into())
    }
    fn get_parameter(&self, _: &ParameterId) -> Option<ParameterValue> { None }

    fn process(&mut self, input: &[f32], output: &mut [f32], context: &ProcessContext) -> Result<(), String> {
        for frame in 0..context.num_frames {
            let left = input[frame * 2];
            let right = input[frame * 2 + 1];
            output[frame] = (left + right) * 0.5;
        }
        Ok(())
    }
}
```

## Performance Considerations

1. **In-Place Processing**: Prefer `InPlacePlugin` when channel count doesn't change - avoids buffer copies
2. **Buffer Sizes**: Larger buffers amortize per-call overhead but increase latency
3. **Parallel Processing**: Enable with `host.set_parallel_enabled(true)` for complex graphs
4. **SIMD**: Use SIMD intrinsics in hot paths (see `simd.rs` for examples)
5. **Memory Allocation**: Pre-allocate buffers in `initialize()`, avoid allocations in `process()`

## Dependencies

- **autoeq-iir**: Biquad filter implementation
- **rustfft/realfft**: FFT processing
- **rubato**: Sample rate conversion
- **ebur128**: EBU R128 loudness measurement
- **symphonia**: Audio file decoding (for tests/fuzzer)
- **criterion**: Benchmarking framework
- **serde**: JSON serialization for plugin parameters

## License

Part of the SOTF (Sound of the Future) project.
