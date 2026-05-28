# sotf-mpd

MPD (Music Player Daemon) protocol server for SOTF. Allows MPD-compatible clients to control SOTF playback.

## Overview

Implements the MPD protocol over TCP so that any MPD client (e.g., `mpc`, ncmpc, MALP, etc.) can control SOTF playback, browse the library, and manage playlists.

## Components

- `MpdServer` — TCP server accepting MPD protocol connections
- `MpdCommand` / `MpdResponse` / `MpdError` — Protocol types for command parsing and response formatting
- `PlayerAdapter` — Trait for connecting the MPD protocol handler to a player backend
- `protocol.rs` — MPD protocol parsing and serialization
- `handler.rs` — Command handler dispatching MPD commands to `PlayerAdapter`

## Features

| Feature | Description | Default |
|---------|-------------|---------|
| `tls` | TLS-encrypted MPD connections via `sotf-tls` and `tokio-rustls` | No |

## Dependencies

- `tokio` — Async TCP server
- `sotf-tls` (optional) — TLS infrastructure
- `tokio-rustls` (optional) — TLS support

## Testing

```bash
cargo test -p sotf-mpd --lib
cargo check -p sotf-mpd && cargo clippy -p sotf-mpd
```

## License

See the root workspace `LICENSE` file.
