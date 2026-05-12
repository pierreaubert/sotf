# 0.5.4

## Fixes (from code review)

- **fix: no-alloc dual-band scratch buffers** (`src/lib.rs`): `initialize()` now
  pre-allocates `lf_buffer` / `hf_buffer` to `MAX_BLOCK_FRAMES (8192) ×
  MAX_AMBI_CHANNELS (16)` so that the audio-thread hot path in `process()`
  never calls `Vec::resize()`.  The old code pre-allocated for 4096 frames but
  silently fell back to a heap allocation in `process()` for any larger block.
  The in-callback `resize()` calls are replaced by `debug_assert!` guards that
  catch oversized blocks early in debug builds.
  Regression test: `test_dual_band_large_block_no_alloc` (5000-frame block).

- **fix: `acn_to_degree_index` bounds guard** (`src/spherical_harmonics.rs`):
  Added `debug_assert!(acn <= channel_count(MAX_ORDER))`.  The
  floating-point `sqrt` truncation is correct for `acn ≤ 15` (MAX_ORDER = 3)
  but would silently produce wrong degree values for `acn ≥ 48`; the assert
  ensures a future increase to MAX_ORDER fails fast rather than producing
  incorrect harmonics.
  New test: `test_acn_to_degree_index_all_valid` verifies round-trip and range
  for every valid ACN index.

## Deferred / not applicable

- **`spherical_harmonics_vector` slice output** (review §6): Called only on
  the control thread during matrix construction; not a real-time concern.
  Would require a cross-crate API change. Deferred.

- **`take()` / `Lr4Crossover` refactor** (review §9): The reviewer notes the
  pattern is "safe" with no current `?` operators between `take()` and
  `restore`.  The suggested fix requires refactoring `Lr4Crossover` in
  `sotf-host`, which is out of scope for this crate. Deferred.

- **`decode_frame` vectorization** (review §8): Speculative; LLVM may already
  unroll the 16-iteration inner loop.  Profiling required before pursuing SIMD
  intrinsics. Deferred.

# 0.5.3

## Changes

- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
- Cleanup: another round of clippy
- Plugins implemented f2,3 7,8,9,10,11,12 and 13 see product features for details
