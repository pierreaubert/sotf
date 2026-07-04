# sotf-cast

AirPlay and Chromecast audio casting for SOTF.

## Components

- `CastDiscovery` -- Network discovery for cast-capable devices
- `CastDevice` / `CastDeviceType` -- Device representation (AirPlay, Chromecast)

### AirPlay (behind `airplay` feature)

- `airplay.rs` -- AirPlay RAOP v1 audio streaming

### Chromecast (behind `chromecast` feature)

- `chromecast.rs` -- Google Cast CASTV2 audio streaming

## Features

| Feature | Description | Default |
|---------|-------------|---------|
| `airplay` | AirPlay RAOP v1 audio casting | Yes |
| `chromecast` | Google Cast CASTV2 audio casting | Yes |

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
