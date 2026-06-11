# sotf-federation

Library federation — multi-source library providers, merge engine, and sync.

## Architecture

Provides a unified interface for aggregating music libraries from multiple sources (local files, DLNA/UPnP servers, MPD servers).

- `provider.rs` — `LibraryProvider` trait: async interface for sources contributing albums/tracks. Also: `ProviderAlbum`, `ProviderTrack`, `ProviderCapabilities`, `ProviderError`, `SourceId`, `SourceType`, `LibraryEvent`
- `registry.rs` — `SourceRegistry`: manages configured providers, `SourceConfig`
- `local_provider.rs` — `LocalFilesProvider`: scans local filesystem for audio files. `LocalProviderConfig`, `LocalTrackInfo`
- `dlna_provider.rs` — `DlnaProvider`: discovers and browses DLNA/UPnP media servers. `DlnaProviderConfig`
- `mpd_provider.rs` — `MpdProvider`: connects to MPD servers. `MpdProviderConfig`
- `identity.rs` — Deterministic UUID generation: `album_uuid()`, `track_uuid()` for cross-instance identity

## Key Public API

- `LibraryProvider` trait — `async fn albums()`, `async fn tracks()`, `async fn capabilities()` (`provider.rs`)
- `SourceRegistry` — `add_source()`, `remove_source()`, `sources()` (`registry.rs`)
- `album_uuid(artist, title) -> Uuid` / `track_uuid(artist, title, album) -> Uuid` — deterministic identity (`identity.rs`)

## Testing

```bash
cargo test -p sotf-federation
```

## Important Notes

- Merge engine and sync scheduler are planned but not yet implemented
- Deterministic UUIDs use UUID v5 for stable cross-instance identity
- Depends on `sotf-engine` for audio metadata types
- Async (tokio) throughout
