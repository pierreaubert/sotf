# sotf-cast

AirPlay and Chromecast audio casting.

## Key Types

- `CastDiscovery` -- Network discovery for cast devices
- `CastDevice` / `CastDeviceType` -- Device representation

## Module Layout

- `discovery.rs` -- mDNS/Bonjour device discovery
- `airplay.rs` -- AirPlay 2 streaming (behind `airplay` feature)
- `chromecast.rs` -- Google Cast streaming (behind `chromecast` feature)

## Features

- `airplay` (default) -- AirPlay 2 audio casting
- `chromecast` (default) -- Google Cast audio casting

## Testing

```bash
cargo test -p sotf-cast --lib
cargo check -p sotf-cast && cargo clippy -p sotf-cast
```
