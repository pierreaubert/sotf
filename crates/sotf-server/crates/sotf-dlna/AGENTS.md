# sotf-dlna

DLNA/UPnP MediaRenderer and MediaServer.

## Key Types

- `DlnaDiscovery` / `DiscoveredDevice` -- SSDP-based device discovery
- `DlnaRenderer` -- UPnP AVTransport MediaRenderer
- `DlnaMediaServer` -- UPnP ContentDirectory MediaServer
- `DlnaDevice` / `DlnaDeviceType` -- Device representation
- `RendererAdapter` / `MediaServerAdapter` -- Traits for player/library integration
- `TransportState` -- Playback transport state

## Module Layout

- `ssdp.rs` -- SSDP multicast discovery
- `xml.rs` -- UPnP XML generation
- `didl.rs` -- DIDL-Lite metadata formatting
- `device.rs` -- Device types
- `renderer.rs` -- MediaRenderer implementation
- `server.rs` -- MediaServer implementation
- `discovery.rs` -- Device discovery

## Testing

```bash
cargo test -p sotf-dlna --lib
cargo check -p sotf-dlna && cargo clippy -p sotf-dlna
```
