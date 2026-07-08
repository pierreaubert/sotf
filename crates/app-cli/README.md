# app-cli

Command-line audio player and recorder for SOTF with full plugin support.

## Overview

Provides two CLI binaries for audio playback and recording with access to the complete SOTF plugin chain:

- **`player-cli`** — Audio player with library scanning, playlist support, and real-time plugin processing
- **`sotf-recorder-cli`** — Test signal generator and recorder for acoustic measurements

## Binaries

### `player-cli`

Feature-rich command-line music player:

- Audio playback with plugin chain (EQ, crossfeed, loudness compensation, etc.)
- Library scanning and playlist management
- Multiple output configurations (2.0, 5.0, 5.1, 7.1, 5.1.2)
- Loudness compensation with configurable reference levels
- Crossfeed presets for headphone listening
- Real-time preflight checks

```bash
# Basic playback
player-cli play /path/to/music

# With specific output config and plugins
player-cli play --upmixer --upmixer-config 5.1 --filter 1000:1.5:3.0 /path/to/music
```

### `sotf-recorder-cli`

Generate and record test signals for acoustic measurements:

- Signal types: tone, two-tone, sweep, white/pink/M-noise, MLS, Dirac
- Configurable duration, sample rate, and output channel
- Multi-channel recording with analysis

```bash
# Generate a log sweep and record
sotf-recorder-cli --signal sweep --duration 10 --sample-rate 48000 --hwaudio-send-to 0 --hwaudio-record-from 0

# Record from specific input channels
sotf-recorder-cli --signal tone --hwaudio-send-to 0 --hwaudio-record-from 0,1,2
```

## Dependencies

- `sotf-engine` — Audio engine
- `sotf-plugins` — Plugin chain
- `cpal` — Cross-platform audio I/O
- `symphonia` — Audio decoding (FLAC, MP3, AAC, ALAC, Vorbis, etc.)
- `clap` — CLI argument parsing
- `tokio` — Async runtime

## Testing

```bash
cargo check -p app-cli
cargo clippy -p app-cli
```

## License

See the root workspace `LICENSE` file.
