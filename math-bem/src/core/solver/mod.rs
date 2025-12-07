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
//! - [`amg_preconditioner`] - AMG preconditioner (better parallel scalability)
//! - [`batched_blas`] - Batched BLAS operations for optimized matrix computations (native only)
//!
//! ## WASM Compatibility
//!
//! With the `wasm` feature enabled, most solvers work including:
//! - SlfmmOperator (SLFMM assembly via wasm-bindgen-rayon)
//! - HierarchicalFmmPreconditioner (parallel block processing via Web Workers)
//! - AmgPreconditioner (parallel AMG V-cycle)
//! - All iterative solvers (GMRES, CGS, BiCGSTAB)
//! - Direct solver (pure Rust LU fallback)
//!
//! Only `batched_blas` remains native-only for optimized BLAS operations.
//!
//! ## Solver Selection Guide
//!
//! For BEM systems, we recommend:
//!
//! | System Size | Solver | Configuration |
//! |-------------|--------|---------------|
//! | N < 1000 | Direct LU | - |
//! | N < 10000 | GMRES(50) + ILU | `GmresConfig::for_small_problems()` |
//! | N > 10000 | GMRES(100) + AMG | `GmresConfig::for_large_bem()` |
//!
//! GMRES is generally preferred for large BEM problems due to:
//! - Monotonic convergence (unlike CGS which can be erratic)
//! - Better handling of non-symmetric matrices
//! - Configurable memory usage via restart parameter
//!
//! ## Preconditioning Strategy
//!
//! BEM systems are ill-conditioned. Simple diagonal (Jacobi) preconditioning
//! is **not sufficient**. Use ILU or AMG preconditioning:
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
//!
//! For better parallel scalability, use AMG:
//!
//! ```ignore
//! use bem::core::solver::{AmgPreconditioner, AmgConfig, gmres_solve_preconditioned, GmresConfig};
//!
//! let amg = AmgPreconditioner::from_csr(&matrix, AmgConfig::for_parallel());
//! let solution = gmres_solve_preconditioned(
//!     |x| matrix.matvec(x),
//!     |r| amg.apply(r),
//!     &rhs,
//!     None,
//!     &GmresConfig::for_large_bem(),
//! );
//! ```

pub mod amg_preconditioner;
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

#[cfg(any(feature = "native", feature = "wasm"))]
pub use fmm_interface::SlfmmOperator;

// ILU preconditioner and solve functions (recommended for BEM)
// ILU is pure Rust and works in WASM
pub use fmm_interface::{
    IluDiagnostics, IluOperator, ilu_diagnostics, solve_tbem_with_ilu, solve_with_ilu,
    solve_with_ilu_operator,
};

// GMRES solver and ILU integration (portable)
pub use fmm_interface::solve_gmres;
pub use fmm_interface::{gmres_solve_with_ilu, gmres_solve_with_ilu_operator};

// Hierarchical FMM preconditioner solvers (uses rayon, works in WASM)
#[cfg(any(feature = "native", feature = "wasm"))]
pub use fmm_interface::{gmres_solve_fmm_hierarchical, gmres_solve_with_hierarchical_precond};

// Batched BLAS FMM solvers (optimized for large problems, native only)
#[cfg(feature = "native")]
pub use fmm_interface::{gmres_solve_fmm_batched, gmres_solve_fmm_batched_with_ilu};

// Frequency-adaptive mesh utilities (portable)
pub use fmm_interface::{
    AdaptiveMeshConfig, estimate_element_count, mesh_resolution_for_frequency_range,
    recommended_mesh_resolution,
};

// Preconditioner types - most are portable
pub use preconditioner::{
    BlockDiagonalPreconditioner, DiagonalPreconditioner as BasicDiagonalPreconditioner,
    IdentityPreconditioner, Preconditioner, RowScalingPreconditioner, SparseNearfieldIlu,
};

// HierarchicalFmmPreconditioner uses rayon for parallel block processing
// Works in WASM via wasm-bindgen-rayon
#[cfg(any(feature = "native", feature = "wasm"))]
pub use preconditioner::HierarchicalFmmPreconditioner;

// ILU configuration types
pub use ilu_preconditioner::{IluMethod, IluPreconditioner, IluScanningDegree, IluSetup};

// AMG preconditioner (portable, better parallel scalability than ILU)
pub use amg_preconditioner::{
    AmgConfig, AmgCoarsening, AmgCycle, AmgDiagnostics, AmgInterpolation, AmgPreconditioner,
    AmgSmoother,
};

// GMRES configuration and solution types (portable)
pub use gmres::{GmresConfig, GmresSolution, gmres_solve, gmres_solve_preconditioned};

// Batched BLAS operations for optimized FMM matvec (native only for parallel)
#[cfg(feature = "native")]
pub use batched_blas::{
    SlfmmMatvecWorkspace, batched_d_matrix_apply, batched_near_field_apply, batched_s_matrix_apply,
    batched_t_matrix_apply, create_batched_matvec, slfmm_matvec_batched,
};

// Direct solver (portable - uses BLAS when available, pure Rust fallback otherwise)
pub use direct::{DirectSolution, direct_solve, direct_solve_lu};
