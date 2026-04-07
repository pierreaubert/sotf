# sotf-player (lib: `sotf_audio_player`)

Shared business logic for all SOTF audio players. When adding features to players, the business logic goes here.

## Key Components

- **Controllers** (`controllers/`): Domain controllers that encapsulate shared business logic — UIs (TUI, GPUI, CLI) are thin wrappers
  - `LibraryController` — Filtering, sorting, pagination, navigation, directory management
  - `QueueController` — Queue mutations returning `QueuePlaybackEffect` for playback coordination
  - `PlaybackController` — Volume, mute, replay gain, play tracking
  - `PluginController` — Plugin chain management, parameter editing (~2657 lines, most complex)
  - `ScanController` — Wraps ReplayGain/Waveform/Bliss scan managers
  - `plugin_param_map.rs` — Maps UI parameter indices to engine parameter IDs
- `MusicLibrary` (`library.rs`): Music library scanner and manager with album/track metadata
- `MusicDatabase` (`database/mod.rs`): SQLite persistence for library data (schema, queries, migrations)
- `Player` (`player.rs`): Playback state wrapper over AudioEngine
- `Queue` (`queue.rs`): Album-based playback queue with track navigation
- `BlissScanner` (`bliss.rs`): Music similarity analysis for recommendations (pure Rust)
- `PluginGraph` (`plugin_graph.rs`): DAG-based visual plugin routing for UI
- `ReplayGainScanner` (`replay_gain_scanner.rs`): Background ReplayGain scanning
- `WaveformScanner` (`waveform_scanner.rs`): Background waveform extraction
- `autoeq/` — AutoEQ integration (speaker, headphone, multi-speaker, spinorama)
- `room_eq_types.rs` — Room EQ configuration types
- `headphone_eq_types.rs` — Headphone EQ configuration types
- `spinorama_eq_types.rs` — Spinorama EQ configuration types
- `recording_types.rs` — Recording session types

## Module Layout

- `controllers/` — Domain controllers (library, queue, playback, plugin, scan)
- `autoeq/` — EQ optimization integration (speaker, headphone, multi-speaker, spinorama)
- `database/` — SQLite schema and queries
- `library.rs` — Album/track management
- `player.rs` — Playback wrapper
- `queue.rs` — Playback queue
- `bliss.rs` — Audio analysis via math-dsp
- `plugin_graph.rs` — DAG-based plugin routing
- `security.rs` — Path validation for file access
- `config.rs` — Player configuration (AppConfig)
- `ui_params/` — UI parameter descriptors

## Features

- `testing` — Bypasses security validation for tests
- `hal` — macOS HAL support

## Binaries

- `sotf-bliss-scan` — Standalone Bliss analysis tool

## Testing

```bash
cargo test -p sotf-player
cargo check -p sotf-player && cargo clippy -p sotf-player
```

## Important Notes

- This is the shared library for all player frontends (TUI, GPUI, CLI)
- Business logic for player features belongs here, not in individual app crates
- Controllers own state; UIs delegate to controllers and only manage UI-specific state
- `QueueController` returns `QueuePlaybackEffect` from mutations for playback coordination
- `PluginController` returns `PluginUpdateEffect` to signal engine reconfiguration needs
- Uses rusqlite for SQLite database access
