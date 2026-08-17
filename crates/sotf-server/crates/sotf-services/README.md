# sotf-services

Core streaming service traits and types for SOTF.

## Overview

Provides the common interface for streaming music services. Backend
implementations live in separate crates:

- `sotf-service-tidal` — Tidal API integration via `reqwest`
- `sotf-service-spotify` — Spotify Connect playback via `librespot`

## Components

- `StreamingService` trait — Common interface for streaming service backends
- `ServiceCredentials` — Authentication credentials
- `ServiceTrack` — Track metadata from streaming services
- `ServiceAlbum` — Album metadata from streaming services
- `PcmStream` — PCM audio stream from a service
- `AudioQuality` — Quality selection (low, normal, high, lossless)

## Dependencies

None (std only).

## Testing

```bash
cargo test -p sotf-services --lib
cargo check -p sotf-services && cargo clippy -p sotf-services
```

## License

See the root workspace `LICENSE` file.
