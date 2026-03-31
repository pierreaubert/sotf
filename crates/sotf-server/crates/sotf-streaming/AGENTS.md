# sotf-streaming

HTTP streaming input for the audio engine.

## Key Types

- `HttpMediaSource` -- Symphonia-compatible `MediaSource` for HTTP streams with byte-range seeking
- `IcyMetadata` -- ICY (SHOUTcast/Icecast) metadata parsing
- `HlsSource` -- HLS streaming (behind `hls` feature)

## Module Layout

- `http_source.rs` -- HTTP MediaSource implementation
- `icy.rs` -- ICY metadata parser

## Features

- `hls` -- HLS streaming support via `m3u8-rs`

## Testing

```bash
cargo test -p sotf-streaming --lib
cargo check -p sotf-streaming && cargo clippy -p sotf-streaming
```
