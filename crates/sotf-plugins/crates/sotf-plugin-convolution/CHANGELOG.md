# 0.5.7

## Fixes

- **[CRITICAL] UPC output ring buffer** (`src/lib.rs`): Replaced broken
  in-place output mapping with a per-sample drain loop and a dedicated
  `output_ring` buffer.  The old code wrote only the last `to_copy` samples of
  each PARTITION_SIZE-block back to the buffer, silently dropping the first
  `PARTITION_SIZE - to_copy` samples when host buffer size was not a multiple
  of `PARTITION_SIZE` (i.e. any buffer size < 1024 or not aligned to 1024,
  which covers most DAWs delivering 32–512 sample frames).  The fix introduces
  exact one-partition (1024-sample) latency on the UPC path, consistent with
  the algorithm.  Added regression tests `test_partial_block_no_output_drop`
  and `test_partial_block_energy_preserved` (review issue #1).

- **Incomplete `reset()`** (`src/lib.rs`): `reset()` now clears
  `input_buffers`, `input_fill`, `output_accum`, the new `output_ring`
  state, all NUPC engines, and resets the mix/gain smoothers to their
  instantaneous values.  Previously only the FDL and fdl_head were cleared,
  leaving stale state on replay.  Added regression test
  `test_reset_clears_all_state` (review issue #2).

- **Parameter smoothing zipper noise** (`src/lib.rs`): NUPC path now calls
  `advance()` once per sample instead of `next_n(nf)` once per buffer (which
  discarded all intermediate values).  UPC path now linearly interpolates
  mix/gain across each partition block using start/end smoother values instead
  of applying a single block-quantized scalar (~21 ms steps).  Both changes
  remove audible zipper noise during automation (review issue #3).

- **Missing `enable_ftz_daz()`** (`src/lib.rs`): Added call at the top of
  `process_in_place`.  Without FTZ/DAZ, denormal values from FFT multiply-adds
  can cause 10–100× CPU spikes.  Matches the pattern used by binaural,
  de-esser, and gate plugins (review issue #6).

## Deferred

- **Issue #4 (IR loading on audio thread)**: Moving `load_ir` to a background
  thread requires cross-crate infrastructure changes and is deferred.

- **Issue #5 (Rebuild IR on sample-rate change in `initialize()`)**: Depends
  on the background-thread mechanism from issue #4; deferred.

- **Issue #7 (Store original time-domain IR)**: Medium refactor; deferred.

- **Issue #8 (Misleading comment in `plugins-spatial/src/nupc.rs`)**: In
  `plugins-spatial` crate, outside scope of this crate's review.

- **Issues #10–15 (nits and advisory improvements)**: Skipped per review
  priority policy.

---

# 0.5.6

## New

- Added missing qa_*.rs files for some plugins
- Added examples: use pnd, mono2stereo and denoiser on old mono tracks
- Added missing parameters for new plugins

## Fixes

- Fixed again parameters for plugins. TODO: think about doing it the hard way with a trait per plugin
- **CRITICAL**: Fixed UPC path dropping output samples when host buffer sizes are not exact multiples of `PARTITION_SIZE` (1024). Output is now buffered in a per-channel ring queue so that all computed samples are emitted.
- **CRITICAL**: Fixed `reset()` not clearing NUPC engines, overlap accumulators, input buffers, or parameter smoothers, which caused state leakage across playback passes.
- **MAJOR**: Fixed broken parameter smoothing in NUPC path — `mix` and `gain` are now advanced per sample instead of held constant across the whole buffer.
- **MAJOR**: Fixed broken parameter smoothing in UPC path — `mix` and `gain` now use a linear ramp across each 1024-sample partition instead of a single scalar value.
- **MAJOR**: Moved IR loading out of the real-time audio thread. `set_parameter("ir_file", ...)` now spawns a background worker thread and swaps in the loaded state asynchronously via `process_in_place`, eliminating file I/O, FFT planning, and heap allocations from the audio callback.

## Changes

- SOTA plugin improvements: shared DSP components + plugin upgrades
- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
- Cleanup: another round of clippy
- Another round of parameters update
- Massive update to plugins, see individual markdown plan for details (wave 5)
