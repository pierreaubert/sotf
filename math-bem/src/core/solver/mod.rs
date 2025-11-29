//! Linear solvers for BEM
//!
//! This module provides various solvers for BEM systems:
//!
//! - [`direct`] - Direct LU factorization (uses BLAS when available, pure Rust fallback otherwise)
//! - [`cgs`] - Conjugate Gradient Squared (for iterative solving)
//! - [`bicgstab`] - BiCGSTAB (alternative iterative solver)
//! - [`gmres`] - GMRES (recommended for large BEM problems)
//! - [`fmm_interface`] - Interface to use FMM operators with iterative solvers
//! - [`preconditioner`] - Basic preconditioners
//! - [`ilu_preconditioner`] - ILU preconditioner (recommended for BEM)
//! - [`batched_blas`] - Batched BLAS operations for optimized matrix computations (native only)
//!
//! ## WASM Compatibility
//!
//! When building for WASM targets, disable the `native` feature. Most solvers work
//! in pure Rust mode, but the following functionality requires `native`:
//! - Parallel processing via rayon (SLFMM, batched operations)
//! - HierarchicalFmmPreconditioner (uses rayon for parallel block processing)
//! - SlfmmOperator (SLFMM assembly uses rayon)
//!
//! The direct solver, block preconditioner, and all iterative solvers work
//! in WASM mode using pure Rust linear algebra fallbacks.
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

#[cfg(feature = "native")]
pub mod batched_blas;
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
};

#[cfg(feature = "native")]
pub use fmm_interface::SlfmmOperator;

// ILU preconditioner and solve functions (recommended for BEM)
// These require native feature for BLAS/LAPACK
#[cfg(feature = "native")]
pub use fmm_interface::{
    ilu_diagnostics, solve_tbem_with_ilu, solve_with_ilu, solve_with_ilu_operator, IluDiagnostics,
    IluOperator,
};

// GMRES solver and ILU integration (native feature for ILU variants)
pub use fmm_interface::solve_gmres;
#[cfg(feature = "native")]
pub use fmm_interface::{gmres_solve_with_ilu, gmres_solve_with_ilu_operator};

// Hierarchical FMM preconditioner solvers (native only)
#[cfg(feature = "native")]
pub use fmm_interface::{gmres_solve_fmm_hierarchical, gmres_solve_with_hierarchical_precond};

// Batched BLAS FMM solvers (optimized for large problems, native only)
#[cfg(feature = "native")]
pub use fmm_interface::{gmres_solve_fmm_batched, gmres_solve_fmm_batched_with_ilu};

// Frequency-adaptive mesh utilities (portable)
pub use fmm_interface::{
    recommended_mesh_resolution, mesh_resolution_for_frequency_range,
    estimate_element_count, AdaptiveMeshConfig,
};

// Preconditioner types - most are portable
pub use preconditioner::{
    DiagonalPreconditioner as BasicDiagonalPreconditioner,
    IdentityPreconditioner, Preconditioner,
    RowScalingPreconditioner, SparseNearfieldIlu,
    BlockDiagonalPreconditioner,
};

// HierarchicalFmmPreconditioner requires native (uses rayon for parallel block processing)
#[cfg(feature = "native")]
pub use preconditioner::HierarchicalFmmPreconditioner;

// ILU configuration types
pub use ilu_preconditioner::{IluMethod, IluPreconditioner, IluScanningDegree, IluSetup};

// GMRES configuration and solution types (portable)
pub use gmres::{gmres_solve, gmres_solve_preconditioned, GmresConfig, GmresSolution};

// Batched BLAS operations for optimized FMM matvec (native only for parallel)
#[cfg(feature = "native")]
pub use batched_blas::{
    SlfmmMatvecWorkspace, slfmm_matvec_batched, create_batched_matvec,
    batched_t_matrix_apply, batched_s_matrix_apply, batched_d_matrix_apply,
    batched_near_field_apply,
};

// Direct solver (portable - uses BLAS when available, pure Rust fallback otherwise)
pub use direct::{direct_solve, direct_solve_lu, DirectSolution};
