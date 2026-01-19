//! Multigrid FEM solver for Helmholtz equation
//!
//! This crate provides a finite element method solver for the Helmholtz equation
//! with adaptive mesh refinement and multigrid acceleration.
//!
//! # Features
//!
//! - **2D and 3D meshes**: Triangles, quadrilaterals, tetrahedra, hexahedra
//! - **Lagrange elements**: P1, P2, P3 polynomial basis functions
//! - **Boundary conditions**: Dirichlet, Neumann, Robin, PML
//! - **Multigrid solver**: V-cycle, W-cycle with geometric coarsening
//! - **Adaptive refinement**: h-refinement with residual-based error estimation
//!
//! # Example
//!
//! ```ignore
//! use math_audio_fem::{FemProblem, FemSolver, mesh};
//!
//! // Create a 2D mesh
//! let mesh = mesh::unit_square_triangles(10);
//!
//! // Define the Helmholtz problem
//! let problem = FemProblem::helmholtz(mesh, k);
//!
//! // Solve
//! let solver = FemSolver::new();
//! let solution = solver.solve(&problem)?;
//! ```

pub mod assembly;
pub mod basis;
pub mod boundary;
pub mod mesh;
pub mod multigrid;
pub mod quadrature;
pub mod solver;

/// Library version
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!version().is_empty());
    }
}
