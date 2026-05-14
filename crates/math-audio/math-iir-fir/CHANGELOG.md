# 0.5.12

- Added support for linear phase crossover

# 0.5.11

## Bug fixes

- `warped_biquad`: `compute_implicit_consts` now uses a type-aware threshold
  (`T::epsilon() * 1000 * coeff_scale`) instead of a hard-coded `1e-15`.
  The fallback preserves sign and clamps magnitude, preventing huge
  `inv_denom` values for small denominators in `f32` (#3).
- `kautz`: `optimize_gains` now solves the regularized least-squares problem
  via QR on the augmented system instead of normal equations + Cholesky.
  This avoids squaring the condition number for ill-conditioned basis sets
  (closely-spaced poles).  Regularization strength increased from `1e-6` to
  `1e-4` to keep gains bounded (#4).
- `filtfilt`: default padding multiplier increased from `3` to `6` for better
  edge handling with narrow high-order filters on short signals.  Added
  `filtfilt_with_padlen` for explicit padding control (#8).

## Added

- Exposed `DEFAULT_FIR_CROSSOVER_TAPS` plus lowpass/highpass coefficient
  accessors on `FirCrossover`, so callers can model/export the same
  linear-phase crossover response used by the realtime splitter.
- Warped Linear Predictive Coding (LPC)
- Kautz filters

# 0.5.10

- added missing LR8 crossvers
- added ZDF: Zero-Delay Feeback state variable

# 0.5.9

- added BiquadBank (can pack 2 or 4 operations in 1 clock depending on the hardware)
- added PeakMatched filters (Vicanek matched analog response)
