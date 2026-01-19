//! Direct solvers for linear systems
//!
//! This module provides direct (non-iterative) solvers:
//! - [`lu_solve`]: LU decomposition with partial pivoting
//! - (Future: Cholesky for SPD systems)

mod lu;

pub use lu::{LuFactorization, lu_solve};
