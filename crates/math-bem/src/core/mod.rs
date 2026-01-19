//! Pure Rust BEM (Boundary Element Method) Solver
//!
//! This module provides a complete BEM solver for acoustic scattering problems,
//! supporting Traditional BEM (TBEM), Single-Level Fast Multipole (SLFMM),
//! and Multi-Level Fast Multipole (MLFMM) methods.
//!
//! ## Architecture
//!
//! - `types`: Core data structures (Mesh, Element, PhysicsParams)
//! - `constants`: Physical and integration constants
//! - `greens`: Green's function computations (via math-wave)
//! - `mesh`: Mesh loading, element operations, and mesh generators
//! - `integration`: Numerical quadrature (Gauss-Legendre, singular)
//! - `assembly`: BEM matrix assembly (TBEM, SLFMM, MLFMM)
//! - `solver`: Linear solvers (Direct, CGS, BiCGSTAB)
//! - `incident`: Incident field computation (plane waves, point sources)
//! - `postprocess`: Result computation at evaluation points
//! - `io`: Input/output (NC.inp format, JSON)
//! - `bem_solver`: High-level API for solving BEM problems
//! - `algebra`: Pure Rust linear algebra fallbacks for WASM portability
//! - `parallel`: Portable parallel iteration (works with native, WASM, or sequential)

pub mod assembly;
pub mod bem_solver;
pub mod constants;
// pub mod greens; // Removed - use math_audio_wave::greens instead
pub mod incident;
pub mod integration;
pub mod io;
pub mod mesh;
pub mod parallel;
pub mod postprocess;
pub mod solver;
pub mod types;

// Re-exports for convenience
pub use bem_solver::{AssemblyMethod, BemError, BemProblem, BemSolution, BemSolver, SolverMethod};
pub use constants::PhysicsParams;
pub use incident::IncidentField;
pub use types::*;
