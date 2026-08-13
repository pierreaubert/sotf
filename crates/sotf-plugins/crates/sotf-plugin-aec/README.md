# sotf-plugin-aec

Acoustic Echo Cancellation plugin — PBFDAF with two-path and post-filter.

## What It Does

Removes echo from audio streams in real-time. When a microphone picks up audio playing through speakers, the AEC plugin subtracts the known playback signal to produce clean microphone audio. Uses a Partitioned Block Frequency Domain Adaptive Filter (PBFDAF) for efficient cancellation.

## Features

- **PBFDAF algorithm**: Efficient frequency-domain adaptive filtering
- **Two-path architecture**: Stable foreground plus a background explorer with double-talk adaptation gating
- **Post-filter**: Leakage-model residual echo suppression with click-free wet/dry switching
- **Real-time processing**: Low-latency operation suitable for live audio
- **Input policy**: Non-finite microphone/reference samples are replaced by silence before adaptation

## Architecture

```
src/
├── lib.rs          # Public module surface
├── lib/            # AecPlugin implementation and tests
├── pbfdaf.rs        # Partitioned adaptive filter
├── two_path.rs      # Foreground/background management and DTD gate
├── post_filter.rs   # Residual-leakage suppressor
└── params.rs        # Canonical schema and serializable state
```

## Testing

```bash
cargo test -p sotf-plugin-aec
```

## License

Part of the SOTF (Sound of the Future) project.
