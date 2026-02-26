# engine (lib: `sotf_audio`)

Core multi-threaded audio processing engine.

## Architecture

4-thread design with lock-free queues between threads:
- **Manager thread** (`engine/manager_thread.rs`): Coordinates threads, routes commands, watches config files and Unix signals (SIGHUP, SIGTERM, SIGINT)
- **Decoder thread** (`engine/decoder_thread.rs`): Reads audio files via Symphonia, decodes to PCM, resamples with rubato
- **Processing thread** (`engine/processing_thread.rs`): Runs plugin chain (EQ, upmixer, effects, analyzers) with hot-reload
- **Playback thread** (`engine/playback_thread.rs`): Outputs audio to hardware via cpal

## Key Public API

- `AudioEngine` (`engine/audio_engine.rs`): Main engine interface (play, pause, seek, volume)
- `AudioEngineManager` (`manager.rs`): High-level streaming manager with file loading, plugin chain config, event system
- `AudioDecoder` (`decoder/decoder.rs`): Multi-format decoder (FLAC, MP3, AAC, ALAC, Vorbis, WAV, OGG, MP4/M4A)
- `AudioStream` (`decoder/stream.rs`): Streaming state machine with seek support
- `EngineConfig` (`engine/config.rs`): Complete engine configuration (frame size, buffer, sample rate, channels, plugins)

## Module Layout

- `engine/` - Core 4-thread implementation and config
- `decoder/` - Symphonia-based decoding and format detection
- `devices.rs` - Audio device enumeration and management (cpal)
- `manager.rs` - High-level streaming API
- `plugins.rs` - Plugin integration layer
- `signal_recorder.rs` - Multi-channel audio capture
- `signal_analysis.rs` - FFT-based frequency/phase analysis
- `signals.rs` - Test signal generation (sine, sweep, pink/white noise)
- `replaygain.rs` - ReplayGain 2.0 calculation

## Features

- `hal` - macOS CoreAudio HAL driver integration

## Testing

```bash
cargo test -p engine --lib
cargo check -p engine && cargo clippy -p engine
```

## Examples

```bash
cargo run --release --example audio_engine_demo
cargo run --release --example config_watcher_demo
```

## Important Notes

- Debug builds on macOS must use standard `opt-level=0` — aggressive optimization causes segfaults in cpal/CoreAudio device enumeration
- Channel count can change between plugins (e.g., stereo in → 5.0 out after upmixer)
- The engine is fully native Rust with no external process dependencies
