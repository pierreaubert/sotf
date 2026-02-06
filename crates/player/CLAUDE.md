# player (lib: `sotf_audio_player`)

Shared business logic for all SOTF audio players. When adding features to players, the business logic goes here.

## Key Components

- `MusicLibrary` (`library.rs`): Music library scanner and manager with album/track metadata
- `MusicDatabase` (`database.rs`): SQLite persistence for library data (schema, queries, migrations)
- `Player` (`player.rs`): Playback state wrapper over AudioEngine
- `BlissScanner` (`bliss.rs`): Music similarity analysis for recommendations
- `PluginGraph` (`plugin_graph.rs`): DAG-based visual plugin routing
- `ReplayGainScanner`: Volume normalization scanning
- `WaveformScanner`: Waveform data extraction

## Module Layout

- `library.rs` - Album/track management
- `database.rs` - SQLite schema and queries
- `player.rs` - Playback wrapper
- `bliss.rs` - Audio analysis via bliss-rs
- `autoeq/` - EQ optimization integration
- `plugin_graph.rs` - DAG-based plugin routing
- `security.rs` - Path validation for file access
- `config.rs` - Player configuration

## Features

- `testing` - Bypasses security validation for tests
- `hal` - macOS HAL support

## Binaries

- `sotf-bliss-scan` - Standalone Bliss analysis tool

## Testing

```bash
cargo test -p player --lib
cargo check -p player && cargo clippy -p player
```

## Important Notes

- This is the shared library for all player frontends (TUI, GPUI, CLI)
- Business logic for player features belongs here, not in individual app crates
- Uses rusqlite for SQLite database access
