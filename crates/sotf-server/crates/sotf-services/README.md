# sotf-services

Streaming service integrations (Spotify, Tidal) for SOTF.

## Overview

Provides a common interface for streaming music services, with optional backend implementations for Spotify and Tidal.

## Components

- `StreamingService` trait — Common interface for streaming service backends
- `ServiceCredentials` — Authentication credentials
- `ServiceTrack` — Track metadata from streaming services
- `PcmStream` — PCM audio stream from a service
- `AudioQuality` — Quality selection (low, normal, high, lossless)

### Spotify (behind `spotify` feature)

Uses `librespot` for Spotify Connect playback.

### Tidal (behind `tidal` feature)

Uses the Tidal API via `reqwest` for track streaming.

## Features

| Feature | Description | Default |
|---------|-------------|---------|
| `spotify` | Spotify Connect via librespot | No |
| `tidal` | Tidal API integration | No |

## Dependencies

- `tokio` — Async runtime
- `librespot-core` / `librespot-playback` / `librespot-metadata` (optional) — Spotify
- `reqwest` (optional) — Tidal HTTP client
- `serde` / `serde_json` (optional) — Serialization

## Testing

```bash
cargo test -p sotf-services --lib
cargo check -p sotf-services && cargo clippy -p sotf-services
```

## License

See the root workspace `LICENSE` file.
