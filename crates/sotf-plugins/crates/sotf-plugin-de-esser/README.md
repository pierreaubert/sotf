# sotf-plugin-de-esser

De-esser — sibilance reduction for audio.

## What It Does

Reduces harsh sibilant sounds ("s", "sh", "ch") in vocals and other audio. Applies frequency-selective dynamic compression to the sibilance range (typically 4-10 kHz) without affecting the rest of the spectrum.

## Features

- **Frequency-selective processing**: Targets only the sibilance band
- **Dynamic compression**: Reduces sibilance only when it exceeds the threshold
- **Transparent operation**: Preserves the natural character of the audio

## Architecture

```
src/
├── lib.rs     # DeEsserPlugin implementation
└── params.rs  # Parameter definitions
```

## Testing

```bash
cargo test -p sotf-plugin-de-esser
```

## License

Part of the SOTF (Sound of the Future) project.
