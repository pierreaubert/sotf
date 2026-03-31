# sotf-mpd

MPD (Music Player Daemon) protocol server for SOTF. Allows MPD-compatible clients to control SOTF playback.

## Components

- `MpdServer` -- TCP server accepting MPD protocol connections
- `MpdCommand` / `MpdResponse` / `MpdError` -- Protocol types for command parsing and response formatting
- `PlayerAdapter` -- Trait for connecting the MPD protocol handler to a player backend
- `protocol.rs` -- MPD protocol parsing and serialization
- `handler.rs` -- Command handler dispatching MPD commands to `PlayerAdapter`

## Dependencies

- `tokio` -- Async TCP server

## Testing

```bash
cargo test -p sotf-mpd --lib
cargo check -p sotf-mpd && cargo clippy -p sotf-mpd
```

## License

See the root workspace `LICENSE` file.
