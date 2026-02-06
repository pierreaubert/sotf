# app-tui (lib: `sotf_audio_player_tui`, binary: `sotf-tui`)

Production-quality terminal UI music player.

## Key Features

- Library scanning with metadata indexing (album art, tags)
- Ratatui-based UI with album art support (via ratatui-image)
- Full plugin chain integration (EQ, upmixer, compressor, etc.)
- Queue management with auto-advance
- ReplayGain support

## Architecture

- UI layer built with ratatui and crossterm
- Business logic delegated to the `player` crate
- Audio processing via the `engine` crate

## Dependencies

- `ratatui` - Terminal UI framework
- `crossterm` - Terminal backend
- `ratatui-image` - Album art rendering

## Testing

```bash
cargo test -p app-tui --lib
cargo check -p app-tui && cargo clippy -p app-tui
```

## Running

```bash
cargo run --bin sotf-tui --release
```
