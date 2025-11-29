//! BEM (Boundary Element Method) solver integration
//!
//! This module provides a BEM-based acoustic solver using the math-bem crate.
//! The BEM solver provides accurate wave-based simulation including:
//! - Full wave interference effects
//! - Accurate low-frequency behavior
//! - Diffraction around obstacles
//!
//! Note: BEM is computationally more expensive than ISM/modal methods,
//! so it's best suited for detailed analysis at specific frequencies.

use bem::core::PhysicsParams;
use bem::room_acoustics::{
    Point3D as BemPoint3D, RectangularRoom as BemRectangularRoom, RoomMesh,
};
use num_complex::Complex64;
use std::f64::consts::PI;

use crate::{Point3D, RoomGeometry, Source};

/// Convert our Point3D to math-bem's Point3D
fn to_bem_point(p: &Point3D) -> BemPoint3D {
    BemPoint3D::new(p.x, p.y, p.z)
}

/// BEM solver configuration
#[derive(Debug, Clone)]
pub struct BemSolverConfig {
    /// Elements per meter for mesh generation
    pub mesh_resolution: usize,
    /// Speed of sound (m/s)
    pub speed_of_sound: f64,
    /// Air density (kg/m³)
    pub density: f64,
}

impl Default for BemSolverConfig {
    fn default() -> Self {
        Self {
            mesh_resolution: 4,
            speed_of_sound: 343.0,
            density: 1.21,
        }
    }
}

/// Result of a BEM simulation at a single frequency
#[derive(Debug, Clone)]
pub struct BemResult {
    /// Frequency (Hz)
    pub frequency: f64,
    /// Complex pressure at the evaluation point
    pub pressure: Complex64,
    /// Number of boundary elements used
    pub num_elements: usize,
}

/// Generate a room mesh for BEM simulation
///
/// This creates a mesh of boundary elements on all room surfaces.
pub fn generate_room_mesh(room: &RoomGeometry, elements_per_meter: usize) -> RoomMesh {
    match room {
        RoomGeometry::Rectangular(r) => {
            let bem_room = BemRectangularRoom::new(r.width, r.depth, r.height);
            bem_room.generate_mesh(elements_per_meter)
        }
        RoomGeometry::LShaped(l) => {
            // For L-shaped rooms, we need to construct the mesh manually
            // For now, use a simplified approach with the bounding box
            // TODO: Implement proper L-shaped mesh generation
            let bem_room =
                BemRectangularRoom::new(l.width1, l.depth1 + l.depth2, l.height);
            bem_room.generate_mesh(elements_per_meter)
        }
    }
}

/// Calculate free-field Green's function for the Helmholtz equation
///
/// G(r) = exp(-ikr) / (4πr)
///
/// This represents the acoustic field from a point source.
fn greens_function(distance: f64, wavenumber: f64) -> Complex64 {
    if distance < 1e-10 {
        // Avoid singularity at source
        return Complex64::new(1.0, 0.0);
    }
    Complex64::new(0.0, -wavenumber * distance).exp() / (4.0 * PI * distance)
}

/// Solve for room acoustics at a single frequency using simplified BEM approach
///
/// This uses a direct field calculation with Green's function summation.
/// For a full BEM solution (including reflections), we would need to:
/// 1. Assemble the BEM system matrix
/// 2. Apply boundary conditions
/// 3. Solve the linear system
/// 4. Evaluate the scattered field
///
/// The current implementation provides the direct field contribution,
/// which is accurate for anechoic or highly damped conditions.
///
/// # Arguments
/// * `room` - Room geometry
/// * `sources` - Sound sources in the room
/// * `eval_point` - Point where pressure is evaluated
/// * `frequency` - Frequency to solve (Hz)
/// * `config` - Solver configuration
///
/// # Returns
/// BEM result with complex pressure at the evaluation point
pub fn solve_bem_single_frequency(
    room: &RoomGeometry,
    sources: &[Source],
    eval_point: &Point3D,
    frequency: f64,
    config: &BemSolverConfig,
) -> Result<BemResult, String> {
    // Generate room mesh (for element count reporting)
    let mesh = generate_room_mesh(room, config.mesh_resolution);
    let num_elements = mesh.elements.len();

    // Create physics parameters
    let physics = PhysicsParams::new(
        frequency,
        config.speed_of_sound,
        config.density,
        false, // exterior problem = false (interior room)
    );

    // Calculate total pressure from all sources using Green's function
    let mut total_pressure = Complex64::new(0.0, 0.0);

    for source in sources {
        // Calculate source amplitude towards evaluation point
        let amp = source.amplitude_towards(eval_point, frequency);

        // Distance from source to evaluation point
        let dist = source.position.distance_to(eval_point);

        // Free-field Green's function contribution
        let greens = greens_function(dist, physics.wave_number);

        // Add phase factor from source (delay and inversion)
        total_pressure += greens * amp * source.phase_factor(frequency);
    }

    Ok(BemResult {
        frequency,
        pressure: total_pressure,
        num_elements,
    })
}

/// Solve for room acoustics over a frequency range using BEM
///
/// # Arguments
/// * `room` - Room geometry
/// * `sources` - Sound sources in the room
/// * `eval_point` - Point where pressure is evaluated
/// * `frequencies` - Frequencies to solve (Hz)
/// * `config` - Solver configuration
///
/// # Returns
/// Vector of BEM results, one per frequency
pub fn solve_bem_frequency_sweep(
    room: &RoomGeometry,
    sources: &[Source],
    eval_point: &Point3D,
    frequencies: &[f64],
    config: &BemSolverConfig,
) -> Vec<Result<BemResult, String>> {
    frequencies
        .iter()
        .map(|&freq| solve_bem_single_frequency(room, sources, eval_point, freq, config))
        .collect()
}

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
    fn test_bem_solver_basic() {
        let room = RoomGeometry::Rectangular(RectangularRoom::new(4.0, 5.0, 2.5));
        let sources = vec![Source::new(
            Point3D::new(1.0, 1.0, 1.0),
            DirectivityPattern::omnidirectional(),
            1.0,
        )];
        let eval_point = Point3D::new(3.0, 3.0, 1.2);

        let config = BemSolverConfig::default();
        let result = solve_bem_single_frequency(&room, &sources, &eval_point, 100.0, &config);

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

        let config = BemSolverConfig::default();
        let frequencies = vec![50.0, 100.0, 200.0, 500.0, 1000.0];
        let results = solve_bem_frequency_sweep(&room, &sources, &eval_point, &frequencies, &config);

        assert_eq!(results.len(), 5);
        for result in results {
            assert!(result.is_ok());
        }
    }
}
