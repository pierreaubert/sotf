//! Iterative solvers for linear systems
//!
//! This module provides Krylov subspace methods for solving large sparse systems:
//! - [`gmres`]: GMRES(m) with restart - best for general non-symmetric systems
//! - [`bicgstab`]: BiCGSTAB - good alternative to GMRES
//! - [`cgs`]: CGS - faster but less stable than BiCGSTAB
//! - [`cg`]: Conjugate Gradient - for symmetric positive definite systems

mod bicgstab;
mod cg;
mod cgs;
mod gmres;
#[cfg(feature = "rayon")]
pub mod gmres_pipelined;

pub use bicgstab::{BiCgstabConfig, BiCgstabSolution, bicgstab};
pub use cg::{CgConfig, CgSolution, cg};
pub use cgs::{CgsConfig, CgsSolution, cgs};
pub use gmres::{
    GmresConfig, GmresSolution, gmres, gmres_preconditioned, gmres_preconditioned_with_guess,
    gmres_with_guess,
};
#[cfg(feature = "rayon")]
pub use gmres_pipelined::gmres_pipelined;
