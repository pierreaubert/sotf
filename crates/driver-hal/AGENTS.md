# driver-hal (lib: `driver_hal`)

Shared memory interface for macOS CoreAudio HAL driver.

## Purpose

Provides memory-mapped IPC between the HAL driver and the SOTF audio engine, with encrypted audio data transfer.

## Key Features

- Memory-mapped shared memory (via `memmap2`)
- Encrypted audio data transfer (ChaCha20-Poly1305)
- Secure IPC protocol between HAL driver and engine

## Dependencies

- `memmap2` - Memory-mapped files
- `chacha20poly1305` - AEAD encryption
- `sha2`, `hex` - Hashing
- `rand` - Random number generation

## Platform

macOS only.

## Testing

```bash
cargo test -p driver-hal --lib
cargo check -p driver-hal && cargo clippy -p driver-hal
```

## Notes

- Security-critical crate: handles encrypted audio data transfer
- Dependencies are crate-local (not in workspace.dependencies)
