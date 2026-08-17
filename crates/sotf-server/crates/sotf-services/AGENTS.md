# sotf-services

Core streaming service traits and types. Provider implementations live in
sibling crates: `sotf-service-tidal` and `sotf-service-spotify`.

## Key Types

- `StreamingService` trait -- Common interface for streaming backends
- `ServiceCredentials` -- Authentication
- `ServiceTrack` -- Track metadata
- `ServiceAlbum` -- Album metadata
- `PcmStream` -- PCM audio stream
- `AudioQuality` -- Quality level selection
- `redact_secret` -- Redaction helper for logging secrets

## Module Layout

- `service.rs` -- Common trait and types (std-only, no dependencies)

## Providers

- `sotf-service-tidal` -- Tidal API integration (`TidalService`)
- `sotf-service-spotify` -- Spotify Connect via librespot (`SpotifyService`)

## Testing

```bash
cargo test -p sotf-services --lib
cargo check -p sotf-services && cargo clippy -p sotf-services
```
