# 0.5.5

## New features

- **ISO 3382 room-acoustic metrics** (`metrics.rs`). Computes reverberation
  time (EDT, T20, T30), clarity (C50, C80), definition (D50), and centre
  time (Ts) from a measured RIR. Each fitted reverberation time carries an
  `r²` so callers can flag non-linear decays (coupled rooms, noise-dominated
  tails). Time integration is anchored at the detected direct-sound arrival
  to keep pre-trigger silence out of the result.
  - `analyze_iso3382(rir, sample_rate) -> Iso3382Metrics`
  - `DecayCurve::from_rir(rir, sr, start, noise_cutoff)` — Schroeder
    backward integration in dB, with Chu's two-pass noise-floor truncation
    to prevent integrated-noise lift.
  - `Iso3382Metrics::fit_is_valid()` checks the conventional ISO 3382-1
    Annex B threshold (`r² ≥ 0.95`).
- **Octave-band and third-octave-band analysis** (`bands.rs`). ISO/IEC 61260
  centre frequencies plus zero-phase Butterworth bandpasses (HP ∘ LP cascade
  through `filtfilt`), parallelised per band via rayon.
  - `ISO_OCTAVE_CENTERS_HZ` (63 Hz … 8 kHz), `ISO_THIRD_OCTAVE_CENTERS_HZ`
    (100 Hz … 10 kHz).
  - `BandWidth::{Octave, ThirdOctave}` with log-symmetric edges.
  - `bandpass(rir, fc, width, sr, order)` — zero-phase Butterworth bandpass.
  - `analyze_iso3382_bands(...)`, `analyze_iso3382_octaves(...)`,
    `analyze_iso3382_third_octaves(...)` — per-band metrics in parallel.

## Bug fixes

- **`detection::find_direct_sound_toa`**: peak-suppression order corrected.
  Previously returned the *first* peak above `−11 dB` in time order, which
  let an early pre-blip (e.g. measurement noise) shadow the actual
  direct-sound peak. Now returns the **earliest peak within a 1 dB tie
  band of the global maximum**, equivalent to "strongest peak" when peaks
  are well separated and to "earliest arrival of the dominant cluster"
  otherwise.
- **`compute_bformat_doa` (`lib.rs`)**: documented the `+I/|I|` sign
  convention used for first-order Ambisonics pseudo-intensity DOA and
  added plane-wave test fixtures (`+X`, `+Y`) that verify it.

## Performance

- **`compute_bformat_doa` (`lib.rs`)**: cut allocation count from ~8 large
  `Vec`s per call to 0 in the no-filter branch and 2 per channel in the
  filter branch. The no-filter branch now borrows the caller's input
  slices directly; the filter branch shares one `f64` scratch + one `f64`
  filtfilt output per channel before materialising the owned `f32` vector.

## Tests

- Added 10 ISO 3382 unit tests: linear-fit primitive, Schroeder
  monotonicity, T20/T30/EDT against synthetic exponential decay, anechoic
  D50/C50/C80, closed-form clarity on a uniform-energy RIR, empty-input
  handling, `fit_is_valid` threshold.
- Added 5 band-filter tests: log-symmetric edges, DC suppression, in-band
  pass-through, out-of-band rejection (> 40 dB), octave dispatch smoke test.
- Added 4 review-driven tests: plane-wave B-format DOA from `+X` and `+Y`;
  `find_direct_sound_toa` tie-band behaviour on two synthetic peak fixtures.
- All 43 lib tests + 1 doctest pass; clippy clean with `-D warnings`.

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
