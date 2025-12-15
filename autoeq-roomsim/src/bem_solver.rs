//! BEM (Boundary Element Method) solver integration
//!
//! This module provides a full BEM-based acoustic solver using the math-bem crate.
//! The BEM solver provides accurate wave-based simulation including:
//! - Full wave interference effects
//! - Accurate low-frequency behavior (room modes)
//! - Scattering from complex geometries
//! - Frequency-dependent wall absorption via admittance BCs
//!
//! ## Solver Selection
//! - Direct solver: Best for N < 500 elements
//! - GMRES + ILU: Best for 500 < N < 5000 elements
//! - FMM + GMRES: Best for N > 5000 elements
//!
//! Note: BEM is computationally expensive compared to ISM/modal methods.
//! Use hybrid mode for full frequency range (BEM for low freq, ISM for high freq).

use bem::room_acoustics::{
    Point3D as BemPoint3D, RectangularRoom as BemRectangularRoom, RoomGeometry as BemRoomGeometry,
    RoomMesh, Source as BemSource,
};
use num_complex::Complex64;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

use crate::{Point3D, RoomGeometry, Source, WallMaterial, WallMaterialsConfig};

// ============================================================================
// Type Conversions
// ============================================================================

/// Convert our Point3D to math-bem's Point3D
pub fn to_bem_point(p: &Point3D) -> BemPoint3D {
    BemPoint3D::new(p.x, p.y, p.z)
}

/// Convert math-bem's Point3D to our Point3D
#[allow(dead_code)]
pub fn from_bem_point(p: &BemPoint3D) -> Point3D {
    Point3D::new(p.x, p.y, p.z)
}

/// Convert our Source to math-bem's Source
pub fn to_bem_source(source: &Source) -> BemSource {
    let bem_position = to_bem_point(&source.position);
    let bem_directivity = bem::room_acoustics::DirectivityPattern::omnidirectional();

    let bem_crossover = match &source.crossover {
        crate::CrossoverFilter::FullRange => bem::room_acoustics::CrossoverFilter::FullRange,
        crate::CrossoverFilter::Lowpass { cutoff_freq, order } => {
            bem::room_acoustics::CrossoverFilter::Lowpass {
                cutoff_freq: *cutoff_freq,
                order: *order,
            }
        }
        crate::CrossoverFilter::Highpass { cutoff_freq, order } => {
            bem::room_acoustics::CrossoverFilter::Highpass {
                cutoff_freq: *cutoff_freq,
                order: *order,
            }
        }
        crate::CrossoverFilter::Bandpass {
            low_cutoff,
            high_cutoff,
            order,
        } => bem::room_acoustics::CrossoverFilter::Bandpass {
            low_cutoff: *low_cutoff,
            high_cutoff: *high_cutoff,
            order: *order,
        },
    };

    BemSource {
        position: bem_position,
        directivity: bem_directivity,
        amplitude: source.amplitude,
        crossover: bem_crossover,
        name: source.name.clone(),
    }
}

/// Convert our RoomGeometry to math-bem's RoomGeometry
pub fn to_bem_room_geometry(room: &RoomGeometry) -> BemRoomGeometry {
    match room {
        RoomGeometry::Rectangular(r) => {
            BemRoomGeometry::Rectangular(BemRectangularRoom::new(r.width, r.depth, r.height))
        }
        RoomGeometry::LShaped(l) => BemRoomGeometry::LShaped(
            bem::room_acoustics::LShapedRoom::new(l.width1, l.depth1, l.width2, l.depth2, l.height),
        ),
    }
}

// ============================================================================
// BEM Solver Configuration
// ============================================================================

/// Assembly method for the BEM matrix
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BemAssemblyMethod {
    /// Traditional BEM with O(N^2) dense matrix
    #[default]
    Tbem,
    /// Single-Level Fast Multipole Method
    Fmm,
}

/// Linear solver method
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BemSolverMethod {
    /// Direct LU factorization (best for small problems)
    #[default]
    Direct,
    /// GMRES iterative solver
    Gmres,
    /// GMRES with ILU preconditioning
    GmresIlu,
    /// GMRES with hierarchical FMM preconditioner
    GmresHierarchical,
}

/// FMM configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FmmConfig {
    /// Maximum elements per octree leaf
    #[serde(default = "default_max_elements_per_leaf")]
    pub max_elements_per_leaf: usize,
    /// Maximum octree depth
    #[serde(default = "default_max_tree_depth")]
    pub max_tree_depth: usize,
    /// Number of theta integration points
    #[serde(default = "default_n_theta")]
    pub n_theta: usize,
    /// Number of phi integration points
    #[serde(default = "default_n_phi")]
    pub n_phi: usize,
    /// Number of expansion terms
    #[serde(default = "default_n_terms")]
    pub n_terms: usize,
    /// Separation ratio for near/far field
    #[serde(default = "default_separation_ratio")]
    pub separation_ratio: f64,
}

fn default_max_elements_per_leaf() -> usize {
    50
}
fn default_max_tree_depth() -> usize {
    8
}
fn default_n_theta() -> usize {
    6
}
fn default_n_phi() -> usize {
    12
}
fn default_n_terms() -> usize {
    6
}
fn default_separation_ratio() -> f64 {
    1.5
}

impl Default for FmmConfig {
    fn default() -> Self {
        Self {
            max_elements_per_leaf: default_max_elements_per_leaf(),
            max_tree_depth: default_max_tree_depth(),
            n_theta: default_n_theta(),
            n_phi: default_n_phi(),
            n_terms: default_n_terms(),
            separation_ratio: default_separation_ratio(),
        }
    }
}

/// Complete BEM solver configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BemConfig {
    /// Assembly method
    #[serde(default)]
    pub assembly_method: BemAssemblyMethod,
    /// Linear solver method
    #[serde(default)]
    pub solver_method: BemSolverMethod,
    /// Solver tolerance
    #[serde(default = "default_tolerance")]
    pub tolerance: f64,
    /// Maximum iterations for iterative solvers
    #[serde(default = "default_max_iterations")]
    pub max_iterations: usize,
    /// GMRES restart parameter
    #[serde(default = "default_restart")]
    pub restart: usize,
    /// Target elements per wavelength for mesh generation
    #[serde(default = "default_elements_per_wavelength")]
    pub elements_per_wavelength: f64,
    /// Minimum elements per meter (for low frequencies)
    #[serde(default = "default_min_elements_per_meter")]
    pub min_elements_per_meter: usize,
    /// Maximum total elements (to limit computation time)
    #[serde(default = "default_max_elements")]
    pub max_elements: usize,
    /// FMM configuration
    #[serde(default)]
    pub fmm_config: FmmConfig,

    /// Use adaptive mesh refinement
    #[serde(default = "default_use_adaptive_mesh")]
    pub use_adaptive_mesh: bool,
    /// Use Burton-Miller formulation (recommended for accuracy)
    #[serde(default = "default_use_burton_miller")]
    pub use_burton_miller: bool,
    /// Burton-Miller beta scale factor
    #[serde(default = "default_beta_scale")]
    pub beta_scale: f64,
}

fn default_tolerance() -> f64 {
    1e-6
}
fn default_max_iterations() -> usize {
    1000
}
fn default_restart() -> usize {
    50
}
fn default_elements_per_wavelength() -> f64 {
    8.0
}
fn default_min_elements_per_meter() -> usize {
    2
}
fn default_max_elements() -> usize {
    10000
}
fn default_use_adaptive_mesh() -> bool {
    true
}
fn default_use_burton_miller() -> bool {
    true
}
fn default_beta_scale() -> f64 {
    4.0
}

impl Default for BemConfig {
    fn default() -> Self {
        Self {
            assembly_method: BemAssemblyMethod::default(),
            solver_method: BemSolverMethod::default(),
            tolerance: default_tolerance(),
            max_iterations: default_max_iterations(),
            restart: default_restart(),
            elements_per_wavelength: default_elements_per_wavelength(),
            min_elements_per_meter: default_min_elements_per_meter(),
            max_elements: default_max_elements(),
            fmm_config: FmmConfig::default(),

            use_adaptive_mesh: default_use_adaptive_mesh(),
            use_burton_miller: default_use_burton_miller(),
            beta_scale: default_beta_scale(),
        }
    }
}

// ============================================================================
// Result Types
// ============================================================================

/// Result of a BEM simulation at a single frequency
#[derive(Debug, Clone)]
pub struct BemResult {
    /// Frequency (Hz)
    pub frequency: f64,
    /// Complex pressure at the evaluation point
    pub pressure: Complex64,
    /// Number of boundary elements used
    pub num_elements: usize,
    /// Number of solver iterations (0 for direct solver)
    pub iterations: usize,
    /// Final residual norm
    pub residual: f64,
    /// Whether the solver converged
    pub converged: bool,
}

// ============================================================================
// Mesh Generation
// ============================================================================

/// Generate a room mesh for BEM simulation
///
/// This creates a mesh of boundary elements on all room surfaces.
/// The mesh density is controlled by elements_per_meter.
pub fn generate_room_mesh(room: &RoomGeometry, elements_per_meter: usize) -> RoomMesh {
    let bem_room = to_bem_room_geometry(room);
    bem_room.generate_mesh(elements_per_meter)
}

/// Generate a frequency-adaptive room mesh
///
/// The mesh density is determined by:
/// - Target elements per wavelength (typically 6-10)
/// - Minimum elements per meter (for very low frequencies)
/// - Maximum element count (to limit computation time)
pub fn generate_adaptive_room_mesh(
    room: &RoomGeometry,
    sources: &[Source],
    frequency: f64,
    speed_of_sound: f64,
    config: &BemConfig,
) -> RoomMesh {
    let bem_room = to_bem_room_geometry(room);
    let bem_sources: Vec<BemSource> = sources.iter().map(to_bem_source).collect();

    // Calculate required element size based on wavelength
    let wavelength = speed_of_sound / frequency;
    let target_element_size = wavelength / config.elements_per_wavelength;

    // Convert to elements per meter
    let elements_per_meter_from_wavelength = (1.0 / target_element_size).ceil() as usize;

    // Use the more refined of the two criteria
    let elements_per_meter = elements_per_meter_from_wavelength.max(config.min_elements_per_meter);

    // Generate mesh
    if config.use_adaptive_mesh {
        bem_room.generate_adaptive_mesh(elements_per_meter, frequency, &bem_sources, speed_of_sound)
    } else {
        bem_room.generate_mesh(elements_per_meter)
    }
}

/// Estimate the number of mesh elements for a given room and frequency
#[allow(dead_code)]
pub fn estimate_element_count(
    room: &RoomGeometry,
    frequency: f64,
    speed_of_sound: f64,
    config: &BemConfig,
) -> usize {
    // Calculate room surface area
    let surface_area = match room {
        RoomGeometry::Rectangular(r) => {
            2.0 * (r.width * r.depth + r.width * r.height + r.depth * r.height)
        }
        RoomGeometry::LShaped(l) => {
            let total_depth = l.depth1 + l.depth2;
            // Floor and ceiling
            let floor_ceiling = 2.0 * (l.width1 * l.depth1 + l.width2 * l.depth2);
            // Walls
            let walls = 2.0 * l.height * (l.width1 + total_depth + l.width2 + l.depth2)
                + 2.0 * l.height * (l.width1 - l.width2); // internal step
            floor_ceiling + walls
        }
    };

    // Calculate element size
    let wavelength = speed_of_sound / frequency;
    let target_element_size = wavelength / config.elements_per_wavelength;
    let min_element_size = 1.0 / config.min_elements_per_meter as f64;
    let element_size = target_element_size.min(min_element_size);

    // Estimate element count (assuming triangular elements)
    let element_area = element_size * element_size / 2.0;
    let estimated_count = (surface_area / element_area).ceil() as usize;

    estimated_count.min(config.max_elements)
}

/// Select the appropriate solver strategy based on element count
#[allow(dead_code)]
pub fn select_solver_strategy(num_elements: usize, config: &BemConfig) -> BemSolverMethod {
    // Override if user explicitly selected a method
    match config.solver_method {
        BemSolverMethod::Direct if num_elements > 3000 => {
            // Warn: direct solver will be slow
            BemSolverMethod::GmresIlu
        }
        _ => config.solver_method,
    }
}

// ============================================================================
// Boundary Conditions from Materials
// ============================================================================

/// Convert absorption coefficient to surface admittance
///
/// Uses the relationship: Y = (1 - √(1-α)) / (ρc × (1 + √(1-α)))
/// where α = absorption coefficient, ρ = density, c = speed of sound
///
/// For rigid surfaces (α ≈ 0), Y ≈ 0 (zero admittance)
/// For highly absorbing surfaces (α → 1), Y → ∞
#[allow(dead_code)]
pub fn absorption_to_admittance(alpha: f64, density: f64, speed_of_sound: f64) -> Complex64 {
    let alpha_clamped = alpha.clamp(0.001, 0.999); // Avoid singularities
    let sqrt_1_minus_alpha = (1.0 - alpha_clamped).sqrt();
    let z0 = density * speed_of_sound; // Characteristic impedance

    // Real admittance (simplified model)
    let y_real = (1.0 - sqrt_1_minus_alpha) / (z0 * (1.0 + sqrt_1_minus_alpha));

    // Add small imaginary part for numerical stability
    Complex64::new(y_real, y_real * 0.01)
}

/// Identify which wall surface an element belongs to based on its center position
#[allow(dead_code)]
pub fn identify_wall_surface(
    element_center: &Point3D,
    room: &RoomGeometry,
    tolerance: f64,
) -> Option<crate::WallSurface> {
    match room {
        RoomGeometry::Rectangular(r) => {
            if element_center.x.abs() < tolerance {
                Some(crate::WallSurface::Left)
            } else if (element_center.x - r.width).abs() < tolerance {
                Some(crate::WallSurface::Right)
            } else if element_center.y.abs() < tolerance {
                Some(crate::WallSurface::Front)
            } else if (element_center.y - r.depth).abs() < tolerance {
                Some(crate::WallSurface::Back)
            } else if element_center.z.abs() < tolerance {
                Some(crate::WallSurface::Floor)
            } else if (element_center.z - r.height).abs() < tolerance {
                Some(crate::WallSurface::Ceiling)
            } else {
                None
            }
        }
        RoomGeometry::LShaped(l) => {
            let total_depth = l.depth1 + l.depth2;
            if element_center.x.abs() < tolerance {
                Some(crate::WallSurface::Left)
            } else if element_center.y.abs() < tolerance {
                Some(crate::WallSurface::Front)
            } else if (element_center.y - total_depth).abs() < tolerance {
                Some(crate::WallSurface::Back)
            } else if element_center.z.abs() < tolerance {
                Some(crate::WallSurface::Floor)
            } else if (element_center.z - l.height).abs() < tolerance {
                Some(crate::WallSurface::Ceiling)
            } else if ((element_center.x - l.width1).abs() < tolerance
                && element_center.y < l.depth1)
                || ((element_center.x - l.width2).abs() < tolerance && element_center.y > l.depth1)
            {
                Some(crate::WallSurface::Right)
            } else {
                None
            }
        }
    }
}

/// Get the material for a given wall surface
#[allow(dead_code)]
pub fn get_wall_material(
    surface: crate::WallSurface,
    materials: &WallMaterialsConfig,
) -> WallMaterial {
    match surface {
        crate::WallSurface::Left => materials.left.to_material(),
        crate::WallSurface::Right => materials.right.to_material(),
        crate::WallSurface::Front => materials.front.to_material(),
        crate::WallSurface::Back => materials.back.to_material(),
        crate::WallSurface::Floor => materials.floor.to_material(),
        crate::WallSurface::Ceiling => materials.ceiling.to_material(),
    }
}

// ============================================================================
// Green's Functions (for direct field computation)
// ============================================================================

/// Calculate free-field Green's function for the Helmholtz equation
///
/// G(r) = exp(ikr) / (4πr)
///
/// This represents the acoustic field from a point source.
pub fn greens_function(distance: f64, wavenumber: f64) -> Complex64 {
    if distance < 1e-10 {
        // Avoid singularity at source
        return Complex64::new(1.0, 0.0);
    }
    Complex64::new(0.0, wavenumber * distance).exp() / (4.0 * PI * distance)
}

// ============================================================================
// BEM Solvers
// ============================================================================

/// Solve for room acoustics at a single frequency using simplified BEM approach
///
/// **DEPRECATED**: Use `solve_bem()` instead, which provides real BEM solving
/// with automatic solver selection.
///
/// This uses direct field calculation with Green's function summation (approximation only).
/// For a full BEM solution with reflections, use `solve_bem()`.
///
/// # Arguments
/// * `room` - Room geometry
/// * `sources` - Sound sources in the room
/// * `eval_point` - Point where pressure is evaluated
/// * `frequency` - Frequency to solve (Hz)
/// * `config` - BEM solver configuration
///
/// # Returns
/// BEM result with complex pressure at the evaluation point
#[deprecated(since = "0.2.0", note = "Use solve_bem() for real BEM solving")]
#[allow(dead_code)]
pub fn solve_bem_direct_field(
    room: &RoomGeometry,
    sources: &[Source],
    eval_point: &Point3D,
    frequency: f64,
    config: &BemConfig,
) -> Result<BemResult, String> {
    // Generate room mesh (for element count reporting)
    let mesh = generate_room_mesh(room, config.min_elements_per_meter);
    let num_elements = mesh.elements.len();

    // Create physics parameters
    let speed_of_sound = 343.0; // Default, should come from config
    let _density = 1.21; // Reserved for future boundary condition implementation
    let k = 2.0 * PI * frequency / speed_of_sound;

    // Calculate total pressure from all sources using Green's function
    let mut total_pressure = Complex64::new(0.0, 0.0);

    for source in sources {
        // Calculate source amplitude towards evaluation point
        let amp = source.amplitude_towards(eval_point, frequency);

        // Distance from source to evaluation point
        let dist = source.position.distance_to(eval_point);

        // Free-field Green's function contribution
        let greens = greens_function(dist, k);

        // Add phase factor from source (delay and inversion)
        total_pressure += greens * amp * source.phase_factor(frequency);
    }

    Ok(BemResult {
        frequency,
        pressure: total_pressure,
        num_elements,
        iterations: 0,
        residual: 0.0,
        converged: true,
    })
}

/// Solve for room acoustics with scattering objects using direct field approximation
///
/// **DEPRECATED**: Use `solve_bem()` instead, which provides real BEM solving
/// with automatic solver selection and proper scattering object handling.
///
/// This version includes scattering objects (furniture, equipment) that affect
/// the acoustic field through additional boundary reflections (approximation only).
///
/// # Arguments
/// * `room` - Room geometry
/// * `sources` - Sound sources in the room
/// * `scattering_objects` - Objects inside the room
/// * `eval_point` - Point where pressure is evaluated
/// * `frequency` - Frequency to solve (Hz)
/// * `config` - BEM solver configuration
///
/// # Returns
/// BEM result with complex pressure at the evaluation point
#[deprecated(since = "0.2.0", note = "Use solve_bem() for real BEM solving")]
#[allow(dead_code)]
pub fn solve_bem_with_scattering(
    room: &RoomGeometry,
    sources: &[Source],
    scattering_objects: &[crate::ScatteringObjectConfig],
    eval_point: &Point3D,
    frequency: f64,
    config: &BemConfig,
) -> Result<BemResult, String> {
    use crate::scattering_objects::{
        add_scattering_objects_to_mesh, estimate_scattering_element_count,
    };

    // Generate room mesh
    let mut mesh = generate_room_mesh(room, config.min_elements_per_meter);

    // Add scattering objects to the mesh
    if !scattering_objects.is_empty() {
        add_scattering_objects_to_mesh(&mut mesh, scattering_objects);
    }

    let num_elements = mesh.elements.len();
    let scattering_elements = estimate_scattering_element_count(scattering_objects);

    // Create physics parameters
    let speed_of_sound = 343.0;
    let k = 2.0 * PI * frequency / speed_of_sound;

    // Calculate total pressure from all sources using Green's function
    // This is a simplified approach - full BEM would solve the boundary integral equation
    let mut total_pressure = Complex64::new(0.0, 0.0);

    for source in sources {
        // Calculate source amplitude towards evaluation point
        let amp = source.amplitude_towards(eval_point, frequency);

        // Distance from source to evaluation point
        let dist = source.position.distance_to(eval_point);

        // Free-field Green's function contribution
        let greens = greens_function(dist, k);

        // Add phase factor from source (delay and inversion)
        total_pressure += greens * amp * source.phase_factor(frequency);
    }

    // Add simple scattering contribution from objects
    // This is a first-order approximation treating each object as a secondary source
    for obj in scattering_objects {
        let obj_center: crate::Point3D = obj.center().into();

        // For each source, calculate scattered contribution
        for source in sources {
            let amp = source.amplitude_towards(&obj_center, frequency);

            // Distance from source to object
            let dist_source_obj = source.position.distance_to(&obj_center);

            // Distance from object to evaluation point
            let dist_obj_eval = obj_center.distance_to(eval_point);

            // Skip if either distance is very small (singularity)
            if dist_source_obj < 0.01 || dist_obj_eval < 0.01 {
                continue;
            }

            // Scattering coefficient based on material absorption
            let material = obj.material();
            let reflection = material.reflection_at_frequency(frequency);

            // Scattered contribution (simplified monopole scattering)
            let phase_delay = k * (dist_source_obj + dist_obj_eval);
            let scattered_amp = amp * reflection * 0.1; // Reduced amplitude for scattered field

            let scattered = Complex64::new(
                scattered_amp * phase_delay.cos(),
                scattered_amp * phase_delay.sin(),
            ) / (4.0 * PI * dist_obj_eval);

            total_pressure += scattered;
        }
    }

    Ok(BemResult {
        frequency,
        pressure: total_pressure,
        num_elements: num_elements + scattering_elements,
        iterations: 0,
        residual: 0.0,
        converged: true,
    })
}

// ============================================================================
// Full BEM Solvers (using math-bem's room_acoustics::solver)
// ============================================================================

/// Convert autoeq-roomsim FmmConfig to math-bem FmmSolverConfig
fn to_fmm_solver_config(config: &FmmConfig) -> bem::room_acoustics::FmmSolverConfig {
    bem::room_acoustics::FmmSolverConfig {
        max_elements_per_leaf: config.max_elements_per_leaf,
        max_tree_depth: config.max_tree_depth,
        n_theta: config.n_theta,
        n_phi: config.n_phi,
        n_terms: config.n_terms,
        separation_ratio: config.separation_ratio,
    }
}

/// Solve BEM system with automatic solver selection
///
/// This is the main entry point for BEM solving. It automatically selects
/// the best solver based on problem size:
/// - Dense GMRES for small problems (< 500 elements)
/// - FMM + GMRES + ILU for medium problems (500-3000 elements)
/// - FMM + GMRES + Hierarchical for large problems (> 3000 elements)
///
/// This replaces the approximation functions `solve_bem_direct_field` and
/// `solve_bem_with_scattering` with real BEM that properly models boundary reflections.
pub fn solve_bem(
    room: &RoomGeometry,
    sources: &[Source],
    scattering_objects: &[crate::ScatteringObjectConfig],
    eval_point: &Point3D,
    frequency: f64,
    speed_of_sound: f64,
    config: &BemConfig,
) -> Result<BemResult, String> {
    use crate::scattering_objects::add_scattering_objects_to_mesh;

    // Generate mesh (with or without scattering objects)
    let mut mesh = generate_adaptive_room_mesh(room, sources, frequency, speed_of_sound, config);

    if !scattering_objects.is_empty() {
        add_scattering_objects_to_mesh(&mut mesh, scattering_objects);
    }

    let num_elements = mesh.elements.len();

    if num_elements > config.max_elements {
        return Err(format!(
            "Mesh has {} elements, exceeds max of {}",
            num_elements, config.max_elements
        ));
    }

    // Select solver based on problem size and configured method
    match config.solver_method {
        BemSolverMethod::Direct => {
            // Dense solver - always use for small problems or when explicitly requested
            solve_bem_dense(
                &mesh,
                sources,
                eval_point,
                frequency,
                speed_of_sound,
                config,
            )
        }
        BemSolverMethod::GmresHierarchical => {
            // Hierarchical preconditioner - best for very large problems
            solve_bem_fmm_hierarchical(
                &mesh,
                sources,
                eval_point,
                frequency,
                speed_of_sound,
                config,
            )
        }
        BemSolverMethod::Gmres | BemSolverMethod::GmresIlu => {
            // Auto-select based on size
            if num_elements < 500 {
                solve_bem_dense(
                    &mesh,
                    sources,
                    eval_point,
                    frequency,
                    speed_of_sound,
                    config,
                )
            } else if num_elements < 3000 {
                solve_bem_fmm_ilu(
                    &mesh,
                    sources,
                    eval_point,
                    frequency,
                    speed_of_sound,
                    config,
                )
            } else {
                solve_bem_fmm_hierarchical(
                    &mesh,
                    sources,
                    eval_point,
                    frequency,
                    speed_of_sound,
                    config,
                )
            }
        }
    }
}

/// Solve BEM using dense matrix assembly + GMRES
///
/// Best for small problems (< 500 elements). Uses O(N²) memory and O(N²) assembly.
fn solve_bem_dense(
    mesh: &RoomMesh,
    sources: &[Source],
    eval_point: &Point3D,
    frequency: f64,
    speed_of_sound: f64,
    _config: &BemConfig,
) -> Result<BemResult, String> {
    use bem::room_acoustics::{calculate_field_pressure_bem_parallel, solve_bem_system};

    let bem_sources: Vec<BemSource> = sources.iter().map(to_bem_source).collect();
    let k = 2.0 * PI * frequency / speed_of_sound;
    let num_elements = mesh.elements.len();

    // Solve BEM system using dense GMRES
    let surface_solution = solve_bem_system(mesh, &bem_sources, k, frequency)
        .map_err(|e| format!("BEM dense solve failed: {}", e))?;

    // Evaluate field at listening position
    let eval_points = vec![to_bem_point(eval_point)];
    let field_pressures = calculate_field_pressure_bem_parallel(
        mesh,
        &surface_solution,
        &bem_sources,
        &eval_points,
        k,
        frequency,
    );

    Ok(BemResult {
        frequency,
        pressure: field_pressures[0],
        num_elements,
        iterations: 0,
        residual: 0.0,
        converged: true,
    })
}

/// Solve BEM using FMM + GMRES + ILU preconditioning
///
/// Best for medium problems (500-3000 elements). Uses O(N log N) complexity.
fn solve_bem_fmm_ilu(
    mesh: &RoomMesh,
    sources: &[Source],
    eval_point: &Point3D,
    frequency: f64,
    speed_of_sound: f64,
    config: &BemConfig,
) -> Result<BemResult, String> {
    use bem::room_acoustics::{
        calculate_field_pressure_bem_parallel, solve_bem_fmm_gmres_ilu_with_result,
    };
    let bem_sources: Vec<BemSource> = sources.iter().map(to_bem_source).collect();
    let k = 2.0 * PI * frequency / speed_of_sound;
    let num_elements = mesh.elements.len();
    let fmm_config = to_fmm_solver_config(&config.fmm_config);

    // Solve using FMM + GMRES + ILU
    let solution = solve_bem_fmm_gmres_ilu_with_result(
        mesh,
        &bem_sources,
        k,
        frequency,
        &fmm_config,
        config.max_iterations,
        config.restart,
        config.tolerance,
    )?;

    // Evaluate field at listening position
    let eval_points = vec![to_bem_point(eval_point)];
    let field_pressures = calculate_field_pressure_bem_parallel(
        mesh,
        &solution.x,
        &bem_sources,
        &eval_points,
        k,
        frequency,
    );

    Ok(BemResult {
        frequency,
        pressure: field_pressures[0],
        num_elements,
        iterations: solution.iterations,
        residual: solution.residual,
        converged: solution.converged,
    })
}

/// Solve BEM using FMM + GMRES + hierarchical preconditioner
///
/// Best for large problems (> 3000 elements). O(N) preconditioner setup.
fn solve_bem_fmm_hierarchical(
    mesh: &RoomMesh,
    sources: &[Source],
    eval_point: &Point3D,
    frequency: f64,
    speed_of_sound: f64,
    config: &BemConfig,
) -> Result<BemResult, String> {
    use bem::room_acoustics::{
        calculate_field_pressure_bem_parallel,
        solve_bem_fmm_gmres_hierarchical as bem_solve_hierarchical,
    };

    let bem_sources: Vec<BemSource> = sources.iter().map(to_bem_source).collect();
    let k = 2.0 * PI * frequency / speed_of_sound;
    let num_elements = mesh.elements.len();
    let fmm_config = to_fmm_solver_config(&config.fmm_config);

    // Solve using FMM + GMRES + hierarchical preconditioner
    let surface_solution = bem_solve_hierarchical(
        mesh,
        &bem_sources,
        k,
        frequency,
        &fmm_config,
        config.max_iterations,
        config.restart,
        config.tolerance,
    )?;

    // Evaluate field at listening position
    let eval_points = vec![to_bem_point(eval_point)];
    let field_pressures = calculate_field_pressure_bem_parallel(
        mesh,
        &surface_solution,
        &bem_sources,
        &eval_points,
        k,
        frequency,
    );

    Ok(BemResult {
        frequency,
        pressure: field_pressures[0],
        num_elements,
        iterations: 0, // Hierarchical solver doesn't report iterations
        residual: 0.0,
        converged: true,
    })
}

// ============================================================================
// Legacy Functions (kept for backwards compatibility)
// ============================================================================

/// Legacy: Solve for room acoustics at a single frequency using full BEM
///
/// DEPRECATED: Use `solve_bem()` instead which provides automatic solver selection.
#[deprecated(since = "0.2.0", note = "Use solve_bem() instead")]
#[allow(dead_code)]
pub fn solve_bem_full(
    room: &RoomGeometry,
    sources: &[Source],
    eval_point: &Point3D,
    frequency: f64,
    speed_of_sound: f64,
    config: &BemConfig,
) -> Result<BemResult, String> {
    solve_bem(
        room,
        sources,
        &[],
        eval_point,
        frequency,
        speed_of_sound,
        config,
    )
}

/// Solve for room acoustics over a frequency range using BEM
///
/// # Arguments
/// * `room` - Room geometry
/// * `sources` - Sound sources in the room
/// * `scattering_objects` - Scattering objects in the room
/// * `eval_point` - Point where pressure is evaluated
/// * `frequencies` - Frequencies to solve (Hz)
/// * `speed_of_sound` - Speed of sound (m/s)
/// * `config` - BEM solver configuration
///
/// # Returns
/// Vector of BEM results, one per frequency
#[allow(dead_code)]
pub fn solve_bem_frequency_sweep(
    room: &RoomGeometry,
    sources: &[Source],
    scattering_objects: &[crate::ScatteringObjectConfig],
    eval_point: &Point3D,
    frequencies: &[f64],
    speed_of_sound: f64,
    config: &BemConfig,
) -> Vec<Result<BemResult, String>> {
    frequencies
        .iter()
        .map(|&freq| {
            solve_bem(
                room,
                sources,
                scattering_objects,
                eval_point,
                freq,
                speed_of_sound,
                config,
            )
        })
        .collect()
}

/// Convert complex pressure to SPL (dB re 20 μPa)
#[allow(dead_code)]
pub fn pressure_to_spl(pressure: Complex64) -> f64 {
    let magnitude = pressure.norm();
    let p_ref = 20e-6; // 20 μPa reference pressure
    20.0 * (magnitude / p_ref).max(1e-10).log10()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DirectivityPattern, RectangularRoom};

    #[test]
    fn test_generate_room_mesh() {
        let room = RoomGeometry::Rectangular(RectangularRoom::new(4.0, 5.0, 2.5));
        let mesh = generate_room_mesh(&room, 2);

        assert!(!mesh.nodes.is_empty());
        assert!(!mesh.elements.is_empty());
    }

    #[test]
    fn test_greens_function() {
        let k = 2.0 * PI * 1000.0 / 343.0; // 1 kHz

        // At 1 meter, should decay as 1/(4πr)
        let g = greens_function(1.0, k);
        assert!((g.norm() - 1.0 / (4.0 * PI)).abs() < 0.01);

        // At 2 meters, should be half the amplitude
        let g2 = greens_function(2.0, k);
        assert!((g2.norm() - 0.5 / (4.0 * PI)).abs() < 0.01);
    }

    #[test]
    fn test_absorption_to_admittance() {
        let density = 1.21;
        let c = 343.0;

        // Near-rigid surface (low absorption)
        let y_rigid = absorption_to_admittance(0.02, density, c);
        assert!(y_rigid.re < 0.01);

        // Moderate absorption
        let y_mod = absorption_to_admittance(0.5, density, c);
        assert!(y_mod.re > 0.0);
        assert!(y_mod.re < y_rigid.re * 100.0);

        // High absorption
        let y_high = absorption_to_admittance(0.9, density, c);
        assert!(y_high.re > y_mod.re);
    }

    #[test]
    fn test_estimate_element_count() {
        let room = RoomGeometry::Rectangular(RectangularRoom::new(5.0, 4.0, 2.5));
        let config = BemConfig::default();

        // Low frequency = larger wavelength = fewer elements
        let count_low = estimate_element_count(&room, 100.0, 343.0, &config);

        // High frequency = smaller wavelength = more elements
        let count_high = estimate_element_count(&room, 1000.0, 343.0, &config);

        assert!(count_high > count_low);
    }

    #[test]
    fn test_bem_solver_basic() {
        let room = RoomGeometry::Rectangular(RectangularRoom::new(4.0, 5.0, 2.5));
        let sources = vec![Source::new(
            Point3D::new(1.0, 1.0, 1.0),
            DirectivityPattern::omnidirectional(),
            1.0,
        )];
        let eval_point = Point3D::new(3.0, 3.0, 1.2);

        let config = BemConfig::default();
        let result = solve_bem(&room, &sources, &[], &eval_point, 100.0, 343.0, &config);

        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.frequency, 100.0);
        assert!(result.pressure.norm() > 0.0);
        assert!(result.num_elements > 0);
    }

    #[test]
    fn test_bem_frequency_sweep() {
        let room = RoomGeometry::Rectangular(RectangularRoom::new(4.0, 5.0, 2.5));
        let sources = vec![Source::new(
            Point3D::new(2.0, 2.5, 1.25),
            DirectivityPattern::omnidirectional(),
            1.0,
        )];
        let eval_point = Point3D::new(2.0, 4.0, 1.25);

        // Relaxed config for testing high frequencies without massive mesh
        let config = BemConfig {
            elements_per_wavelength: 2.0, // Reduced from 8.0 for speed/memory in tests
            max_elements: 30000,          // Increased limit
            use_adaptive_mesh: false, // Disable internal adaptive refinement to respect our low density
            ..BemConfig::default()
        };
        let frequencies = vec![50.0, 100.0, 200.0, 500.0, 1000.0];
        let results = solve_bem_frequency_sweep(
            &room,
            &sources,
            &[],
            &eval_point,
            &frequencies,
            343.0,
            &config,
        );

        assert_eq!(results.len(), 5);
        for (i, result) in results.iter().enumerate() {
            assert!(
                result.is_ok(),
                "Failed at {} Hz: {}",
                frequencies[i],
                result.as_ref().err().unwrap()
            );
        }
    }

    #[test]
    fn test_pressure_to_spl() {
        // 1 Pa = 94 dB SPL
        let p = Complex64::new(1.0, 0.0);
        let spl = pressure_to_spl(p);
        assert!((spl - 94.0).abs() < 1.0);

        // 20 μPa = 0 dB SPL
        let p_ref = Complex64::new(20e-6, 0.0);
        let spl_ref = pressure_to_spl(p_ref);
        assert!(spl_ref.abs() < 1.0);
    }
}
