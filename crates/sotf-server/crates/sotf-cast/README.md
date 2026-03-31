# sotf-cast

AirPlay and Chromecast audio casting for SOTF.

## Components

- `CastDiscovery` -- Network discovery for cast-capable devices
- `CastDevice` / `CastDeviceType` -- Device representation (AirPlay, Chromecast)

### AirPlay (behind `airplay` feature)

- `airplay.rs` -- AirPlay 2 audio streaming

### Chromecast (behind `chromecast` feature)

- `chromecast.rs` -- Google Cast protocol audio streaming

## Features

| Feature | Description | Default |
|---------|-------------|---------|
| `airplay` | AirPlay 2 audio casting | Yes |
| `chromecast` | Google Cast audio casting | Yes |

## Dependencies

- `tokio` -- Async networking
- `uuid` -- Device UUIDs

## Testing

```bash
cargo test -p sotf-cast --lib
cargo check -p sotf-cast && cargo clippy -p sotf-cast
```

## License

See the root workspace `LICENSE` file.
