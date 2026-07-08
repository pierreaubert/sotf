# app-cli

Command-line interfaces for audio playback and recording. Binaries only, no library.

## Binaries

- `player-cli` - CLI audio player with plugin support (EQ filters, upmixer)
- `sotf-recorder-cli` - Audio recording tool for measurements

## Usage

```bash
# Play a file with filters
cargo run --bin player-cli --release -- play audio.flac --filter 1000:1.5:3.0 --upmixer

# List available audio devices
cargo run --bin sotf-recorder-cli --release -- --list-devices

# Record a 1 kHz tone from hardware channel 0
cargo run --bin sotf-recorder-cli --release -- --signal tone --freq 1000 --duration 5 --hwaudio-send-to 0 --hwaudio-record-from 0
```

## Testing

```bash
cargo check -p app-cli && cargo clippy -p app-cli
```

## Notes

- Uses the `player` crate for business logic and `engine` for audio processing
- Filter format: `frequency:q:gain_db`
