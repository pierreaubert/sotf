# sotf-engine (lib: `sotf_audio`)

Core multi-threaded audio processing engine.

## Architecture

4-thread design (+ GC thread) with lock-free queues between threads:
- **Manager thread** (`engine/manager_thread.rs`): Coordinates threads, routes commands, watches config files and Unix signals (SIGHUP, SIGTERM, SIGINT)
- **Decoder thread** (`engine/decoder_thread.rs`): Reads audio files via Symphonia, decodes to PCM, resamples with rubato
- **Processing thread** (`engine/processing_thread.rs`): Runs plugin chain (EQ, upmixer, effects, analyzers) with hot-reload
- **Playback thread** (`engine/playback_thread.rs`): Feeds hardware via cpal; the backend owns callback scheduling
- **GC thread** (`engine/gc_thread.rs`): Deferred deallocation of old plugin chains off the audio path

iOS uses a real RemoteIO AudioUnit playback backend under the historical
`playback_thread_stub` name. `config_watcher_stub.rs` and `devices_stub.rs` are
true stubs: iOS has no live file/signal watching and exposes only the system
output shape. Runtime output sample-rate/channel reconfiguration is not yet
supported on iOS.

## Key Public API

- `AudioEngine` (`engine/audio_engine.rs`): Main engine interface (play, pause, seek, volume, plugin chain updates)
- `EmbeddedAudioEngine` (`engine/embedded_engine.rs`): Device-free DAW render API with an external sample clock and sample-offset automation
- `AudioEngineManager` (`manager.rs`): High-level streaming manager with file loading, plugin chain config, event system
- `AudioDecoder` (`decoder/core.rs`): Multi-format decoder (FLAC, MP3, AAC, ALAC, Vorbis, WAV, OGG, MP4/M4A, IAMF)
- `AudioStream` (`decoder/stream.rs`): Streaming state machine with seek support
- `EngineConfig` (`engine/config.rs`): Complete engine configuration (frame size, buffer, sample rate, channels, plugins)
- `PluginSettings` / `PluginChain` (`plugins/`): Plugin chain configuration and management
- `SharedAudioState` (`devices.rs`): Shared audio device state for multi-consumer access

## Module Layout

- `engine/` — Core 4-thread implementation, config, GC thread, RT priority
- `decoder/` — Symphonia-based decoding, format detection, IAMF support (behind `iamf` feature)
- `plugins/` — Plugin chain building (`chain.rs`), PluginSettings, PluginType, EQ/matrix helpers
- `plugin_param_accessors.rs` — Centralized parameter access for UI consumers
- `manager.rs` — High-level streaming API
- `devices.rs` — Audio device enumeration and management (cpal)
- `signal_recorder.rs` — Multi-channel audio capture for measurements
- `preflight.rs` — System preflight checks (audio group, permissions)
- `replaygain.rs` — ReplayGain 2.0 calculation
- `waveform.rs` — Waveform visualization data extraction

Signal analysis and test signal generation are re-exported from the `math-dsp` crate.

## Features

- `hal` — macOS CoreAudio HAL driver integration
- `asio` — ASIO audio backend on Windows
- `iamf` — IAMF decoder support (adds `sotf-iamf` + `sotf-plugin-ambisonics`)
- `streaming` — outbound HTTP PCM network streaming
- `hls` — HLS support on top of `streaming`
- `playback-runtime-harness` — internal fuzz/allocation harness; do not enable in production

## Testing

```bash
cargo test -p sotf-engine
cargo check -p sotf-engine && cargo clippy -p sotf-engine
cargo test -p sotf-engine --features playback-runtime-harness --test playback_runtime_allocation_tests
```

## Important Notes

- Debug builds on macOS must use standard `opt-level=0` — aggressive optimization causes segfaults in cpal/CoreAudio device enumeration
- Channel count can change between plugins (e.g., stereo in → 5.0 out after upmixer)
- The engine is fully native Rust with no external process dependencies
- Per-frame allocations on the audio thread cause crackling — always pre-allocate in `build()`, reuse via `Option::take()` during `process()`
- GC thread handles deferred deallocation of old plugin chains
- Output clipping (`sample.clamp(-1.0, 1.0)`) in cpal callback prevents saturation
- The engine elevates the processing worker. cpal/CoreAudio callbacks own the deadline-critical consumer work and backend scheduling; the playback feeder stays normal priority. Verify the selected backend's callback scheduling on Linux/Windows.
- Embedders may enable `EngineConfig::watch_config` for file reloads, but should leave the separate `EngineConfig::watch_signals` opt-in disabled unless they intentionally want process-global SIGINT/SIGTERM/SIGHUP handlers
