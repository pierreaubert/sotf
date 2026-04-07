# sotf-plugin-aec

Acoustic Echo Cancellation plugin — PBFDAF with two-path and post-filter.

## What It Does

Removes echo from audio streams in real-time. When a microphone picks up audio playing through speakers, the AEC plugin subtracts the known playback signal to produce clean microphone audio. Uses a Partitioned Block Frequency Domain Adaptive Filter (PBFDAF) for efficient cancellation.

## Features

- **PBFDAF algorithm**: Efficient frequency-domain adaptive filtering
- **Two-path architecture**: Robust convergence with separate foreground/background filters
- **Post-filter**: Residual echo suppression for additional cleanup
- **Real-time processing**: Low-latency operation suitable for live audio

## Architecture

```
src/
├── lib.rs     # AecPlugin implementation
└── params.rs  # Parameter definitions
```

## Testing

```bash
cargo test -p sotf-plugin-aec
```

## License

Part of the SOTF (Sound of the Future) project.
