# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.6] - 2026-05-30

### Added
- Added `SsirResult::direct_sound_doa()` for callers that need the detected
  direct-sound direction without indexing the first segment.
- Added `schroeder_curve()` as a public direct-sound-anchored convenience
  wrapper around `DecayCurve::from_rir`.

### Fixed
- `detection::find_direct_sound_toa` now follows the SSIR 11 dB rule using
  unsuppressed local maxima in time order, so a stronger nearby reflection can
  no longer hide the earlier direct arrival through min-distance suppression.
- Short-RIR direct-sound fallback now prefers the earliest sample when absolute
  peak magnitudes tie.
- Plateau local maxima now report the first plateau sample, avoiding a
  one-sample-late bias on broadened arrivals.
- LER reflection detection now excludes the direct-sound window from the local
  median and can emit multiple local maxima from the same analysis window,
  leaving TOA/DOA validation to merge only non-distinct candidates.
- Reflection merging now re-checks the previous neighbor after replacing an
  event with a higher-energy later event, removing order-dependent leftovers.

### Changed
- Clarified LER window behavior, the peak-relative onset heuristic, and the
  Abel-Huang echo-density mixing-time documentation.

### Tests
- Added regression coverage for direct-sound min-distance suppression,
  short-RIR ties, plateau peaks, same-window reflections, merge re-anchoring,
  direct-sound DOA access, and the public Schroeder curve helper.

## [0.5.5] - 2025-05-13

### Added
- **ISO 3382 room-acoustic metrics** (`metrics.rs`). Computes reverberation
  time (EDT, T20, T30), clarity (C50, C80), definition (D50), and centre
  time (Ts) from a measured RIR. Each fitted reverberation time carries an
  `r²` so callers can flag non-linear decays (coupled rooms, noise-dominated
  tails). Time integration is anchored at the detected direct-sound arrival
  to keep pre-trigger silence out of the result.
- **Octave-band and third-octave-band analysis** (`bands.rs`). ISO/IEC 61260
  centre frequencies plus zero-phase Butterworth bandpasses (HP ∘ LP cascade
  through `filtfilt`), parallelised per band via rayon.

### Fixed
- `detection::find_direct_sound_toa`: peak-suppression order corrected.
  Previously returned the *first* peak above `−11 dB` in time order, which
  let an early pre-blip shadow the actual direct-sound peak. Now returns the
  **earliest peak within a 1 dB tie band of the global maximum**.
- `compute_bformat_doa`: documented the `+I/|I|` sign convention used for
  first-order Ambisonics pseudo-intensity DOA and added plane-wave test
  fixtures (`+X`, `+Y`) that verify it.

### Performance
- `compute_bformat_doa`: cut allocation count from ~8 large `Vec`s per call
  to 0 in the no-filter branch and 2 per channel in the filter branch.

### Tests
- Added 10 ISO 3382 unit tests: linear-fit primitive, Schroeder
  monotonicity, T20/T30/EDT against synthetic exponential decay, anechoic
  D50/C50/C80, closed-form clarity on a uniform-energy RIR, empty-input
  handling, `fit_is_valid` threshold.
- Added 5 band-filter tests: log-symmetric edges, DC suppression, in-band
  pass-through, out-of-band rejection (> 40 dB), octave dispatch smoke test.
- Added 4 review-driven tests: plane-wave B-format DOA from `+X` and `+Y`;
  `find_direct_sound_toa` tie-band behaviour on two synthetic peak fixtures.
- All 43 lib tests + 1 doctest pass; clippy clean with `-D warnings`.

## [0.5.4] - 2025-05-13

### Fixed
- `segmentation::build_segments`: Fixed short-segment merge logic. The condition `i > 1`
  incorrectly exempted the first reflection from merging; changed to `i > 0` so that only
  the direct sound is always kept, matching the documented behavior.
- `detection::find_direct_sound_toa`: Signals with fewer than 3 samples now fall back
  to the global maximum instead of returning `None`, allowing very short RIRs to be analyzed.
- `detection::median_of`: NaN values no longer corrupt the median. Non-finite values
  are partitioned out before sorting; if no finite values remain, `NaN` is returned.

### Tests
- Added `test_first_reflection_merged_when_too_short` (segmentation).
- Added `test_find_direct_sound_toa_short_rir` (detection).
- Added `test_median_of_with_nan` (detection).
- All 27 lib tests + 1 doctest pass; clippy clean.

## [0.5.3] - 2025-05-13

### Performance
- Local Energy Ratio windows now run in parallel.
- Echo-density windows are computed in parallel.
- Reflection onset refinement is parallelized.
- SRIR B-format channel filtering and DOA vector generation now use Rayon.
- Added Rayon dependency.

## [0.5.2] - 2025-05-13

### Fixed
- Bumped math crates to 0.5: iir-fir now also works with f32, rir is band limited and linear phase.
- Moved many functions from sotf-host to math-dsp and math-iir-fir.
