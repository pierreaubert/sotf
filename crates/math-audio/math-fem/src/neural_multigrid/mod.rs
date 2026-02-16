//! Neural Multigrid (Wave-ADR-NS) solver for the Helmholtz equation
//!
//! Deep-learning-enhanced multigrid method targeting heterogeneous Helmholtz
//! equations at high wavenumbers. Decomposes error into characteristic and
//! non-characteristic components:
//!
//! - **Non-characteristic error**: Smoothed by Chebyshev-accelerated iterations
//! - **Characteristic error**: Corrected by an ADR equation from WKB decomposition
//!
//! This implementation uses classical numerical methods (Fast Sweeping for eikonal,
//! analytical Chebyshev parameter estimates) — no ML framework needed. Neural
//! components (FNO, CNN) can be added later behind a feature flag.
//!
//! # Usage
//!
//! ```ignore
//! use math_audio_fem::neural_multigrid::{NeuralMultigridConfig, solve_neural_multigrid};
//!
//! let config = NeuralMultigridConfig::for_wavenumber(k);
//! let solution = solve_neural_multigrid(
//!     &mesh, degree, wavenumber, &rhs, &dirichlet_bcs, &config
//! )?;
//! ```
//!
//! # Reference
//!
//! Cui & Jiang (2024), "Neural Multigrid for Heterogeneous Helmholtz Equations",
//! arXiv:2404.02493

pub mod adr;
pub mod chebyshev;
pub mod config;
pub mod eikonal;
pub mod hierarchy;
pub mod solver;

pub use config::NeuralMultigridConfig;
pub use hierarchy::WaveHierarchy;
pub use solver::solve_neural_multigrid;
