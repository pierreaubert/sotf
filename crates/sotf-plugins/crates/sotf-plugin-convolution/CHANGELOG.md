# 0.5.11

## Fixes

- Async IR requests now carry generations and expose `ConvolutionLoadStatus`; stale completions,
  clears, rate reloads, and failed replacements cannot replace the last-known-good response.
- IR installs use prebuilt `Arc` state, and replaced FFT/NUPC buffers plus failure strings are
  retired on background threads. Installation and reclamation are allocation-free in callbacks.
- Empty, loading, failed, and cleared states now delay dry audio by the advertised backend latency.
  Active replacement and clear boundaries use a 128-sample bounded discontinuity transition.
- UPC captures mix/gain smoothing once per input sample and carries that exact envelope with the
  delayed output partition, eliminating retroactive and partition-boundary automation errors.
- NUPC no longer builds an unused UPC backend. Output channels mapped to the same IR channel share
  immutable NUPC spectra, FFT plans, and direct-head taps while retaining independent history.
- IR loading rejects missing sample rates, empty channels, more than 32 channels, responses longer
  than 30 seconds, and backend estimates above 512 MiB before FFT planning.
- Construction now rejects zero channels/rate, non-finite or out-of-range mix/gain, and invalid head
  sizes instead of silently clamping serialized configuration.
- QA and fuzzing now load real deterministic/random IR files and exercise active UPC/NUPC paths.

## Tests

- Added generation/failure preservation, load status, inactive latency, callback install retirement,
  bounded transition, exact UPC automation, active-backend deduplication, metadata/resource-limit,
  and construction-contract regressions.

# 0.5.10

## Fixes

- Compensated Rubato's FFT resampler startup delay and flushed its filter tail
  through the whole-clip API before trimming to the rounded target length. IR
  impulses now retain their absolute timing and final response energy when
  converting between 44.1, 48, and 96 kHz sample rates.
- Added cross-rate start- and tail-impulse regression coverage for all supported
  rate pairs.

# 0.5.9

## Fixes

- Corrected NUPC scheduling so every partition level retains its absolute IR
  offset, including tails following a zero-latency time-domain head. Added
  direct-convolution oracle tests across sparse partition boundaries.
- Activated the previously dormant 724-line DSP regression suite.
- Rebuilt host-visible parameters after construction and successful IR loads,
  initialized smoothers at configured values, clamped construction inputs, and
  aligned direct construction with the declared NUPC default.
- Cancelled pending async loads on clear, preserved the last-known-good IR after
  replacement failures, and reset all UPC/NUPC streaming state on installation.
- Replaced Rayon dispatch from the audio callback with a deterministic
  single-threaded SIMD partition accumulation loop.
- Switched compile metadata to a conservative processing boundary because
  smoothed parameters and IR swaps are time varying.

# 0.5.8

## Fixes

- **[11] Resampler chunk allocation cleanup** (`src/lib.rs`): `resample_ir`
  now reuses pre-allocated input/output chunk buffers instead of allocating
  `to_vec()` chunks for every channel on every resampler iteration.

- **[9] End-to-end NUPC IR loading coverage** (`src/lib.rs`): Added a regression
  test that writes a WAV IR, loads it through `from_params` with `use_nupc=true`,
  verifies NUPC engines are built, and processes a block successfully.

- **[10] Right-sized Rayon accumulator pool** (`src/lib.rs`): Capped the
  per-thread accumulator pool by `num_partitions`, avoiding oversized zeroing and
  merge work when the Rayon thread count exceeds the partition count.

- **[12] `head_taps` setter clamps before casting** (`src/lib.rs`, `src/params.rs`):
  `head_taps` is consistently `usize`; incoming `f64` values are clamped to the
  PARAMS range before conversion. Added a regression test for low/high clamps.

- **[13] UPC stereo IR mapping cycles channels** (`src/lib.rs`): Added a
  regression test proving UPC maps a stereo IR across multichannel output as
  L/R/L/R instead of clamping all extra channels to the final IR channel.

- **[15] Removed aggressive final denormal threshold flush** (`src/lib.rs`):
  The callback still enables FTZ/DAZ, but no longer calls
  `flush_denormals_inplace` with the shared `1e-30` threshold. Added a test that
  preserves tiny normal samples below that old cleanup threshold.

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

- **Issue #14 (automatic IR gain normalization)**: Optional product behavior;
  deferred until the host/UI has a clear opt-in control.

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
