# 0.5.4

## Bug fixes

- **`segmentation::build_segments`**: Fixed short-segment merge logic. The condition `i > 1`
  incorrectly exempted the first reflection from merging; changed to `i > 0` so that only
  the direct sound is always kept, matching the documented behavior.
- **`detection::find_direct_sound_toa`**: Signals with fewer than 3 samples have no local
  maxima in the traditional sense. Now falls back to the global maximum instead of
  returning `None`, allowing very short RIRs to be analyzed.
- **`detection::median_of`**: NaN values no longer corrupt the median. Non-finite values
  are partitioned out before sorting; if no finite values remain, `NaN` is returned.

## Tests

- Added `test_first_reflection_merged_when_too_short` (segmentation).
- Added `test_find_direct_sound_toa_short_rir` (detection).
- Added `test_median_of_with_nan` (detection).
- All 27 lib tests + 1 doctest pass; clippy clean.

# 0.5.3

## Performance

- crates/math-audio/math-rir/src/detection.rs:84: Local Energy Ratio windows now run in
  parallel, then detections are sorted and merged as before.
- crates/math-audio/math-rir/src/mixing_time.rs:61: echo-density windows are computed in
  parallel, with the consecutive-threshold scan kept ordered.
- crates/math-audio/math-rir/src/segmentation.rs:49: reflection onset refinement is
  parallelized before the ordered short-segment merge.
- crates/math-audio/math-rir/src/lib.rs:226: SRIR B-format channel filtering and DOA
  vector generation now use Rayon.
- Added Rayon to crates/math-audio/math-rir/Cargo.toml, updating Cargo.lock.

# 0.5.2

## Bug fixes

- Bumped math crates to 0.5: iir-fir now also work with f32, rir is band limited and linear phase
- Move many functions from sotf-host to math-dsp and math-iir-fir
