//! Direct solvers for linear systems
//!
//! This module provides direct (non-iterative) solvers:
//! - [`lu_solve`]: LU decomposition with partial pivoting
//! - (Future: Cholesky for SPD systems)

pub(crate) mod lu;

pub use lu::{LuError, LuFactorization, lu_factorize, lu_solve};
