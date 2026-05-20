# sotf-streaming

HTTP streaming input for the SOTF audio engine.

## Overview

Provides `MediaSource` implementations for streaming audio over HTTP, enabling the SOTF engine to play internet radio, podcast feeds, and other HTTP-based audio sources.

## Components

- `HttpMediaSource` — Symphonia-compatible `MediaSource` for HTTP streams with byte-range seeking
- `IcyMetadata` — ICY (SHOUTcast/Icecast) metadata parsing for internet radio streams
- `HlsSource` — HLS (HTTP Live Streaming) source (behind `hls` feature)

## Features

| Feature | Description | Default |
|---------|-------------|---------|
| `hls` | HLS streaming support via `m3u8-rs` | No |

## Dependencies

- `symphonia-core` — `MediaSource` trait
- `reqwest` — HTTP client (blocking)
- `m3u8-rs` (optional) — HLS playlist parsing

## Testing

```bash
cargo test -p sotf-streaming --lib
cargo check -p sotf-streaming && cargo clippy -p sotf-streaming
```

## License

See the root workspace `LICENSE` file.
