//! Preconditioners for iterative solvers
//!
//! Preconditioners approximate A^(-1) to accelerate convergence of iterative methods.
//!
//! # Available Preconditioners
//!
//! - **DiagonalPreconditioner** (Jacobi): Simple diagonal scaling, fully parallel
//! - **IluPreconditioner**: Sequential ILU(0), best convergence
//! - **IluColoringPreconditioner**: Parallel ILU via level scheduling
//! - **IluFixedPointPreconditioner**: Parallel ILU via Jacobi iteration
//! - **AdditiveSchwarzPreconditioner**: Domain decomposition with overlap (parallel)
//! - **AmgPreconditioner**: Algebraic multigrid with parallel coarsening and smoothing

mod amg;
mod diagonal;
mod ilu;
mod ilu_parallel;
mod schwarz;

pub use amg::{
    AmgCoarsening, AmgConfig, AmgCycle, AmgDiagnostics, AmgInterpolation, AmgPreconditioner,
    AmgSmoother,
};
pub use diagonal::DiagonalPreconditioner;
pub use ilu::IluPreconditioner;
pub use ilu_parallel::{IluColoringPreconditioner, IluFixedPointPreconditioner};
pub use schwarz::AdditiveSchwarzPreconditioner;

// Re-export IdentityPreconditioner from traits
pub use crate::traits::IdentityPreconditioner;
