# plugins-spatial

Shared spatial DSP helpers used by spatial-audio SOTF plugins (upmixer, downmix, ambisonics, binaural, beamformer).

Modules:
- `lib.rs` — interleaved-buffer validation helpers (`validate_interleaved_io`, `InterleavedBufferSizes`).
- `nupc.rs` — non-uniform partitioned convolution primitives.

Pure DSP only — host integration lives in the per-plugin crates.
