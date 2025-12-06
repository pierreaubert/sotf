//! Preconditioners for iterative solvers
//!
//! Preconditioners approximate A^(-1) to accelerate convergence of iterative methods.

mod diagonal;
mod ilu;

pub use diagonal::DiagonalPreconditioner;
pub use ilu::IluPreconditioner;

// Re-export IdentityPreconditioner from traits
pub use crate::traits::IdentityPreconditioner;
