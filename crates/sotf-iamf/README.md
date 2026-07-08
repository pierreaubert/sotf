# sotf-iamf (lib: `sotf_iamf`)

Pure Rust IAMF (Immersive Audio Model and Formats) decoder implementing IAMF v1.1.0 bitstream parsing and rendering.

> **Release support level: Experimental.** See [`RELEASE_SCOPE.md`](RELEASE_SCOPE.md) for the current completeness gap analysis. In short: descriptor/temporal-unit parsing and LPCM substream decoding are functional, but Opus/AAC/FLAC substream decoding is not yet implemented.

## Overview

Decodes IAMF bitstreams and renders to target speaker layouts. No C/C++ dependencies — reuses SOTF's Ambisonics decoder and speaker configs from `sotf-host`.

## Features

- **IAMF v1.1.0** bitstream parsing (OBU-based)
- **Codec support**: LPCM substreams natively; Opus, AAC, and FLAC parsing only (decode requires engine-level integration)
- **Ambisonics rendering** via `sotf-plugin-ambisonics`
- **Speaker layout rendering** to standard configurations
- **Zero-allocation decode path**: All intermediate buffers pre-allocated during `open()`
- **Bounded parsing**: leb128-derived counts are capped against remaining payload bytes and a hard 64 MiB ceiling

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
use std::io::Cursor;

let data = fs::read("audio.iamf")?;
let mut decoder = IamfDecoder::open(Cursor::new(&data))?;
let spec = decoder.spec().clone();
let mut output = vec![0.0_f32; spec.output_channels as usize * spec.num_samples_per_frame as usize];
let frames = decoder.decode_next(&mut output)?;
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
