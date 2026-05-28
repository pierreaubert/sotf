# 0.5.23

## Fixes

- Removed the redundant post-shift zero-fill of STFT input-buffer tail samples. Those slots are
  overwritten before becoming active input, so the fill only added unnecessary writes per hop.

# 0.5.22

## Bug fixes (from code review)

- **Fixed 6 dB magnitude calibration error** (`lib.rs:388`): The periodic Hann analysis
  window has a coherent gain of 0.5, so interior FFT bins (k≠0 and k≠Nyquist) needed
  the magnitude scale compensated by ×2 (`4/N` instead of `2/N`). DC and Nyquist bins
  are unaffected. Before this fix a -20 dBFS sine measured as -26 dB inside the
  compressor, meaning thresholds were systematically off by ~6 dB for all tonal content.

- **Fixed tonal/transient mask applied after envelope smoother** (`lib.rs:423-452`): The
  tonal/transient mask was previously multiplied onto `bin_envelopes` after smoothing,
  allowing the envelope follower to track energy in bins that were subsequently zeroed
  out. The mask is now applied to `target_gr` before the one-pole smoother, so the
  envelope only responds to the component the user actually wants to compress. The
  `TonalTransientSeparator::process()` call now updates masks at the end of each hop
  for use in the next hop (correct one-hop lag; computing and using masks in the same
  pass would require two per-bin iterations).

- **Fixed latency over-reporting** (`lib.rs:793`): `latency_samples()` now returns
  `fft_size - hop_size` (the actual causal STFT delay) instead of `fft_size`. For
  N=2048, this corrects the reported latency from 2048 to 1536 samples, preventing
  hosts from over-compensating by ~10 ms.

- **Fixed attack/release zero-guard missing in constructor** (`lib.rs:311-312`): Added
  the same `<= 0.0` guards that `recompute_coefficients()` had, preventing a
  `exp(+inf) = inf` coefficient if a negative value were passed via direct struct
  construction.

- **Fixed cache-unfriendly input copy** (`lib.rs:833-837`): Swapped the channel/frame
  loop order so the interleaved source buffer is read contiguously (frame-major outer
  loop), which is cache-friendly for any channel count.

- **Removed redundant `windowed_buf` copy** (`lib.rs:396-402`): The windowed samples
  are now written directly into `fft_processors[ch].time_buffer`, eliminating an
  `fft_size`-element copy per channel per hop. `windowed_buf` field removed from
  `StftState`.

## New tests

- `test_fft_roundtrip_no_compression_below_threshold` — ratio=1.0 identity: output RMS
  must match input RMS within 5%, catches magnitude calibration errors.
- `test_magnitude_calibration_6db_hann_fix` — sine at -20 dBFS with threshold=-25 dB
  must be compressed; before the fix it was not (measured as -26 dB).
- `test_zero_attack_release_coefficients` — `attack_ms=0`/`release_ms=0` constructor
  must produce finite coefficients (0.0 = instant response).
- `test_stereo_independence` — L/R channels with different frequencies are each
  independently compressed.

## Deferred (cross-crate or low-priority)

- **Boundary asymmetry in spectral smoothing** (medium, `lib.rs:499-511`): the forward
  pass skips bin 0 and the backward pass skips the last bin. Fix is a one-line change
  per pass (start from k=0 in both directions). Deferred because it requires careful
  re-verification against the COLA identity and the impact is subtle.
- **Block-constant mix/threshold parameters** (medium): interpolating `mix` across the
  buffer requires per-sample smoother polling; deferred as it needs the `Smoother` API
  to support per-sample `next()` calls without advancing by a full block.
- **`fast_median` O(n²) sorting network** (medium): lives in `math-dsp` crate, out of
  scope for this crate.
- **No-SIMD vectorization**: improvement, not a bug; deferred.
- **`cached_parameters` rebuilt on every set_parameter**: minor allocation concern, not
  a correctness bug; deferred.

# 0.5.21

- Removed the hot-path dry_buffer.resize() path entirely by mixing/delta-monitoring against the original sample right before overwrite.
- Added strict buffer-size validation so malformed host buffers return Err instead of panicking.
- Advanced threshold smoothing per STFT hop instead of jumping to the block-end value before the first hop in large blocks.
- Marked FFT size as structural/setup because changing it rebuilds the STFT state.
- Added regressions for buffer mismatch and mix=0 passthrough during latency fill.
