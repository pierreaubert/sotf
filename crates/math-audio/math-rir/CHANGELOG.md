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
