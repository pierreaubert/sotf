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
