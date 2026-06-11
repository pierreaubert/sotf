# sotf-mpd

MPD (Music Player Daemon) protocol server.

## Key Types

- `MpdServer` -- TCP server accepting MPD protocol connections
- `MpdCommand` / `MpdResponse` / `MpdError` -- Protocol types
- `PlayerAdapter` -- Trait bridging MPD commands to a player backend

## Module Layout

- `protocol.rs` -- MPD protocol parsing and serialization
- `handler.rs` -- Command dispatch to `PlayerAdapter`
- `server.rs` -- Async TCP server

## Testing

```bash
cargo test -p sotf-mpd --lib
cargo check -p sotf-mpd && cargo clippy -p sotf-mpd
```
