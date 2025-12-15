//! Advanced Room Acoustics Simulator with Full BEM
//!
//! This simulator integrates:
//! - JSON configuration loading
//! - FMM (Fast Multipole Method) for O(N) complexity
//! - ILU preconditioner for GMRES
//! - Adaptive integration for near-singular elements
//! - Parallel assembly and field evaluation
//!
//! Usage:
//!   cargo run --release --bin room_simulator_bem -- --config configs/example_multi_source.json
//!   cargo run --release --bin room_simulator_bem -- --help

use bem::core::solver::{GmresConfig, gmres_solve_fmm_batched_with_ilu, gmres_solve_with_ilu};
use bem::room_acoustics::*;
// Re-import FMM solver types from room_acoustics (they're re-exported from solver.rs)
// FmmSolverConfig, solve_bem_fmm_gmres_ilu are available via bem::room_acoustics
use clap::{Parser, ValueEnum};
use ndarray::Array1;
use num_complex::Complex64;
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "room-simulator-bem")]
#[command(about = "Advanced room acoustics simulator with the BEM (Boundary Element Method) algorithm", long_about = None)]
struct Args {
    /// Path to JSON configuration file
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Output JSON file path
    #[arg(short, long, default_value = "output.json")]
    output: PathBuf,

    /// Override solver method
    #[arg(short, long)]
    solver: Option<SolverMethod>,

    /// Number of parallel threads (default: all cores)
    #[arg(short = 't', long)]
    threads: Option<usize>,

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SolverMethod {
    /// Simple direct GMRES (current implementation)
    Direct,
    /// GMRES with ILU preconditioner
    GmresIlu,
    /// FMM + GMRES (not yet implemented)
    Fmm,
    /// FMM + GMRES + ILU
    FmmIlu,
    /// FMM + GMRES + ILU with batched BLAS operations (optimized for large problems)
    FmmBatched,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Set number of threads if specified
    if let Some(threads) = args.threads {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build_global()
            .expect("Failed to set thread pool");
        println!("Using {} threads\n", threads);
    }

    // Load configuration
    let config = if let Some(config_path) = &args.config {
        println!("Loading configuration from: {}", config_path.display());
        RoomConfig::from_file(config_path)?
    } else {
        println!("No configuration file specified, using default rectangular room");
        create_default_config()
    };

    // Display configuration summary
    print_config_summary(&config);

    // Convert to simulation
    let simulation = config.to_simulation()?;

    // Determine solver method
    let solver_method = args.solver.unwrap_or(match config.solver.method.as_str() {
        "gmres+ilu" => SolverMethod::GmresIlu,
        "fmm+gmres" => SolverMethod::Fmm,
        "fmm+gmres+ilu" => SolverMethod::FmmIlu,
        "fmm+batched" | "fmm+gmres+batched" => SolverMethod::FmmBatched,
        _ => SolverMethod::Direct,
    });

    println!("\n=== Running Simulation ===");
    println!("Solver method: {:?}", solver_method);

    // Run simulation based on solver method
    let output_data = match solver_method {
        SolverMethod::Direct => run_direct_gmres(&simulation, &config, args.verbose)?,
        SolverMethod::GmresIlu => run_gmres_with_ilu(&simulation, &config, args.verbose)?,
        SolverMethod::Fmm | SolverMethod::FmmIlu => {
            run_fmm_gmres_ilu(&simulation, &config, args.verbose)?
        }
        SolverMethod::FmmBatched => run_fmm_batched(&simulation, &config, args.verbose)?,
    };

    // Save results
    println!("\nSaving results to: {}", args.output.display());
    fs::write(&args.output, serde_json::to_string_pretty(&output_data)?)?;
    println!("Done!");

    Ok(())
}

fn create_default_config() -> RoomConfig {
    RoomConfig {
        room: RoomGeometryConfig::Rectangular {
            width: 5.0,
            depth: 4.0,
            height: 2.5,
        },
        sources: vec![SourceConfig {
            name: "Main Speaker".to_string(),
            position: Point3DConfig {
                x: 2.5,
                y: 0.5,
                z: 1.2,
            },
            amplitude: 1.0,
            directivity: DirectivityConfig::Omnidirectional,
            crossover: CrossoverConfig::FullRange,
        }],
        listening_positions: vec![Point3DConfig {
            x: 2.5,
            y: 2.0,
            z: 1.2,
        }],
        frequencies: FrequencyConfig {
            min_freq: 50.0,
            max_freq: 500.0,
            num_points: 20,
            spacing: "logarithmic".to_string(),
        },
        solver: SolverConfig::default(),
        visualization: VisualizationConfig::default(),
        metadata: MetadataConfig::default(),
    }
}

fn print_config_summary(config: &RoomConfig) {
    println!("\n=== Configuration Summary ===");
    match &config.room {
        RoomGeometryConfig::Rectangular {
            width,
            depth,
            height,
        } => {
            println!(
                "Room: Rectangular {:.1}m × {:.1}m × {:.1}m",
                width, depth, height
            );
        }
        RoomGeometryConfig::LShaped {
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
            CrossoverConfig::Lowpass { cutoff_freq, order } => {
                println!("    Lowpass: {:.0}Hz, order {}", cutoff_freq, order)
            }
            CrossoverConfig::Highpass { cutoff_freq, order } => {
                println!("    Highpass: {:.0}Hz, order {}", cutoff_freq, order)
            }
            CrossoverConfig::Bandpass {
                low_cutoff,
                high_cutoff,
                order,
            } => println!(
                "    Bandpass: {:.0}-{:.0}Hz, order {}",
                low_cutoff, high_cutoff, order
            ),
            _ => {}
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
}

fn run_direct_gmres(
    simulation: &RoomSimulation,
    config: &RoomConfig,
    verbose: bool,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    println!("\n=== Direct GMRES Solver ===");

    let mesh = simulation.room.generate_mesh(config.solver.mesh_resolution);
    println!(
        "Mesh: {} nodes, {} elements",
        mesh.nodes.len(),
        mesh.elements.len()
    );

    let lp = simulation.listening_positions[0];
    let mut lp_spl_values = Vec::new();

    for (idx, &freq) in simulation.frequencies.iter().enumerate() {
        if verbose || idx % 5 == 0 {
            println!(
                "\nFrequency {}/{}: {:.1} Hz",
                idx + 1,
                simulation.frequencies.len(),
                freq
            );
        }

        let k = simulation.wavenumber(freq);

        // Solve using current implementation
        let surface_dpdn = solve_bem_system(&mesh, &simulation.sources, k, freq)
            .map_err(|e| format!("BEM solve failed: {}", e))?;

        // Compute SPL at listening position
        let lp_pressure = calculate_field_pressure_bem_parallel(
            &mesh,
            &surface_dpdn,
            &simulation.sources,
            &[lp],
            k,
            freq,
        );

        let lp_spl = pressure_to_spl(lp_pressure[0]);
        lp_spl_values.push(lp_spl);

        if verbose {
            println!("  SPL at LP: {:.1} dB", lp_spl);
        }
    }

    // Build output JSON
    Ok(create_output_json(
        simulation,
        config,
        lp_spl_values,
        "direct_gmres",
    ))
}

fn run_gmres_with_ilu(
    simulation: &RoomSimulation,
    config: &RoomConfig,
    verbose: bool,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    println!("\n=== GMRES + ILU Preconditioner ===");

    // Use first frequency for adaptive mesh sizing
    let first_freq = simulation.frequencies[0];
    let mesh = if config.solver.adaptive_meshing.unwrap_or(false) {
        simulation.room.generate_adaptive_mesh(
            config.solver.mesh_resolution,
            first_freq,
            &simulation.sources,
            simulation.speed_of_sound,
        )
    } else {
        simulation.room.generate_mesh(config.solver.mesh_resolution)
    };
    println!(
        "Mesh: {} nodes, {} elements",
        mesh.nodes.len(),
        mesh.elements.len()
    );

    // GMRES configuration
    let gmres_config = GmresConfig {
        max_iterations: config.solver.gmres.max_iter,
        restart: config.solver.gmres.restart,
        tolerance: config.solver.gmres.tolerance,
        print_interval: if verbose { 1 } else { 0 },
    };

    let lp = simulation.listening_positions[0];
    let mut lp_spl_values = Vec::new();

    // Store per-source SPL values
    let num_sources = simulation.sources.len();
    let mut source_spl_values: Vec<Vec<f64>> = vec![Vec::new(); num_sources];

    // Store BEM solutions for spatial visualization
    let mut bem_solutions: Vec<(f64, Array1<Complex64>)> = Vec::new();

    for (idx, &freq) in simulation.frequencies.iter().enumerate() {
        if verbose || idx % 5 == 0 {
            println!(
                "\nFrequency {}/{}: {:.1} Hz",
                idx + 1,
                simulation.frequencies.len(),
                freq
            );
        }

        let k = simulation.wavenumber(freq);

        // Build BEM matrix (parallel)
        if verbose {
            println!("  Building BEM matrix...");
        }
        let matrix = if config.solver.adaptive_integration {
            build_bem_matrix_adaptive(&mesh, k, true)
        } else {
            build_bem_matrix_parallel(&mesh, k)
        };

        // Build RHS (parallel)
        if verbose {
            println!("  Computing RHS...");
        }
        let rhs = calculate_incident_field_derivative_parallel(&mesh, &simulation.sources, k, freq);

        // Solve with GMRES + ILU
        if verbose {
            println!("  Solving with GMRES+ILU...");
        }

        let solution = gmres_solve_with_ilu(&matrix, &rhs, &gmres_config);

        // Compute SPL at listening position
        let lp_pressure = calculate_field_pressure_bem_parallel(
            &mesh,
            &solution.x,
            &simulation.sources,
            &[lp],
            k,
            freq,
        );

        let lp_spl = pressure_to_spl(lp_pressure[0]);
        lp_spl_values.push(lp_spl);

        // Compute per-source SPL contributions
        for (src_idx, source) in simulation.sources.iter().enumerate() {
            let src_pressure = calculate_field_pressure_bem_parallel(
                &mesh,
                &solution.x,
                std::slice::from_ref(source), // Single source
                &[lp],
                k,
                freq,
            );
            let src_spl = pressure_to_spl(src_pressure[0]);
            source_spl_values[src_idx].push(src_spl);
        }

        if verbose {
            println!(
                "  Iterations: {}, Residual: {:.2e}, SPL: {:.1} dB",
                solution.iterations, solution.residual, lp_spl
            );
        }

        // Store solution for spatial field computation if needed
        if config.visualization.generate_slices {
            let slice_indices = &config.visualization.slice_frequency_indices;
            if slice_indices.is_empty() || slice_indices.contains(&idx) {
                bem_solutions.push((freq, solution.x.clone()));
            }
        }
    }

    Ok(create_output_json_with_slices(
        simulation,
        config,
        &mesh,
        lp_spl_values,
        &source_spl_values,
        &bem_solutions,
        "gmres_ilu",
    ))
}

fn run_fmm_gmres_ilu(
    simulation: &RoomSimulation,
    config: &RoomConfig,
    verbose: bool,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    println!("\n=== FMM + GMRES + ILU Solver ===");

    // Use first frequency for adaptive mesh sizing
    let first_freq = simulation.frequencies[0];
    let mesh = if config.solver.adaptive_meshing.unwrap_or(false) {
        simulation.room.generate_adaptive_mesh(
            config.solver.mesh_resolution,
            first_freq,
            &simulation.sources,
            simulation.speed_of_sound,
        )
    } else {
        simulation.room.generate_mesh(config.solver.mesh_resolution)
    };
    println!(
        "Mesh: {} nodes, {} elements",
        mesh.nodes.len(),
        mesh.elements.len()
    );

    // FMM configuration
    let fmm_config = FmmSolverConfig::default();
    println!("FMM configuration:");
    println!(
        "  Max elements per leaf: {}",
        fmm_config.max_elements_per_leaf
    );
    println!("  Max tree depth: {}", fmm_config.max_tree_depth);
    println!("  Separation ratio: {}", fmm_config.separation_ratio);

    let lp = simulation.listening_positions[0];
    let mut lp_spl_values = Vec::new();

    // Store per-source SPL values
    let num_sources = simulation.sources.len();
    let mut source_spl_values: Vec<Vec<f64>> = vec![Vec::new(); num_sources];

    // Store BEM solutions for spatial visualization
    let mut bem_solutions: Vec<(f64, Array1<Complex64>)> = Vec::new();

    for (idx, &freq) in simulation.frequencies.iter().enumerate() {
        if verbose || idx % 5 == 0 {
            println!(
                "\nFrequency {}/{}: {:.1} Hz",
                idx + 1,
                simulation.frequencies.len(),
                freq
            );
        }

        let k = simulation.wavenumber(freq);

        // Solve using FMM + GMRES
        if verbose {
            println!("  Building FMM system and solving...");
        }

        let solution = if config.solver.ilu.use_hierarchical {
            // Use hierarchical FMM preconditioner (O(N) setup)
            solve_bem_fmm_gmres_hierarchical(
                &mesh,
                &simulation.sources,
                k,
                freq,
                &fmm_config,
                config.solver.gmres.max_iter,
                config.solver.gmres.restart,
                config.solver.gmres.tolerance,
            )
            .map_err(|e| format!("FMM solve (hierarchical) failed: {}", e))?
        } else {
            // Use ILU preconditioner (O(N²) setup via dense matrix extraction)
            solve_bem_fmm_gmres_ilu(
                &mesh,
                &simulation.sources,
                k,
                freq,
                &fmm_config,
                config.solver.gmres.max_iter,
                config.solver.gmres.restart,
                config.solver.gmres.tolerance,
            )
            .map_err(|e| format!("FMM solve (ILU) failed: {}", e))?
        };

        // Compute SPL at listening position
        let lp_pressure = calculate_field_pressure_bem_parallel(
            &mesh,
            &solution,
            &simulation.sources,
            &[lp],
            k,
            freq,
        );

        let lp_spl = pressure_to_spl(lp_pressure[0]);
        lp_spl_values.push(lp_spl);

        // Compute per-source SPL contributions
        for (src_idx, source) in simulation.sources.iter().enumerate() {
            let src_pressure = calculate_field_pressure_bem_parallel(
                &mesh,
                &solution,
                std::slice::from_ref(source),
                &[lp],
                k,
                freq,
            );
            let src_spl = pressure_to_spl(src_pressure[0]);
            source_spl_values[src_idx].push(src_spl);
        }

        if verbose {
            println!("  SPL at LP: {:.1} dB", lp_spl);
        }

        // Store solution for spatial field computation if needed
        if config.visualization.generate_slices {
            let slice_indices = &config.visualization.slice_frequency_indices;
            if slice_indices.is_empty() || slice_indices.contains(&idx) {
                bem_solutions.push((freq, solution.clone()));
            }
        }
    }

    Ok(create_output_json_with_slices(
        simulation,
        config,
        &mesh,
        lp_spl_values,
        &source_spl_values,
        &bem_solutions,
        "fmm_gmres_ilu",
    ))
}

fn run_fmm_batched(
    simulation: &RoomSimulation,
    config: &RoomConfig,
    verbose: bool,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    println!("\n=== FMM + GMRES + Batched BLAS Solver ===");

    // Use first frequency for adaptive mesh sizing
    let first_freq = simulation.frequencies[0];
    let mesh = if config.solver.adaptive_meshing.unwrap_or(false) {
        simulation.room.generate_adaptive_mesh(
            config.solver.mesh_resolution,
            first_freq,
            &simulation.sources,
            simulation.speed_of_sound,
        )
    } else {
        simulation.room.generate_mesh(config.solver.mesh_resolution)
    };
    println!(
        "Mesh: {} nodes, {} elements",
        mesh.nodes.len(),
        mesh.elements.len()
    );

    // FMM configuration
    let fmm_config = FmmSolverConfig::default();
    println!("FMM configuration:");
    println!(
        "  Max elements per leaf: {}",
        fmm_config.max_elements_per_leaf
    );
    println!("  Max tree depth: {}", fmm_config.max_tree_depth);
    println!("  Separation ratio: {}", fmm_config.separation_ratio);
    println!("  Batched BLAS: enabled");

    // GMRES configuration
    let gmres_config = GmresConfig {
        max_iterations: config.solver.gmres.max_iter,
        restart: config.solver.gmres.restart,
        tolerance: config.solver.gmres.tolerance,
        print_interval: if verbose { 1 } else { 0 },
    };

    let lp = simulation.listening_positions[0];
    let mut lp_spl_values = Vec::new();

    // Store per-source SPL values
    let num_sources = simulation.sources.len();
    let mut source_spl_values: Vec<Vec<f64>> = vec![Vec::new(); num_sources];

    // Store BEM solutions for spatial visualization
    let mut bem_solutions: Vec<(f64, Array1<Complex64>)> = Vec::new();

    for (idx, &freq) in simulation.frequencies.iter().enumerate() {
        if verbose || idx % 5 == 0 {
            println!(
                "\nFrequency {}/{}: {:.1} Hz",
                idx + 1,
                simulation.frequencies.len(),
                freq
            );
        }

        let k = simulation.wavenumber(freq);

        // Build FMM system using batched operations
        if verbose {
            println!("  Building FMM system with batched BLAS...");
        }

        // Use the room_acoustics helper to build the FMM system
        // Returns (SlfmmSystem, elements, nodes_array)
        let (fmm_system, _elements, _nodes) =
            build_fmm_system(&mesh, &simulation.sources, k, freq, &fmm_config)
                .map_err(|e| format!("FMM system build failed: {}", e))?;

        // Solve using batched GMRES with ILU
        if verbose {
            println!("  Solving with batched GMRES+ILU...");
        }

        let solution_result =
            gmres_solve_fmm_batched_with_ilu(&fmm_system, &fmm_system.rhs, &gmres_config);

        if verbose {
            println!(
                "  Iterations: {}, Residual: {:.2e}, Converged: {}",
                solution_result.iterations, solution_result.residual, solution_result.converged
            );
        }

        // Compute SPL at listening position
        let lp_pressure = calculate_field_pressure_bem_parallel(
            &mesh,
            &solution_result.x,
            &simulation.sources,
            &[lp],
            k,
            freq,
        );

        let lp_spl = pressure_to_spl(lp_pressure[0]);
        lp_spl_values.push(lp_spl);

        // Compute per-source SPL contributions
        for (src_idx, source) in simulation.sources.iter().enumerate() {
            let src_pressure = calculate_field_pressure_bem_parallel(
                &mesh,
                &solution_result.x,
                std::slice::from_ref(source),
                &[lp],
                k,
                freq,
            );
            let src_spl = pressure_to_spl(src_pressure[0]);
            source_spl_values[src_idx].push(src_spl);
        }

        if verbose {
            println!("  SPL at LP: {:.1} dB", lp_spl);
        }

        // Store solution for spatial field computation if needed
        if config.visualization.generate_slices {
            let slice_indices = &config.visualization.slice_frequency_indices;
            if slice_indices.is_empty() || slice_indices.contains(&idx) {
                bem_solutions.push((freq, solution_result.x.clone()));
            }
        }
    }

    Ok(create_output_json_with_slices(
        simulation,
        config,
        &mesh,
        lp_spl_values,
        &source_spl_values,
        &bem_solutions,
        "fmm_batched",
    ))
}

fn create_output_json_with_slices(
    simulation: &RoomSimulation,
    config: &RoomConfig,
    mesh: &RoomMesh,
    lp_spl_values: Vec<f64>,
    source_spl_values: &[Vec<f64>],
    bem_solutions: &[(f64, Array1<Complex64>)],
    solver_name: &str,
) -> serde_json::Value {
    let lp = simulation.listening_positions[0];

    let (room_width, room_depth, room_height) = match &simulation.room {
        RoomGeometry::Rectangular(r) => (r.width, r.depth, r.height),
        RoomGeometry::LShaped(r) => (r.width1.max(r.width2), r.depth1 + r.depth2, r.height),
    };

    let edges = simulation.room.get_edges();

    let mut output = serde_json::json!({
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
    });

    // Generate spatial slices if requested
    if config.visualization.generate_slices && !bem_solutions.is_empty() {
        use ndarray::Array2;

        println!("\n=== Generating Spatial Slices ===");
        println!(
            "Resolution: {}x{} points",
            config.visualization.slice_resolution, config.visualization.slice_resolution
        );
        println!("Computing slices at {} frequencies...", bem_solutions.len());

        let res = config.visualization.slice_resolution;
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

        for (slice_idx, (freq, surface_pressure)) in bem_solutions.iter().enumerate() {
            println!(
                "  Slice {}/{}: {:.1} Hz",
                slice_idx + 1,
                bem_solutions.len(),
                freq
            );

            let k = simulation.wavenumber(*freq);

            // Horizontal slice (XY at LP.z)
            let mut h_field_points = Vec::new();
            for &y in &y_points {
                for &x in &x_points {
                    h_field_points.push(Point3D::new(x, y, lp.z));
                }
            }

            let h_pressures = calculate_field_pressure_bem_parallel(
                mesh,
                surface_pressure,
                &simulation.sources,
                &h_field_points,
                k,
                *freq,
            );

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

            let v_pressures = calculate_field_pressure_bem_parallel(
                mesh,
                surface_pressure,
                &simulation.sources,
                &v_field_points,
                k,
                *freq,
            );

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

        output["horizontal_slices"] = serde_json::json!(horizontal_slices);
        output["vertical_slices"] = serde_json::json!(vertical_slices);

        println!(
            "Generated {} horizontal and {} vertical slices",
            horizontal_slices.len(),
            vertical_slices.len()
        );
    }

    output
}

fn create_output_json(
    simulation: &RoomSimulation,
    config: &RoomConfig,
    lp_spl_values: Vec<f64>,
    solver_name: &str,
) -> serde_json::Value {
    let lp = simulation.listening_positions[0];

    let (room_width, room_depth, room_height) = match &simulation.room {
        RoomGeometry::Rectangular(r) => (r.width, r.depth, r.height),
        RoomGeometry::LShaped(r) => (r.width1.max(r.width2), r.depth1 + r.depth2, r.height),
    };

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
