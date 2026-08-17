# driver-common

Platform-agnostic audio driver trait for system-wide audio capture.

## Architecture

Single-file crate (`lib.rs`) defining the `AudioDriver` trait and supporting types. No platform-specific code.

```
lib.rs
  AudioDriver   -- Core trait: initialize/shutdown, read_audio, config negotiation, engine readiness
  NullDriver    -- No-op fallback (compiles everywhere, returns zero frames)
  DriverStatus  -- Runtime status: platform_supported, driver_installed, capture_active, sample_rate, channels, buffer_frames
  DriverConfig  -- Configuration request: sample_rate, buffer_frames, channel_count
  ConfigResult  -- Accepted, Negotiated { actual_rate, actual_frames, actual_channels }, Error(DriverError)
  DriverError   -- Structured driver/config failure reason
```

## Key Public API

- `AudioDriver` trait -- `initialize()`, `shutdown()`, `status()`, `read_audio()`, `available_frames()`, `sample_rate()`, `channel_count()`, `request_config()`, `poll_config_change()`, `acknowledge_config_change()`, `set_engine_ready()`
- `NullDriver` -- Fallback implementation: `platform_supported: false`, reads zero frames, always compiles
- `DriverStatus` -- Serializable status snapshot
- `DriverConfig` -- Sample rate + buffer size + channel count request
- `ConfigResult` -- Three-way result for config negotiation
- `DriverError` -- Structured errors for unavailable drivers, invalid config, timeouts, I/O, etc.

## Testing

```bash
cargo test -p driver-common
```

## Important Notes

- `AudioDriver` is `Send + 'static` for single-owner use as `Box<dyn AudioDriver>` in the daemon; it is intentionally not `Sync`
- `read_audio()` returns the number of complete *frames* -- caller provides a buffer of `frame_count * channel_count` floats
- `read_frames()` is the frame-count convenience wrapper for new callers
- `DriverConfig::default()` / `keep_current()` preserve current settings; `0` remains the wire-level sentinel for compatibility
- Driver-initiated config changes use the `poll_config_change()` / `acknowledge_config_change()` handshake
- Platform implementations: macOS HAL (`driver-hal`), Linux PipeWire (planned), Windows APO (planned)
- `NullDriver` allows the daemon to compile and run gracefully on unsupported platforms
