# sotf-federation

Library federation — multi-source library providers, merge engine, and sync.

## Architecture

Provides a unified interface for aggregating music libraries from multiple sources (local files, DLNA/UPnP servers, MPD servers, streaming services).

- `provider.rs` — `LibraryProvider` trait: async interface for sources contributing albums/tracks. Also: `ProviderAlbum`, `ProviderTrack`, `ProviderCapabilities`, `ProviderError`, `SourceId`, `SourceType`, `LibraryEvent`
- `registry.rs` — `SourceRegistry`: manages configured providers, `SourceConfig`
- `local_provider.rs` — `LocalFilesProvider`: scans local filesystem for audio files. `LocalProviderConfig`, `LocalTrackInfo`
- `dlna_provider.rs` — `DlnaProvider`: discovers and browses DLNA/UPnP media servers. `DlnaProviderConfig`
- `mpd_provider.rs` — `MpdProvider`: connects to MPD servers. `MpdProviderConfig`
- `tidal_provider.rs` — `TidalProvider` (feature `tidal`): Tidal favorites via `sotf-service-tidal`. `TidalProviderConfig` (access/refresh token, client id, country code, quality string; `Debug` redacts tokens). Async `new()` authenticates (refresh-token exchange first, then access-token validation); `new_with_token_persister()` additionally reports the rotated single-use refresh token via a `TidalTokenPersister` callback (`Fn(&str, Option<&str>) + Send + Sync`) so callers can persist it. `with_service()` / `connect_with()` are test seams
- `spotify_provider.rs` — `SpotifyProvider` (feature `spotify`): Spotify saved albums via `sotf-service-spotify`. `SpotifyProviderConfig` (librespot credential `cache_dir`). Async `new()` restores the session from cached credentials; `with_service()` is the test seam
- `service_common.rs` — shared helpers for the streaming providers: `ServiceError`→`ProviderError` mapping, `ServiceTrack`→`ProviderTrack` mapping, bounded album-art download (8 MiB cap, `image/*` only)
- `identity.rs` — Deterministic UUID generation: `album_uuid()`, `track_uuid()` for cross-instance identity

## Key Public API

- `LibraryProvider` trait — `async fn albums()`, `async fn tracks()`, `async fn capabilities()` (`provider.rs`)
- `SourceRegistry` — `add_source()`, `remove_source()`, `sources()` (`registry.rs`)
- `album_uuid(artist, title) -> Uuid` / `track_uuid(artist, title, album) -> Uuid` — deterministic identity (`identity.rs`)

## Features

- `tidal` — Tidal provider (`sotf-service-tidal`, `sotf-services`, `reqwest`)
- `spotify` — Spotify provider (`sotf-service-spotify`, `sotf-services`, `reqwest`)

## Testing

```bash
cargo test -p sotf-federation
cargo test -p sotf-federation --features tidal,spotify   # incl. mock-server integration tests
```

## Important Notes

- Merge engine and sync scheduler are planned but not yet implemented
- Deterministic UUIDs use UUID v5 for stable cross-instance identity
- Depends on `sotf-engine` for audio metadata types
- Async (tokio) throughout
- Streaming-service providers expose tracks as `AudioSource::ServiceStream { service, track_id }`; the engine's service-stream resolver mints fresh stream URLs / PCM streams at decode time — providers never call `start_stream` during a scan
- Sync service calls run inside `tokio::task::spawn_blocking` (the service crates' `StreamingService` API is sync); construction (`new`) is async for the same reason
- Streaming provider secrets: never log tokens unredacted; `TidalProviderConfig`'s `Debug` uses `sotf_services::redact_secret`

