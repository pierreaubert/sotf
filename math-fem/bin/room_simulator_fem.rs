//! Room Acoustics Simulator using FEM
//!
//! This simulator uses the Finite Element Method to solve the Helmholtz equation
//! for room acoustics. Unlike BEM which works on surface meshes, FEM uses volume
//! meshes and can handle more complex material properties and boundary conditions.
//!
//! Usage:
//!   cargo run --release --bin roomsim-fem -- --config configs/example_room.json
//!   cargo run --release --bin roomsim-fem -- --help

use clap::{Parser, ValueEnum};
use fem::assembly::HelmholtzProblem;
use fem::basis::PolynomialDegree;
use fem::mesh::{ElementType, Mesh, Point};
use fem::solver::{self, GmresConfigF64, SolverConfig, SolverType};
use num_complex::Complex64;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

// Import common types from math-xem-common
use xem_common::{
    create_default_config, create_output_json, print_config_summary, pressure_to_spl, Point3D,
    RoomConfig, RoomSimulation, Source,
};

#[derive(Parser, Debug)]
#[command(name = "room-simulator-fem")]
#[command(about = "Room acoustics simulator using Finite Element Method")]
struct Args {
    /// Path to JSON configuration file
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Output JSON file path
    #[arg(short, long, default_value = "output_fem.json")]
    output: PathBuf,

    /// Override solver method
    #[arg(short, long, default_value = "gmres")]
    solver: CliSolverType,

    /// Override preconditioner
    #[arg(short, long)]
    preconditioner: Option<CliPreconditionerType>,

    /// Krylov subspace size (restart)
    #[arg(long, default_value = "50")]
    krylov_size: usize,

    /// Number of domains for Schwarz decomposition
    #[arg(long, default_value = "8")]
    schwarz_domains: usize,

    /// Number of parallel threads (default: all cores)
    #[arg(short = 't', long)]
    threads: Option<usize>,

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliSolverType {
    /// Direct solver (LU decomposition)
    Direct,
    /// GMRES iterative solver
    Gmres,
    /// Pipelined GMRES
    Pipelined,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliPreconditionerType {
    /// ILU(0) preconditioner
    Ilu,
    /// Jacobi (diagonal) preconditioner
    Jacobi,
    /// Parallel ILU with graph coloring
    IluColoring,
    /// Parallel ILU with fixed-point iteration
    IluFixedpoint,
    /// Additive Schwarz domain decomposition
    Schwarz,
    /// Algebraic Multigrid (AMG)
    Amg,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let args = Args::parse();

    // Set number of threads if specified
    if let Some(threads) = args.threads {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build_global()
            .expect("Failed to set thread pool");
        println!("Using {} threads\n", threads);
    } else {
        println!("Using {} threads (rayon default)\n", rayon::current_num_threads());
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

    // Determine internal solver type based on CLI arguments
    let internal_solver_type = match (args.solver, args.preconditioner) {
        (CliSolverType::Direct, _) => SolverType::Direct,
        (CliSolverType::Gmres, None) => SolverType::Gmres,
        (CliSolverType::Gmres, Some(CliPreconditionerType::Ilu)) => SolverType::GmresIlu,
        (CliSolverType::Gmres, Some(CliPreconditionerType::Jacobi)) => SolverType::GmresJacobi,
        (CliSolverType::Gmres, Some(CliPreconditionerType::IluColoring)) => SolverType::GmresIluColoring,
        (CliSolverType::Gmres, Some(CliPreconditionerType::IluFixedpoint)) => SolverType::GmresIluFixedPoint,
        (CliSolverType::Gmres, Some(CliPreconditionerType::Schwarz)) => SolverType::GmresSchwarz,
        (CliSolverType::Gmres, Some(CliPreconditionerType::Amg)) => SolverType::GmresAmg,
        (CliSolverType::Pipelined, None) => SolverType::GmresPipelined,
        (CliSolverType::Pipelined, Some(CliPreconditionerType::Ilu)) => SolverType::GmresPipelinedIlu,
        (CliSolverType::Pipelined, Some(CliPreconditionerType::Amg)) => SolverType::GmresPipelinedAmg,
        // Invalid combinations fallback or error
        (solver, precond) => {
             return Err(format!("Unsupported solver/preconditioner combination: {:?} + {:?}", solver, precond).into());
        }
    };

    // Run simulation
    let output_data = run_fem_simulation(&simulation, &config, internal_solver_type, args.krylov_size, args.schwarz_domains, args.verbose)?;

    // Save results
    println!("\nSaving results to: {}", args.output.display());
    fs::write(&args.output, serde_json::to_string_pretty(&output_data)?)?;
    println!("Done!");

    Ok(())
}

/// Create a tetrahedral mesh for the room
fn create_room_mesh(simulation: &RoomSimulation, elements_per_meter: usize) -> Mesh {
    let (width, depth, height) = simulation.room.dimensions();

    // Create a structured grid of nodes
    let nx = (width * elements_per_meter as f64).ceil() as usize + 1;
    let ny = (depth * elements_per_meter as f64).ceil() as usize + 1;
    let nz = (height * elements_per_meter as f64).ceil() as usize + 1;

    let dx = width / (nx - 1) as f64;
    let dy = depth / (ny - 1) as f64;
    let dz = height / (nz - 1) as f64;

    // Create 3D mesh
    let mut mesh = Mesh::new(3);

    // Generate nodes
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                mesh.add_node(Point::new_3d(
                    i as f64 * dx,
                    j as f64 * dy,
                    k as f64 * dz,
                ));
            }
        }
    }

    // Generate tetrahedral elements by subdividing hex cells
    // Each hex is divided into 5 tetrahedra (consistent decomposition)
    for k in 0..(nz - 1) {
        for j in 0..(ny - 1) {
            for i in 0..(nx - 1) {
                // Hex vertices
                let v0 = k * ny * nx + j * nx + i;
                let v1 = v0 + 1;
                let v2 = v0 + nx;
                let v3 = v2 + 1;
                let v4 = v0 + ny * nx;
                let v5 = v4 + 1;
                let v6 = v4 + nx;
                let v7 = v6 + 1;

                // 5-tet decomposition of hex (consistent for structured grids)
                mesh.add_element(ElementType::Tetrahedron, vec![v0, v1, v3, v7]);
                mesh.add_element(ElementType::Tetrahedron, vec![v0, v3, v2, v7]);
                mesh.add_element(ElementType::Tetrahedron, vec![v0, v2, v6, v7]);
                mesh.add_element(ElementType::Tetrahedron, vec![v0, v6, v4, v7]);
                mesh.add_element(ElementType::Tetrahedron, vec![v0, v4, v5, v7]);
            }
        }
    }

    // Detect boundaries for Neumann BC (rigid walls)
    mesh.detect_boundaries();

    mesh
}

/// Run FEM simulation for all frequencies
fn run_fem_simulation(
    simulation: &RoomSimulation,
    config: &RoomConfig,
    solver_type: SolverType,
    krylov_size: usize,
    schwarz_domains: usize,
    verbose: bool,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let solver_name = format!("{:?}", solver_type);
    println!("\n=== {} Solver ===", solver_name.to_uppercase());

    // Create mesh
    let mesh = create_room_mesh(simulation, config.solver.mesh_resolution);
    println!(
        "Mesh: {} nodes, {} elements",
        mesh.num_nodes(),
        mesh.num_elements()
    );

    // Configure solver
    let solver_config = SolverConfig {
        solver_type,
        gmres: GmresConfigF64 {
            max_iterations: config.solver.gmres.max_iter,
            restart: krylov_size,
            tolerance: config.solver.gmres.tolerance,
            print_interval: if verbose { 10 } else { 0 },
        },
        verbosity: if verbose { 1 } else { 0 },
        schwarz_subdomains: schwarz_domains,
        schwarz_overlap: 2, // Default overlap
    };

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

        let k = Complex64::new(simulation.wavenumber(freq), 0.0);

        // Create source function from room sources
        let sources = &simulation.sources;
        let source_fn = |x: f64, y: f64, z: f64| -> Complex64 {
            compute_source_term(x, y, z, sources, freq)
        };

        // Assemble Helmholtz problem
        let assemble_start = Instant::now();
        let problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k, source_fn);
        let assemble_time = assemble_start.elapsed();

        if verbose && idx == 0 {
            let csr = problem.matrix.to_csr();
            println!(
                "  System: {} DOFs, {} non-zeros (sparsity: {:.2}%)",
                problem.num_dofs(),
                csr.nnz(),
                csr.sparsity() * 100.0
            );
        }

        println!(
            "  [Assembly] time: {:.1}ms",
            assemble_time.as_secs_f64() * 1000.0
        );

        // Solve the system
        let solution = solver::solve(&problem, &solver_config)?;

        if verbose {
            println!(
                "  Solved in {} iterations (residual: {:.2e})",
                solution.iterations, solution.residual
            );
        }

        // Evaluate pressure at listening position
        let lp_pressure = evaluate_solution_at_point(&mesh, &solution.values, lp);
        let lp_spl = pressure_to_spl(lp_pressure);
        lp_spl_values.push(lp_spl);

        if verbose {
            println!("  SPL at LP: {:.1} dB", lp_spl);
        }
    }

    Ok(create_output_json(
        simulation,
        config,
        lp_spl_values,
        &solver_name,
    ))
}

/// Compute source term at a point from all sources
fn compute_source_term(x: f64, y: f64, z: f64, sources: &[Source], frequency: f64) -> Complex64 {
    let point = Point3D::new(x, y, z);
    let mut total = Complex64::new(0.0, 0.0);

    for source in sources {
        // Gaussian source distribution centered at source position
        let r = source.position.distance_to(&point);
        let sigma = 0.1; // Source width in meters

        // Gaussian envelope
        let envelope = (-r * r / (2.0 * sigma * sigma)).exp();

        // Get directional amplitude
        let amplitude = source.amplitude_towards(&point, frequency);

        total += Complex64::new(amplitude * envelope, 0.0);
    }

    total
}

/// Evaluate FEM solution at a specific point using nearest-neighbor interpolation
fn evaluate_solution_at_point(
    mesh: &Mesh,
    solution: &ndarray::Array1<Complex64>,
    point: Point3D,
) -> Complex64 {
    // Find the nearest node and use its value
    // A proper implementation would use shape function interpolation within elements

    let mut min_dist = f64::MAX;
    let mut nearest_node = 0;

    for (i, node) in mesh.nodes.iter().enumerate() {
        let dist =
            (node.x - point.x).powi(2) + (node.y - point.y).powi(2) + (node.z - point.z).powi(2);
        if dist < min_dist {
            min_dist = dist;
            nearest_node = i;
        }
    }

    solution[nearest_node]
}
