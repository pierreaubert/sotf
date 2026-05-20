# sotf-iamf (lib: `sotf_iamf`)

Pure Rust IAMF (Immersive Audio Model and Formats) decoder implementing IAMF v1.1.0 bitstream parsing and rendering.

## Overview

Decodes IAMF bitstreams and renders to target speaker layouts. No C/C++ dependencies — reuses SOTF's Ambisonics decoder and speaker configs from `sotf-host`.

## Features

- **IAMF v1.1.0** bitstream parsing (OBU-based)
- **Codec support**: Opus, AAC, FLAC, PCM substreams
- **Ambisonics rendering** via `sotf-plugin-ambisonics`
- **Speaker layout rendering** to standard configurations
- **Zero-allocation decode path**: All intermediate buffers pre-allocated during `open()`

## Module Layout

- `codec/` — Audio codec implementations (Opus, AAC, FLAC, PCM)
- `obu/` — OBU (Open Bitstream Unit) parsing
- `renderer/` — Element rendering to target speaker layouts
- `mixer.rs` — Mix state for combining audio elements
- `types.rs` — IAMF bitstream types (descriptors, parameters, elements)
- `error.rs` — Error types (`IamfError`, `IamfResult`)

## Key Types

- `IamfDecoder` — Main decoder. Pre-allocates all buffers during `open()` to avoid heap allocations in the decode hot path.

## Usage

```rust
use sotf_iamf::IamfDecoder;
use std::fs;

let data = fs::read("audio.iamf")?;
let mut decoder = IamfDecoder::open(&data)?;
let frames = decoder.decode_next()?;
```

## Dependencies

- `sotf-host` — Speaker configurations and plugin infrastructure
- `sotf-plugin-ambisonics` — Ambisonics rendering

## Features

This crate has no feature flags. It is enabled in `sotf-engine` via the `iamf` feature.

## Testing

```bash
cargo test -p sotf-iamf --lib
cargo check -p sotf-iamf && cargo clippy -p sotf-iamf
```

## License

See the root workspace `LICENSE` file.
