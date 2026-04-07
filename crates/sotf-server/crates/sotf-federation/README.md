# sotf-federation

Library federation — multi-source library providers, merge engine, and sync.

## What It Does

Aggregates music libraries from multiple sources into a single unified view. Whether your music is on local disk, a DLNA/UPnP media server, or an MPD server, sotf-federation provides a common interface to discover and browse albums and tracks across all sources.

## Features

- **Multi-source aggregation**: Local files, DLNA/UPnP, MPD servers
- **Unified provider trait**: Common async interface for all source types
- **Deterministic identity**: Stable UUIDs for albums/tracks across instances
- **Source registry**: Manage and configure multiple library sources
- **Event system**: Library change notifications

## Usage

```rust
use sotf_federation::{SourceRegistry, LocalFilesProvider, LocalProviderConfig};

let mut registry = SourceRegistry::new();

// Add a local files source
let config = LocalProviderConfig { path: "/music".into() };
let provider = LocalFilesProvider::new(config);
registry.add_source(Box::new(provider));

// Query albums across all sources
for source in registry.sources() {
    let albums = source.albums().await?;
    for album in albums {
        println!("{} - {}", album.artist, album.title);
    }
}
```

## Supported Sources

| Source | Provider | Description |
|--------|----------|-------------|
| Local files | `LocalFilesProvider` | Scans filesystem for audio files |
| DLNA/UPnP | `DlnaProvider` | Discovers and browses DLNA media servers |
| MPD | `MpdProvider` | Connects to Music Player Daemon servers |

## Architecture

```
src/
├── lib.rs             # Re-exports
├── provider.rs        # LibraryProvider trait and types
├── registry.rs        # SourceRegistry — manages providers
├── local_provider.rs  # Local filesystem provider
├── dlna_provider.rs   # DLNA/UPnP provider
├── mpd_provider.rs    # MPD server provider
└── identity.rs        # Deterministic UUID generation
```

## Testing

```bash
cargo test -p sotf-federation
```

## License

Part of the SOTF (Sound of the Future) project.
