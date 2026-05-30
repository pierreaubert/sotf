# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.20] - 2026-05-30

### Fixed
- `adaa`: ADAA2 near-coincident fallback now evaluates the published
  three-sample centroid instead of a middle-sample-biased weighted average.
- `adaa`: `dilog_neg` now uses the exact `Li_2(-1)` value near `z = 1`,
  avoiding slow alternating-series convergence at the worst-conditioned point.
- `dynamics_core`: expand-mode gate hold samples are cached when hold time or
  sample rate changes, removing the per-sample hold-time multiply from the hot
  path.
- `ebur128`: true-peak mode now documents the BS.1770-4 48 kHz FIR-table
  assumption and logs a warning for non-48 kHz meters while preserving
  native-rate analysis.

### Changed
- `stft`: removed unused `DualWindowStft` COLA state and added explicit
  coverage for the current analysis-window fill latency contract.
- `rtpghi` and `simd`: clarified zero-allocation RTPGHI scratch ordering,
  compile-time SIMD feature selection, and scoped FTZ/DAZ usage.

## [0.5.19] - 2025-05-13

### Added
- Added `binaural_loudness` module: streaming binaural-loudness meter
  (`BinauralLoudness`) applying ITU-R BS.1770-4 K-weighting and gated
  integration to a 2-channel ear-signal pair. Provides momentary,
  short-term, and integrated LUFS; cumulative sample peak and true peak
  per ear; interleaved or separate L/R input; reset; and a
  `BinauralLoudnessResult` snapshot. One-shot helper `measure_binaural`
  for offline analysis.
- Added surround → binaural downmix path: `BinauralDownmix` carries a
  per-channel `[L_ear, R_ear]` linear gain matrix; preset constructors
  `BinauralDownmix::bs775(SurroundLayout::{FiveZero, FiveOne, SevenOne})`
  implement ITU-R BS.775 stereo-downmix coefficients (centre / surrounds
  at −3 dB, LFE excluded per BS.1770-4). `BinauralLoudness::add_surround_f32`
  and `measure_binaural_from_surround` feed multichannel programmes
  through the matrix into the binaural meter.

## [0.5.18] - 2025-05-13

### Fixed
- `analysis::compute_rt60_broadband` now uses Schroeder backward integration
  with least-squares T30/T20 extrapolation and fit-quality rejection instead
  of first-crossing timing. This makes octave-band RT60 estimates less prone
  to inflated values from noisy or flattened decay tails.
- `analysis::compute_rt60_spectrum` now trims late steady-state noise on each
  band-filtered impulse before fitting RT60 and logs the selected fit method,
  `r²`, and fit window for easier diagnosis.

## [0.5.17] - 2025-05-13

### Fixed
- `ebur128`: `gating_blocks` changed from `Vec` to `VecDeque` to eliminate
  O(n) `remove(0)` shifts on the audio hot path once the 1-hour cap is
  reached (#1).
- `instantaneous_frequency`: phase unwrapping now uses `rem_euclid` instead
  of `%` for robust wrap-to-π behavior with negative differences (#2).
- `audio_features::utils::geometric_mean` now asserts that the input length
  is a multiple of 8. Previously `chunks_exact(8)` silently dropped the
  remainder, producing wrong results for non-multiple-of-8 slices (#3).
- `audio_features::spectral`: spectral flatness no longer hardcodes 256
  bins; it uses the largest multiple of 8 `<= norms.len()` (#4).
- `audio_features::chroma`: `pip_track` now returns empty pitch/mag vectors
  instead of erroring when the frequency mask is empty (#5).
- `analysis::compute_thd_from_ir`: harmonic extraction window minimum is now
  frequency-dependent instead of a fixed 256 samples (#6).
- `analysis::compute_coherence_from_realizations`: now returns `Err` for
  `N < 4` instead of silently returning γ² = 1 (#7).
- `fast_exp2`: documented the silent `[-126, 126]` clamp (#9).
- `fdn`: documented the rationale for the `±4` safety clamp (#10).
- Synchronized version strings in `README.md` and `CLAUDE.md` with
  `Cargo.toml` (#8).

## [0.5.16] - 2025-05-13

### Added
- Added reusable binaural transfer-matrix DSP primitives for RoomEQ CTC:
  regularized and weighted matrix inverse solves, approximate minimax
  worst-position reweighting, per-position reconstruction errors, FIR synthesis
  from half-spectra, sweep deconvolution, loopback/direct-peak alignment,
  harmonic residue suppression, direct-peak windowing, and complex
  frequency-dependent windowing.
- Added reusable psychoacoustic DSP primitives for expensive perceptual losses:
  Bark-scale conversion and aggregation, Zwicker-style specific/total loudness,
  sharpness, listening-level calibration, pairwise sensory roughness, cached
  feature extraction, and stereo HRTF/CTC convolution helpers.
- Added reusable frequency-response helpers for linear DSP modeling: complex
  biquad response, FIR response, and LR4 low/high crossover response.

### Performance
- Added parallelisation in FDW computation.

## [0.5.15] - 2025-05-13

### Added
- Added Frequency-Dependent Windowing (FDW) analysis for impulse responses,
  including Morlet-style frequency-dependent gates, FDW-gated magnitude, and
  direct/total time-frequency energy ratios for correction-depth consumers.

## [0.5.14] - 2025-05-13

### Added
- Added new signals: Dirac and MLS.

## [0.5.13] - 2025-05-13

### Changed
- Switched to `oxiblas-ndarray` for BLAS operations. Replaced ndarray's
  built-in dot product and matrix multiplication with oxiblas-ndarray's
  pure-Rust BLAS implementation for better performance on all platforms
  without requiring external BLAS libraries (OpenBLAS, Accelerate, MKL).

## [0.5.12] - 2025-05-13

### Added
- Added multi-sweep coherence and noise-floor primitives:
  - `compute_coherence_from_realizations` — per-bin γ² across N complex spectra.
  - `deconvolve_sweep` — inverse-filter deconvolution of one recorded log sweep.
  - `estimate_noise_floor_db_from_silence` — per-bin dB over a Hann-windowed FFT.

## [0.5.11] - 2025-05-13

### Added
- Added FCMLA instruction in SIMD (ARM8.3+ works on Apple ARM).

## [0.5.10] - 2025-05-13

### Added
- Added proper signal recording specialized on delays detection with narrowband probe.
