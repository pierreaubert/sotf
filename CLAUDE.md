# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

AutoEQ is a sophisticated audio equalization tool that automatically optimizes speaker and headphone frequency response. The project consists of:

1. **CLI tools** for speaker/headphone EQ optimization using measurements from spinorama.org or custom data
2. **Tauri-based desktop application** (SOTF - Sound of the Future) with real-time audio processing
3. **Native Audio Engine** (`src-audio`) with multi-threaded processing, plugin system, and Symphonia for audio decoding
4. **Optimization algorithms** including Differential Evolution, NLopt algorithms, and metaheuristic approaches

## Architecture

### Workspace Structure

This is a Cargo workspace with distinct crates:

- **`src-autoeq/`**: Core CLI for EQ optimization, contains the main `autoeq` binary and optimization logic
- **`src-audio/`**: Native audio processing engine with plugin system, decoding, and analysis tools
- **`src-tauri/`**: Tauri backend serving as bridge between Rust audio code and TypeScript frontend
- **`src-ui-frontend/`**: TypeScript/Vite frontend for the desktop application
- **`src-de/`**: Differential Evolution optimizer forked from SciPy with NLopt/MetaHeuristics interfaces
- **`src-iir/`**: IIR filter implementations (autoeq-iir) and parametric EQ utilities (Biquad struct)
- **`src-cea2034/`**: CEA2034 (Spinorama) speaker measurement metrics
- **`src-testfunctions/`**: Test functions for validating optimization algorithms
- **`src-env/`**: Shared environment utilities and constants

### Audio Architecture (`src-audio`)

The audio subsystem uses a **native multi-threaded audio engine** with a flexible plugin system:

#### AudioEngine (`src-audio/src/engine/`)

Multi-threaded audio processing engine with 4 threads:
- **Thread 1**: Decoder - Reads audio files, decodes to PCM, resamples
- **Thread 2**: Processing - Runs plugin chain (EQ, upmixer, effects, analyzers)
- **Thread 3**: Playback - Outputs to audio hardware via cpal
- **Thread 4**: Manager - Coordinates threads, handles commands, watches config files

Key components:
- `audio_engine.rs`: Main AudioEngine API (play, pause, seek, volume control)
- `manager_thread.rs`: Thread coordination, command routing, config watching
- `decoder_thread.rs`: Audio file decoding via Symphonia
- `processing_thread.rs`: Plugin chain processing with hot-reload support
- `playback_thread.rs`: Hardware audio output via cpal
- `config.rs`: EngineConfig with plugin chain, sample rate, channels
- `config_watcher.rs`: File watching and Unix signal handling (SIGHUP, SIGTERM, SIGINT)
- `types.rs`: PluginConfig, PlaybackState, AudioEngineState

#### Plugin System (`src-audio/src/plugins/`)

Flexible plugin architecture supporting:

**Processing Plugins** (transform audio):
- `plugin_eq.rs`: Parametric EQ with biquad filters (uses autoeq-iir::Biquad)
- `plugin_gain.rs`: Simple volume control
- `plugin_compressor.rs`: Dynamic range compression
- `plugin_gate.rs`: Noise gate
- `plugin_limiter.rs`: Peak limiter
- `plugin_upmixer.rs`: Stereo → 5.0 surround upmixing via FFT-based spatial processing
- `plugin_resampler.rs`: Sample rate conversion
- `plugin_loudness_compensation.rs`: Equal-loudness contour compensation
- `plugin_matrix.rs`: Channel matrix mixing

**Analyzer Plugins** (extract data, don't modify audio):
- `analyzer_spectrum.rs`: FFT-based spectrum analysis
- `analyzer_loudness_monitor.rs`: EBU R128 loudness measurement

**Plugin Architecture**:
- `plugin.rs`: Core `Plugin` trait with `process()`, `initialize()`, `reset()`
- `host.rs`: `PluginHost` chains plugins together, handles channel count changes
- `parameters.rs`: Parameter system for plugin control (gain, frequency, etc.)
- `mod.rs`: Plugin factory - creates plugins from `PluginConfig` JSON

Plugins are instantiated from JSON configuration:
```rust
PluginConfig {
    plugin_type: "EQ",
    parameters: json!({
        "filters": [
            {"filter_type": "peak", "frequency": 1000.0, "q": 1.5, "gain_db": 3.0}
        ]
    })
}
```

#### Audio Decoding (`src-audio/src/decoder/`)

Symphonia-based multi-format decoder supporting:
- **Formats**: FLAC, MP3, AAC, Vorbis, WAV, OGG
- **Architecture**:
  - `decoder.rs`: Core decoder with `AudioDecoder` trait
  - `stream.rs`: Streaming state machine with seek support
  - `format_detection.rs`: Automatic format detection

#### AudioStreamingManager (`src-audio/src/manager.rs`)

High-level API for audio playback:
- File loading with format detection
- Plugin chain configuration
- Playback control (play, pause, resume, seek)
- Volume and mute control
- Real-time analyzer support (loudness, spectrum)
- Event system for end-of-stream, errors

**Usage**:
```rust
let mut manager = AudioStreamingManager::new();
manager.load_file("audio.flac").await?;

// Build plugin chain
let plugins = vec![
    create_upmixer_plugin(),
    create_eq_plugin(&filters),
];

manager.start_playback(None, plugins, 5).await?;
```

#### Analysis Tools

- `signal_analysis.rs`: FFT-based frequency/phase analysis, impulse response
- `signal_recorder.rs`: Multi-channel audio capture for measurements
- `signals.rs`: Test signal generation (sine, sweep, pink noise, white noise)

#### Legacy: CamillaDSP Integration (`src-audio/src.camilla/`)

**Note**: CamillaDSP integration is being phased out in favor of the native AudioEngine. It remains only for:
- Recording functionality (signal_recorder.rs still uses it)
- Backward compatibility

The CamillaDSP code is in a separate directory (`src.camilla/`) and includes:
- Subprocess management
- YAML config generation
- WebSocket control (port 1234)
- stdin audio streaming

**Do not modify files in `src.camilla/` unless specifically working on legacy recording code.**

### Tauri Backend (`src-tauri`)

The Tauri layer exposes Rust audio functionality to TypeScript frontend via commands:

- `tauri_audio_streaming.rs`: File playback, seek, pause/resume using AudioStreamingManager
- `tauri_audio_recording.rs`: Multi-channel recording via legacy AudioManager
- `tauri_audio_spectrum.rs`: Real-time spectrum monitoring
- `tauri_audio_loudness.rs`: Loudness measurement
- `tauri_compute_eq.rs`: EQ response computation
- `tauri_generate_eq.rs`: Export EQ to various formats (APO, RME, AUPreset)
- `tauri_optim.rs`: Run AutoEQ optimization with cancellation support
- `tauri_speakers.rs`: Query spinorama.org speaker database

State management uses `tokio::sync::Mutex` wrapping `AudioManager` and `AudioStreamingManager`.

**Important**: Tauri commands should use `PluginConfig` to configure audio processing, not raw parameters.

### Optimization (`src-autoeq`)

The core optimization workflow:

1. **Data Input** (`read/`): Load measurements from spinorama.org API or CSV files
2. **Signal Processing** (`signal.rs`): Smoothing, interpolation, frequency domain operations
3. **Loss Functions** (`loss.rs`):
   - `speaker-flat`: Minimize deviation from flat response
   - `speaker-score`: Optimize Harman/Olive score (bass boost + PIR flatness)
   - `headphone-score`: Target Harman headphone curve
4. **Optimization** (`optim.rs`, `optim_de.rs`, `optim_nlopt.rs`, `optim_mh.rs`):
   - Global optimizers: DE, ISRES, AGS, ORIGDIRECT, PSO
   - Local optimizers: COBYLA, Nelder-Mead
   - Supports constraints (frequency spacing, Q limits, dB bounds)
5. **Output** (`x2peq.rs`): Convert solution to parametric EQ filters (uses autoeq-iir::Biquad)

**Optimization Parameters** (see `src-autoeq/src/cli.rs`):
- `-n`: Number of PEQ filters
- `--min-q`, `--max-q`: Q factor bounds (sharpness)
- `--min-db`, `--max-db`: Gain bounds
- `--min-freq`, `--max-freq`: Frequency range
- `--algo`: Optimizer selection (e.g., `autoeq:de`, `cobyla`)
- `--strategy`: DE mutation strategy (e.g., `currenttobest1bin`)

## Development Commands

All commands use `just` (justfile runner). Install with `cargo install just`.

### Building

```bash
# Build everything (release mode)
just build
# or
just prod

# Build workspace only
just prod-workspace

# Build specific binaries
just prod-autoeq
just prod-sotf-audio

# Development build (debug mode)
just dev
```

### Testing

```bash
# Run all tests (Rust + TypeScript)
just test

# Rust tests only
just test-rust

# TypeScript tests only
just test-ts

# Generate audio test files
just test-generate
```

### Formatting

```bash
# Format everything
just fmt

# Rust only
just fmt-rust

# TypeScript only
just fmt-ts
```

### Quality Assurance

```bash
# Run QA tests on specific speakers
just qa

# Individual QA tests
just qa-ascilab-6b
just qa-jbl-m2-flat
just qa-beyerdynamic-dt1990pro
```

### Cross-Compilation

```bash
# See all cross targets
just cross

# Linux x86_64 from macOS ARM
just cross-macos-arm-2-linux-x86

# Linux ARM64 from macOS ARM
just cross-macos-arm-2-linux-arm64

# Windows MSVC from macOS ARM
just cross-macos-arm-2-win-x86-msvc
```

Or use the automated script:
```bash
./scripts/build-cross.sh
```

This creates `dist/` with binaries for all platforms.

### Static Binary Builds

Build standalone static binaries of `sotf_player_tui` for distribution:

```bash
# Build all static binaries (Linux, Windows, macOS)
just cross-static-all

# Build individual platforms
just cross-static-linux-x86       # Linux x86_64 (musl)
just cross-static-linux-arm64     # Linux ARM64 (musl)
just cross-static-windows-x86     # Windows x86_64 (static CRT)
just cross-static-macos           # macOS universal binary

# Build for current platform
just build-static-local
```

**Platform Notes:**
- **Linux**: Truly static binaries using musl libc - no runtime dependencies
- **Windows**: Static CRT linkage with minimal dependencies
- **macOS**: Universal binary (Intel + Apple Silicon) - limited static linking due to Apple restrictions

See [docs/STATIC_BUILDS.md](docs/STATIC_BUILDS.md) for detailed documentation.

### npm Commands (Frontend)

```bash
# Development server
npm run dev

# Build TypeScript + Vite
npm run build

# Tauri dev mode (hot reload)
npm run tauri dev

# Tauri production build
npm run tauri build

# Tests
npm run test              # Run all tests
npm run test:unit         # Unit tests only
npm run test:e2e          # E2E tests only

# Linting/Formatting
npm run lint
npm run fmt
```

### Running Binaries

```bash
# AutoEQ CLI
cargo run --bin autoeq --release -- --speaker="KEF R3" --version=asr --measurement=CEA2034 --algo=cobyla

# Download spinorama.org database
cargo run --bin download --release

# Audio playback/recording tool with native engine
cargo run --bin sotf_audio --release -- play audio.flac
cargo run --bin sotf_audio --release -- play audio.flac --filter 1000:1.5:3.0 --upmixer
cargo run --bin sotf_audio --release -- play audio.flac --loudness-compensation -18,6,6 --lufs

# Benchmarking
cargo run --bin benchmark_autoeq_speaker --release -- --qa --jobs 1
cargo run --bin benchmark_convergence --release
```

### Audio Engine Examples

```bash
# Run audio engine examples
cargo run --release --example audio_engine_demo
cargo run --release --example config_watcher_demo
```

## Important Technical Notes

### BLAS Libraries

Platform-specific BLAS backends (configured in Cargo.toml):
- **macOS**: Accelerate framework
- **Linux**: OpenBLAS
- **Windows x64**: Intel MKL
- **Windows ARM**: OpenBLAS

### Audio Backend

The project uses a **native multi-threaded audio engine** built with:
- **cpal**: Cross-platform audio I/O
- **Symphonia**: Audio decoding (FLAC, MP3, AAC, Vorbis, WAV)
- **rustfft**: FFT processing for spectrum analysis and upmixer
- **Custom plugin system**: Modular audio processing chain

**CamillaDSP is legacy** and only used for recording. The native engine handles all playback.

### Plugin System Architecture

Plugins are configured via JSON and loaded dynamically:

1. **CLI/Tauri** creates `Vec<PluginConfig>` with JSON parameters
2. **AudioStreamingManager** passes plugins to EngineConfig
3. **ManagerThread** sends plugins to ProcessingThread
4. **ProcessingThread** builds PluginHost from configs
5. **PluginHost** chains plugins and processes audio

Channel count can change between plugins (e.g., upmixer: 2ch → 5ch).

### Environment Variables

- `AUTOEQ_DIR`: Project root for test infrastructure (CSV traces, generated data)

### Git Workflow

- Main branch: `master`
- Workspace version: 0.2.466 (managed in root Cargo.toml)
- Pre-commit hooks configured (`.pre-commit-config.yaml`)

## API Integration

The CLI can fetch data from spinorama.org:

```bash
# List speakers
curl http://api.spinorama.org/v1/speakers

# List versions for a speaker
curl http://api.spinorama.org/v1/speakers/{speaker}/versions

# List measurements
curl http://api.spinorama.org/v1/speakers/{speaker}/versions/{version}/measurements
```

## Examples and Demos

```bash
# Run all examples
just examples

# AutoEQ examples
just examples-autoeq

# DE optimizer examples
just examples-de

# IIR filter examples
just examples-iir

# Audio engine examples
just examples-audio
```

## Key Data Structures

### autoeq-iir::Biquad

Core filter representation used throughout the codebase:
```rust
pub struct Biquad {
    pub filter_type: BiquadFilterType,
    pub frequency: f64,
    pub q: f64,
    pub gain_db: f64,
    // ... biquad coefficients
}
```

Filter types: Peak, Lowshelf, Highshelf, Lowpass, Highpass, Bandpass, Notch, etc.

### PluginConfig

Plugin configuration for AudioEngine:
```rust
pub struct PluginConfig {
    pub plugin_type: String,      // "EQ", "upmixer", "gain", etc.
    pub parameters: serde_json::Value,  // Plugin-specific JSON config
}
```

### EngineConfig

Complete engine configuration:
```rust
pub struct EngineConfig {
    pub frame_size: usize,         // Processing block size
    pub buffer_ms: u32,            // Queue buffer size
    pub output_sample_rate: u32,   // Hardware sample rate
    pub input_channels: usize,     // Source channel count
    pub output_channels: usize,    // Final output channels (after plugins)
    pub plugins: Vec<PluginConfig>,
    pub volume: f32,
    pub muted: bool,
    pub config_path: Option<PathBuf>,  // For config file watching
    pub watch_config: bool,        // Enable signal handlers
}
```

## Recent Major Changes

- **Native AudioEngine**: Replaced CamillaDSP for playback (2024)
- **Plugin System**: Modular audio processing with hot-reload support
- **File Reorganization**: Plugins renamed with prefixes (`plugin_*`, `analyzer_*`)
- **Signal Analysis**: `analysis.rs` → `signal_analysis.rs`
- **Upmixer Fix**: Added `input_channels` to EngineConfig to fix plugin chain initialization
