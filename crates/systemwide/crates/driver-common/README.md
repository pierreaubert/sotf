# driver-common

Platform-agnostic audio driver trait for system-wide audio capture.

## What It Does

Defines the `AudioDriver` trait that all platform-specific audio capture drivers must implement. The SOTF daemon holds a `Box<dyn AudioDriver>` and uses it uniformly regardless of platform. A `NullDriver` fallback is included for platforms without a driver implementation.

## Features

- `AudioDriver` trait for single-owner platform-independent audio capture
- `NullDriver` fallback that compiles on all platforms
- Configuration negotiation (sample rate, buffer size, channel count)
- Driver-initiated config change detection
- Structured `DriverError` values for driver/config failures
- Serializable `DriverStatus` for reporting to UIs

## Usage

```rust
use driver_common::{AudioDriver, NullDriver, DriverConfig};

// Use NullDriver as fallback
let mut driver: Box<dyn AudioDriver> = Box::new(NullDriver::new());
driver.initialize().unwrap();

let status = driver.status();
if !status.platform_supported {
    println!("No audio capture driver for this platform");
}

// Read audio (returns 0 samples for NullDriver)
let mut buffer = vec![0.0f32; 1024];
let samples_read = driver.read_audio(&mut buffer);
let frames_read = driver.read_frames(&mut buffer);
```

## Architecture

Single-file crate with one trait, one fallback implementation, and three supporting types:

| Type | Purpose |
|------|---------|
| `AudioDriver` | Core trait for platform drivers |
| `NullDriver` | No-op fallback (zero frames, always compiles) |
| `DriverStatus` | Runtime status snapshot |
| `DriverConfig` | Sample rate, buffer size, and channel count request |
| `ConfigResult` | Three-way config negotiation result |
| `DriverError` | Structured driver failure reason |

`DriverConfig::default()` / `DriverConfig::keep_current()` preserve every
current setting. The public fields keep the daemon wire convention where `0`
means "keep current"; helper constructors such as
`DriverConfig::with_channel_count(8)` make that intent explicit.

## Testing

```bash
cargo test -p driver-common
```

## License

Part of the SOTF (Sound of the Future) project.
