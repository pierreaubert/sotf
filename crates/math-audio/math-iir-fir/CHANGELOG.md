# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.14] - 2026-05-30

### Changed
- `generate_fir_from_response` now validates that frequency points are finite,
  positive, non-empty, and strictly increasing, and that target magnitudes are
  finite before log-frequency interpolation.
- Updated Kautz documentation to describe the QR-based regularized
  least-squares solve.
- Documented the rare symmetric-numerator fallback used by Orfanidis shelves
  and PeakMatched filters when their quadratic split is near-degenerate.
- Clarified that K-weighting is an approximation and documented
  `ScopedFlushToZero` FTZ/DAZ behavior across architectures.

## [0.5.13] - 2025-05-13

### Changed
- Optimised initialization of parameters.

## [0.5.12] - 2025-05-13

### Added
- Added support for linear phase crossover.
- Exposed `DEFAULT_FIR_CROSSOVER_TAPS` plus lowpass/highpass coefficient
  accessors on `FirCrossover`.
- Added Warped Linear Predictive Coding (LPC).
- Added Kautz filters.

### Fixed
- `warped_biquad`: `compute_implicit_consts` now uses a type-aware threshold
  (`T::epsilon() * 1000 * coeff_scale`) instead of a hard-coded `1e-15`,
  preventing huge `inv_denom` values for small denominators in `f32` (#3).
- `kautz`: `optimize_gains` now solves the regularized least-squares problem
  via QR on the augmented system instead of normal equations + Cholesky.
  Regularization strength increased from `1e-6` to `1e-4` (#4).
- `filtfilt`: default padding multiplier increased from `3` to `6`. Added
  `filtfilt_with_padlen` for explicit padding control (#8).

## [0.5.10] - 2025-05-13

### Added
- Added missing LR8 crossovers.
- Added ZDF (Zero-Delay Feedback) state variable filter.

## [0.5.9] - 2025-05-13

### Added
- Added `BiquadBank` (can pack 2 or 4 operations in 1 clock depending on hardware).
- Added PeakMatched filters (Vicanek matched analog response).
