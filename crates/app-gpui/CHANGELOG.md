# 0.5.14

## Fixes

### Recording evaluation — magnitude plot was vertically flipped

- The "MAGNITUDE (dB)" chart in the recording evaluation screen
  (`components/recording/evaluating.rs::render_magnitude_chart`) was
  rendering every measured curve upside-down. A stray unary minus in
  the per-point normalization (`-(mag - normalization_offset)`) was
  flipping the sign of the offset-relative magnitude, so real room
  modes appeared as nulls and real cancellations appeared as peaks.
  The formula is now `mag - normalization_offset` and the chart
  matches both the raw `L.wav` / `R.wav` Welch PSD and the curves
  stored in `dsp.json` (which are also what `scripts/display-roomeq.py`
  has been displaying correctly all along).
- Phase, group-delay, distortion, RT60, clarity, impulse-response, and
  spectrogram charts were checked in the same pass and do *not* have
  the same bug — they use straight `mag - offset` or no normalization
  at all.
