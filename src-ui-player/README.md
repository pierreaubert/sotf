# SOTF TUI Music Player

A Terminal User Interface (TUI) music player built with Ratatui for the SOTF audio engine.

## Features

- **Library Management**: Add/remove directories and automatically scan for music files
- **Album Organization**: Automatically organizes music by artist and album using metadata tags
- **Search**: Filter albums with a real-time search box
- **Queue System**: Build and manage a playback queue
- **Audio Playback**: Integrated with the native SOTF audio engine
- **Keyboard Navigation**: Vim-style keybindings for efficient navigation

## Architecture

The TUI player consists of several modules:

- **`library.rs`**: Music library scanner that discovers audio files and extracts metadata
  - Supports FLAC, MP3, M4A, AAC, OGG, Opus, WAV
  - Uses Symphonia for metadata extraction
  - Organizes tracks into albums

- **`app.rs`**: Application state management
  - Tracks current screen (Library, Directory Manager, Queue)
  - Manages selection state and user input
  - Handles queue operations

- **`ui.rs`**: TUI rendering with Ratatui
  - Three main screens: Library, Directory Manager, Queue
  - Search box with live filtering
  - Status bar showing playback info and keybindings

- **`events.rs`**: Event handling and keyboard input
  - Modal input system (Normal, Search, AddDirectory)
  - Generates PlayerCommand events for audio control

- **`player.rs`**: Integration with AudioStreamingManager
  - Async audio playback control
  - Volume management
  - Position tracking

## Usage

```bash
# Basic usage
cargo run --bin sotf_player_tui

# Start with directories pre-configured
cargo run --bin sotf_player_tui -- -d ~/Music -d ~/Downloads/Music

# Auto-scan on startup
cargo run --bin sotf_player_tui -- -d ~/Music --scan
```

## Keyboard Controls

### Global

- `1`: Switch to Library screen
- `2`: Switch to Directory Manager screen
- `3`: Switch to Queue screen
- `Ctrl+Q` or `ESC`: Quit application

### Library Screen

- `/`: Enter search mode
- `↑`/`k`: Move selection up
- `↓`/`j`: Move selection down
- `a` or `Enter`: Add selected album to queue
- `q`: Switch to Queue screen

### Directory Manager Screen

- `a`: Enter add directory mode
- `↑`/`k`: Move selection up
- `↓`/`j`: Move selection down
- `d` or `Delete`: Remove selected directory
- `s`: Scan library for music

### Queue Screen

- `↑`/`k`: Move selection up
- `↓`/`j`: Move selection down
- `p`: Play from start of queue
- `Space`: Pause/Resume playback
- `n`: Next track
- `d` or `Delete`: Remove selected item from queue
- `c`: Clear entire queue
- `+`/`=`: Increase volume
- `-`: Decrease volume

## Building

The TUI player is part of the SOTF workspace. To build:

```bash
# Build in release mode
cargo build --release --bin sotf_player_tui

# Build in debug mode
cargo build --bin sotf_player_tui
```

### System Requirements

On Linux, you'll need OpenBLAS or another BLAS implementation:

```bash
# Debian/Ubuntu
sudo apt-get install libopenblas-dev

# Fedora/RHEL
sudo dnf install openblas-devel

# Arch Linux
sudo pacman -S openblas
```

## Dependencies

Key dependencies include:

- **ratatui**: Modern TUI framework
- **crossterm**: Terminal manipulation
- **sotf_audio**: Native audio engine
- **symphonia**: Audio metadata extraction
- **tokio**: Async runtime for audio control
- **walkdir**: Directory traversal

## Future Enhancements

Potential improvements:

- [ ] Playlist support (save/load playlists)
- [ ] Track-level queue (not just albums)
- [ ] Seek control (seek within tracks)
- [ ] Equalizer controls
- [ ] Visualizations (spectrum analyzer)
- [ ] Configuration file support
- [ ] Album art display (with sixel/kitty graphics protocol)
- [ ] Lyrics display
- [ ] Smart playlists and filters
- [ ] Shuffle and repeat modes
- [ ] Gapless playback

## Integration with SOTF Audio Engine

The player uses `AudioStreamingManager` from `sotf_audio`:

```rust
let manager = AudioStreamingManager::new();
manager.load_file(&path).await?;
manager.start_playback(None, plugins, output_channels).await?;
```

This provides:
- Multi-format audio decoding via Symphonia
- Plugin chain support (EQ, upmixer, effects)
- Real-time playback control
- Event notifications for end-of-stream

## License

GPL-3.0-or-later (same as parent project)
