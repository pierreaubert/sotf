//! Output JSON formatting for room acoustics simulations

use crate::config::{
    BoundaryConfig, MetadataConfig, RoomConfig, RoomSimulation, VisualizationConfig,
};
use crate::geometry::RoomGeometry;
use crate::types::{Point3D, RoomMesh, pressure_to_spl};
use ndarray::{Array1, Array2};
use num_complex::Complex64;
use serde::{Deserialize, Serialize};

/// Result of room simulation at one frequency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrequencyResult {
    /// Frequency (Hz)
    pub frequency: f64,
    /// SPL values at each listening position
    pub spl_at_lp: Vec<f64>,
}

/// Complete simulation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResults {
    /// List of frequencies simulated
    pub frequencies: Vec<f64>,
    /// Frequency response at each listening position
    pub lp_frequency_responses: Vec<Vec<f64>>,
    /// Horizontal slice data (if generated)
    pub horizontal_slice: Option<SliceData>,
    /// Vertical slice data (if generated)
    pub vertical_slice: Option<SliceData>,
}

/// Pressure field data on a 2D slice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SliceData {
    /// X coordinates
    pub x: Vec<f64>,
    /// Y or Z coordinates
    pub y: Vec<f64>,
    /// SPL grid data
    pub spl: Array2<f64>,
    /// Frequency of the slice
    pub frequency: f64,
}

/// Create output JSON without slices
pub fn create_output_json(
    simulation: &RoomSimulation,
    config: &RoomConfig,
    lp_spl_values: Vec<f64>,
    solver_name: &str,
) -> serde_json::Value {
    let lp = simulation.listening_positions[0];
    let (room_width, room_depth, room_height) = simulation.room.dimensions();
    let edges = simulation.room.get_edges();

    serde_json::json!({
        "room": {
            "type": match simulation.room {
                RoomGeometry::Rectangular(_) => "rectangular",
                RoomGeometry::LShaped(_) => "lshaped",
            },
            "width": room_width,
            "depth": room_depth,
            "height": room_height,
            "edges": edges.iter().map(|(p1, p2)| {
                vec![
                    vec![p1.x, p1.y, p1.z],
                    vec![p2.x, p2.y, p2.z],
                ]
            }).collect::<Vec<_>>(),
        },
        "sources": simulation.sources.iter().map(|s| {
            serde_json::json!({
                "name": s.name,
                "position": [s.position.x, s.position.y, s.position.z],
            })
        }).collect::<Vec<_>>(),
        "listening_position": [lp.x, lp.y, lp.z],
        "frequencies": simulation.frequencies,
        "frequency_response": lp_spl_values,
        "solver": solver_name,
        "metadata": {
            "description": config.metadata.description,
            "author": config.metadata.author,
            "date": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        },
    })
}

/// Create output JSON with per-source responses
pub fn create_output_json_with_sources(
    simulation: &RoomSimulation,
    config: &RoomConfig,
    lp_spl_values: Vec<f64>,
    source_spl_values: &[Vec<f64>],
    solver_name: &str,
) -> serde_json::Value {
    let lp = simulation.listening_positions[0];
    let (room_width, room_depth, room_height) = simulation.room.dimensions();
    let edges = simulation.room.get_edges();

    serde_json::json!({
        "room": {
            "type": match simulation.room {
                RoomGeometry::Rectangular(_) => "rectangular",
                RoomGeometry::LShaped(_) => "lshaped",
            },
            "width": room_width,
            "depth": room_depth,
            "height": room_height,
            "edges": edges.iter().map(|(p1, p2)| {
                vec![
                    vec![p1.x, p1.y, p1.z],
                    vec![p2.x, p2.y, p2.z],
                ]
            }).collect::<Vec<_>>(),
        },
        "sources": simulation.sources.iter().map(|s| {
            serde_json::json!({
                "name": s.name,
                "position": [s.position.x, s.position.y, s.position.z],
            })
        }).collect::<Vec<_>>(),
        "listening_position": [lp.x, lp.y, lp.z],
        "frequencies": simulation.frequencies,
        "frequency_response": lp_spl_values,
        "source_responses": source_spl_values.iter().enumerate().map(|(idx, spl_vals)| {
            serde_json::json!({
                "source_name": simulation.sources[idx].name,
                "source_index": idx,
                "spl": spl_vals,
            })
        }).collect::<Vec<_>>(),
        "solver": solver_name,
        "metadata": {
            "description": config.metadata.description,
            "author": config.metadata.author,
            "date": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        },
    })
}

/// Trait for computing field pressure at points (implemented by BEM/FEM solvers)
pub trait FieldPressureCalculator {
    /// Compute pressure at field points given the surface solution
    fn calculate_field_pressure(
        &self,
        mesh: &RoomMesh,
        surface_solution: &Array1<Complex64>,
        field_points: &[Point3D],
        wavenumber: f64,
        frequency: f64,
    ) -> Vec<Complex64>;
}

/// Generate spatial slices for visualization
pub fn generate_spatial_slices<F>(
    simulation: &RoomSimulation,
    config: &VisualizationConfig,
    mesh: &RoomMesh,
    solutions: &[(f64, Array1<Complex64>)],
    calculate_pressure: F,
) -> (Vec<serde_json::Value>, Vec<serde_json::Value>)
where
    F: Fn(&RoomMesh, &Array1<Complex64>, &[Point3D], f64, f64) -> Vec<Complex64>,
{
    let lp = simulation.listening_positions[0];
    let (room_width, room_depth, room_height) = simulation.room.dimensions();

    let res = config.slice_resolution;
    let x_points: Vec<f64> = (0..res)
        .map(|i| i as f64 * room_width / (res - 1) as f64)
        .collect();
    let y_points: Vec<f64> = (0..res)
        .map(|i| i as f64 * room_depth / (res - 1) as f64)
        .collect();
    let z_points: Vec<f64> = (0..res)
        .map(|i| i as f64 * room_height / (res - 1) as f64)
        .collect();

    let mut horizontal_slices = Vec::new();
    let mut vertical_slices = Vec::new();

    for (freq, surface_pressure) in solutions.iter() {
        let k = simulation.wavenumber(*freq);

        // Horizontal slice (XY at LP.z)
        let mut h_field_points = Vec::new();
        for &y in &y_points {
            for &x in &x_points {
                h_field_points.push(Point3D::new(x, y, lp.z));
            }
        }

        let h_pressures = calculate_pressure(mesh, surface_pressure, &h_field_points, k, *freq);

        let mut h_spl_grid = Array2::zeros((res, res));
        for (idx, p) in h_pressures.iter().enumerate() {
            let i = idx % res;
            let j = idx / res;
            h_spl_grid[[j, i]] = pressure_to_spl(*p);
        }

        horizontal_slices.push(serde_json::json!({
            "x": x_points,
            "y": y_points,
            "spl": h_spl_grid.iter().cloned().collect::<Vec<f64>>(),
            "shape": [res, res],
            "frequency": freq,
        }));

        // Vertical slice (XZ at LP.y)
        let mut v_field_points = Vec::new();
        for &z in &z_points {
            for &x in &x_points {
                v_field_points.push(Point3D::new(x, lp.y, z));
            }
        }

        let v_pressures = calculate_pressure(mesh, surface_pressure, &v_field_points, k, *freq);

        let mut v_spl_grid = Array2::zeros((res, res));
        for (idx, p) in v_pressures.iter().enumerate() {
            let i = idx % res;
            let j = idx / res;
            v_spl_grid[[j, i]] = pressure_to_spl(*p);
        }

        vertical_slices.push(serde_json::json!({
            "x": x_points,
            "z": z_points,
            "spl": v_spl_grid.iter().cloned().collect::<Vec<f64>>(),
            "shape": [res, res],
            "frequency": freq,
        }));
    }

    (horizontal_slices, vertical_slices)
}

/// Print configuration summary to stdout
pub fn print_config_summary(config: &RoomConfig) {
    println!("\n=== Configuration Summary ===");
    match &config.room {
        crate::config::RoomGeometryConfig::Rectangular {
            width,
            depth,
            height,
        } => {
            println!(
                "Room: Rectangular {:.1}m × {:.1}m × {:.1}m",
                width, depth, height
            );
        }
        crate::config::RoomGeometryConfig::LShaped {
            width1,
            depth1,
            width2,
            depth2,
            height,
        } => {
            println!("Room: L-shaped");
            println!("  Main: {:.1}m × {:.1}m", width1, depth1);
            println!("  Extension: {:.1}m × {:.1}m", width2, depth2);
            println!("  Height: {:.1}m", height);
        }
    }

    println!("\nSources: {}", config.sources.len());
    for source in &config.sources {
        println!(
            "  - {}: ({:.2}, {:.2}, {:.2})",
            source.name, source.position.x, source.position.y, source.position.z
        );
        match &source.crossover {
            crate::config::CrossoverConfig::Lowpass { cutoff_freq, order } => {
                println!("    Lowpass: {:.0}Hz, order {}", cutoff_freq, order)
            }
            crate::config::CrossoverConfig::Highpass { cutoff_freq, order } => {
                println!("    Highpass: {:.0}Hz, order {}", cutoff_freq, order)
            }
            crate::config::CrossoverConfig::Bandpass {
                low_cutoff,
                high_cutoff,
                order,
            } => println!(
                "    Bandpass: {:.0}-{:.0}Hz, order {}",
                low_cutoff, high_cutoff, order
            ),
            _ => {} // Ignore other crossover types
        }
    }

    println!(
        "\nFrequencies: {:.0} Hz to {:.0} Hz ({} points)",
        config.frequencies.min_freq, config.frequencies.max_freq, config.frequencies.num_points
    );

    println!("\nSolver Configuration:");
    println!("  Method: {}", config.solver.method);
    println!(
        "  Mesh resolution: {} elements/meter",
        config.solver.mesh_resolution
    );
    println!(
        "  Adaptive integration: {}",
        config.solver.adaptive_integration
    );

    // Boundary summary
    println!("\nBoundaries:");
    let b = &config.boundaries;
    let format_bc = |s: &crate::config::SurfaceConfig| match s {
        crate::config::SurfaceConfig::Rigid => "Rigid".to_string(),
        crate::config::SurfaceConfig::Absorption { coefficient } => {
            format!("Abs α={:.2}", coefficient)
        }
        crate::config::SurfaceConfig::Impedance { real, imag } => {
            format!("Z={:.1}+{:.1}i", real, imag)
        }
    };

    println!("  Default walls: {}", format_bc(&b.walls));
    println!("  Floor:         {}", format_bc(&b.floor));
    println!("  Ceiling:       {}", format_bc(&b.ceiling));
}

/// Create a default room configuration for testing
pub fn create_default_config() -> RoomConfig {
    RoomConfig {
        room: crate::config::RoomGeometryConfig::Rectangular {
            width: 5.0,
            depth: 4.0,
            height: 2.5,
        },
        sources: vec![crate::config::SourceConfig {
            name: "Main Speaker".to_string(),
            position: crate::config::Point3DConfig {
                x: 2.5,
                y: 0.5,
                z: 1.2,
            },
            amplitude: 1.0,
            directivity: crate::config::DirectivityConfig::Omnidirectional,
            crossover: crate::config::CrossoverConfig::FullRange,
        }],
        listening_positions: vec![crate::config::Point3DConfig {
            x: 2.5,
            y: 2.0,
            z: 1.2,
        }],
        frequencies: crate::config::FrequencyConfig {
            min_freq: 50.0,
            max_freq: 500.0,
            num_points: 20,
            spacing: "logarithmic".to_string(),
        },
        solver: crate::config::SolverConfig::default(),
        boundaries: BoundaryConfig::default(),
        visualization: VisualizationConfig::default(),
        metadata: MetadataConfig::default(),
    }
}
