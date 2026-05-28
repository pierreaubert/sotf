# 0.5.4

## Fixes (from code review)

- **docs: dual-band latency semantics** (`src/lib.rs`): `latency_samples()` now
  documents that dual-band LR4 processing has frequency-dependent group delay near
  the 700 Hz crossover but no fixed linear-phase delay to report to the host.
  Regression test: `test_dual_band_reports_no_fixed_host_latency`.

- **fix: no-alloc dual-band scratch buffers** (`src/lib.rs`): `initialize()` now
  pre-allocates `lf_buffer` / `hf_buffer` to `MAX_BLOCK_FRAMES (8192) ×
  MAX_AMBI_CHANNELS (16)` so that the audio-thread hot path in `process()`
  never calls `Vec::resize()`.  The old code pre-allocated for 4096 frames but
  silently fell back to a heap allocation in `process()` for any larger block.
  The in-callback `resize()` calls are replaced by `debug_assert!` guards that
  catch oversized blocks early in debug builds.
  Regression test: `test_dual_band_large_block_no_alloc` (5000-frame block).

- **fix: per-speaker harmonic buffer reuse** (`src/spherical_harmonics.rs`, `src/decode_matrix.rs`):
  `spherical_harmonics_vector` now takes a mutable output slice (`&mut [f64]`) and
  writes in place. `DecodeMatrix::build` uses a reusable scratch buffer to populate
  each speaker's SH row, removing per-speaker temporary allocation during matrix
  build.
  Unit tests in `src/spherical_harmonics.rs` still cover first/second-order values
  and ACN ordering.

- **fix: improve `decode_frame` loop structure** (`src/decode_matrix.rs`):
  replaced iterator `take()` loops with direct row/input slice access and indexed
  accumulation in the small fixed-size dot product. This gives LLVM a simpler loop
  shape and avoids extra iterator overhead in the decode inner loop.

- **fix: remove crossover move in dual-band process** (`src/lib.rs`):
  dual-band processing now uses `self.crossover.as_mut()` directly instead of
  `take()` + restore. Crossover state remains in place for the call and no longer
  risks becoming `None` via panic during processing.

- **fix: `acn_to_degree_index` bounds guard** (`src/spherical_harmonics.rs`):
  Added `debug_assert!(acn <= channel_count(MAX_ORDER))`.  The
  floating-point `sqrt` truncation is correct for `acn ≤ 15` (MAX_ORDER = 3)
  but would silently produce wrong degree values for `acn ≥ 48`; the assert
  ensures a future increase to MAX_ORDER fails fast rather than producing
  incorrect harmonics.
  New test: `test_acn_to_degree_index_all_valid` verifies round-trip and range
  for every valid ACN index.

# 0.5.3

## Changes

- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
- Cleanup: another round of clippy
- Plugins implemented f2,3 7,8,9,10,11,12 and 13 see product features for details
