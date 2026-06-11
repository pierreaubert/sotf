# systemwide

System-wide audio processing subsystem: OS audio capture, plugin-chain processing, and hardware output.

## Purpose

Provides a background daemon (`sotf-daemon`) that intercepts system audio via a platform driver, runs it through the SOTF plugin chain, and outputs to physical devices. External GUIs control it over a Unix socket with JSON messages.

## Sub-crates

- **`driver-common`** (`driver_common`) -- Platform-agnostic `AudioDriver` trait and `NullDriver` fallback. No platform-specific code.
- **`driver-hal`** (`driver_hal`) -- macOS-only. Shared-memory bridge to the Swift CoreAudio HAL driver. Audio data is encrypted with ChaCha20-Poly1305. Communicates via `/tmp/sotf-{uid}/audio.shm`.
- **`daemon`** (`sotf-daemon`) -- Background daemon binary. Coordinates the driver, audio engine, plugin host, and IPC (JSON over Unix domain socket). Feature flag `hal` enables macOS HAL support.

## Key types

- `driver_common::AudioDriver` -- trait implemented by each platform driver
- `driver_common::NullDriver` -- no-op fallback (compiles everywhere)
- `driver_common::DriverStatus` / `DriverConfig` / `ConfigResult` -- driver status and configuration
- `driver_hal::HalInputReader` / `HalOutputWriter` -- shared-memory reader/writer (macOS)
- `daemon::DriverManager` -- runtime driver lifecycle management

## Data flow

```
OS audio apps → Swift HAL Driver → shared memory → daemon (read) → plugin chain → daemon (write) → shared memory → HAL Driver → physical output
```

On non-macOS platforms, `NullDriver` is used and capture is inactive.

## IPC

The daemon listens on a Unix socket. Protocol: one JSON object per line.

Commands include: `load_plugins`, `get_status`, `set_volume`, `stop`.

Clients: Swift menubar app (`configbar/`), GPUI configbar, CLI tools.

## Platform notes

- macOS: requires `--features hal` for real audio capture
- Linux/Windows: planned (PipeWire, APO), `AudioDriver` trait is ready
- All platforms: daemon compiles and runs with `NullDriver`

## Dependencies

- `sotf-plugins`, `sotf-engine` -- audio processing
- `cpal` -- audio output
- `memmap2`, `chacha20poly1305` -- shared memory + encryption (driver-hal)
- `tokio` -- async runtime (daemon)

## Testing

```bash
cargo test -p driver-common --lib
cargo test -p driver-hal --lib
cargo check -p sotf-daemon && cargo clippy -p sotf-daemon
```

## Security

- Per-user shared memory path (`/tmp/sotf-{uid}/`)
- Audio data encrypted in shared memory (ChaCha20-Poly1305)
- Unix socket peer credential verification
- Secure socket directory permissions
