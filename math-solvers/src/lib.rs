//! High-performance linear solvers for BEM and FEM
//!
//! This crate provides a collection of iterative and direct solvers for linear systems,
//! along with sparse matrix representations and preconditioners.
//!
//! # Features
//!
//! - **Iterative Solvers**: GMRES, BiCGSTAB, CGS, CG
//! - **Direct Solvers**: LU decomposition (with BLAS and pure-Rust fallbacks)
//! - **Preconditioners**: Jacobi, ILU, block diagonal
//! - **Sparse Matrices**: CSR format with efficient matrix-vector products
//! - **Generic Scalar Types**: Works with Complex64, Complex32, f64, f32
//!
//! # Example
//!
//! ```ignore
//! use solvers::{CsrMatrix, gmres, GmresConfig};
//! use num_complex::Complex64;
//!
//! // Create a sparse system matrix
//! let matrix = CsrMatrix::from_dense(&dense_matrix, 1e-10);
//!
//! // Solve with GMRES
//! let config = GmresConfig::default();
//! let solution = gmres(&matrix, &rhs, &config)?;
//! ```

pub mod direct;
pub mod iterative;
pub mod parallel;
pub mod preconditioners;
pub mod sparse;
pub mod traits;

// Re-export main types
pub use sparse::{CsrBuilder, CsrMatrix};
pub use traits::{ComplexField, LinearOperator, Preconditioner};

// Re-export iterative solvers
pub use iterative::{
    BiCgstabConfig, BiCgstabSolution, CgConfig, CgSolution, CgsConfig, CgsSolution, GmresConfig,
    GmresSolution, bicgstab, cg, cgs, gmres,
};

// Re-export direct solvers
pub use direct::{LuFactorization, lu_solve};

// Re-export preconditioners
pub use preconditioners::{
    AdditiveSchwarzPreconditioner, AmgCoarsening, AmgConfig, AmgCycle, AmgDiagnostics,
    AmgInterpolation, AmgPreconditioner, AmgSmoother, DiagonalPreconditioner,
    IdentityPreconditioner, IluColoringPreconditioner, IluFixedPointPreconditioner,
    IluPreconditioner,
};
