# sotf-dlna

DLNA/UPnP MediaRenderer and MediaServer for SOTF.

## Components

- `DlnaDiscovery` -- SSDP-based device discovery on the local network
- `DlnaRenderer` -- UPnP AVTransport MediaRenderer (receive audio from DLNA controllers)
- `DlnaMediaServer` -- UPnP ContentDirectory MediaServer (serve library to DLNA clients)
- `DlnaDevice` / `DlnaDeviceType` -- Device representation
- `RendererAdapter` / `MediaServerAdapter` -- Traits for connecting to player/library backends
- `TransportState` -- Playback transport state

### Internal Modules

- `ssdp.rs` -- SSDP multicast discovery
- `xml.rs` -- UPnP XML description/response generation
- `didl.rs` -- DIDL-Lite metadata formatting
- `device.rs` -- Device information types

## Dependencies

- `tokio` -- Async networking
- `uuid` -- Device UUIDs

## Testing

```bash
cargo test -p sotf-dlna --lib
cargo check -p sotf-dlna && cargo clippy -p sotf-dlna
```

## License

See the root workspace `LICENSE` file.
