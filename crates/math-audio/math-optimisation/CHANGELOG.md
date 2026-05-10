# 0.5.8

## New features

- Added `continuous_area` module for continuous-prior loss integration. It is
  generic over dimension via const generics (`Prior<const D: usize>` /
  `Quadrature<const D: usize>`), with `Uniform` / axis-aligned `Gaussian` /
  `Custom` priors, Sobol / Latin-Hypercube / Gauss–Legendre tensor-product
  quadrature, and `ExpectedValue` / `WorstCase` / `CVaR` scalarisations.
  `evaluate_area_loss` collapses a per-position loss `L(params, p)` into one
  outer-loop scalar suitable for any optimiser. Reuses the existing
  `init_sobol`, `init_latin_hypercube`, and `differential_evolution` (for the
  `WorstCase` inner search). Includes hand-derived Beasley-Springer-Moro
  inverse-normal CDF and Abramowitz-Stegun erf for the Gaussian inverse-CDF
  transform.

## Performance

- Added parallelisation where it can help: CMA-ES

# 0.5.7

## New features

- Added a generic Gaussian-process Bayesian optimisation backend for expensive
  bounded continuous objectives. It supports Matérn-5/2 ARD kernels with
  marginal-likelihood lengthscale fitting, Cholesky-based GP solves, latent
  posterior-uncertainty stopping, Sobol-seeded initial designs, EI, real
  Monte-Carlo q-EI, Thompson acquisition, parallel batch evaluation through
  `ParallelConfig`, and real Monte-Carlo qEHVI for small expensive Pareto
  searches.

# 0.5.6

- Fix a 1 line bug in the new cobyla implementation.

# 0.5.5

- Added ISRES and Cobyla. Used them to drop the dependency to nlopt.

# 0.5.4

## Switch to oxiblas-ndarray for BLAS operations

Replaced ndarray's built-in matrix-vector multiply with oxiblas-ndarray's
pure-Rust BLAS implementation in the linear penalty evaluation.

- `mod.rs`, `impl_helpers.rs`: `lp.a.dot(&x.view())` replaced with
  `matvec(&lp.a, &x.to_owned())` from oxiblas-ndarray.
- Added `oxiblas-ndarray` dependency.

Cargo version 0.5.3 -> 0.5.4.

# 0.5.3

- Added Sobol initialisation which was in another crate for historical reasons

# 0.5.2

- Added LSHADE algorithm

# 0.5.1

- Bug fixes

# 0.5.0 == 0.4.27

- Split from AutoEQ
- Added a Levenberg-Marquardt bounded nonlinear least-squares solver
