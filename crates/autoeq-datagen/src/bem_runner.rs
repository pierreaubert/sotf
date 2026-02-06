//! BEM simulation driver
//!
//! Uses `math_audio_bem` to solve room acoustics per source per frequency,
//! returning complex pressure at each listening position.

use anyhow::Result;
use math_audio_bem::room_acoustics::{
    calculate_field_pressure_bem_parallel, solve_bem_system,
};
use math_audio_xem_common::{Point3D, RoomSimulation};
use ndarray::Array1;
use num_complex::Complex64;
use rayon::prelude::*;

use crate::SimulationOutput;

/// Mesh resolution (elements per meter on the room surface).
/// At 500 Hz max (wavelength ~0.69m) this gives ~5 elements per wavelength.
const MESH_RESOLUTION: usize = 3;

/// Run BEM simulation for each source independently.
///
/// For every source, sweeps all frequencies and computes the complex pressure
/// at each listening position.  Returns `[source_idx][lp_idx][freq_idx]`.
pub fn run_bem(simulation: &RoomSimulation) -> Result<SimulationOutput> {
    let mesh = simulation.room.generate_mesh(MESH_RESOLUTION);
    log::info!(
        "BEM mesh: {} nodes, {} elements",
        mesh.num_nodes(),
        mesh.num_elements()
    );

    let n_sources = simulation.sources.len();
    let n_lps = simulation.listening_positions.len();
    let n_freqs = simulation.frequencies.len();

    let lp_points: Vec<Point3D> = simulation.listening_positions.clone();

    // For each source, solve independently across all frequencies
    let mut pressures: Vec<Vec<Vec<Complex64>>> =
        vec![vec![vec![Complex64::new(0.0, 0.0); n_freqs]; n_lps]; n_sources];

    for (src_idx, source) in simulation.sources.iter().enumerate() {
        log::info!(
            "BEM: solving source {} of {} ({})",
            src_idx + 1,
            n_sources,
            source.name
        );

        let single_source = vec![source.clone()];

        // Solve frequencies in parallel
        let freq_results: Vec<(usize, Array1<Complex64>)> = simulation
            .frequencies
            .par_iter()
            .enumerate()
            .map(|(freq_idx, &freq)| {
                let k = simulation.wavenumber(freq);

                // Solve BEM system for this single source at this frequency
                let surface_pressure = solve_bem_system(&mesh, &single_source, k, freq)
                    .unwrap_or_else(|e| {
                        log::warn!(
                            "BEM solve failed for source {} freq {:.1} Hz: {}",
                            source.name,
                            freq,
                            e
                        );
                        Array1::zeros(mesh.num_elements())
                    });

                // Evaluate field pressure at listening positions
                let field_pressure = calculate_field_pressure_bem_parallel(
                    &mesh,
                    &surface_pressure,
                    &single_source,
                    &lp_points,
                    k,
                    freq,
                );

                (freq_idx, field_pressure)
            })
            .collect();

        // Store results
        for (freq_idx, field_pressure) in freq_results {
            for (lp_idx, &pressure) in field_pressure.iter().enumerate() {
                pressures[src_idx][lp_idx][freq_idx] = pressure;
            }
        }
    }

    Ok(SimulationOutput {
        frequencies: simulation.frequencies.clone(),
        pressures,
        source_names: simulation
            .sources
            .iter()
            .map(|s| s.name.clone())
            .collect(),
    })
}
