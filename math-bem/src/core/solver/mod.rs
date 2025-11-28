//! Linear solvers for BEM
//!
//! This module provides various solvers for BEM systems:
//!
//! - [`direct`] - Direct LU factorization (for small systems)
//! - [`cgs`] - Conjugate Gradient Squared (for iterative solving)
//! - [`bicgstab`] - BiCGSTAB (alternative iterative solver)
//! - [`gmres`] - GMRES (recommended for large BEM problems)
//! - [`fmm_interface`] - Interface to use FMM operators with iterative solvers
//! - [`preconditioner`] - Basic preconditioners
//! - [`ilu_preconditioner`] - ILU preconditioner (recommended for BEM)
//!
//! ## Solver Selection Guide
//!
//! For BEM systems, we recommend:
//!
//! | System Size | Solver | Configuration |
//! |-------------|--------|---------------|
//! | N < 1000 | Direct LU | - |
//! | N < 10000 | GMRES(50) + ILU | `GmresConfig::for_small_problems()` |
//! | N > 10000 | GMRES(100) + ILU | `GmresConfig::for_large_bem()` |
//!
//! GMRES is generally preferred for large BEM problems due to:
//! - Monotonic convergence (unlike CGS which can be erratic)
//! - Better handling of non-symmetric matrices
//! - Configurable memory usage via restart parameter
//!
//! ## Preconditioning Strategy
//!
//! BEM systems are ill-conditioned. Simple diagonal (Jacobi) preconditioning
//! is **not sufficient**. Use ILU preconditioning:
//!
//! ```ignore
//! use bem::core::solver::{
//!     gmres_solve_with_ilu, GmresConfig, IluMethod, IluScanningDegree
//! };
//!
//! let config = GmresConfig::for_large_bem();
//! let solution = gmres_solve_with_ilu(
//!     &matrix,
//!     &rhs,
//!     IluMethod::Tbem,
//!     IluScanningDegree::Fine,
//!     &config,
//! );
//! ```

pub mod bicgstab;
pub mod cgs;
pub mod direct;
pub mod fmm_interface;
pub mod gmres;
pub mod ilu_preconditioner;
pub mod preconditioner;

// Core operator types
pub use fmm_interface::{
    CsrOperator, DenseOperator, DiagonalPreconditioner, LinearOperator, MlfmmOperator,
    SlfmmOperator,
};

// ILU preconditioner and solve functions (recommended for BEM)
pub use fmm_interface::{
    ilu_diagnostics, solve_tbem_with_ilu, solve_with_ilu, solve_with_ilu_operator, IluDiagnostics,
    IluOperator,
};

// GMRES solver and ILU integration
pub use fmm_interface::{gmres_solve_with_ilu, gmres_solve_with_ilu_operator, solve_gmres};

// Hierarchical FMM preconditioner solvers
pub use fmm_interface::{gmres_solve_fmm_hierarchical, gmres_solve_with_hierarchical_precond};

// Frequency-adaptive mesh utilities
pub use fmm_interface::{
    recommended_mesh_resolution, mesh_resolution_for_frequency_range,
    estimate_element_count, AdaptiveMeshConfig,
};

// Preconditioner types
pub use preconditioner::{
    BlockDiagonalPreconditioner, DiagonalPreconditioner as BasicDiagonalPreconditioner,
    HierarchicalFmmPreconditioner, IdentityPreconditioner, Preconditioner,
    RowScalingPreconditioner, SparseNearfieldIlu,
};

// ILU configuration types
pub use ilu_preconditioner::{IluMethod, IluPreconditioner, IluScanningDegree, IluSetup};

// GMRES configuration and solution types
pub use gmres::{gmres_solve, gmres_solve_preconditioned, GmresConfig, GmresSolution};
