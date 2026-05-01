# plugins-spatial

Shared spatial DSP helpers used by spatial-audio SOTF plugins (upmixer, downmix, ambisonics, binaural, beamformer, …).

## Modules

- `lib.rs` — interleaved-buffer validation helpers (`validate_interleaved_io`, `InterleavedBufferSizes`, `checked_interleaved_samples`). Centralises plugin-side I/O size checks so each spatial plugin reports identical errors.
- `nupc.rs` — non-uniform partitioned convolution primitives shared by spatial reverb / convolution paths.

## Testing

```bash
cargo check -p plugins-spatial && cargo clippy -p plugins-spatial
cargo test -p plugins-spatial
```

## Important Notes

- Pure DSP: no `Plugin` trait impls — host integration lives in the per-plugin crates.
- All public functions are real-time-safe; do not introduce allocations or locks here.
- Keep dependency surface tight (just `sotf-host` + `rustfft`) so spatial plugins can pull this in without dragging the whole plugin tree.
