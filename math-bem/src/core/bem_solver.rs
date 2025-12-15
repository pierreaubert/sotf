//! High-level BEM Solver API
//!
//! This module provides a unified, high-level interface for acoustic BEM simulations.
//! It integrates mesh generation, system assembly, linear solving, and post-processing.
//!
//! The Direct solver method works in both native and WASM modes. When the `native` feature
//! is enabled, it uses optimized BLAS/LAPACK. Otherwise, it falls back to a pure Rust
//! LU factorization implementation.
//!
//! # Example
//!
//! ```ignore
//! use bem::core::{BemSolver, BemProblem, IncidentField};
//!
//! // Create a rigid sphere scattering problem
//! let problem = BemProblem::rigid_sphere_scattering(
//!     0.1,        // radius
//!     1000.0,     // frequency
//!     343.0,      // speed of sound
//!     1.21,       // density
//! );
//!
//! // Configure solver
//! let solver = BemSolver::new()
//!     .with_mesh_refinement(3)
//!     .with_solver_method(SolverMethod::Direct);
//!
//! // Solve
//! let solution = solver.solve(&problem)?;
//!
//! // Evaluate field at a point
//! let pressure = solution.evaluate_pressure(&[0.0, 0.0, 0.2]);
//! ```

use ndarray::{Array1, Array2};
use num_complex::Complex64;
use std::f64::consts::PI;

use crate::core::assembly::tbem::build_tbem_system_with_beta;
use crate::core::incident::IncidentField;
use crate::core::mesh::generators::{generate_icosphere_mesh, generate_sphere_mesh};
use crate::core::postprocess::pressure::{FieldPoint, compute_total_field};
use crate::core::types::{BoundaryCondition, Element, Mesh, PhysicsParams};
use solvers::direct::lu_solve;

/// Solver method for the linear system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SolverMethod {
    /// Direct LU factorization (best for small problems)
    #[default]
    Direct,
    /// Conjugate Gradient Squared (iterative)
    Cgs,
    /// BiCGSTAB (iterative, more stable than CGS)
    BiCgStab,
}

/// Assembly method for the BEM matrix
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AssemblyMethod {
    /// Traditional BEM with O(N^2) dense matrix
    #[default]
    Tbem,
    /// Single-Level Fast Multipole Method
    Slfmm,
    /// Multi-Level Fast Multipole Method
    Mlfmm,
}

/// Boundary condition type for the problem
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryConditionType {
    /// Rigid surface (zero normal velocity)
    Rigid,
    /// Soft surface (zero pressure)
    Soft,
    /// Impedance boundary condition
    Impedance,
}

/// Definition of a BEM problem
#[derive(Debug, Clone)]
pub struct BemProblem {
    /// Problem geometry mesh
    pub mesh: Mesh,
    /// Physical parameters
    pub physics: PhysicsParams,
    /// Incident field
    pub incident_field: IncidentField,
    /// Boundary condition type
    pub bc_type: BoundaryConditionType,
    /// Use Burton-Miller formulation (recommended for exterior problems)
    pub use_burton_miller: bool,
}

impl BemProblem {
    /// Create a rigid sphere scattering problem with plane wave incidence
    ///
    /// # Arguments
    /// * `radius` - Sphere radius (m)
    /// * `frequency` - Excitation frequency (Hz)
    /// * `speed_of_sound` - Speed of sound (m/s)
    /// * `density` - Medium density (kg/m�)
    pub fn rigid_sphere_scattering(
        radius: f64,
        frequency: f64,
        speed_of_sound: f64,
        density: f64,
    ) -> Self {
        // Determine mesh resolution based on ka
        let k = 2.0 * PI * frequency / speed_of_sound;
        let ka = k * radius;

        // Rule of thumb: ~10 elements per wavelength
        // For a sphere, this translates to subdivision level
        let subdivisions = if ka < 1.0 {
            2 // Low frequency: coarse mesh ok
        } else if ka < 5.0 {
            3 // Medium frequency
        } else {
            4 // High frequency: need finer mesh
        };

        let mesh = generate_icosphere_mesh(radius, subdivisions);
        let physics = PhysicsParams::new(frequency, speed_of_sound, density, false);
        let incident_field = IncidentField::plane_wave_z();

        Self {
            mesh,
            physics,
            incident_field,
            bc_type: BoundaryConditionType::Rigid,
            use_burton_miller: true,
        }
    }

    /// Create a rigid sphere scattering problem with custom mesh resolution
    pub fn rigid_sphere_scattering_custom(
        radius: f64,
        frequency: f64,
        speed_of_sound: f64,
        density: f64,
        n_theta: usize,
        n_phi: usize,
    ) -> Self {
        let mesh = generate_sphere_mesh(radius, n_theta, n_phi);
        let physics = PhysicsParams::new(frequency, speed_of_sound, density, false);
        let incident_field = IncidentField::plane_wave_z();

        Self {
            mesh,
            physics,
            incident_field,
            bc_type: BoundaryConditionType::Rigid,
            use_burton_miller: true,
        }
    }

    /// Set the incident field
    pub fn with_incident_field(mut self, field: IncidentField) -> Self {
        self.incident_field = field;
        self
    }

    /// Set the boundary condition type
    pub fn with_boundary_condition(mut self, bc_type: BoundaryConditionType) -> Self {
        self.bc_type = bc_type;
        self
    }

    /// Enable/disable Burton-Miller formulation
    pub fn with_burton_miller(mut self, use_bm: bool) -> Self {
        self.use_burton_miller = use_bm;
        self
    }

    /// Get the wave number times radius (ka)
    pub fn ka(&self) -> f64 {
        self.physics.wave_number * self.mesh_radius()
    }

    /// Estimate mesh radius from bounding box
    fn mesh_radius(&self) -> f64 {
        // Estimate radius from mesh nodes
        let mut max_r = 0.0f64;
        for i in 0..self.mesh.nodes.nrows() {
            let r = (self.mesh.nodes[[i, 0]].powi(2)
                + self.mesh.nodes[[i, 1]].powi(2)
                + self.mesh.nodes[[i, 2]].powi(2))
            .sqrt();
            max_r = max_r.max(r);
        }
        max_r
    }
}

/// BEM solver configuration
#[derive(Debug, Clone)]
pub struct BemSolver {
    /// Linear solver method
    pub solver_method: SolverMethod,
    /// Matrix assembly method
    pub assembly_method: AssemblyMethod,
    /// Maximum iterations for iterative solvers
    pub max_iterations: usize,
    /// Tolerance for iterative solvers
    pub tolerance: f64,
    /// Verbose output
    pub verbose: bool,
    /// Burton-Miller β scale factor (default: 4.0 for best accuracy at ka ~ 1)
    pub beta_scale: f64,
}

impl Default for BemSolver {
    fn default() -> Self {
        Self {
            solver_method: SolverMethod::Direct,
            assembly_method: AssemblyMethod::Tbem,
            max_iterations: 1000,
            tolerance: 1e-8,
            verbose: false,
            beta_scale: 4.0, // Empirically optimal for ka ~ 1
        }
    }
}

impl BemSolver {
    /// Create a new solver with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the linear solver method
    pub fn with_solver_method(mut self, method: SolverMethod) -> Self {
        self.solver_method = method;
        self
    }

    /// Set the assembly method
    pub fn with_assembly_method(mut self, method: AssemblyMethod) -> Self {
        self.assembly_method = method;
        self
    }

    /// Set maximum iterations for iterative solvers
    pub fn with_max_iterations(mut self, max_iter: usize) -> Self {
        self.max_iterations = max_iter;
        self
    }

    /// Set tolerance for iterative solvers
    pub fn with_tolerance(mut self, tol: f64) -> Self {
        self.tolerance = tol;
        self
    }

    /// Enable verbose output
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Solve a BEM problem
    ///
    /// # Arguments
    /// * `problem` - The BEM problem definition
    ///
    /// # Returns
    /// Solution containing surface pressures and methods to evaluate fields
    pub fn solve(&self, problem: &BemProblem) -> Result<BemSolution, BemError> {
        if self.verbose {
            log::info!(
                "Solving BEM problem: {} elements, ka = {:.3}",
                problem.mesh.elements.len(),
                problem.ka()
            );
        }

        // Step 1: Prepare elements with boundary conditions
        let elements = self.prepare_elements(problem);

        // Step 2: Assemble system matrix and RHS
        let (matrix, rhs) =
            self.assemble_system(&elements, &problem.mesh.nodes, &problem.physics)?;

        // Step 3: Add incident field contribution to RHS
        let rhs = self.add_incident_field_rhs(
            rhs,
            &elements,
            &problem.incident_field,
            &problem.physics,
            problem.use_burton_miller,
        );

        // Step 4: Solve linear system
        let surface_pressure = self.solve_linear_system(&matrix, &rhs)?;

        if self.verbose {
            log::info!(
                "Solution complete. Max surface pressure: {:.6}",
                surface_pressure
                    .iter()
                    .map(|p| p.norm())
                    .fold(0.0f64, f64::max)
            );
        }

        Ok(BemSolution {
            surface_pressure,
            elements,
            nodes: problem.mesh.nodes.clone(),
            incident_field: problem.incident_field.clone(),
            physics: problem.physics.clone(),
        })
    }

    /// Prepare elements with boundary conditions
    fn prepare_elements(&self, problem: &BemProblem) -> Vec<Element> {
        let mut elements = problem.mesh.elements.clone();

        // Set boundary conditions based on problem type
        let bc = match problem.bc_type {
            BoundaryConditionType::Rigid => {
                // Zero normal velocity (Neumann BC)
                BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)])
            }
            BoundaryConditionType::Soft => {
                // Zero pressure (Dirichlet BC)
                BoundaryCondition::Pressure(vec![Complex64::new(0.0, 0.0)])
            }
            BoundaryConditionType::Impedance => {
                // Default impedance (plane wave)
                let z0 = problem.physics.density * problem.physics.speed_of_sound;
                BoundaryCondition::VelocityWithAdmittance {
                    velocity: vec![Complex64::new(0.0, 0.0)],
                    admittance: Complex64::new(1.0 / z0, 0.0),
                }
            }
        };

        // Assign BC and DOF addresses to each element
        for (i, elem) in elements.iter_mut().enumerate() {
            elem.boundary_condition = bc.clone();
            elem.dof_addresses = vec![i];
        }

        elements
    }

    /// Assemble the BEM system matrix
    fn assemble_system(
        &self,
        elements: &[Element],
        nodes: &Array2<f64>,
        physics: &PhysicsParams,
    ) -> Result<(Array2<Complex64>, Array1<Complex64>), BemError> {
        match self.assembly_method {
            AssemblyMethod::Tbem => {
                // Use scaled Burton-Miller β for better accuracy
                let beta = physics.burton_miller_beta_scaled(self.beta_scale);
                let system = build_tbem_system_with_beta(elements, nodes, physics, beta);
                Ok((system.matrix, system.rhs))
            }
            AssemblyMethod::Slfmm | AssemblyMethod::Mlfmm => {
                // FMM methods not yet integrated into high-level API
                Err(BemError::NotImplemented(
                    "FMM assembly not yet available in high-level API".to_string(),
                ))
            }
        }
    }

    /// Add incident field contribution to RHS
    fn add_incident_field_rhs(
        &self,
        mut rhs: Array1<Complex64>,
        elements: &[Element],
        incident_field: &IncidentField,
        physics: &PhysicsParams,
        use_burton_miller: bool,
    ) -> Array1<Complex64> {
        // Collect element centers and normals
        let n = elements.len();
        let mut centers = Array2::zeros((n, 3));
        let mut normals = Array2::zeros((n, 3));

        for (i, elem) in elements.iter().enumerate() {
            for j in 0..3 {
                centers[[i, j]] = elem.center[j];
                normals[[i, j]] = elem.normal[j];
            }
        }

        // Compute incident field RHS contribution with scaled β
        let incident_rhs = if use_burton_miller {
            let beta = physics.burton_miller_beta_scaled(self.beta_scale);
            incident_field.compute_rhs_with_beta(&centers, &normals, physics, beta)
        } else {
            incident_field.compute_rhs(&centers, &normals, physics, false)
        };

        // Add to system RHS
        rhs = rhs + incident_rhs;

        rhs
    }

    /// Solve the linear system
    fn solve_linear_system(
        &self,
        matrix: &Array2<Complex64>,
        rhs: &Array1<Complex64>,
    ) -> Result<Array1<Complex64>, BemError> {
        match self.solver_method {
            SolverMethod::Direct => {
                // Uses BLAS when native feature is enabled, pure Rust fallback otherwise
                lu_solve(matrix, rhs).map_err(|e| BemError::SolverFailed(e.to_string()))
            }
            SolverMethod::Cgs | SolverMethod::BiCgStab => {
                // Iterative solvers not yet integrated
                Err(BemError::NotImplemented(
                    "Iterative solvers not yet available in high-level API".to_string(),
                ))
            }
        }
    }
}

/// Solution of a BEM problem
#[derive(Debug, Clone)]
pub struct BemSolution {
    /// Surface pressure at each element
    pub surface_pressure: Array1<Complex64>,
    /// Elements used in the solution
    pub elements: Vec<Element>,
    /// Node coordinates
    pub nodes: Array2<f64>,
    /// Incident field used
    pub incident_field: IncidentField,
    /// Physics parameters
    pub physics: PhysicsParams,
}

impl BemSolution {
    /// Evaluate total pressure at a single point
    pub fn evaluate_pressure(&self, point: &[f64; 3]) -> Complex64 {
        let eval_points =
            Array2::from_shape_vec((1, 3), vec![point[0], point[1], point[2]]).unwrap();

        let field_points = compute_total_field(
            &eval_points,
            &self.elements,
            &self.nodes,
            &self.surface_pressure,
            None,
            &self.incident_field,
            &self.physics,
        );

        field_points[0].p_total
    }

    /// Evaluate total pressure at multiple points
    pub fn evaluate_pressure_field(&self, points: &Array2<f64>) -> Vec<FieldPoint> {
        compute_total_field(
            points,
            &self.elements,
            &self.nodes,
            &self.surface_pressure,
            None,
            &self.incident_field,
            &self.physics,
        )
    }

    /// Get max surface pressure magnitude
    pub fn max_surface_pressure(&self) -> f64 {
        self.surface_pressure
            .iter()
            .map(|p| p.norm())
            .fold(0.0f64, f64::max)
    }

    /// Get mean surface pressure magnitude
    pub fn mean_surface_pressure(&self) -> f64 {
        let sum: f64 = self.surface_pressure.iter().map(|p| p.norm()).sum();
        sum / self.surface_pressure.len() as f64
    }

    /// Number of DOFs in the solution
    pub fn num_dofs(&self) -> usize {
        self.surface_pressure.len()
    }
}

/// BEM solver errors
#[derive(Debug, Clone)]
pub enum BemError {
    /// Feature not yet implemented
    NotImplemented(String),
    /// Linear solver failed
    SolverFailed(String),
    /// Invalid mesh
    InvalidMesh(String),
    /// Invalid parameters
    InvalidParameters(String),
}

impl std::fmt::Display for BemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BemError::NotImplemented(msg) => write!(f, "Not implemented: {}", msg),
            BemError::SolverFailed(msg) => write!(f, "Solver failed: {}", msg),
            BemError::InvalidMesh(msg) => write!(f, "Invalid mesh: {}", msg),
            BemError::InvalidParameters(msg) => write!(f, "Invalid parameters: {}", msg),
        }
    }
}

impl std::error::Error for BemError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bem_problem_creation() {
        let problem = BemProblem::rigid_sphere_scattering(0.1, 1000.0, 343.0, 1.21);

        assert!(problem.mesh.elements.len() > 0);
        assert!(problem.mesh.nodes.nrows() > 0);
        assert!(problem.ka() > 0.0);
    }

    #[test]
    fn test_bem_solver_creation() {
        let solver = BemSolver::new()
            .with_solver_method(SolverMethod::Direct)
            .with_assembly_method(AssemblyMethod::Tbem)
            .with_verbose(false);

        assert_eq!(solver.solver_method, SolverMethod::Direct);
        assert_eq!(solver.assembly_method, AssemblyMethod::Tbem);
    }

    // Direct solver tests work in both native and WASM modes
    #[test]
    fn test_bem_solver_small_problem() {
        // Very small problem for quick test
        let problem = BemProblem::rigid_sphere_scattering_custom(
            0.1,   // radius
            100.0, // very low frequency for quick test
            343.0, 1.21, 4, // coarse mesh
            8,
        );

        let solver = BemSolver::new();
        let result = solver.solve(&problem);

        assert!(result.is_ok());
        let solution = result.unwrap();
        assert!(solution.num_dofs() > 0);
        assert!(solution.max_surface_pressure() > 0.0);
    }

    #[test]
    fn test_field_evaluation() {
        // Very small problem
        let problem = BemProblem::rigid_sphere_scattering_custom(0.1, 100.0, 343.0, 1.21, 4, 8);

        let solver = BemSolver::new();
        let solution = solver.solve(&problem).unwrap();

        // Evaluate at a point outside the sphere
        let p = solution.evaluate_pressure(&[0.0, 0.0, 0.2]);

        // Should have some pressure (incident + scattered)
        assert!(p.norm() > 0.0);
    }
}
