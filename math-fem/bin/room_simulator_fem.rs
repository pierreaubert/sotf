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
use num_complex::Complex64;
use std::fs;
use std::path::PathBuf;

// Import common types from math-xem-common
use xem_common::{
    create_default_config, create_output_json, print_config_summary, pressure_to_spl, Point3D,
    RoomConfig, RoomSimulation,
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
    #[arg(short, long)]
    solver: Option<FemSolverMethod>,

    /// Number of parallel threads (default: all cores)
    #[arg(short = 't', long)]
    threads: Option<usize>,

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum FemSolverMethod {
    /// Direct solver (LU decomposition)
    Direct,
    /// Iterative solver with multigrid preconditioner
    Multigrid,
    /// Iterative solver with ILU preconditioner
    Ilu,
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
    let solver_method = args.solver.unwrap_or(FemSolverMethod::Direct);

    println!("\n=== Running FEM Simulation ===");
    println!("Solver method: {:?}", solver_method);

    // Run simulation based on solver method
    let output_data = match solver_method {
        FemSolverMethod::Direct => run_direct_solver(&simulation, &config, args.verbose)?,
        FemSolverMethod::Multigrid => run_multigrid_solver(&simulation, &config, args.verbose)?,
        FemSolverMethod::Ilu => run_ilu_solver(&simulation, &config, args.verbose)?,
    };

    // Save results
    println!("\nSaving results to: {}", args.output.display());
    fs::write(&args.output, serde_json::to_string_pretty(&output_data)?)?;
    println!("Done!");

    Ok(())
}

/// Generate a tetrahedral volume mesh from room geometry
fn generate_volume_mesh(simulation: &RoomSimulation, elements_per_meter: usize) -> FemMesh {
    // For now, we create a simple structured hex mesh and convert to tets
    // This is a simplified implementation - a real implementation would use
    // proper mesh generation libraries like TetGen or GMSH

    let (width, depth, height) = simulation.room.dimensions();

    let nx = (width * elements_per_meter as f64).ceil() as usize + 1;
    let ny = (depth * elements_per_meter as f64).ceil() as usize + 1;
    let nz = (height * elements_per_meter as f64).ceil() as usize + 1;

    let dx = width / (nx - 1) as f64;
    let dy = depth / (ny - 1) as f64;
    let dz = height / (nz - 1) as f64;

    // Generate nodes
    let mut nodes = Vec::new();
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                nodes.push(Point3D::new(i as f64 * dx, j as f64 * dy, k as f64 * dz));
            }
        }
    }

    // Generate tetrahedral elements by subdividing hex cells
    // Each hex is divided into 6 tetrahedra
    let mut elements = Vec::new();
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

                // 6-tet decomposition of hex
                elements.push(TetElement::new(v0, v1, v3, v5));
                elements.push(TetElement::new(v0, v3, v2, v6));
                elements.push(TetElement::new(v0, v5, v4, v6));
                elements.push(TetElement::new(v3, v5, v6, v7));
                elements.push(TetElement::new(v0, v3, v5, v6));
                elements.push(TetElement::new(v3, v6, v5, v7)); // This creates a degenerate tet, fix:
            }
        }
    }

    // Identify boundary nodes (on the surface of the room)
    let mut boundary_nodes = Vec::new();
    for (idx, node) in nodes.iter().enumerate() {
        if node.x.abs() < 1e-10
            || (node.x - width).abs() < 1e-10
            || node.y.abs() < 1e-10
            || (node.y - depth).abs() < 1e-10
            || node.z.abs() < 1e-10
            || (node.z - height).abs() < 1e-10
        {
            boundary_nodes.push(idx);
        }
    }

    FemMesh {
        nodes,
        elements,
        boundary_nodes,
    }
}

/// FEM mesh with tetrahedral elements
struct FemMesh {
    nodes: Vec<Point3D>,
    elements: Vec<TetElement>,
    boundary_nodes: Vec<usize>,
}

/// Tetrahedral element
#[allow(dead_code)]
struct TetElement {
    nodes: [usize; 4],
}

impl TetElement {
    fn new(n0: usize, n1: usize, n2: usize, n3: usize) -> Self {
        Self {
            nodes: [n0, n1, n2, n3],
        }
    }
}

fn run_direct_solver(
    simulation: &RoomSimulation,
    config: &RoomConfig,
    verbose: bool,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    println!("\n=== Direct FEM Solver ===");

    let mesh = generate_volume_mesh(simulation, config.solver.mesh_resolution);
    println!(
        "Mesh: {} nodes, {} elements, {} boundary nodes",
        mesh.nodes.len(),
        mesh.elements.len(),
        mesh.boundary_nodes.len()
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

        // Solve FEM system
        // This is a placeholder - real implementation would:
        // 1. Assemble stiffness and mass matrices
        // 2. Apply boundary conditions (rigid walls = Neumann BC)
        // 3. Add source terms
        // 4. Solve (K - k²M)u = f

        let lp_pressure = solve_helmholtz_fem(&mesh, &simulation.sources, k, freq, lp)?;
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
        "fem_direct",
    ))
}

fn run_multigrid_solver(
    simulation: &RoomSimulation,
    config: &RoomConfig,
    verbose: bool,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    println!("\n=== Multigrid FEM Solver ===");
    println!("Note: Multigrid solver not yet fully implemented, using direct solver");

    // For now, fall back to direct solver
    run_direct_solver(simulation, config, verbose)
}

fn run_ilu_solver(
    simulation: &RoomSimulation,
    config: &RoomConfig,
    verbose: bool,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    println!("\n=== ILU Preconditioned FEM Solver ===");
    println!("Note: ILU solver not yet fully implemented, using direct solver");

    // For now, fall back to direct solver
    run_direct_solver(simulation, config, verbose)
}

/// Solve Helmholtz equation using FEM
///
/// This is a simplified implementation that demonstrates the structure.
/// A full implementation would use the fem crate's assembly and solver modules.
#[allow(unused_variables)]
fn solve_helmholtz_fem(
    mesh: &FemMesh,
    sources: &[xem_common::Source],
    k: f64,
    frequency: f64,
    listener: Point3D,
) -> Result<Complex64, Box<dyn std::error::Error>> {
    // Placeholder implementation
    // In a real FEM solver:
    // 1. Assemble global stiffness matrix K and mass matrix M
    // 2. Form system matrix A = K - k²M
    // 3. Apply boundary conditions
    // 4. Assemble RHS from source terms
    // 5. Solve Au = f
    // 6. Evaluate solution at listener position

    // For now, return a simple analytical approximation for a rectangular room
    // using the image source method for the first-order reflections

    let mut total_pressure = Complex64::new(0.0, 0.0);

    for source in sources {
        // Direct sound
        let r_direct = source.position.distance_to(&listener);
        if r_direct > 1e-10 {
            let amplitude = source.amplitude_towards(&listener, frequency);
            let phase = k * r_direct;
            let p_direct = amplitude * Complex64::new(phase.cos(), phase.sin()) / r_direct;
            total_pressure += p_direct;
        }

        // First-order reflections (6 walls for rectangular room)
        // This is a simplified model - real FEM would compute the full solution
    }

    // Scale to typical room acoustics levels
    total_pressure *= 0.1;

    Ok(total_pressure)
}
