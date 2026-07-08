# sotf-player (lib: `sotf_audio_player`)

Shared business logic for all SOTF audio players (TUI, GPUI, CLI). When adding features to players, the business logic goes here — individual app crates should be thin UI wrappers that delegate to these controllers and modules.

## Key Components

- **Controllers** (`controllers/`): Domain controllers that encapsulate shared business logic so UIs become thin wrappers
  - `LibraryController` — Filtering, sorting, pagination, navigation, directory management
  - `QueueController` — Queue mutations returning `QueuePlaybackEffect` for playback coordination
  - `PlaybackController` — Volume, mute, replay gain, play tracking
  - `PluginController` — Plugin chain management, parameter editing, presets
  - `ScanController` — Wraps ReplayGain/Waveform/Bliss scan managers
- **MusicLibrary** (`library.rs`): Music library scanner and manager with album/track metadata, playlists, channel-type filtering
- **MusicDatabase** (`database/`): SQLite persistence for library data (schema, queries, migrations)
- **Player** (`player.rs`): Playback state wrapper over AudioEngine
- **Queue** (`queue.rs`): Album-based playback queue with track navigation
- **BlissScanner** (`bliss.rs`): Music similarity analysis for recommendations (pure Rust, via `math-dsp`)
- **PluginGraph** (`plugin_graph.rs`): DAG-based visual plugin routing for UI
- **ReplayGainScanner** (`replay_gain_scanner.rs`): Background ReplayGain scanning
- **WaveformScanner** (`waveform_scanner.rs`): Background waveform data extraction

## Module Layout

```
src/
├── lib.rs                    # Public API exports and re-exports
├── controllers/
│   ├── mod.rs                # Controller module root
│   ├── library.rs            # LibraryController — filtering, sorting, navigation
│   ├── queue.rs              # QueueController — queue mutations → QueuePlaybackEffect
│   ├── playback.rs           # PlaybackController — volume, mute, replay gain
│   ├── plugin.rs             # PluginController — plugin chain management
│   ├── plugin_param_map.rs   # param_index_to_engine_param mapping
│   └── scan.rs               # ScanController — ReplayGain/Waveform/Bliss managers
├── autoeq/
│   ├── mod.rs                # AutoEQ integration root
│   ├── speaker.rs            # Speaker EQ optimization
│   ├── headphone.rs          # Headphone EQ optimization
│   ├── multi_speaker.rs      # Multi-speaker EQ optimization
│   ├── spinorama.rs          # Spinorama data integration
│   ├── params.rs             # EQ optimization parameters
│   └── types.rs              # Shared types
├── database/
│   └── mod.rs                # SQLite schema, queries, migrations
├── library.rs                # Album/track management, scanning, metadata
├── library_scanner.rs        # Background library scanning
├── library_stats.rs          # Library statistics (track counts, duration, etc.)
├── player.rs                 # Playback wrapper over AudioEngine
├── queue.rs                  # Playback queue (album-based)
├── config.rs                 # Player configuration (AppConfig)
├── bliss.rs                  # Audio similarity analysis via math-dsp
├── plugin_graph.rs           # DAG-based plugin routing for UI
├── recommendation.rs         # Music recommendation engine
├── play_tracker.rs           # Play count tracking
├── level_meter.rs            # Channel group definitions for level meters
├── audio_device.rs           # Audio output device state
├── security.rs               # Path validation for file access
├── replay_gain_scanner.rs    # Background ReplayGain scanning
├── waveform_scanner.rs       # Background waveform extraction
├── ui_params/                # UI parameter descriptors
├── headphone_eq_types.rs     # Headphone EQ configuration types
├── spinorama_eq_types.rs     # Spinorama EQ configuration types
├── room_eq_types.rs          # Room EQ configuration types
└── recording_types.rs        # Recording session types
```

## Features

| Feature | Description | Default |
|---------|-------------|---------|
| `testing` | Bypasses security validation for tests | No |
| `hal` | macOS HAL support (passes through to `sotf-engine/hal`) | No |

## Binaries

- `sotf-bliss-scan` — Standalone Bliss audio similarity analysis tool

## Schema stability

See [`SCHEMA.md`](./SCHEMA.md) for the stable-versus-internal field
classification and version-compatibility rules for persisted player config
and state types (`AppConfig`, `ServerConfig`, `RoomEqOptimizerConfig`,
remote server/token stores, metadata services, and recording device configs).

## Testing

```bash
cargo test -p sotf-player
cargo check -p sotf-player && cargo clippy -p sotf-player
```

### Test Files

Tests are in `tests/`:

- `library_tests.rs` — Library scanning and metadata
- `database_tests.rs` — SQLite persistence
- `concurrent_db_tests.rs` — Concurrent database access
- `album_art_tests.rs` — Album art thumbnail extraction
- `plugin_chain_tests.rs` — Plugin chain configuration
- `replay_gain_tests.rs` — ReplayGain scanning
- `param_index_consistency.rs` — Parameter index mapping consistency
- `error_handling_tests.rs` — Error handling paths

## Architecture Notes

- This is the shared library for all player frontends (TUI, GPUI, CLI)
- Business logic for player features belongs here, not in individual app crates
- Controllers own state and expose operations; UIs are thin wrappers that delegate
- `QueueController` returns `QueuePlaybackEffect` from mutations so the UI knows what playback action to take
- `PluginController` returns `PluginUpdateEffect` to signal when the engine needs reconfiguration
- Uses `rusqlite` for SQLite database access
- Uses `parking_lot` mutexes for shared state
- Re-exports plugin types from `sotf-engine` and `sotf-plugins` for convenience

## License

See the root workspace `LICENSE` file.
