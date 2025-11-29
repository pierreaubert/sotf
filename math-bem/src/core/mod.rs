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
//! - `greens`: Green's function computations (Helmholtz kernel)
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

pub mod algebra;
pub mod parallel;
pub mod types;
pub mod constants;
pub mod greens;
pub mod mesh;
pub mod integration;
pub mod assembly;
pub mod solver;
pub mod incident;
pub mod postprocess;
pub mod io;
pub mod bem_solver;

// Re-exports for convenience
pub use types::*;
pub use constants::PhysicsParams;
pub use incident::IncidentField;
pub use bem_solver::{BemSolver, BemProblem, BemSolution, BemError, SolverMethod, AssemblyMethod};
