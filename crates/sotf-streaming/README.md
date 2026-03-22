# sotf-streaming

HTTP streaming input for the SOTF audio engine. Provides `MediaSource` implementations for streaming audio over HTTP.

## Components

- `HttpMediaSource` -- Symphonia-compatible `MediaSource` for HTTP streams with byte-range seeking
- `IcyMetadata` -- ICY (SHOUTcast/Icecast) metadata parsing for internet radio streams
- `HlsSource` -- HLS (HTTP Live Streaming) source (behind `hls` feature)

## Features

| Feature | Description | Default |
|---------|-------------|---------|
| `hls` | HLS streaming support via `m3u8-rs` | No |

## Dependencies

- `symphonia-core` -- `MediaSource` trait
- `reqwest` -- HTTP client (blocking)
- `m3u8-rs` -- HLS playlist parsing (optional)

## Testing

```bash
cargo test -p sotf-streaming --lib
cargo check -p sotf-streaming && cargo clippy -p sotf-streaming
```

## License

See the root workspace `LICENSE` file.
