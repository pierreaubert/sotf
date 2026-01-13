# engine

A native multi-threaded audio processing engine written in pure Rust. Designed for high-performance audio playback with a flexible plugin system, hot-reload capabilities, and comprehensive format support.

## Features

- **Multi-threaded Architecture**: 4 concurrent threads with lock-free queues for low-latency playback
- **Plugin System**: Modular audio processing with hot-reload support (EQ, compressor, upmixer, analyzers, etc.)
- **Multi-format Decoding**: FLAC, MP3, AAC, WAV, AIFF, Vorbis via Symphonia
- **High-quality Resampling**: Automatic sample rate conversion using rubato
- **Config Watching**: Live config file updates and Unix signal handling (SIGHUP, SIGTERM)
- **Cross-platform**: macOS, Linux, and Windows support via cpal
- **Signal Analysis**: FFT-based spectrum analysis, ReplayGain 2.0, waveform extraction
- **Signal Generation**: Sine, sweep, pink/white noise for testing and calibration

## Architecture

The engine uses a 4-thread architecture with dedicated queues for each stage:

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Manager Thread                               │
│  - Command routing        - Config file watching                    │
│  - Thread coordination    - Unix signal handling (SIGHUP, SIGTERM)  │
│  - Shared state management                                          │
└─────────────────────────────────────────────────────────────────────┘
         │                    │                    │
         ▼                    ▼                    ▼
┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
│  Decoder Thread │  │ Processing      │  │ Playback Thread │
├─────────────────┤  │ Thread          │  ├─────────────────┤
│ • File I/O      │  ├─────────────────┤  │ • cpal output   │
│ • Symphonia     │  │ • Plugin chain  │  │ • Lock-free     │
│   decoding      │  │ • Channel       │  │   ring buffer   │
│ • Resampling    │  │   mixing        │  │ • Real-time     │
│ • Seek handling │  │ • Hot-reload    │  │   priority      │
└────────┬────────┘  └────────┬────────┘  └────────┬────────┘
         │                    │                    │
         ▼                    ▼                    ▼
     Queue 1              Queue 2              Hardware
   (DecoderMsg)        (ProcessMsg)         Audio Output
```

### Thread Responsibilities

1. **Manager Thread**: Receives commands from the public API, coordinates other threads, watches config files and handles Unix signals (SIGHUP for reload, SIGTERM/SIGINT for shutdown).

2. **Decoder Thread**: Reads audio files using Symphonia, decodes to PCM f32, resamples to target sample rate, and sends audio frames to the processing thread.

3. **Processing Thread**: Runs the plugin chain (EQ, effects, analyzers), handles hot-reload of plugin configurations, and can change channel counts (e.g., 2ch → 5.1ch upmixing).

4. **Playback Thread**: Outputs audio to hardware via cpal using a lock-free ring buffer. Runs at highest priority to prevent underruns.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
engine = { path = "../engine" }
```

## Quick Start

### Basic Playback

```rust
use sotf_audio_engine::engine::{AudioEngine, EngineConfig};

fn main() -> Result<(), String> {
    // Create engine with default configuration
    let config = EngineConfig::default();
    let mut engine = AudioEngine::new(config)?;

    // Start playback
    engine.play("music.flac")?;
    engine.set_volume(0.8)?;

    // Monitor state
    let state = engine.get_state();
    println!("Playing: {:?}", state.current_file);
    println!("Position: {:.2}s / {:?}s", state.position, state.duration);

    // Playback control
    engine.pause()?;
    engine.seek(30.0)?;  // Seek to 30 seconds
    engine.resume()?;

    // Clean shutdown
    engine.shutdown()?;
    Ok(())
}
```

### With Plugin Chain

```rust
use sotf_audio_engine::engine::{AudioEngine, EngineConfig};
use sotf_audio_engine::types::PluginConfig;
use serde_json::json;

fn main() -> Result<(), String> {
    // Configure EQ plugin
    let eq_plugin = PluginConfig {
        plugin_type: "EQ".to_string(),
        parameters: json!({
            "filters": [
                {
                    "filter_type": "peak",
                    "frequency": 100.0,
                    "q": 0.7,
                    "gain_db": 3.0
                },
                {
                    "filter_type": "highshelf",
                    "frequency": 8000.0,
                    "q": 0.7,
                    "gain_db": -2.0
                }
            ]
        }),
    };

    // Configure gain plugin
    let gain_plugin = PluginConfig {
        plugin_type: "gain".to_string(),
        parameters: json!({ "gain_db": -3.0 }),
    };

    // Create engine with plugins
    let mut config = EngineConfig::default();
    config.plugins = vec![eq_plugin, gain_plugin];

    let mut engine = AudioEngine::new(config)?;
    engine.play("music.flac")?;

    Ok(())
}
```

### Config File Watching

```rust
use sotf_audio_engine::engine::{AudioEngine, EngineConfig};
use std::path::PathBuf;

fn main() -> Result<(), String> {
    let mut config = EngineConfig::default();
    config.config_path = Some(PathBuf::from("engine_config.yaml"));
    config.watch_config = true;  // Enable file watching and signal handling

    let mut engine = AudioEngine::new(config)?;
    engine.play("music.flac")?;

    // Engine will automatically reload on:
    // - Config file changes (debounced 300ms)
    // - SIGHUP signal
    // And shutdown gracefully on:
    // - SIGTERM / SIGINT signals

    Ok(())
}
```

## Configuration

### EngineConfig

```rust
pub struct EngineConfig {
    pub version: u32,                   // Config version (default: 1)
    pub frame_size: usize,              // Processing block size (default: 1024)
    pub buffer_ms: u32,                 // Queue buffer latency (default: 200ms)
    pub output_sample_rate: u32,        // Target sample rate (default: 48000)
    pub input_channels: usize,          // Source channels (default: 2)
    pub output_channels: usize,         // Output channels (default: 2)
    pub output_device: Option<String>,  // None = default device
    pub plugins: Vec<PluginConfig>,     // Initial plugin chain
    pub volume: f32,                    // Initial volume 0.0-1.0 (default: 1.0)
    pub muted: bool,                    // Start muted (default: false)
    pub config_path: Option<PathBuf>,   // Config file path for watching
    pub watch_config: bool,             // Enable config/signal watching
}
```

### Key Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `frame_size` | 1024 | Audio frames per processing block. Lower = less latency, more CPU overhead |
| `buffer_ms` | 200 | Total buffering between threads. Higher = more resilient, more latency |
| `output_sample_rate` | 48000 | All audio is resampled to this rate |
| `output_channels` | 2 | Final output channel count (can differ from input if upmixer used) |

### YAML Configuration Example

```yaml
version: 1
frame_size: 1024
buffer_ms: 200
output_sample_rate: 48000
input_channels: 2
output_channels: 2
volume: 0.8
muted: false
plugins:
  - plugin_type: EQ
    parameters:
      filters:
        - filter_type: peak
          frequency: 1000.0
          q: 1.5
          gain_db: 3.0
  - plugin_type: gain
    parameters:
      gain_db: -6.0
```

Load from file:

```rust
let config = EngineConfig::load_from_file(&PathBuf::from("config.yaml"))?;
```

## Plugin System

Plugins are configured via JSON and loaded dynamically. The processing thread chains plugins together, allowing audio to flow through each processor in sequence.

### Available Plugins

#### Processing Plugins (Transform Audio)

| Plugin Type | Description | Parameters |
|-------------|-------------|------------|
| `EQ` | Parametric equalizer | `filters`: array of filter definitions |
| `gain` | Volume control | `gain_db`: gain in decibels |
| `compressor` | Dynamic range compression | `threshold_db`, `ratio`, `attack_ms`, `release_ms` |
| `gate` | Noise gate | `threshold_db`, `attack_ms`, `release_ms` |
| `limiter` | Peak limiter | `threshold_db`, `release_ms` |
| `upmixer` | Stereo → surround | `output_channels`: 5, 6, 7, or 8 |
| `resampler` | Sample rate conversion | `target_sample_rate` |
| `loudness_compensation` | Equal-loudness contour | `reference_level_db` |
| `matrix` | Channel matrix mixing | `matrix`: 2D coefficient array |
| `delay` | Time delay | `delay_ms`, `feedback` |
| `crossover` | Frequency band split | `frequencies`: array of crossover points |

#### Analyzer Plugins (Extract Data, Don't Modify Audio)

| Plugin Type | Description | Data Access |
|-------------|-------------|-------------|
| `loudness_monitor` | EBU R128 loudness | `get_plugin_data()` returns LUFS values |
| `spectrum` | FFT spectrum analysis | `get_plugin_data()` returns frequency bins |

### EQ Filter Types

```rust
pub enum BiquadFilterType {
    Peak,       // Bell/parametric
    Lowshelf,   // Low shelf
    Highshelf,  // High shelf
    Lowpass,    // Low pass
    Highpass,   // High pass
    Bandpass,   // Band pass
    Notch,      // Band reject
}
```

### EQ Example

```rust
let eq_config = PluginConfig {
    plugin_type: "EQ".to_string(),
    parameters: json!({
        "filters": [
            {
                "filter_type": "highpass",
                "frequency": 80.0,
                "q": 0.707
            },
            {
                "filter_type": "peak",
                "frequency": 250.0,
                "q": 2.0,
                "gain_db": -4.0
            },
            {
                "filter_type": "peak",
                "frequency": 3000.0,
                "q": 1.5,
                "gain_db": 2.0
            },
            {
                "filter_type": "highshelf",
                "frequency": 10000.0,
                "q": 0.7,
                "gain_db": -2.0
            }
        ]
    }),
};
```

### Hot-Reload

Plugin chains can be updated without stopping playback:

```rust
// Update plugin chain at runtime
let new_plugins = vec![/* new configuration */];
engine.update_plugin_chain(new_plugins)?;

// Or update individual plugin parameters
engine.set_plugin_parameter(
    0,                          // Plugin index
    "gain_db".to_string(),      // Parameter ID
    "-6.0".to_string(),         // New value
)?;
```

### Accessing Analyzer Data

```rust
// Get spectrum data from analyzer plugin at index 2
let spectrum_data = engine.get_plugin_data(2)?;
if let Some(spectrum) = spectrum_data.downcast_ref::<SpectrumData>() {
    for (freq, magnitude) in &spectrum.bins {
        println!("{:.1} Hz: {:.1} dB", freq, magnitude);
    }
}
```

## API Reference

### AudioEngine

```rust
impl AudioEngine {
    // Lifecycle
    pub fn new(config: EngineConfig) -> Result<Self, String>
    pub fn new_default() -> Result<Self, String>
    pub fn shutdown(&mut self) -> Result<(), String>

    // Playback Control
    pub fn play<P: Into<PathBuf>>(&mut self, path: P) -> Result<(), String>
    pub fn pause(&mut self) -> Result<(), String>
    pub fn resume(&mut self) -> Result<(), String>
    pub fn stop(&mut self) -> Result<(), String>
    pub fn seek(&mut self, position: f64) -> Result<(), String>  // Seconds

    // Volume Control
    pub fn set_volume(&mut self, volume: f32) -> Result<(), String>  // 0.0-1.0
    pub fn set_mute(&mut self, muted: bool) -> Result<(), String>

    // Plugin Control
    pub fn update_plugin_chain(&mut self, plugins: Vec<PluginConfig>) -> Result<(), String>
    pub fn set_plugin_parameter(
        &mut self,
        plugin_index: usize,
        param_id: String,
        value: String,
    ) -> Result<(), String>
    pub fn set_bypass(&mut self, bypass: bool) -> Result<(), String>

    // Queries
    pub fn get_state(&self) -> AudioEngineState
    pub fn get_position(&mut self) -> Result<f64, String>
    pub fn get_plugin_data(&mut self, index: usize) -> Result<Arc<dyn Any + Send + Sync>, String>
    pub fn reload_config(&mut self) -> Result<(), String>
}
```

### AudioEngineState

```rust
pub struct AudioEngineState {
    pub playback_state: PlaybackState,    // Stopped, Playing, Paused
    pub current_file: Option<PathBuf>,
    pub position: f64,                     // Current position in seconds
    pub duration: Option<f64>,             // Total duration in seconds
    pub sample_rate: u32,
    pub num_channels: usize,
    pub volume: f32,
    pub muted: bool,
    pub processing_bypassed: bool,
    pub underruns: u64,                    // Playback underrun count
    pub last_error: Option<String>,
    pub seeking: bool,
}
```

### PlaybackState

```rust
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}
```

## Decoder Support

### Supported Formats

| Format | Extensions | Lossless | Notes |
|--------|------------|----------|-------|
| FLAC | `.flac` | Yes | Full metadata support |
| WAV | `.wav` | Yes | PCM 16/24/32-bit |
| AIFF | `.aiff`, `.aif` | Yes | Apple lossless |
| MP3 | `.mp3` | No | MPEG Layer III |
| AAC | `.m4a`, `.mp4`, `.aac` | No | MPEG-4 Audio |
| Vorbis | `.ogg`, `.oga` | No | OGG container |
| ALAC | `.m4a` | Yes | Apple Lossless in MP4 |

### Decoder API

```rust
use sotf_audio_engine::decoder::{create_decoder, probe_file, AudioFormat};

// Probe file without full decoding
let (format, spec) = probe_file("audio.flac")?;
println!("Format: {:?}", format);
println!("Sample rate: {} Hz", spec.sample_rate);
println!("Channels: {}", spec.channels);
println!("Duration: {:?}", spec.duration());

// Create full decoder
let mut decoder = create_decoder("audio.flac")?;
while let Some(audio) = decoder.decode_next()? {
    // audio.samples contains interleaved f32 samples [-1.0, 1.0]
    process_audio(&audio.samples);
}
```

### AudioSpec

```rust
pub struct AudioSpec {
    pub sample_rate: u32,       // Hz (44100, 48000, 96000, etc.)
    pub channels: u16,
    pub bits_per_sample: u16,   // Original bit depth (16, 24, 32)
    pub total_frames: Option<u64>,
}

impl AudioSpec {
    pub fn duration(&self) -> Option<Duration>
    pub fn bytes_per_frame(&self) -> u32
}
```

### Error Handling

```rust
pub enum AudioDecoderError {
    FileNotFound(String),
    UnsupportedFormat(String),
    InvalidFile(String),
    DecodingFailed(String),
    StreamEnded,
    IoError(String),
    ConfigError(String),
    SeekFailed(String),
}

// User-friendly error messages for UI display
pub fn user_friendly_error(error: &AudioDecoderError) -> String
```

## Signal Analysis

### ReplayGain 2.0

Calculate loudness normalization values using EBU R128:

```rust
use sotf_audio_engine::replaygain::analyze_file;

let info = analyze_file("audio.flac")?;
println!("Gain adjustment: {:.2} dB", info.gain);
println!("Peak level: {:.4}", info.peak);
```

### Waveform Extraction

Generate waveform visualization data:

```rust
use sotf_audio_engine::waveform::analyze_waveform;

let waveform = analyze_waveform("audio.flac")?;
// Returns 128 amplitude values (0-255) representing RMS per time segment
for (i, amplitude) in waveform.iter().enumerate() {
    println!("Segment {}: {}", i, amplitude);
}
```

### FFT Spectrum Analysis

```rust
use sotf_audio_engine::signal_analysis::analyze_recording;

let samples: Vec<f32> = /* recorded audio */;
let spectrum = analyze_recording(&samples, 48000, 20.0, 20000.0);

for (freq, magnitude_db) in spectrum {
    println!("{:.1} Hz: {:.1} dB", freq, magnitude_db);
}
```

### Latency Estimation

```rust
use sotf_audio_engine::signal_analysis::estimate_latency;

let reference = /* original signal */;
let measured = /* recorded signal */;
if let Some(latency_ms) = estimate_latency(&reference, &measured, 48000) {
    println!("System latency: {:.2} ms", latency_ms);
}
```

### Microphone Compensation

```rust
use sotf_audio_engine::signal_analysis::MicrophoneCompensation;

let mic_comp = MicrophoneCompensation::from_file("mic_calibration.txt")?;
let compensated = mic_comp.apply_to_sweep(
    &recording,
    20.0,      // Start frequency
    20000.0,   // End frequency
    48000,     // Sample rate
    false,     // Apply compensation (not inverse)
);
```

## Signal Generation

### Test Signals

```rust
use sotf_audio_engine::signals::*;

// Pure sine tone
let tone = gen_tone(
    1000.0,  // Frequency (Hz)
    0.5,     // Amplitude (0.0-1.0)
    48000,   // Sample rate
    1.0,     // Duration (seconds)
);

// Two-tone for IMD testing
let two_tone = gen_two_tone(
    1000.0, 0.5,   // Tone 1: freq, amplitude
    1500.0, 0.5,   // Tone 2: freq, amplitude
    48000, 1.0,
);

// Logarithmic frequency sweep
let sweep = gen_log_sweep(
    20.0,     // Start frequency
    20000.0,  // End frequency
    0.5,      // Amplitude
    48000,    // Sample rate
    5.0,      // Duration
);

// Noise signals
let white = gen_white_noise(0.3, 48000, 2.0);
let pink = gen_pink_noise(0.3, 48000, 2.0);
let m_noise = gen_m_weighted_noise(0.3, 48000, 2.0);
```

### Signal Types Enum

```rust
pub enum SignalType {
    Tone,
    TwoTone,
    Sweep,
    WhiteNoise,
    PinkNoise,
    MNoise,
}
```

## Audio Devices

### Device Enumeration

```rust
use sotf_audio_engine::devices::get_audio_devices;

let devices = get_audio_devices()?;

for (host, device_list) in &devices {
    println!("Host: {}", host);
    for device in device_list {
        println!("  {} ({})",
            device.name,
            if device.is_input { "input" } else { "output" }
        );
        if device.is_default {
            println!("    [DEFAULT]");
        }
        for config in &device.supported_configs {
            println!("    {} Hz, {} ch", config.sample_rate, config.channels);
        }
    }
}
```

### AudioDevice

```rust
pub struct AudioDevice {
    pub name: String,
    pub is_input: bool,
    pub is_default: bool,
    pub supported_configs: Vec<AudioConfig>,
    pub default_config: Option<AudioConfig>,
    pub available_sample_rates: Vec<u32>,
}

pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub buffer_size: Option<u32>,
    pub sample_format: String,  // "f32", "i16", "u16"
}
```

## Command-Line Tools

### sotf_player_cli

Full-featured command-line audio player:

```bash
# Basic playback
cargo run --bin sotf_player_cli --release -- play audio.flac

# With EQ filter (frequency:Q:gain_db)
cargo run --bin sotf_player_cli --release -- play audio.flac \
    --filter "100:0.7:3.0" \
    --filter "1000:2.0:-2.0" \
    --filter "8000:0.7:-1.0"

# With upmixer (stereo to 5.1)
cargo run --bin sotf_player_cli --release -- play audio.flac --upmixer

# With loudness compensation
cargo run --bin sotf_player_cli --release -- play audio.flac --loudness-compensation

# Combined
cargo run --bin sotf_player_cli --release -- play audio.flac \
    --filter "100:0.7:3.0" \
    --upmixer \
    --loudness-compensation
```

### sotf_recorder_cli

Audio recording and signal generation:

```bash
# Record audio
cargo run --bin sotf_recorder_cli --release -- \
    record \
    --duration 10 \
    --output recording.wav

# Generate and record sweep
cargo run --bin sotf_recorder_cli --release -- \
    record \
    --duration 5 \
    --signal-type sweep \
    --output sweep_recording.wav
```

### generate_audio_tests

Create test audio files:

```bash
cargo run --bin generate_audio_tests --release
```

### wav2csv

Analyze WAV file spectrum to CSV:

```bash
cargo run --bin wav2csv --release -- audio.wav > spectrum.csv
```

## Examples

Run the included examples:

```bash
# Basic playback demo
cargo run --release --example audio_engine_demo

# Config watching demo
cargo run --release --example config_watcher_demo

# Plugin graph demo
cargo run --release --example plugin_graph_demo

# Decoder test
cargo run --release --example audio_decoder_test
```

## Testing

```bash
# Run all tests
cargo test

# Run specific test file
cargo test --test engine_playback_tests

# Run with output
cargo test -- --nocapture
```

### Test Files

Tests are located in `tests/`:

- `engine_playback_tests.rs` - Basic playback scenarios
- `engine_decoder_tests.rs` - Decoder + resampling
- `engine_manager_tests.rs` - Manager coordination
- `engine_stress_tests.rs` - High load testing
- `engine_types_tests.rs` - Message types + state
- `engine_mute_solo_tests.rs` - Volume control
- `decoder_integration_tests.rs` - Format detection
- `e2e_audio.rs` - End-to-end playback
- `replaygain_tests.rs` - ReplayGain analysis
- `waveform_tests.rs` - Waveform generation

## Technical Notes

### Thread Safety

- **Manager Thread**: Uses `std::sync::mpsc` for command communication
- **Decoder → Processing**: Bounded `sync_mpsc` channel (capacity = queue_capacity_frames)
- **Processing → Playback**: Lock-free ring buffer via `rtrb` crate
- **Shared State**: `Arc<Mutex<AudioEngineState>>` for state queries
- **Playback Thread**: Uses `parking_lot::Mutex` for faster lock acquisition

### Sample Format

- All audio is converted to **f32 samples normalized to [-1.0, 1.0]**
- **Interleaved format**: `[L0, R0, L1, R1, ...]` for stereo
- Frame = one sample per channel (stereo frame = 2 samples)

### Resampling

- Automatic resampling from any source sample rate to `output_sample_rate`
- Uses rubato for high-quality sinc interpolation
- Applied in decoder thread before processing

### Channel Count Changes

If a plugin changes the channel count (e.g., upmixer: 2ch → 6ch):

1. Plugin notifies processing thread of new channel count
2. Processing thread notifies manager
3. Manager notifies playback thread
4. Playback thread rebuilds cpal stream with new channel count
5. Seamless transition with no audio dropout

### Unix Signal Handling

When `watch_config = true`:

| Signal | Action |
|--------|--------|
| SIGHUP | Reload configuration file |
| SIGTERM | Graceful shutdown |
| SIGINT | Graceful shutdown |

### Performance Considerations

- **Frame Size**: Smaller = lower latency, more CPU overhead
- **Buffer Size**: Larger = more resilient to system load, higher latency
- **Lock-free Playback**: Critical path uses `rtrb` ring buffer
- **Hot-reload**: Plugin chain updates are pre-built off audio thread

### Platform-specific Notes

**macOS**:
- Optional HAL driver integration for system-wide processing
- Uses CoreAudio via cpal
- Handles AudioUnit plugin loading

**Linux**:
- ALSA or PulseAudio via cpal
- Run preflight checks for audio group membership

**Windows**:
- WASAPI via cpal
- Supports exclusive mode for low latency

## Dependencies

Key dependencies:

```toml
# Audio I/O
cpal = "0.15"                    # Cross-platform audio

# Decoding
symphonia = "0.5"                # Multi-format decoder
symphonia-bundle-flac = "0.5"
symphonia-bundle-mp3 = "0.5"
symphonia-codec-aac = "0.5"
symphonia-codec-vorbis = "0.5"

# Signal Processing
rustfft = "6.2"                  # FFT
realfft = "3.4"                  # Real-only FFT
rubato = "0.16"                  # Resampling
ebur128 = "0.1"                  # Loudness measurement

# Concurrency
parking_lot = "0.12"             # Fast mutexes
rtrb = "0.3"                     # Lock-free ring buffer
signal-hook = "0.3"              # Unix signals

# File Operations
notify = "7"                     # File watching
hound = "3.5"                    # WAV I/O

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
```

## License

See the root workspace `LICENSE` file.
