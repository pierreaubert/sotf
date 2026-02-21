# midi (lib: `sotf_audio_player_midi`)

MIDI device management and control for the SOTF audio system.

## Key Components

- MIDI I/O via midir library
- Device configuration persistence
- Background MIDI handling thread

## Dependencies

- `midir` - Cross-platform MIDI I/O
- `tokio` - Async runtime
- `parking_lot` - Synchronization

## Testing

```bash
cargo test -p midi --lib
cargo check -p midi && cargo clippy -p midi
```

## Examples

```bash
cargo run --release --example <example_name> -p midi
```
