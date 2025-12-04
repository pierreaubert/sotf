# SOTF GPUI Music Player

A modern GUI music player built with GPUI for the SOTF audio engine. This is a clone of the TUI player (`src-tui-player`) but using GPUI instead of terminal-based UI.

## About GPUI

GPUI is the UI framework from [Zed](https://zed.dev), a high-performance code editor. It provides:
- GPU-accelerated rendering
- Reactive data model
- Native look and feel
- Cross-platform support (macOS, Linux, Windows)

## Features

- **Library Management**: Browse and search your music collection
- **Queue System**: Build and manage a playback queue with auto-advance
- **Audio Plugin System**: Configure EQ, upmixer, dynamics processors, and more
- **Audio Playback**: Integrated with the native SOTF audio engine
- **Device Selection**: Choose output audio devices
- **Real-time Monitoring**: Loudness (LUFS) and spectrum analysis
- **Keyboard Shortcuts**: Efficient navigation and control

## Building

The GPUI player is part of the SOTF workspace but excluded from default build due to its heavy dependencies:

```bash
# Build the GPUI player specifically
cargo build --release -p sotf-gpui-player

# Or build directly in the directory
cd src-gpui-player
cargo build --release
```

## Running

```bash
# Run from workspace root
cargo run --release --bin sotf_player_gpui

# Run from src-gpui-player directory
cargo run --release
```

## Architecture

The GPUI player shares most backend code with the TUI player:

### Shared Modules (from `src-tui-player`)
- **`library.rs`**: Music library scanner (FLAC, MP3, M4A, AAC, OGG, Opus, WAV)
- **`database.rs`**: SQLite persistence for library metadata
- **`config.rs`**: Configuration file handling
- **`plugins.rs`**: Audio plugin configuration (EQ, Upmixer, Compressor, etc.)
- **`player.rs`**: Integration with `AudioStreamingManager`

### GPUI-Specific Modules
- **`main.rs`**: GPUI application setup and window creation
- **`app.rs`**: Application state adapted for GPUI's reactive model
- **`ui.rs`**: GPUI views and rendering

## Keyboard Shortcuts

### Global
- **Cmd/Ctrl+Q**: Quit application
- **1**: Switch to Library screen
- **2**: Switch to Queue screen
- **3**: Switch to Plugins screen
- **4**: Switch to Devices screen

### Playback Control
- **Space**: Play/Pause
- **S**: Stop playback
- **N**: Next track
- **P**: Previous track
- **+** or **=**: Increase volume
- **-**: Decrease volume

## Screens

### Library Screen
Browse all albums in your library with search functionality.

### Queue Screen
View and manage the current playback queue. Shows:
- All queued albums
- Current track position within each album
- Currently playing album (highlighted)

### Plugins Screen
Configure the audio processing chain:
- EQ (parametric equalizer)
- Upmixer (stereo to surround)
- Compressor
- Limiter
- Gate
- Loudness Compensation

### Devices Screen
Select the audio output device.

## Integration with SOTF Audio Engine

The player uses `AudioStreamingManager` from `sotf_audio` for:
- Multi-format audio decoding via Symphonia
- Plugin chain processing
- Real-time playback control
- Loudness and spectrum monitoring

Example:
```rust
let player = Player::new();
player.load_and_play(path, plugins, output_channels, device_name)?;
player.set_volume(0.5)?;
player.pause()?;
player.resume()?;
```

## Differences from TUI Player

| Feature | TUI Player | GPUI Player |
|---------|-----------|-------------|
| **UI Framework** | Ratatui (terminal) | GPUI (native GUI) |
| **Rendering** | Terminal characters | GPU-accelerated graphics |
| **Input** | Keyboard only | Keyboard + Mouse |
| **Portability** | SSH-friendly | Desktop only |
| **Performance** | Lightweight | Higher resource usage |
| **Visuals** | ASCII art | Modern UI |

## System Requirements

### Linux
```bash
# Debian/Ubuntu
sudo apt-get install libopenblas-dev libxcb-shape0-dev libxcb-xfixes0-dev

# Fedora/RHEL
sudo dnf install openblas-devel libxcb-devel

# Arch Linux
sudo pacman -S openblas libxcb
```

### macOS
No additional dependencies required (uses Accelerate framework).

### Windows
Uses Intel MKL or OpenBLAS (automatically configured in Cargo.toml).

## Development Status

✅ **Implemented:**
- Basic GPUI application structure
- Window management and layouts
- Screen navigation (Library, Queue, Plugins, Devices)
- Playback control integration
- Real-time state updates
- Keyboard shortcuts

🚧 **In Progress:**
- Interactive album selection (mouse clicks)
- Search functionality UI
- Plugin parameter editing
- Spectrum analyzer visualization
- Directory management UI

📋 **Planned:**
- Drag-and-drop queue reordering
- Album art display
- Waveform visualization
- Playlist management
- Themes and customization

## Contributing

When contributing to the GPUI player:

1. Ensure shared modules (`library.rs`, `database.rs`, etc.) remain compatible with both TUI and GPUI versions
2. Follow GPUI best practices for reactive state management
3. Test on multiple platforms if possible
4. Keep keyboard shortcuts consistent with the TUI player where applicable

## License

GPL-3.0-or-later (same as parent project)
