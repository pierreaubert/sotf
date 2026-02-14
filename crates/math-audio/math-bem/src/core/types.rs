//! Core type definitions for BEM solver
//!
//! This module defines the fundamental data structures used throughout the BEM implementation,
//! matching the C++ structures from NC_TypeDefinition.h while using idiomatic Rust types.

use ndarray::{Array1, Array2};
use num_complex::Complex64;
use serde::{Deserialize, Serialize};

// ============================================================================
// Physical Constants and Parameters
// ============================================================================

/// Physical parameters for the acoustic problem
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsParams {
    /// Speed of sound in the medium (m/s)
    pub speed_of_sound: f64,
    /// Density of the medium (kg/m³)
    pub density: f64,
    /// Current frequency (Hz)
    pub frequency: f64,
    /// Wave number k = ω/c = 2πf/c
    pub wave_number: f64,
    /// Angular frequency ω = 2πf
    pub omega: f64,
    /// Wave length λ = c/f
    pub wave_length: f64,
    /// Harmonic time factor: +1 for exp(+ikr), -1 for exp(-ikr)
    pub harmonic_factor: f64,
    /// Factor for sound pressure: ρ * ω * harmonic_factor
    pub pressure_factor: f64,
    /// Tau: +1 for external problem, -1 for internal problem
    pub tau: f64,
}

impl PhysicsParams {
    /// Create new physics parameters from frequency and medium properties
    pub fn new(frequency: f64, speed_of_sound: f64, density: f64, is_internal: bool) -> Self {
        use std::f64::consts::PI;
        let omega = 2.0 * PI * frequency;
        let wave_number = omega / speed_of_sound;
        let wave_length = speed_of_sound / frequency;
        let harmonic_factor = 1.0; // Convention: exp(+ikr)
        let tau = if is_internal { -1.0 } else { 1.0 };

        Self {
            speed_of_sound,
            density,
            frequency,
            wave_number,
            omega,
            wave_length,
            harmonic_factor,
            pressure_factor: density * omega * harmonic_factor,
            tau,
        }
    }

    /// Burton-Miller coupling constant: β = i/k (for external problems)
    ///
    /// This is the traditional formulation. For better conditioning at low
    /// frequencies, use `burton_miller_beta_bounded` instead.
    pub fn burton_miller_beta(&self) -> Complex64 {
        if self.tau > 0.0 {
            Complex64::new(0.0, self.harmonic_factor / self.wave_number)
        } else {
            Complex64::new(0.0, 0.0)
        }
    }

    /// Bounded Burton-Miller coupling: β = i / (k + k_ref)
    ///
    /// This formulation avoids the 1/k divergence at low frequencies.
    /// A typical choice for k_ref is 1/(average element size), which ensures
    /// the hypersingular contribution is properly scaled relative to the
    /// geometry-dependent edge integral term.
    ///
    /// # Arguments
    /// * `k_ref` - Reference wavenumber (should be ~1/element_size for best conditioning)
    pub fn burton_miller_beta_bounded(&self, k_ref: f64) -> Complex64 {
        if self.tau > 0.0 {
            Complex64::new(0.0, self.harmonic_factor / (self.wave_number + k_ref))
        } else {
            Complex64::new(0.0, 0.0)
        }
    }

    /// Floored Burton-Miller beta to prevent ill-conditioning at high frequencies
    ///
    /// At high k, the traditional β = i/k becomes very small, making the matrix
    /// diagonal shrink faster than the RHS grows. This leads to O(k²) solution error.
    ///
    /// This method uses β = i * max(1/k, η_min) where η_min is chosen to keep
    /// |β * E_self| bounded away from zero.
    ///
    /// # Arguments
    /// * `edge_e_magnitude` - Approximate magnitude of the edge E integral (~50-100 typical)
    /// * `min_beta_e` - Minimum acceptable |β*E| (default: 5.0 for good conditioning)
    pub fn burton_miller_beta_floored(&self, edge_e_magnitude: f64, min_beta_e: f64) -> Complex64 {
        if self.tau > 0.0 {
            // Traditional: η = 1/k
            let eta_traditional = 1.0 / self.wave_number;

            // Minimum η to keep |β*E| ≥ min_beta_e
            let eta_min = min_beta_e / edge_e_magnitude;

            // Use the larger of the two
            let eta = eta_traditional.max(eta_min);

            Complex64::new(0.0, self.harmonic_factor * eta)
        } else {
            Complex64::new(0.0, 0.0)
        }
    }

    /// Optimal Burton-Miller beta based on mesh size (bounded version)
    ///
    /// Uses β = i/(k + k_ref) to bound β at low frequencies.
    /// Note: This may not help at high frequencies where the problem is different.
    ///
    /// # Arguments
    /// * `element_size` - Characteristic element size (edge length or sqrt(area))
    pub fn burton_miller_beta_optimal(&self, element_size: f64) -> Complex64 {
        if self.tau > 0.0 {
            let k_ref = 1.0 / element_size;
            Complex64::new(0.0, self.harmonic_factor / (self.wave_number + k_ref))
        } else {
            Complex64::new(0.0, 0.0)
        }
    }

    /// Scaled Burton-Miller beta for better numerical conditioning
    ///
    /// Uses β = scale × i/k where scale > 1 improves diagonal dominance.
    /// Testing shows scale = 2.0 gives significantly better accuracy than the
    /// standard β = i/k, especially at low to medium frequencies (ka < 2).
    ///
    /// The improvement comes from increased diagonal dominance which improves
    /// matrix conditioning, outweighing the slight increase in row sum bias.
    ///
    /// # Arguments
    /// * `scale` - Scaling factor (recommended: 2.0)
    pub fn burton_miller_beta_scaled(&self, scale: f64) -> Complex64 {
        if self.tau > 0.0 {
            Complex64::new(0.0, self.harmonic_factor * scale / self.wave_number)
        } else {
            Complex64::new(0.0, 0.0)
        }
    }

    /// Adaptive Burton-Miller beta that varies with frequency
    ///
    /// Automatically selects the optimal β scale based on ka (dimensionless frequency).
    /// Empirically determined from testing against Mie analytical solutions:
    ///
    /// | ka range | Optimal β scale | formulation |
    /// |----------|-----------------|-------------|
    /// | < 0.5    | 1.0             | Fixed (+K)  |
    /// | 0.5-0.92 | 4.0             | Orig (-K)   |
    /// | 0.92-1.2 | 4.0             | Orig (-K)   |
    /// | 1.2-1.8  | 8.0             | Orig (-K)   |
    /// | > 1.8    | 16.0            | Orig (-K)   |
    ///
    /// This provides stability at low frequencies (where +K formulation is used)
    /// and accuracy at resonances (where -K formulation is used).
    ///
    /// # Arguments
    /// * `radius` - Characteristic size of the scatterer (e.g., sphere radius)
    ///
    /// # Returns
    /// Tuple of (beta, scale) where scale is the selected β scale factor
    pub fn burton_miller_beta_adaptive(&self, radius: f64) -> (Complex64, f64) {
        if self.tau <= 0.0 {
            return (Complex64::new(0.0, 0.0), 1.0);
        }

        // Compute ka = k * radius (dimensionless frequency)
        let ka = self.wave_number * radius;

        // Select optimal scale based on ka
        // These values were empirically determined from Mie solution comparisons
        let scale = if ka < 0.5 {
            1.0 // Low frequencies with Fixed Formulation (+K) are stable, standard beta works best
        } else if ka < 1.2 {
            4.0 // Transition region / Sweet spot around ka=1
        } else if ka < 1.8 {
            8.0 // Intermediate range
        } else {
            16.0 // High frequencies also benefit from high β
        };

        let beta = Complex64::new(0.0, self.harmonic_factor * scale / self.wave_number);
        (beta, scale)
    }

    /// Get optimal β scale for a given ka value
    ///
    /// Returns just the scale factor without computing β.
    /// Useful when you need to know the scale for matrix assembly.
    pub fn optimal_beta_scale(ka: f64) -> f64 {
        if ka < 0.85 {
            32.0
        } else if ka < 0.92 {
            8.0
        } else if ka < 1.2 {
            4.0
        } else if ka < 1.8 {
            8.0
        } else {
            16.0
        }
    }

    /// Coupling constant γ (typically 1.0)
    pub fn gamma(&self) -> f64 {
        1.0
    }
}

// ============================================================================
// Mesh Structures
// ============================================================================

/// Element type (triangular or quadrilateral)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ElementType {
    /// 3-node triangle
    Tri3,
    /// 4-node quadrilateral
    Quad4,
}

impl ElementType {
    /// Number of nodes for this element type
    pub fn num_nodes(&self) -> usize {
        match self {
            ElementType::Tri3 => 3,
            ElementType::Quad4 => 4,
        }
    }
}

/// Element property (from listElementProperty in C++)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ElementProperty {
    /// Surface element (boundary)
    Surface = 0,
    /// Middle face element
    MidFace = 1,
    /// Evaluation element (for post-processing)
    Evaluation = 2,
}

impl ElementProperty {
    /// Check if this is an evaluation element
    pub fn is_evaluation(&self) -> bool {
        matches!(self, ElementProperty::Evaluation)
    }

    /// Check if this is a surface element
    pub fn is_surface(&self) -> bool {
        matches!(self, ElementProperty::Surface)
    }
}

/// Boundary condition type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BoundaryCondition {
    /// Velocity prescribed (Neumann BC)
    Velocity(Vec<Complex64>),
    /// Pressure prescribed (Dirichlet BC)
    Pressure(Vec<Complex64>),
    /// Velocity with surface admittance
    VelocityWithAdmittance {
        /// Velocity values at each node
        velocity: Vec<Complex64>,
        /// Surface admittance value
        admittance: Complex64,
    },
    /// Transfer admittance
    TransferAdmittance {
        /// Transfer admittance value
        admittance: Complex64,
    },
    /// Transfer with surface admittance
    TransferWithSurfaceAdmittance {
        /// Transfer admittance value
        transfer_admittance: Complex64,
        /// Surface admittance value
        surface_admittance: Complex64,
    },
}

impl BoundaryCondition {
    /// Get the boundary condition type index (matching ibval in C++)
    pub fn type_index(&self) -> i32 {
        match self {
            BoundaryCondition::Velocity(_) => 0,
            BoundaryCondition::Pressure(_) => 1,
            BoundaryCondition::VelocityWithAdmittance { .. } => 2,
            BoundaryCondition::TransferAdmittance { .. } => 3,
            BoundaryCondition::TransferWithSurfaceAdmittance { .. } => 4,
        }
    }

    /// Returns true if admittance is prescribed
    pub fn has_admittance(&self) -> bool {
        matches!(
            self,
            BoundaryCondition::VelocityWithAdmittance { .. }
                | BoundaryCondition::TransferWithSurfaceAdmittance { .. }
        )
    }

    /// Get admittance value if present
    pub fn admittance(&self) -> Option<Complex64> {
        match self {
            BoundaryCondition::VelocityWithAdmittance { admittance, .. } => Some(*admittance),
            BoundaryCondition::TransferWithSurfaceAdmittance {
                surface_admittance, ..
            } => Some(*surface_admittance),
            _ => None,
        }
    }
}

/// A boundary element
#[derive(Debug, Clone)]
pub struct Element {
    /// Global node indices (connectivity)
    pub connectivity: Vec<usize>,
    /// Element type (Tri3 or Quad4)
    pub element_type: ElementType,
    /// Element property (Surface, MidFace, Evaluation)
    pub property: ElementProperty,
    /// Unit normal vector at element center (3 components)
    pub normal: Array1<f64>,
    /// Normal vectors at each node (for higher-order normals)
    pub node_normals: Array2<f64>,
    /// Element center (centroid) coordinates
    pub center: Array1<f64>,
    /// Element area
    pub area: f64,
    /// Boundary condition
    pub boundary_condition: BoundaryCondition,
    /// Element group index
    pub group: usize,
    /// DOF addresses (row numbers in coefficient matrix)
    pub dof_addresses: Vec<usize>,
}

impl Element {
    /// Number of nodes
    pub fn num_nodes(&self) -> usize {
        self.element_type.num_nodes()
    }

    /// Number of DOFs per element (typically 1 for constant elements)
    pub fn num_dofs(&self) -> usize {
        // For collocation BEM with constant elements
        1
    }
}

/// Boundary element mesh
#[derive(Debug, Clone)]
pub struct Mesh {
    /// Node coordinates (num_nodes × 3)
    pub nodes: Array2<f64>,
    /// Elements
    pub elements: Vec<Element>,
    /// External node numbers (for output compatibility)
    pub external_node_numbers: Vec<i32>,
    /// External element numbers (for output compatibility)
    pub external_element_numbers: Vec<i32>,
    /// Number of boundary mesh nodes
    pub num_boundary_nodes: usize,
    /// Number of evaluation mesh nodes
    pub num_evaluation_nodes: usize,
    /// Number of boundary elements
    pub num_boundary_elements: usize,
    /// Number of evaluation elements
    pub num_evaluation_elements: usize,
    /// Symmetry planes (0 = none, +1 = symmetric, -1 = antisymmetric)
    pub symmetry_planes: [i32; 3],
    /// Symmetry plane coordinates
    pub symmetry_coordinates: [f64; 3],
    /// Number of element reflections due to symmetry
    pub num_reflections: usize,
}

impl Mesh {
    /// Total number of nodes
    pub fn num_nodes(&self) -> usize {
        self.nodes.nrows()
    }

    /// Total number of elements
    pub fn num_elements(&self) -> usize {
        self.elements.len()
    }

    /// Number of DOFs (rows in coefficient matrix)
    pub fn num_dofs(&self) -> usize {
        // For collocation BEM: one DOF per surface element
        self.elements
            .iter()
            .filter(|e| e.property != ElementProperty::Evaluation)
            .count()
    }

    /// Get node coordinates for an element
    pub fn element_nodes(&self, element: &Element) -> Array2<f64> {
        let n = element.num_nodes();
        let mut coords = Array2::zeros((n, 3));
        for (i, &node_idx) in element.connectivity.iter().enumerate() {
            for j in 0..3 {
                coords[[i, j]] = self.nodes[[node_idx, j]];
            }
        }
        coords
    }

    /// Average element area
    pub fn average_element_area(&self) -> f64 {
        let surface_elements: Vec<_> = self
            .elements
            .iter()
            .filter(|e| e.property == ElementProperty::Surface)
            .collect();
        if surface_elements.is_empty() {
            return 0.0;
        }
        surface_elements.iter().map(|e| e.area).sum::<f64>() / surface_elements.len() as f64
    }
}

// ============================================================================
// Cluster Structures (for FMM)
// ============================================================================

/// A cluster of boundary elements (for Fast Multipole Method)
#[derive(Debug, Clone)]
pub struct Cluster {
    /// Number of the element group to which the cluster belongs
    pub element_group: usize,
    /// Number of elements in the cluster
    pub num_elements: usize,
    /// Element property (Surface, MidFace, Evaluation)
    pub element_property: ElementProperty,
    /// True if all elements have the same number of nodes
    pub is_monotype: bool,
    /// True if admittance boundary conditions are prescribed
    pub has_admittance: bool,
    /// Number of unknown DOFs in the cluster
    pub num_dofs: usize,
    /// Number of DOFs per element
    pub dofs_per_element: usize,
    /// Element indices in this cluster
    pub element_indices: Vec<usize>,
    /// Center coordinates of the cluster
    pub center: Array1<f64>,
    /// Radius of the cluster (distance from center to farthest element)
    pub radius: f64,
    /// Near-field cluster indices
    pub near_clusters: Vec<usize>,
    /// Far-field cluster indices
    pub far_clusters: Vec<usize>,
    /// Original cluster index (for reflections)
    pub original_cluster: usize,
    /// Reflection number (0-7)
    pub reflection_number: usize,
    /// Reflection factor (+1 or -1)
    pub reflection_factor: i32,
    /// Mirror image flag
    pub is_mirror: bool,
    /// Reflection directions [x, y, z]
    pub reflection_dirs: [bool; 3],
    /// Father cluster index (for multi-level)
    pub father: Option<usize>,
    /// Son cluster indices (for multi-level)
    pub sons: Vec<usize>,
    /// Level in the cluster tree
    pub level: usize,
    /// Fan-out clusters (far-field but fathers are near-field)
    pub fan_clusters: Vec<usize>,
}

impl Cluster {
    /// Create a new empty cluster
    pub fn new(center: Array1<f64>) -> Self {
        Self {
            element_group: 0,
            num_elements: 0,
            element_property: ElementProperty::Surface,
            is_monotype: true,
            has_admittance: false,
            num_dofs: 0,
            dofs_per_element: 1,
            element_indices: Vec::new(),
            center,
            radius: 0.0,
            near_clusters: Vec::new(),
            far_clusters: Vec::new(),
            original_cluster: 0,
            reflection_number: 0,
            reflection_factor: 1,
            is_mirror: false,
            reflection_dirs: [false, false, false],
            father: None,
            sons: Vec::new(),
            level: 0,
            fan_clusters: Vec::new(),
        }
    }
}

/// Parameters for a cluster tree level (for multi-level FMM)
#[derive(Debug, Clone)]
pub struct ClusterLevel {
    /// All clusters at this level
    pub clusters: Vec<Cluster>,
    /// Number of original (non-reflected) clusters
    pub num_original: usize,
    /// Maximum cluster radius at this level
    pub max_radius: f64,
    /// Average cluster radius
    pub avg_radius: f64,
    /// Minimum cluster radius
    pub min_radius: f64,
    /// Number of expansion terms for FMM
    pub expansion_terms: usize,
    /// Number of integration points on unit sphere
    pub sphere_points: usize,
    /// Number of theta direction points
    pub theta_points: usize,
    /// Number of phi direction points
    pub phi_points: usize,
    /// Gauss points coordinates in theta direction
    pub gauss_theta: Vec<f64>,
    /// Gauss weights in theta direction
    pub gauss_weights: Vec<f64>,
    /// Unit sphere quadrature points (x, y, z for each point)
    pub sphere_coords: Array2<f64>,
    /// Unit sphere quadrature weights
    pub sphere_weights: Vec<f64>,
}

impl ClusterLevel {
    /// Create a new cluster level with given expected capacity
    pub fn new(expected_clusters: usize) -> Self {
        Self {
            clusters: Vec::with_capacity(expected_clusters),
            num_original: 0,
            max_radius: 0.0,
            avg_radius: 0.0,
            min_radius: f64::MAX,
            expansion_terms: 4,
            sphere_points: 0,
            theta_points: 4,
            phi_points: 8,
            gauss_theta: Vec::new(),
            gauss_weights: Vec::new(),
            sphere_coords: Array2::zeros((0, 3)),
            sphere_weights: Vec::new(),
        }
    }
}

// ============================================================================
// Solver Configuration
// ============================================================================

/// BEM method selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BemMethod {
    /// Traditional BEM (O(N²) dense matrix)
    Traditional = 0,
    /// Single-Level Fast Multipole BEM
    SingleLevelFmm = 1,
    /// Multi-Level Fast Multipole BEM
    MultiLevelFmm = 3,
}

/// Linear solver selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SolverMethod {
    /// Conjugate Gradient Squared
    Cgs = 0,
    /// Quasi-Minimal Residual CGSTAB
    QmrCgstab = 1,
    /// Stabilized BiCGSTAB
    BiCgstabStabilized = 2,
    /// BiCGSTAB
    BiCgstab = 3,
    /// Direct Gaussian elimination (LU factorization)
    Direct = 4,
}

/// Preconditioner selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Preconditioner {
    /// Incomplete LU, second order scanning
    IluSecondOrder = -2,
    /// Incomplete LU, first order scanning
    IluFirstOrder = -1,
    /// Incomplete LU, zero order scanning
    IluZeroOrder = 0,
    /// Diagonal scaling
    Scaling = 1,
    /// No preconditioner
    None = 2,
}

/// Solver configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverConfig {
    /// BEM method
    pub method: BemMethod,
    /// Linear solver
    pub solver: SolverMethod,
    /// Preconditioner
    pub preconditioner: Preconditioner,
    /// Maximum iterations for iterative solvers
    pub max_iterations: usize,
    /// Convergence tolerance
    pub tolerance: f64,
    /// Minimum expansion terms for FMM
    pub min_expansion_terms: usize,
    /// Far-field cluster factor (typically 2/√3 ≈ 1.155)
    pub far_field_factor: f64,
    /// Average cluster edge length (0 = auto)
    pub cluster_edge_length: f64,
}

impl Default for SolverConfig {
    fn default() -> Self {
        Self {
            method: BemMethod::Traditional,
            solver: SolverMethod::Cgs,
            preconditioner: Preconditioner::IluZeroOrder,
            max_iterations: 250,
            tolerance: 1e-6,
            min_expansion_terms: 5,
            far_field_factor: 2.0 / 3.0_f64.sqrt(),
            cluster_edge_length: 0.0,
        }
    }
}

// ============================================================================
// Incident Waves
// ============================================================================

/// Incident plane wave
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaneWave {
    /// Direction of propagation (unit vector)
    pub direction: Array1<f64>,
    /// Amplitude (complex)
    pub amplitude: Complex64,
    /// Curve reference for frequency-dependent amplitude
    pub curve_ref: Option<usize>,
}

/// Point source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointSource {
    /// Location of the source
    pub position: Array1<f64>,
    /// Source strength (complex)
    pub strength: Complex64,
    /// Curve reference for frequency-dependent strength
    pub curve_ref: Option<usize>,
}

// ============================================================================
// Results
// ============================================================================

/// Solver result
#[derive(Debug, Clone)]
pub struct SolverResult {
    /// Solution vector (pressure or velocity potential at DOFs)
    pub solution: Array1<Complex64>,
    /// Number of iterations (for iterative solvers)
    pub iterations: usize,
    /// Final residual norm
    pub residual: f64,
    /// Converged flag
    pub converged: bool,
    /// Solve time in seconds
    pub solve_time: f64,
}

impl SolverResult {
    /// Create a converged result
    pub fn converged(solution: Array1<Complex64>, iterations: usize, residual: f64) -> Self {
        Self {
            solution,
            iterations,
            residual,
            converged: true,
            solve_time: 0.0,
        }
    }

    /// Create a non-converged result
    pub fn not_converged(solution: Array1<Complex64>, iterations: usize, residual: f64) -> Self {
        Self {
            solution,
            iterations,
            residual,
            converged: false,
            solve_time: 0.0,
        }
    }
}

/// Integration result for element kernels
#[derive(Debug, Clone, Default)]
pub struct IntegrationResult {
    /// ∫ G dS (single layer potential)
    pub g_integral: Complex64,
    /// ∫ ∂G/∂n_y dS (double layer potential)
    pub dg_dn_integral: Complex64,
    /// ∫ ∂G/∂n_x dS (adjoint double layer)
    pub dg_dnx_integral: Complex64,
    /// ∫ ∂²G/(∂n_x ∂n_y) dS (hypersingular)
    pub d2g_dnxdny_integral: Complex64,
    /// Contribution to right-hand side
    pub rhs_contribution: Complex64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;

    #[test]
    fn test_physics_params() {
        let params = PhysicsParams::new(1000.0, 343.0, 1.21, false);
        assert!((params.wave_number - 2.0 * std::f64::consts::PI * 1000.0 / 343.0).abs() < 1e-10);
        assert!((params.wave_length - 0.343).abs() < 1e-10);
        assert_eq!(params.tau, 1.0);
    }

    #[test]
    fn test_element_type() {
        assert_eq!(ElementType::Tri3.num_nodes(), 3);
        assert_eq!(ElementType::Quad4.num_nodes(), 4);
    }

    #[test]
    fn test_boundary_condition() {
        let bc = BoundaryCondition::Velocity(vec![Complex64::new(1.0, 0.0)]);
        assert_eq!(bc.type_index(), 0);
        assert!(!bc.has_admittance());

        let bc = BoundaryCondition::VelocityWithAdmittance {
            velocity: vec![Complex64::new(1.0, 0.0)],
            admittance: Complex64::new(0.1, 0.0),
        };
        assert!(bc.has_admittance());
        assert_eq!(bc.admittance(), Some(Complex64::new(0.1, 0.0)));
    }

    #[test]
    fn test_cluster_creation() {
        let center = array![0.0, 0.0, 0.0];
        let cluster = Cluster::new(center);
        assert_eq!(cluster.num_elements, 0);
        assert_eq!(cluster.reflection_factor, 1);
    }
}
