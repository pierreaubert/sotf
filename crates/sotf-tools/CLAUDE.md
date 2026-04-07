# tools

Utility binaries for test data generation and file conversion. No library.

## Binaries

- `generate-audio-tests` - Generate test audio signals (sine, sweep, noise) for testing
- `sofa-to-sqlite` - Convert SOFA (HRTF) files to SQLite (requires `sofa_support` feature)

## Features

- `sofa_support` (default) - SOFA file support via netCDF

## Testing

```bash
cargo check -p sotf-tools && cargo clippy -p sotf-tools
```

## Usage

```bash
cargo run --bin generate-audio-tests --release
cargo run --bin sofa-to-sqlite --release --features sofa_support
```
