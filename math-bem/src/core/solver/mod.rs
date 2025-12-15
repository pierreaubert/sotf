//! Linear solvers for BEM
//!
//! This module provides various solvers for BEM systems, re-exporting
//! functionality from `math-solvers`.
//!
//! - [`direct`] - Direct LU factorization
//! - [`cgs`] - Conjugate Gradient Squared
//! - [`bicgstab`] - BiCGSTAB
//! - [`gmres`] - GMRES (recommended for large BEM problems)
//! - [`fmm_interface`] - Interface to use FMM operators with iterative solvers
//! - [`preconditioner`] - Preconditioners (ILU, AMG, etc.)
//! - [`batched_blas`] - Batched BLAS operations (native only)

#[cfg(feature = "native")]
pub mod batched_blas;
pub mod fmm_interface;

// Re-export core solver functionality from math-solvers
pub use solvers::direct;
// Modules are private in math-solvers, so we can't re-export them directly.
// We access their contents via the iterative module or specific re-exports.

pub use solvers::traits::{LinearOperator, Preconditioner};

// Core operator types from fmm_interface
pub use fmm_interface::{CsrOperator, DenseOperator, DiagonalPreconditioner, MlfmmOperator};

#[cfg(any(feature = "native", feature = "wasm"))]
pub use fmm_interface::SlfmmOperator;

// Helper functions for solving BEM systems
pub use fmm_interface::{
    gmres_solve_tbem_with_ilu, gmres_solve_with_ilu, gmres_solve_with_ilu_operator, solve_bicgstab,
    solve_cgs, solve_gmres, solve_tbem_with_ilu, solve_with_ilu, solve_with_ilu_operator,
};

// Hierarchical FMM preconditioner
#[cfg(any(feature = "native", feature = "wasm"))]
pub use fmm_interface::{
    HierarchicalFmmPreconditioner, SparseNearfieldIlu, gmres_solve_fmm_hierarchical,
    gmres_solve_with_hierarchical_precond,
};

// Batched BLAS solvers
#[cfg(feature = "native")]
pub use fmm_interface::{gmres_solve_fmm_batched, gmres_solve_fmm_batched_with_ilu};

// Adaptive mesh utilities
pub use fmm_interface::{
    AdaptiveMeshConfig, estimate_element_count, mesh_resolution_for_frequency_range,
    recommended_mesh_resolution,
};

// Re-export specific configuration types for convenience
pub use solvers::iterative::{BiCgstabConfig, BiCgstabSolution};
pub use solvers::iterative::{CgsConfig, CgsSolution};
pub use solvers::iterative::{GmresConfig, GmresSolution};
pub use solvers::preconditioners::IluPreconditioner;
pub use solvers::preconditioners::{AmgConfig, AmgPreconditioner};

// Batched BLAS operations (native only)
#[cfg(feature = "native")]
pub use batched_blas::{
    SlfmmMatvecWorkspace, batched_d_matrix_apply, batched_near_field_apply, batched_s_matrix_apply,
    batched_t_matrix_apply, create_batched_matvec, slfmm_matvec_batched,
};
