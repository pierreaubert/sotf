//! Room Acoustics Simulator using BEM
//!
//! This module implements a 3D room acoustics simulator for calculating sound pressure
//! levels (SPL) at listening positions from directional sources with frequency-dependent
//! radiation patterns.
//!
//! **Note**: The solver functionality requires either the `native` or `wasm` feature for parallel processing.
//! Data structures (RoomGeometry, Source, etc.) are always available.

// Room acoustics is still experimental; allow missing docs for now
#![allow(missing_docs)]

mod config;
#[cfg(any(feature = "native", feature = "wasm"))]
mod solver;

pub use config::*;
#[cfg(any(feature = "native", feature = "wasm"))]
pub use solver::*;

// Re-export common types from math-xem-common
pub use xem_common::{
    CrossoverFilter, DirectivityPattern, FrequencyResult, LShapedRoom, ListeningPosition, Point3D,
    RectangularRoom, RoomGeometry, RoomMesh, RoomSimulation, SimulationResults, SliceData, Source,
    SurfaceElement, constants, log_space, pressure_to_spl, wavenumber,
};
// Re-export output utilities
pub use xem_common::{
    create_default_config, create_output_json, create_output_json_with_sources,
    generate_spatial_slices, print_config_summary,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn test_rectangular_room() {
        let room = RectangularRoom::new(5.0, 4.0, 3.0);
        assert_eq!(room.width, 5.0);
        assert_eq!(room.depth, 4.0);
        assert_eq!(room.height, 3.0);
    }

    #[test]
    fn test_omnidirectional_pattern() {
        let pattern = DirectivityPattern::omnidirectional();
        // Should be 1.0 in all directions
        assert!((pattern.interpolate(0.0, 0.0) - 1.0).abs() < 1e-6);
        assert!((pattern.interpolate(PI / 2.0, PI) - 1.0).abs() < 1e-6);
        assert!((pattern.interpolate(PI, 0.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_log_space() {
        let freqs = log_space(20.0, 20000.0, 200);
        assert_eq!(freqs.len(), 200);
        assert!((freqs[0] - 20.0).abs() < 1e-6);
        assert!((freqs[199] - 20000.0).abs() < 1e-6);
        // Check logarithmic spacing
        assert!(freqs[1] / freqs[0] > 1.0);
    }

    #[test]
    fn test_point_distance() {
        let p1 = Point3D::new(0.0, 0.0, 0.0);
        let p2 = Point3D::new(3.0, 4.0, 0.0);
        assert!((p1.distance_to(&p2) - 5.0).abs() < 1e-6);
    }
}
