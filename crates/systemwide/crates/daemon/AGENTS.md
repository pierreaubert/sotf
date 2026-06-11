# daemon (lib: `sotf-daemon`, binary: `sotf-daemon`)

Background daemon for system-wide audio processing on macOS.

## Purpose

Runs as a background service providing audio processing through the macOS HAL driver integration.

## Features

- `hal` - Required for the binary (macOS HAL integration)

## Dependencies

- `plugins` - Audio plugin system
- `engine` - Audio processing engine
- `cpal` - Audio I/O
- `tokio` - Async runtime
- `parking_lot` - Synchronization

## Platform

macOS only (requires HAL driver).

## Testing

```bash
cargo check -p daemon && cargo clippy -p daemon
```
