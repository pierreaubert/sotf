# CLAUDE.md


This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Interaction Rules
- **Verify Compilation**: Always ensure the code compiles (`cargo check` or `cargo build`) before submitting an answer or marking a task as complete.
- **Engine & Plugins crates**: When modifying `crates/engine/` or `crates/plugins/`, always create a dedicated PR (not part of a larger PR) and run a code review before merging.


## Project Overview

This is a Rust-heavy audio DSP project. Primary language is Rust with Python scripts for visualization/plotting. Always run `cargo test` after making changes and ensure all tests pass before considering work complete.

SOTF (Sound of the Future) is a comprehensive audio optimization and playback system. The project consists of:

1. **AutoEQ CLI tools** for speaker/headphone EQ optimization using measurements from spinorama.org or custom data
3. **Native Audio Engine** (`engine`) with multi-threaded processing, plugin system, and Symphonia for audio decoding
4. **Audio Players**: TUI player (`player-tui`), CLI tools, and experimental GPUI player
5. **Optimization algorithms** including Differential Evolution, NLopt algorithms, and metaheuristic approaches
6. **macOS-specific features**: CoreAudio HAL driver and menubar configuration app

## Architecture

### Workspace Structure

This is a Cargo workspace with distinct crates organized by functionality:

**Audio Engine & Players:**
- **`engine/`**: Core audio processing engine with multi-threaded architecture, plugin system, and Symphonia decoding
- **`player/`**: High-level audio playback API and utilities
- **`player-tui/`**: Terminal UI music player with library scanning (production quality)
- **`player-gpui/`**: Experimental GPUI-based player (not in default build)
- **`player-midi/`**: MIDI integration support

**Audio Plugins:**
- **`plugins/`**: Plugin implementations (EQ, compressor, upmixer, analyzers, etc.)
- **`plugins-ffi/`**: FFI interface for Audio Unit integration
- **`plugins-au/`**: macOS Audio Unit plugin implementation

**AutoEQ & Optimization:**
- **`autoeq/`**: Core CLI for EQ optimization with multiple binaries (autoeq, roomeq, benchmarks)
- **`autoeq-cea2034/`**: CEA2034 (Spinorama) speaker measurement metrics
- **`autoeq-env/`**: Shared environment utilities and constants

**Mathematical Libraries:**
- **`math-de/`**: Differential Evolution optimizer forked from SciPy with NLopt/MetaHeuristics interfaces
- **`math-iir/`**: IIR filter implementations (autoeq-iir) and parametric EQ utilities (Biquad struct)
- **`math-testfunctions/`**: Test functions for validating optimization algorithms
- **`math-convexhull3d/`**: 3D convex hull computation
- **`math-bem/`**: Boundary Element Method solver (experimental)

**macOS-specific:**
- **`sotf-macos-hal/`**: CoreAudio HAL driver for system-wide audio processing
- **`sotf-macos-configbar/`**: Menubar configuration app for HAL driver

**Other:**
- **`sotf-head-scanner/`**: Experimental head scanning app for HRTF generation (not in default build)

### Audio Architecture (`engine`)

The audio subsystem uses a **native multi-threaded audio engine** with a flexible plugin system:

#### AudioEngine (`engine/src/engine/`)

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

#### Plugin System (`plugins/src/`)

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

#### Audio Decoding (`engine/src/decoder/`)

Symphonia-based multi-format decoder supporting:
- **Formats**: FLAC, MP3, AAC, ALAC, Vorbis, WAV, OGG, MP4/M4A
- **Architecture**:
  - `decoder.rs`: Core decoder with `AudioDecoder` trait
  - `stream.rs`: Streaming state machine with seek support
  - `format_detection.rs`: Automatic format detection

#### AudioStreamingManager (`engine/src/manager.rs`)

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

#### Analysis and Recording Tools

- `signal_analysis.rs`: FFT-based frequency/phase analysis, impulse response
- `signal_recorder.rs`: Multi-channel audio capture for measurements
- `signals.rs`: Test signal generation (sine, sweep, pink noise, white noise)
- `replaygain.rs`: ReplayGain calculation for volume normalization
- `devices.rs`: Audio device enumeration and management

### Optimization (`autoeq`)

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

**Optimization Parameters** (see `autoeq/src/cli.rs`):
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
just prod-sotf

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

**Platform Details:**

- **Linux (musl)**: Truly static binaries with ZERO runtime dependencies
  - Uses musl libc instead of glibc
  - OpenBLAS statically linked
  - Portable across all Linux distributions
  - Verify with: `ldd target/x86_64-unknown-linux-musl/release/sotf_player_tui` (should show "not a dynamic executable")

- **Windows (MSVC)**: Static CRT linkage with minimal dependencies
  - Static C runtime library
  - Minimal system DLL dependencies
  - Compatible with Windows 10 and later

- **macOS**: Universal binary (Intel + Apple Silicon)
  - **NOT fully static** - Apple requires dynamic linking to system frameworks
  - CoreAudio, CoreFoundation, and other frameworks dynamically linked
  - Supports both x86_64 and ARM64 in a single binary
  - Requires macOS 15.0+ (deployment target)

**Verification Steps:**

```bash
# Linux - check for dynamic dependencies
ldd target/x86_64-unknown-linux-musl/release/sotf_player_tui

# Check binary size
ls -lh target/*/release/sotf_player_tui*

# Test the binary
./target/x86_64-unknown-linux-musl/release/sotf_player_tui --help
```

**Technical Notes:**
- BLAS support via static OpenBLAS (compiled from source by openblas-src crate)
- Audio dependencies (cpal, symphonia) compiled directly into binary
- Larger binary size (~15-50MB) compared to dynamic linking
- Build time longer due to static compilation of dependencies

### Running Binaries

```bash
# AutoEQ CLI - main optimization tool
cargo run --bin autoeq --release -- --speaker="KEF R3" --version=asr --measurement=CEA2034 --algo=cobyla

# Room EQ optimization
cargo run --bin roomeq --release

# Download spinorama.org database
cargo run --bin autoeq_download_speakers --release

# Audio playback CLI (native engine)
cargo run --bin sotf_player_cli --release -- play audio.flac
cargo run --bin sotf_player_cli --release -- play audio.flac --filter 1000:1.5:3.0 --upmixer

# Audio recording CLI
cargo run --bin sotf_recorder_cli --release

# TUI music player (production quality)
cargo run --bin sotf_player_tui --release

# Benchmarking
cargo run --bin autoeq_benchmark_speaker --release -- --qa --jobs 1
cargo run --bin benchmark_convergence --release

# Test signal generation
cargo run --bin generate_audio_tests --release
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
- **Symphonia**: Audio decoding (FLAC, MP3, AAC, ALAC, Vorbis, WAV, OGG, MP4/M4A)
- **rustfft/realfft**: FFT processing for spectrum analysis and upmixer
- **rubato**: High-quality sample rate conversion
- **ebur128**: EBU R128 loudness measurement
- **Custom plugin system**: Modular audio processing chain with hot-reload support

The engine is fully native Rust with no external dependencies like CamillaDSP.

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
- Workspace version: 0.5.3 (managed in root Cargo.toml)
- Pre-commit hooks configured (`.pre-commit-config.yaml`)
- Individual crate versions may differ (e.g., autoeq is at 0.2.250)

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

## Key Applications

### TUI Music Player (`player-tui`)

Production-quality terminal music player with:
- **Library scanning**: Scans and indexes audio files with metadata
- **SQLite database**: Stores album/track information for fast browsing
- **Ratatui UI**: Full-featured terminal interface with navigation
- **Plugin support**: Full access to audio engine plugins (EQ, upmixer, etc.)
- **ReplayGain**: Automatic volume normalization
- **Status**: Production quality, suitable for daily use

Run with: `cargo run --bin sotf_player_tui --release`

### Desktop Application (Tauri)

Cross-platform desktop app with:
- **TypeScript frontend**: Bulma CSS framework with Plotly visualizations
- **Real-time visualization**: Spectrum analyzer, loudness meters
- **EQ optimization UI**: Visual interface for AutoEQ workflows
- **File management**: Audio file browser and playback controls

Run with: `npm run tauri dev` (development) or `npm run tauri build` (production)

## Common Development Workflows

### Testing Changes

```bash
# Check compilation without building
cargo check --all-targets

# Run Rust tests
just test-rust
# or
cargo test --lib

# Run TypeScript tests
just test-ts
# or
npm run test

# Run all tests
just test
```

### Working on Audio Engine

```bash
# Build and run TUI player for testing
cargo run --bin sotf_player_tui --release

# Run with specific plugins
cargo run --bin sotf_player_cli --release -- play audio.flac --upmixer --filter 1000:1.5:3.0

# Test signal generation
cargo run --bin generate_audio_tests --release

# Run examples
cargo run --release --example audio_engine_demo
```

### Working on AutoEQ Optimization

```bash
# Run optimization on a speaker
cargo run --bin autoeq --release -- \
  --speaker="KEF R3" \
  --version=asr \
  --measurement=CEA2034 \
  --algo=autoeq:de \
  -n 7

# Run on custom headphone measurements
cargo run --bin autoeq --release -- \
  --curve ./path/to/measurement.csv \
  --target ./data_tests/targets/harman-over-ear-2018.csv \
  --loss headphone-score \
  -n 5

# Benchmark convergence
cargo run --bin benchmark_convergence --release
```

### Working on Plugins

Plugin implementations are in `plugins/src/`:
- Each plugin implements the `Plugin` trait from `plugin.rs`
- Plugins are registered in the factory in `mod.rs`
- Test plugins with the TUI player or CLI player
- Use `plugin_fuzzer` binary for stress testing

```bash
# Fuzz test plugins
cargo run --bin plugin_fuzzer --release
```

### macOS-specific Development

```bash
# Build HAL driver
just prod-hal

# Build menubar config app
just prod-configbar

# Build both
just prod-macos

# Build Audio Units
just build-au-rust    # Build Rust FFI
just build-au-swift   # Build Swift AU wrapper
just install-au       # Install to ~/Library/Audio/Plug-Ins/
just validate-au      # Run auval validation
```

## Important Development Notes

### Build Configuration

- **Debug builds on macOS**: Use standard `opt-level=0` settings. Aggressive optimization in debug mode causes segfaults in cpal/CoreAudio device enumeration.
- **Release builds**: Use LTO and single codegen-unit for maximum performance
- **Panic strategy**: Set to "unwind" to allow tests to run properly

### Dependencies

- **BLAS**: Platform-specific (Accelerate on macOS, OpenBLAS on Linux, Intel MKL on Windows x64)
- **Audio**: cpal, Symphonia (multi-format), rubato (resampling), ebur128 (loudness)
- **FFT**: rustfft (complex) and realfft (real-only, faster)
- **Async**: Uses both tokio and native threads (not fully tokio-native)

### Plugin Architecture Flow

1. Create `PluginConfig` with JSON parameters in CLI/Tauri
2. Pass to `AudioStreamingManager` → `EngineConfig`
3. `ManagerThread` forwards to `ProcessingThread`
4. `ProcessingThread` builds `PluginHost` chain
5. Audio flows through plugin chain with dynamic channel counts

### File Naming Conventions

- **Binaries**: Use underscores (e.g., `sotf_player_cli`, `autoeq_download_speakers`)
- **Crates**: Use hyphens (e.g., `engine`, `math-iir`)
- **Module files**: Use underscores (e.g., `signal_analysis.rs`, `plugin_eq.rs`)

### Testing Strategy

- **Rust tests**: `cargo test --lib` (library tests only)
- **TypeScript tests**: `npm run test` or `vitest`
- **QA tests**: `just qa` runs optimization benchmarks
- **Binary tests**: Run specific binaries with test data

### Known Platform Differences

- **macOS**: Cannot create fully static binaries (system frameworks required)
- **Linux**: Can create true static binaries via musl targets
- **Windows**: Static CRT with minimal DLL dependencies

## Recent Major Changes

- **Native AudioEngine**: Replaced CamillaDSP for playback (2024)
- **Plugin System**: Modular audio processing with hot-reload support
- **Crate Reorganization**: Renamed from `src-*` to `sotf-*` and `math-*` prefixes
- **TUI Player**: Production-quality terminal music player with library management
- **Static Binary Support**: Cross-compilation for musl/static binaries
- **Workspace version bump**: Now at 0.5.3 (individual crates may vary)
- Read @GPUI.md before working on GPUI code.
- when adding features to the players the business logic goes into the common library player/src
- never use unsafe without asking

## Debugging Guidelines

When fixing audio bugs (crackling, saturation, speed issues), always investigate the full signal chain — don't stop at the first symptom fix. Check: sample rate mismatches, frame allocation in hot paths, plugin propagation, and normalization. A surface-level fix often masks a deeper root cause.

## Python / Visualization

Python visualization scripts use a virtual environment. Always check for and activate the local venv before running Python scripts. Use `pyright` from the local venv for type checking.

## Known Gotchas

When working with clap CLI argument structs, be aware that flattened plugin structs can cause ID conflicts if they share field names (e.g., 'enabled'). Always check for duplicate field names across flattened structs.

## Domain Knowledge — Room EQ

For roomeq/EQ work: filters must be placed within measurement data frequency bounds. Passband detection uses relative-to-peak thresholds, not absolute dB values. Always verify optimizer frequency ranges against actual data bounds.

## Workflow Rules

Before implementing a fix, read any relevant format docs or specifications the user points to. Do not start editing code until you understand the data format or protocol involved.
