# sotf-tui (lib: `sotf_audio_player_tui`)

Terminal-based music player frontend for SOTF.

## Overview

A production-quality TUI music player built with [ratatui](https://github.com/ratatui/ratatui). Provides library scanning, album/track browsing, playlist management, Room EQ configuration, and full access to the SOTF plugin chain — all in the terminal.

## Features

- **Library browsing**: Album and track navigation with cover art support (non-Windows)
- **Playback control**: Play, pause, skip, seek, shuffle, repeat
- **Plugin chain**: Full access to EQ, compressor, crossfeed, and all SOTF plugins
- **Room EQ**: Interactive room equalization with sweep recording, Bayesian optimization, and CTC matrix capture
- **Streaming services**: Optional Spotify and Tidal integration (`spotify` / `tidal` features)
- **Media controls**: System media key / Now Playing integration
- **HAL support**: Optional hardware abstraction layer for direct device control

## Usage

```bash
# Run the TUI
cargo run -p sotf-tui

# With streaming services
cargo run -p sotf-tui --features "spotify tidal"

# With hardware abstraction layer
cargo run -p sotf-tui --features hal
```

## Features

| Feature  | Description                                      | Default |
|----------|--------------------------------------------------|---------|
| `hal`    | Hardware abstraction layer for direct I/O        | No      |
| `onnx`   | ONNX runtime for AI-powered features             | No      |
| `iamf`   | IAMF immersive audio decoding                    | No      |
| `tidal`  | Tidal streaming integration                      | No      |
| `spotify`| Spotify Connect integration                      | No      |

## Architecture

- `lib.rs` — TUI library (`sotf_audio_player_tui`)
- `main.rs` — Binary entry point
- `ui/` — Terminal UI components and screens
- `events/` — Event handling (keyboard input, playback events)
- `tests/` — Unit and integration tests (parameter sync, etc.)

## Dependencies

- `sotf-player` — Shared audio player library (business logic, database, plugins)
- `sotf-engine` / `sotf-plugins` — Audio engine and plugin chain
- `ratatui` / `crossterm` — Terminal UI framework
- `tokio` — Async runtime
- `rusqlite` — Embedded database (bundled)

## Testing

```bash
cargo test -p sotf-tui --lib
cargo test -p sotf-tui --test tests_parameter_sync
cargo check -p sotf-tui && cargo clippy -p sotf-tui
```

## License

See the root workspace `LICENSE` file.
