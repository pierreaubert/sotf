//! AutoEQ data generation using BEM and FEM acoustic solvers
//!
//! Generates CSV measurement files and roomeq JSON configs from simulated
//! room acoustics for testing the RoomEQ optimizer.

use num_complex::Complex64;

pub mod bem_runner;
pub mod csv_export;
pub mod fem_runner;
pub mod roomeq_config_gen;
pub mod scenarios;

/// Output from a BEM or FEM simulation.
///
/// Pressures are indexed as `[source_idx][lp_idx][freq_idx]`.
#[derive(Debug, Clone)]
pub struct SimulationOutput {
    /// Frequency points in Hz
    pub frequencies: Vec<f64>,
    /// Complex pressure at each (source, listening position, frequency)
    pub pressures: Vec<Vec<Vec<Complex64>>>,
    /// Source names matching the source index
    pub source_names: Vec<String>,
}
