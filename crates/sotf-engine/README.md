# sotf-engine (lib: `sotf_audio`)

A native multi-threaded audio processing engine written in pure Rust. Designed for high-performance audio playback with a flexible plugin system, hot-reload capabilities, and comprehensive format support.

## Features

- **Multi-threaded Architecture**: 4 concurrent threads with lock-free queues for low-latency playback
- **Plugin System**: Modular audio processing with hot-reload support (EQ, compressor, upmixer, analyzers, etc.)
- **Multi-format Decoding**: FLAC, MP3, AAC, WAV, AIFF, ALAC, Vorbis via Symphonia; IAMF via optional feature
- **High-quality Resampling**: Automatic sample rate conversion using rubato
- **Config Watching**: Live config file updates and Unix signal handling (SIGHUP, SIGTERM)
- **Cross-platform**: macOS, Linux, Windows, and iOS (stub) support via cpal
- **Signal Analysis**: FFT-based spectrum analysis, ReplayGain 2.0, waveform extraction (via `math-dsp`)
- **Signal Generation**: Sine, sweep, pink/white noise for testing and calibration (via `math-dsp`)
- **Recording**: Multi-channel audio capture with measurement analysis

## Architecture

The engine uses a 4-thread architecture with dedicated queues for each stage:

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Manager Thread                              │
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

A 5th thread (GC thread) handles deferred deallocation to keep the audio threads allocation-free.

### Thread Responsibilities

1. **Manager Thread** (`engine/manager_thread.rs`): Receives commands from the public API, coordinates other threads, watches config files and handles Unix signals (SIGHUP for reload, SIGTERM/SIGINT for shutdown).

2. **Decoder Thread** (`engine/decoder_thread.rs`): Reads audio files using Symphonia, decodes to PCM f32, resamples to target sample rate, and sends audio frames to the processing thread.

3. **Processing Thread** (`engine/processing_thread.rs`): Runs the plugin chain (EQ, effects, analyzers), handles hot-reload of plugin configurations, and can change channel counts (e.g., 2ch → 5.1ch upmixing).

4. **Playback Thread** (`engine/playback_thread.rs`): Outputs audio to hardware via cpal using a lock-free ring buffer. Uses real-time thread priority to prevent underruns.

5. **GC Thread** (`engine/gc_thread.rs`): Deferred deallocation thread that frees old plugin chains and buffers off the audio path.

## Module Layout

```
src/
├── lib.rs                    # Public API exports
├── engine/
│   ├── mod.rs                # Engine module root
│   ├── audio_engine.rs       # AudioEngine — main public API
│   ├── manager_thread.rs     # Thread coordination, command routing
│   ├── decoder_thread.rs     # Audio file decoding via Symphonia
│   ├── processing_thread.rs  # Plugin chain processing with hot-reload
│   ├── playback_thread.rs    # Hardware audio output via cpal
│   ├── playback_thread_stub.rs # iOS stub (no cpal)
│   ├── config.rs             # EngineConfig (frame size, buffer, sample rate, channels, plugins)
│   ├── config_watcher.rs     # File watching + Unix signal handling
│   ├── config_watcher_stub.rs # iOS stub
│   ├── gc_thread.rs          # Deferred deallocation thread
│   ├── rt_priority.rs        # Real-time thread priority helpers
│   └── types.rs              # PluginConfig, PlaybackState, AudioEngineState, messages
├── decoder/
│   ├── mod.rs                # Decoder module root
│   ├── core.rs               # AudioSpec, AudioDecoder trait
│   ├── formats.rs            # Symphonia-based multi-format decoding
│   ├── stream.rs             # Streaming state machine with seek support
│   ├── error.rs              # AudioDecoderError types
│   └── iamf.rs               # IAMF decoder (behind `iamf` feature)
├── manager.rs                # AudioEngineManager — high-level streaming API
├── plugins/
│   ├── mod.rs                # PluginSettings, PluginType, PluginChain, EQFilter
│   ├── chain.rs              # Plugin chain building and management
│   ├── eq.rs                 # EQ-specific plugin configuration helpers
│   ├── matrix.rs             # Matrix plugin configuration helpers
│   └── utility.rs            # Shared plugin utilities
├── plugin_param_accessors.rs # Centralized parameter access for UI
├── devices.rs                # Audio device enumeration and management (cpal)
├── devices_stub.rs           # iOS stub for devices
├── preflight.rs              # System preflight checks (audio group, permissions)
├── signal_recorder.rs        # Multi-channel audio capture for measurements
├── replaygain.rs             # ReplayGain 2.0 calculation
└── waveform.rs               # Waveform visualization data extraction
```

Signal analysis and test signal generation are re-exported from the `math-dsp` crate.

## Key Public API

- `AudioEngine` (`engine/audio_engine.rs`): Main engine interface — play, pause, seek, volume, plugin chain updates
- `AudioEngineManager` (`manager.rs`): High-level streaming manager with file loading, plugin chain config, event system
- `AudioDecoder` (`decoder/core.rs`): Multi-format decoder (FLAC, MP3, AAC, ALAC, Vorbis, WAV, OGG, MP4/M4A)
- `AudioStream` (`decoder/stream.rs`): Streaming state machine with seek support
- `EngineConfig` (`engine/config.rs`): Complete engine configuration
- `PluginSettings` / `PluginChain` (`plugins/`): Plugin chain configuration and management
- `SharedAudioState` (`devices.rs`): Shared audio device state for multi-consumer access

## Features

| Feature | Description | Default |
|---------|-------------|---------|
| `hal` | macOS CoreAudio HAL driver integration | No |
| `asio` | ASIO audio backend on Windows | No |
| `iamf` | IAMF (Immersive Audio Model and Formats) decoder support | No |

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

## Decoder Support

| Format | Extensions | Lossless | Notes |
|--------|------------|----------|-------|
| FLAC | `.flac` | Yes | Full metadata support |
| WAV | `.wav` | Yes | PCM 16/24/32-bit |
| AIFF | `.aiff`, `.aif` | Yes | Apple lossless |
| MP3 | `.mp3` | No | MPEG Layer III |
| AAC | `.m4a`, `.mp4`, `.aac` | No | MPEG-4 Audio |
| Vorbis | `.ogg`, `.oga` | No | OGG container |
| ALAC | `.m4a` | Yes | Apple Lossless in MP4 |
| IAMF | `.iamf` | Varies | Immersive Audio (optional feature) |

## Testing

```bash
# Run all tests
cargo test -p sotf-engine

# Run specific test file
cargo test -p sotf-engine --test engine_playback_tests

# Check + clippy
cargo check -p sotf-engine && cargo clippy -p sotf-engine
```

### Test Files

Tests are in `tests/`:

- `engine_playback_tests.rs` — Basic playback scenarios
- `engine_decoder_tests.rs` — Decoder + resampling
- `engine_manager_tests.rs` — Manager coordination
- `engine_stress_tests.rs` — High load testing
- `engine_types_tests.rs` — Message types + state
- `engine_mute_solo_tests.rs` — Volume control
- `engine_allocation_tests.rs` — Allocation-free audio path verification
- `engine_latency_tests.rs` — Latency measurement
- `fft_plugin_chain_tests.rs` — FFT-based plugin chain tests
- `decoder_integration_tests.rs` — Format detection
- `e2e_audio.rs` — End-to-end playback
- `replaygain_tests.rs` — ReplayGain analysis
- `waveform_tests.rs` — Waveform generation
- `test_*_loopback.rs` — Per-plugin loopback integration tests (EQ, gain, compressor, limiter, gate, delay, matrix, crossover, upmixer, channel mute/solo)
- `windows_*.rs` — Windows-specific tests

## Technical Notes

### iOS Support

On iOS, `cpal`, `notify`, `ctrlc`, and `signal-hook` are not available. The engine provides stub implementations (`devices_stub.rs`, `playback_thread_stub.rs`, `config_watcher_stub.rs`) that compile on iOS but disable hardware audio output and file watching.

### Thread Safety

- **Manager Thread**: Uses `std::sync::mpsc` for command communication
- **Decoder → Processing**: Bounded `sync_mpsc` channel
- **Processing → Playback**: Lock-free ring buffer via `rtrb`
- **Shared State**: `Arc<Mutex<AudioEngineState>>` for state queries
- **Playback Thread**: Uses `parking_lot::Mutex` for faster lock acquisition

### Sample Format

- All audio is converted to **f32 samples normalized to [-1.0, 1.0]**
- **Interleaved format**: `[L0, R0, L1, R1, ...]` for stereo
- Frame = one sample per channel (stereo frame = 2 samples)

### Performance

- Pre-allocate buffers in `build()`, reuse via `Option::take()` during `process()` — zero allocation on audio thread
- GC thread handles deferred deallocation of old plugin chains
- Output clipping (`sample.clamp(-1.0, 1.0)`) in cpal callback prevents saturation
- Real-time thread priority for playback thread via `rt_priority.rs`

## License

See the root workspace `LICENSE` file.
