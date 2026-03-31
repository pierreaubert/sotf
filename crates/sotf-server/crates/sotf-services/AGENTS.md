# sotf-services

Streaming service integrations (Spotify, Tidal).

## Key Types

- `StreamingService` trait -- Common interface for streaming backends
- `ServiceCredentials` -- Authentication
- `ServiceTrack` -- Track metadata
- `PcmStream` -- PCM audio stream
- `AudioQuality` -- Quality level selection

## Module Layout

- `service.rs` -- Common trait and types
- `spotify.rs` -- Spotify Connect via librespot (behind `spotify` feature)
- `tidal.rs` -- Tidal API (behind `tidal` feature)

## Features

- `spotify` -- Spotify Connect via librespot
- `tidal` -- Tidal API integration

## Testing

```bash
cargo test -p sotf-services --lib
cargo check -p sotf-services && cargo clippy -p sotf-services
```
