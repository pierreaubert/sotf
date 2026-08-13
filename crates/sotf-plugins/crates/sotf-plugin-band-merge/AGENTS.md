# sotf-plugin-band-merge

Merge frequency bands back together.

## Architecture

- `lib.rs` — Main plugin struct, implements `Plugin` trait
- `params.rs` — Parameter definitions and JSON deserialization


## Key Public API

- Main plugin struct implementing `sotf_host::plugin::Plugin`
- Plugin parameters via `params.rs`

## Testing

```bash
cargo test -p sotf-plugin-band-merge
```

## Important Notes

- Companion to sotf-plugin-band-split — must be used in pairs
- Input channel count = base channels × number of bands; output = base channels
- Initialize before processing; callbacks must match the initialized sample rate
  and exact overflow-checked interleaved buffer lengths.
- `bands` is structural. Gain and mute use the same allocation-free 10 ms
  smoother and reset preserves the configured mute/gain target.
- `reconstruction_error_db` is an on-demand normalized RMS error metric; the
  armed callback path must never allocate or log.
