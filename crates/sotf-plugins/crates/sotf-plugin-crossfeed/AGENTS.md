# sotf-plugin-crossfeed

Stereo crossfeed for headphones — simulates speaker-like stereo.

## Architecture

- `lib.rs` — Main plugin struct, implements `InPlacePlugin` trait
- `params.rs` — Parameter definitions and JSON deserialization


## Key Public API

- Main plugin struct implementing `sotf_host::plugin::InPlacePlugin`
- Plugin parameters via `params.rs`

## Testing

```bash
cargo test -p sotf-plugin-crossfeed
```

## Important Notes

- Stereo only (2 channels)
- Blends left/right channels with frequency-dependent filtering to reduce the exaggerated stereo separation of headphones
