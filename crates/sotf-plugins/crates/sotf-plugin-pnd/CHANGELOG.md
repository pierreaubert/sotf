# 0.5.3

- Fixed `current_drift_estimate()` reading the circular drift-history buffer as
  if it were linear: after the buffer wrapped, stale values from the start of
  the array were used instead of the most-recent entries. The fix correctly
  unrolls the ring starting from `drift_write_pos`. (analysis.rs:~220)
- Fixed `latency_samples()` always returning `RESAMPLER_CHUNK_SIZE` (1024) even
  when the phase vocoder is active. It now returns `PV_FFT_SIZE + PV_HOP_SIZE`
  (2560) when `phase_vocoder` is true, so hosts can align the signal
  correctly. (lib.rs:~1022)
- Fixed `reset()` not flushing rubato's internal interpolation delay lines.
  `init_resampler()` is now called on reset to re-create the resampler from
  scratch, preventing clicks or phase discontinuities after a transport
  stop/start. (lib.rs:~1002)
- Fixed `process_phase_vocoder()` reading `correction_strength` directly instead
  of advancing `correction_strength_smoother`, which caused zipper noise on
  rapid parameter changes in the phase-vocoder path. (lib.rs:~481)
- Fixed phase-vocoder diagnostic cache zeroing `matched_partials` and
  `total_peaks` instead of forwarding the real analyzer values, making
  monitoring inconsistent between the two processing paths. (lib.rs:~519)
- Tightened `test_pnd_known_drift_correction`: removed the no-op `if` branch
  that let the test pass even when drift was not corrected; the test now
  asserts `output_error < input_error` unconditionally.
- Added `test_drift_history_wraps_correctly` to verify circular-buffer median
  correctness after wrap.
- Added `test_latency_samples_reports_pv_latency_when_vocoder_active`.
- Added `test_reset_reinitializes_resampler`.
- Added `test_pv_path_uses_correction_strength_smoother`.

## Deferred (noted for future work)

- Phase vocoder does not map energy to shifted bins (§3.1 in review): full
  architectural redesign required; deferred.
- Complex FFT on real data in phase vocoder (§3.2): requires integrating
  `realfft`; deferred.
- Phase locking for vocoder (§3.3): enhancement, not a bug; deferred.
- Block-size-dependent drift smoothing in vocoder path (§4.2): requires a
  sample counter; deferred.
- Phase vocoder arbitrary block size via ring buffer (§4.3): deferred.
- One-to-many partial matching in analyzer (§4.1): medium complexity; deferred.
- Performance improvements (§5.x: block-based push, per-sample loops, FFT plan
  sharing): deferred.
- Duplicate `PndPluginParams` / `Params` structs (§6.1): cross-crate refactor;
  deferred.

# 0.5.2

- Added input/output buffer validation in both resampler and phase-vocoder paths, so bad host buffers return Err instead of slicing/panicking.
- Added prepared-capacity checks for oversized blocks and output-ring overflow instead of relying on debug assertions or ring overruns.
- Removed phase-vocoder hot-path resize() by rejecting blocks larger than prepared capacity.
- Fixed PndAnalyzer::reset() leaving median_scratch length zero, which could panic after the next valid drift estimate.
- Preallocated analyzer peak/ratio scratch and max drift-history storage so analysis-window changes do not reallocate.
- Marked analysis-window, multi-channel analysis, and phase-vocoder controls as structural/setup.
- Fixed from_params() returning stale cached parameter values.
