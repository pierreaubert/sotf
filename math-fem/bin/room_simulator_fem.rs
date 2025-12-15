//! Room Acoustics Simulator using FEM
//!
//! This simulator uses the Finite Element Method to solve the Helmholtz equation
//! for room acoustics. Unlike BEM which works on surface meshes, FEM uses volume
//! meshes and can handle more complex material properties and boundary conditions.
//!
//! ## Parallelism
//!
//! This simulator is designed to scale to many-core machines (64+ cores):
//! - Frequency loop is parallelized (embarrassingly parallel)
//! - Matrix assembly uses parallel element processing
//! - RHS assembly is parallelized over elements
//! - Element location uses parallel search
//!
//! Usage:
//!   cargo run --release --bin roomsim-fem -- --config configs/example_room.json
//!   cargo run --release --bin roomsim-fem -- --help

use clap::{Parser, ValueEnum};
use fem::assembly::{
    HelmholtzMatrix, MassMatrix, StiffnessMatrix, assemble_mass, assemble_stiffness,
};
use fem::basis::PolynomialDegree;
use fem::mesh::{ElementType, Mesh, Point};
use fem::solver::{self, GmresConfigF64, SolverConfig, SolverType};
use num_complex::Complex64;
use rayon::prelude::*;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

// Import common types from math-xem-common
use xem_common::{
    Point3D, RoomConfig, RoomSimulation, Source, create_default_config, create_output_json,
    pressure_to_spl, print_config_summary,
};

/// Default source width in meters (Gaussian sigma)
const DEFAULT_SOURCE_WIDTH: f64 = 0.1;

/// Memory estimation for batch size planning
#[derive(Debug, Clone)]
struct MemoryEstimate {
    /// Number of DOFs (degrees of freedom)
    n_dofs: usize,
    /// Number of non-zeros in stiffness matrix
    k_nnz: usize,
    /// Number of non-zeros in mass matrix
    m_nnz: usize,
    /// GMRES restart (Krylov subspace size)
    krylov_size: usize,
    /// Solver type for preconditioner estimation
    solver_type: SolverType,
    /// Number of Schwarz subdomains
    schwarz_domains: usize,
}

impl MemoryEstimate {
    /// Size of Complex64 in bytes
    const COMPLEX_SIZE: usize = 16; // 2 * f64
    /// Size of usize in bytes
    const USIZE_SIZE: usize = 8;
    /// Size of f64 in bytes
    const F64_SIZE: usize = 8;

    /// Estimate base memory (mesh + K + M matrices) in bytes
    fn base_memory(&self) -> usize {
        // Mesh nodes: 3 * f64 per node (x, y, z)
        let mesh_nodes = self.n_dofs * 3 * Self::F64_SIZE;

        // Mesh elements: ~6 tets per hex cell, 4 node indices per tet
        // Rough estimate: 6 * n_dofs / some_factor
        let mesh_elements = self.n_dofs * 4 * Self::USIZE_SIZE;

        // Stiffness matrix (COO format stored during assembly, then CSR)
        // COO: 3 values per entry (row, col, val)
        let k_storage = self.k_nnz * (2 * Self::USIZE_SIZE + Self::COMPLEX_SIZE);

        // Mass matrix
        let m_storage = self.m_nnz * (2 * Self::USIZE_SIZE + Self::COMPLEX_SIZE);

        mesh_nodes + mesh_elements + k_storage + m_storage
    }

    /// Estimate per-frequency memory in bytes
    /// This is the memory needed for ONE concurrent frequency solve
    fn per_frequency_memory(&self) -> usize {
        // Helmholtz CSR matrix: similar nnz to K
        let helmholtz_csr = self.k_nnz * (Self::USIZE_SIZE + Self::COMPLEX_SIZE)
            + (self.n_dofs + 1) * Self::USIZE_SIZE;

        // RHS vector
        let rhs = self.n_dofs * Self::COMPLEX_SIZE;

        // Solution vector
        let solution = self.n_dofs * Self::COMPLEX_SIZE;

        // GMRES working vectors:
        // - V matrix: (n_dofs x krylov_size) complex
        // - H matrix: (krylov_size+1 x krylov_size) complex
        // - Various work vectors: ~5 * n_dofs
        let gmres_v = self.n_dofs * self.krylov_size * Self::COMPLEX_SIZE;
        let gmres_h = (self.krylov_size + 1) * self.krylov_size * Self::COMPLEX_SIZE;
        let gmres_work = 5 * self.n_dofs * Self::COMPLEX_SIZE;

        // Preconditioner memory (highly variable)
        let precond = self.preconditioner_memory();

        helmholtz_csr + rhs + solution + gmres_v + gmres_h + gmres_work + precond
    }

    /// Estimate preconditioner memory in bytes
    fn preconditioner_memory(&self) -> usize {
        match self.solver_type {
            SolverType::Direct => {
                // Dense LU: O(n²) - very expensive!
                self.n_dofs * self.n_dofs * Self::COMPLEX_SIZE
            }
            SolverType::Gmres | SolverType::GmresPipelined => {
                // No preconditioner
                0
            }
            SolverType::GmresIlu
            | SolverType::GmresPipelinedIlu
            | SolverType::GmresIluColoring
            | SolverType::GmresIluFixedPoint => {
                // ILU(0): L and U have similar nnz to original matrix
                // Factor ~2x for L+U storage
                2 * self.k_nnz * (Self::USIZE_SIZE + Self::COMPLEX_SIZE)
            }
            SolverType::GmresJacobi => {
                // Diagonal only: n_dofs complex values
                self.n_dofs * Self::COMPLEX_SIZE
            }
            SolverType::GmresSchwarz => {
                // Each subdomain has local ILU
                // Subdomain size ≈ n_dofs / num_domains, with overlap
                // Each subdomain ILU ≈ local_nnz * 2
                let subdomain_size =
                    (self.n_dofs as f64 / self.schwarz_domains as f64 * 1.5) as usize;
                let local_nnz = (self.k_nnz as f64 / self.schwarz_domains as f64 * 1.5) as usize;
                let per_subdomain = local_nnz * 2 * (Self::USIZE_SIZE + Self::COMPLEX_SIZE)
                    + subdomain_size * Self::USIZE_SIZE; // index mapping
                per_subdomain * self.schwarz_domains
            }
            SolverType::GmresAmg | SolverType::GmresPipelinedAmg => {
                // AMG: multiple levels, each ~1/4 size of previous
                // Typical operator complexity: 1.5-2.0x
                // Plus interpolation/restriction operators
                let operator_complexity = 1.8;
                let interp_complexity = 0.5;
                let amg_matrices =
                    (operator_complexity * self.k_nnz as f64) as usize * Self::COMPLEX_SIZE;
                let amg_interp =
                    (interp_complexity * self.k_nnz as f64) as usize * Self::COMPLEX_SIZE;
                amg_matrices + amg_interp
            }
        }
    }

    /// Estimate memory for warm-start anchor storage in bytes
    fn anchor_storage_memory(&self, n_anchors: usize) -> usize {
        // Each anchor stores a full solution vector
        n_anchors * self.n_dofs * Self::COMPLEX_SIZE
    }

    /// Calculate recommended batch size given available memory
    fn recommended_batch_size(&self, available_memory_gb: f64, n_threads: usize) -> usize {
        let available_bytes = (available_memory_gb * 1024.0 * 1024.0 * 1024.0) as usize;

        // Reserve memory for base structures
        let base = self.base_memory();

        // Reserve ~20% for system overhead and safety margin
        let usable = ((available_bytes as f64) * 0.80) as usize;

        if usable <= base {
            return 1; // Can barely fit base structures
        }

        let remaining = usable - base;
        let per_freq = self.per_frequency_memory();

        if per_freq == 0 {
            return n_threads;
        }

        // How many frequencies can we solve concurrently?
        let max_concurrent = remaining / per_freq;

        // Clamp to reasonable bounds
        max_concurrent.max(1).min(n_threads * 4)
    }

    /// Format memory size for display
    fn format_bytes(bytes: usize) -> String {
        const KB: usize = 1024;
        const MB: usize = KB * 1024;
        const GB: usize = MB * 1024;

        if bytes >= GB {
            format!("{:.2} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.1} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.1} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} B", bytes)
        }
    }

    /// Print memory breakdown
    fn print_summary(
        &self,
        n_frequencies: usize,
        n_threads: usize,
        available_gb: Option<f64>,
        cpu_info: Option<&CpuCoreInfo>,
    ) {
        let base = self.base_memory();
        let per_freq = self.per_frequency_memory();
        let precond = self.preconditioner_memory();

        println!("\n=== Memory Estimation ===");
        println!("  DOFs: {}", self.n_dofs);
        println!("  Matrix nnz: K={}, M={}", self.k_nnz, self.m_nnz);
        println!();
        println!("  Base memory (mesh + K + M): {}", Self::format_bytes(base));
        println!(
            "  Per-frequency memory: {} (precond: {})",
            Self::format_bytes(per_freq),
            Self::format_bytes(precond)
        );
        println!();

        // Memory for all frequencies in parallel
        let all_parallel = base + per_freq * n_threads;
        println!(
            "  {} threads concurrent: {}",
            n_threads,
            Self::format_bytes(all_parallel)
        );

        // For heterogeneous CPUs, also show P-core only estimate
        if let Some(info) = cpu_info {
            if info.is_heterogeneous {
                if let Some(p_cores) = info.perf_cores {
                    let p_core_mem = base + per_freq * p_cores;
                    println!(
                        "  {} P-cores only concurrent: {}",
                        p_cores,
                        Self::format_bytes(p_core_mem)
                    );
                }
            }
        }

        // Memory for all frequencies at once (worst case)
        let all_at_once = base + per_freq * n_frequencies;
        println!(
            "  All {} frequencies at once: {}",
            n_frequencies,
            Self::format_bytes(all_at_once)
        );

        // Recommendations based on available memory
        if let Some(gb) = available_gb {
            let recommended = self.recommended_batch_size(gb, n_threads);
            let batch_memory = base + per_freq * recommended;
            println!();
            println!("  Available memory: {:.1} GB", gb);
            println!(
                "  Recommended batch size: {} (uses ~{})",
                recommended,
                Self::format_bytes(batch_memory)
            );

            if recommended < n_threads {
                println!(
                    "  WARNING: Memory-constrained! Cannot saturate {} threads",
                    n_threads
                );
                println!("           Consider reducing mesh resolution or Krylov size");
            }
        }

        println!();
    }
}

/// Get available system memory in GB
fn get_system_memory_gb() -> Option<f64> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let output = Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .ok()?;
        let mem_str = String::from_utf8_lossy(&output.stdout);
        let bytes: u64 = mem_str.trim().parse().ok()?;
        Some(bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
    #[cfg(target_os = "linux")]
    {
        let meminfo = std::fs::read_to_string("/proc/meminfo").ok()?;
        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let kb: u64 = parts[1].parse().ok()?;
                    return Some(kb as f64 / (1024.0 * 1024.0));
                }
            }
        }
        None
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        None
    }
}

/// CPU core information for heterogeneous architectures
#[derive(Debug, Clone)]
struct CpuCoreInfo {
    /// Total number of cores
    total_cores: usize,
    /// Number of performance cores (None if homogeneous)
    perf_cores: Option<usize>,
    /// Number of efficiency cores (None if homogeneous)
    efficiency_cores: Option<usize>,
    /// Whether this is a heterogeneous architecture
    is_heterogeneous: bool,
}

impl CpuCoreInfo {
    /// Detect CPU core configuration
    fn detect() -> Self {
        let total_cores = rayon::current_num_threads();

        #[cfg(target_os = "macos")]
        {
            use std::process::Command;

            // Try to get P-core and E-core counts on Apple Silicon
            let perf_cores = Command::new("sysctl")
                .args(["-n", "hw.perflevel0.logicalcpu"])
                .output()
                .ok()
                .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok());

            let efficiency_cores = Command::new("sysctl")
                .args(["-n", "hw.perflevel1.logicalcpu"])
                .output()
                .ok()
                .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok());

            if perf_cores.is_some() && efficiency_cores.is_some() {
                return Self {
                    total_cores,
                    perf_cores,
                    efficiency_cores,
                    is_heterogeneous: true,
                };
            }
        }

        #[cfg(target_os = "linux")]
        {
            // On Linux, check for Intel hybrid architecture via /sys
            // E-cores typically have lower max frequency
            // This is a heuristic - proper detection would need cpuid
            if let Ok(entries) = std::fs::read_dir("/sys/devices/system/cpu") {
                let mut max_freqs: Vec<u64> = Vec::new();
                for entry in entries.flatten() {
                    let path = entry.path().join("cpufreq/cpuinfo_max_freq");
                    if let Ok(freq_str) = std::fs::read_to_string(&path) {
                        if let Ok(freq) = freq_str.trim().parse::<u64>() {
                            max_freqs.push(freq);
                        }
                    }
                }

                if !max_freqs.is_empty() {
                    let max_freq = *max_freqs.iter().max().unwrap();
                    let min_freq = *max_freqs.iter().min().unwrap();

                    // If there's >20% difference in max freq, likely heterogeneous
                    if max_freq > 0 && (max_freq - min_freq) as f64 / max_freq as f64 > 0.2 {
                        let perf_count = max_freqs.iter().filter(|&&f| f == max_freq).count();
                        let eff_count = max_freqs.len() - perf_count;

                        return Self {
                            total_cores,
                            perf_cores: Some(perf_count),
                            efficiency_cores: Some(eff_count),
                            is_heterogeneous: true,
                        };
                    }
                }
            }
        }

        // Homogeneous architecture
        Self {
            total_cores,
            perf_cores: None,
            efficiency_cores: None,
            is_heterogeneous: false,
        }
    }

    /// Get recommended thread count for compute-intensive work
    fn recommended_threads(&self) -> usize {
        if self.is_heterogeneous {
            // Use only P-cores for latency-sensitive batch work
            self.perf_cores.unwrap_or(self.total_cores)
        } else {
            self.total_cores
        }
    }

    /// Print CPU info and recommendations
    fn print_info(&self) {
        if self.is_heterogeneous {
            println!(
                "  CPU: {} cores ({} performance + {} efficiency)",
                self.total_cores,
                self.perf_cores.unwrap_or(0),
                self.efficiency_cores.unwrap_or(0)
            );
            println!(
                "  TIP: Use --threads {} to use only P-cores (faster per-batch)",
                self.perf_cores.unwrap_or(self.total_cores)
            );
            println!(
                "       Or use all {} cores for maximum throughput with more batches",
                self.total_cores
            );
        } else {
            println!("  CPU: {} cores (homogeneous)", self.total_cores);
        }
    }
}

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

    /// Source width in meters (Gaussian sigma)
    #[arg(long, default_value_t = DEFAULT_SOURCE_WIDTH)]
    source_width: f64,

    /// Number of parallel threads (default: all cores)
    #[arg(short = 't', long)]
    threads: Option<usize>,

    /// Frequency batch size for parallel processing (0 = all at once)
    /// Use smaller batches to reduce memory usage on large problems
    #[arg(long, default_value = "0")]
    batch_size: usize,

    /// Enable warm starting with solution interpolation
    /// When enabled, uses a hierarchical approach:
    /// - First pass: solve anchor frequencies (every Nth) from cold start
    /// - Second pass: solve intermediate frequencies with interpolated initial guess
    #[arg(long)]
    warm_start: bool,

    /// Anchor stride for warm starting (solve every Nth frequency first)
    /// Only used when --warm-start is enabled. Default: 4
    #[arg(long, default_value = "4")]
    anchor_stride: usize,

    /// Available system memory in GB for batch size estimation
    /// Default: auto-detect from system
    #[arg(long)]
    memory_gb: Option<f64>,

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

    // Detect CPU configuration
    let cpu_info = CpuCoreInfo::detect();

    // Set number of threads if specified
    if let Some(threads) = args.threads {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build_global()
            .expect("Failed to set thread pool");
        println!("Using {} threads (user-specified)\n", threads);
    } else {
        // On heterogeneous architectures, suggest using P-cores only
        if cpu_info.is_heterogeneous {
            cpu_info.print_info();
            println!();
        } else {
            println!(
                "Using {} threads (all cores)\n",
                rayon::current_num_threads()
            );
        }
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
        (CliSolverType::Gmres, Some(CliPreconditionerType::IluColoring)) => {
            SolverType::GmresIluColoring
        }
        (CliSolverType::Gmres, Some(CliPreconditionerType::IluFixedpoint)) => {
            SolverType::GmresIluFixedPoint
        }
        (CliSolverType::Gmres, Some(CliPreconditionerType::Schwarz)) => SolverType::GmresSchwarz,
        (CliSolverType::Gmres, Some(CliPreconditionerType::Amg)) => SolverType::GmresAmg,
        (CliSolverType::Pipelined, None) => SolverType::GmresPipelined,
        (CliSolverType::Pipelined, Some(CliPreconditionerType::Ilu)) => {
            SolverType::GmresPipelinedIlu
        }
        (CliSolverType::Pipelined, Some(CliPreconditionerType::Jacobi)) => {
            SolverType::GmresJacobi // fallback to regular Jacobi
        }
        (CliSolverType::Pipelined, Some(CliPreconditionerType::Schwarz)) => {
            SolverType::GmresSchwarz // fallback to regular Schwarz
        }
        (CliSolverType::Pipelined, Some(CliPreconditionerType::Amg)) => {
            SolverType::GmresPipelinedAmg
        }
        (CliSolverType::Pipelined, Some(CliPreconditionerType::IluColoring)) => {
            SolverType::GmresIluColoring // fallback
        }
        (CliSolverType::Pipelined, Some(CliPreconditionerType::IluFixedpoint)) => {
            SolverType::GmresIluFixedPoint // fallback
        }
    };

    // Get available memory (from CLI or auto-detect)
    let available_memory_gb = args.memory_gb.or_else(get_system_memory_gb);

    // Run simulation
    let output_data = run_fem_simulation(
        &simulation,
        &config,
        internal_solver_type,
        args.krylov_size,
        args.schwarz_domains,
        args.source_width,
        args.batch_size,
        args.warm_start,
        args.anchor_stride,
        available_memory_gb,
        &cpu_info,
        args.verbose,
    )?;

    // Save results
    println!("\nSaving results to: {}", args.output.display());
    fs::write(&args.output, serde_json::to_string_pretty(&output_data)?)?;
    println!("Done!");

    Ok(())
}

/// Create a tetrahedral mesh for the room using conforming 6-tetrahedron decomposition
///
/// Uses an alternating decomposition pattern based on (i+j+k) % 2 to ensure
/// conforming meshes at cell boundaries (shared faces use the same diagonal).
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
                mesh.add_node(Point::new_3d(i as f64 * dx, j as f64 * dy, k as f64 * dz));
            }
        }
    }

    // Generate tetrahedral elements by subdividing hex cells
    // Use 6-tetrahedron decomposition with alternating pattern for conforming mesh
    for k in 0..(nz - 1) {
        for j in 0..(ny - 1) {
            for i in 0..(nx - 1) {
                // Hex vertices indexed as:
                //     v6----v7
                //    /|    /|
                //   v4----v5|
                //   | v2--|-v3
                //   |/    |/
                //   v0----v1
                let v0 = k * ny * nx + j * nx + i;
                let v1 = v0 + 1;
                let v2 = v0 + nx;
                let v3 = v2 + 1;
                let v4 = v0 + ny * nx;
                let v5 = v4 + 1;
                let v6 = v4 + nx;
                let v7 = v6 + 1;

                // Alternate decomposition based on cell parity for conforming mesh
                // This ensures adjacent cells share the same face diagonal
                if (i + j + k) % 2 == 0 {
                    // Type A: 6-tet decomposition using diagonal v0-v7
                    mesh.add_element(ElementType::Tetrahedron, vec![v0, v1, v3, v7]);
                    mesh.add_element(ElementType::Tetrahedron, vec![v0, v1, v7, v5]);
                    mesh.add_element(ElementType::Tetrahedron, vec![v0, v5, v7, v4]);
                    mesh.add_element(ElementType::Tetrahedron, vec![v0, v3, v2, v7]);
                    mesh.add_element(ElementType::Tetrahedron, vec![v0, v2, v6, v7]);
                    mesh.add_element(ElementType::Tetrahedron, vec![v0, v6, v4, v7]);
                } else {
                    // Type B: 6-tet decomposition using diagonal v1-v6
                    mesh.add_element(ElementType::Tetrahedron, vec![v1, v0, v2, v6]);
                    mesh.add_element(ElementType::Tetrahedron, vec![v1, v0, v6, v4]);
                    mesh.add_element(ElementType::Tetrahedron, vec![v1, v4, v6, v5]);
                    mesh.add_element(ElementType::Tetrahedron, vec![v1, v2, v3, v6]);
                    mesh.add_element(ElementType::Tetrahedron, vec![v1, v3, v7, v6]);
                    mesh.add_element(ElementType::Tetrahedron, vec![v1, v7, v5, v6]);
                }
            }
        }
    }

    // Detect boundaries for Neumann BC (rigid walls)
    mesh.detect_boundaries();

    mesh
}

/// Result from solving a single frequency
struct FrequencyResult {
    frequency: f64,
    spl_values: Vec<f64>, // SPL at each listening position
    iterations: usize,
    residual: f64,
}

/// Extended result that also stores the solution vector for warm starting
struct FrequencyResultWithSolution {
    frequency: f64,
    solution: ndarray::Array1<Complex64>,
    spl_values: Vec<f64>,
    iterations: usize,
    residual: f64,
}

/// Run FEM simulation for all frequencies with parallel processing
///
/// Key parallelization strategies:
/// 1. Stiffness/Mass matrices assembled once (parallel element processing)
/// 2. Frequency loop parallelized - each frequency solved independently
/// 3. RHS assembly parallelized over elements
/// 4. Element location for listening positions done in parallel
///
/// Warm starting strategy (when enabled):
/// 1. First pass: Solve anchor frequencies (every Nth) in parallel with cold start
/// 2. Second pass: Solve intermediate frequencies with interpolated initial guess
#[allow(clippy::too_many_arguments)]
fn run_fem_simulation(
    simulation: &RoomSimulation,
    config: &RoomConfig,
    solver_type: SolverType,
    krylov_size: usize,
    schwarz_domains: usize,
    source_width: f64,
    batch_size: usize,
    warm_start: bool,
    anchor_stride: usize,
    available_memory_gb: Option<f64>,
    cpu_info: &CpuCoreInfo,
    verbose: bool,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let solver_name = format!("{:?}", solver_type);
    println!("\n=== {} Solver (Parallel) ===", solver_name.to_uppercase());

    // Create mesh
    let mesh_start = Instant::now();
    let mesh = create_room_mesh(simulation, config.solver.mesh_resolution);
    println!(
        "Mesh: {} nodes, {} elements (created in {:.1}ms)",
        mesh.num_nodes(),
        mesh.num_elements(),
        mesh_start.elapsed().as_secs_f64() * 1000.0
    );

    // Configure solver (per-thread config, no print to avoid interleaving)
    let solver_config = SolverConfig {
        solver_type,
        gmres: GmresConfigF64 {
            max_iterations: config.solver.gmres.max_iter,
            restart: krylov_size,
            tolerance: config.solver.gmres.tolerance,
            print_interval: 0, // Disable per-iteration printing in parallel
        },
        verbosity: 0, // Disable verbose in parallel to avoid interleaved output
        schwarz_subdomains: schwarz_domains,
        schwarz_overlap: 2,
    };

    // Assemble stiffness and mass matrices ONCE (frequency-independent)
    // These use parallel assembly internally when "parallel" feature is enabled
    println!("\nAssembling stiffness and mass matrices (one-time cost)...");
    let matrix_start = Instant::now();
    let stiffness = assemble_stiffness(&mesh, PolynomialDegree::P1);
    let mass = assemble_mass(&mesh, PolynomialDegree::P1);
    let matrix_time = matrix_start.elapsed();
    println!(
        "  Matrix assembly: {:.1}ms (K: {} nnz, M: {} nnz)",
        matrix_time.as_secs_f64() * 1000.0,
        stiffness.nnz(),
        mass.nnz()
    );

    // Memory estimation
    let n_threads = rayon::current_num_threads();
    let n_freqs_for_estimate = simulation.frequencies.len();
    let mem_estimate = MemoryEstimate {
        n_dofs: mesh.num_nodes(),
        k_nnz: stiffness.nnz(),
        m_nnz: mass.nnz(),
        krylov_size,
        solver_type,
        schwarz_domains,
    };

    if verbose {
        mem_estimate.print_summary(
            n_freqs_for_estimate,
            n_threads,
            available_memory_gb,
            Some(cpu_info),
        );
    }

    // Determine effective batch size
    let effective_batch_size = if batch_size == 0 {
        // Auto-determine based on memory if available
        if let Some(gb) = available_memory_gb {
            let recommended = mem_estimate.recommended_batch_size(gb, n_threads);
            if verbose {
                println!("  Using auto-determined batch size: {}", recommended);
            }
            recommended
        } else {
            n_freqs_for_estimate // All at once if no memory info
        }
    } else {
        batch_size
    };

    // Pre-locate elements containing listening positions (parallel search)
    let listening_positions = &simulation.listening_positions;
    println!(
        "\nLocating {} listening positions in mesh...",
        listening_positions.len()
    );
    let locate_start = Instant::now();
    let lp_elements: Vec<Option<usize>> = listening_positions
        .par_iter()
        .map(|lp| find_containing_element_parallel(&mesh, *lp))
        .collect();
    println!(
        "  Element location: {:.1}ms",
        locate_start.elapsed().as_secs_f64() * 1000.0
    );

    // Report how many positions were found
    let found_count = lp_elements.iter().filter(|e| e.is_some()).count();
    if found_count < listening_positions.len() {
        println!(
            "  Warning: {} of {} positions are outside mesh (will use nearest-neighbor)",
            listening_positions.len() - found_count,
            listening_positions.len()
        );
    }

    // Prepare frequency data
    let frequencies = &simulation.frequencies;
    let n_freqs = frequencies.len();
    let sources = &simulation.sources;

    let solve_start = Instant::now();
    let progress_counter = AtomicUsize::new(0);

    // Process frequencies - either with warm start or cold start
    let all_results = if warm_start && n_freqs > anchor_stride {
        run_hierarchical_solve(
            &mesh,
            &stiffness,
            &mass,
            &solver_config,
            sources,
            simulation,
            frequencies,
            source_width,
            listening_positions,
            &lp_elements,
            anchor_stride,
            &progress_counter,
            &solve_start,
            verbose,
        )
    } else {
        // Original cold-start parallel approach
        println!(
            "\nProcessing {} frequencies on {} threads (batch size: {})...",
            n_freqs,
            rayon::current_num_threads(),
            effective_batch_size
        );

        let mut all_results: Vec<FrequencyResult> = Vec::with_capacity(n_freqs);

        for batch_start in (0..n_freqs).step_by(effective_batch_size) {
            let batch_end = (batch_start + effective_batch_size).min(n_freqs);
            let batch_freqs = &frequencies[batch_start..batch_end];

            // Process this batch in parallel
            let batch_results: Vec<FrequencyResult> = batch_freqs
                .par_iter()
                .enumerate()
                .map(|(_batch_idx, &freq)| {
                    // Solve for this frequency
                    let result = solve_single_frequency(
                        &mesh,
                        &stiffness,
                        &mass,
                        &solver_config,
                        sources,
                        freq,
                        simulation.wavenumber(freq),
                        source_width,
                        listening_positions,
                        &lp_elements,
                    );

                    // Update progress
                    let completed = progress_counter.fetch_add(1, Ordering::Relaxed) + 1;
                    if verbose || completed % 10 == 0 || completed == n_freqs {
                        let elapsed = solve_start.elapsed().as_secs_f64();
                        let rate = completed as f64 / elapsed;
                        let remaining = (n_freqs - completed) as f64 / rate;
                        print!(
                            "\r  Progress: {}/{} ({:.1}%) - {:.1} freq/s - ETA: {:.0}s    ",
                            completed,
                            n_freqs,
                            100.0 * completed as f64 / n_freqs as f64,
                            rate,
                            remaining
                        );
                        use std::io::Write;
                        std::io::stdout().flush().ok();
                    }

                    FrequencyResult {
                        frequency: freq,
                        spl_values: result.0,
                        iterations: result.1,
                        residual: result.2,
                    }
                })
                .collect();

            all_results.extend(batch_results);
        }

        all_results
    };

    println!(); // New line after progress

    let solve_time = solve_start.elapsed();
    let freq_per_sec = n_freqs as f64 / solve_time.as_secs_f64();
    println!(
        "\nSolve complete: {:.2}s total ({:.1} frequencies/sec)",
        solve_time.as_secs_f64(),
        freq_per_sec
    );

    // Sort results by frequency (parallel processing may disorder them)
    all_results.sort_by(|a, b| a.frequency.partial_cmp(&b.frequency).unwrap());

    // Report solve statistics
    if verbose {
        let total_iters: usize = all_results.iter().map(|r| r.iterations).sum();
        let avg_iters = total_iters as f64 / n_freqs as f64;
        let max_residual = all_results
            .iter()
            .map(|r| r.residual)
            .fold(0.0f64, |a, b| a.max(b));
        println!(
            "  Average iterations: {:.1}, Max residual: {:.2e}",
            avg_iters, max_residual
        );
    }

    // Collect SPL values for each listening position
    let n_lps = listening_positions.len();
    let mut all_lp_spl_values: Vec<Vec<f64>> = vec![Vec::with_capacity(n_freqs); n_lps];
    for result in &all_results {
        for (lp_idx, &spl) in result.spl_values.iter().enumerate() {
            all_lp_spl_values[lp_idx].push(spl);
        }
    }

    // Use first listening position for backward compatibility
    Ok(create_output_json(
        simulation,
        config,
        all_lp_spl_values[0].clone(),
        &solver_name,
    ))
}

/// Solve for a single frequency (called in parallel)
///
/// Returns (spl_values, iterations, residual)
fn solve_single_frequency(
    mesh: &Mesh,
    stiffness: &StiffnessMatrix,
    mass: &MassMatrix,
    solver_config: &SolverConfig,
    sources: &[Source],
    frequency: f64,
    wavenumber: f64,
    source_width: f64,
    listening_positions: &[Point3D],
    lp_elements: &[Option<usize>],
) -> (Vec<f64>, usize, f64) {
    let k = Complex64::new(wavenumber, 0.0);

    // Combine stiffness and mass into Helmholtz matrix: A = K - k²M
    let helmholtz = HelmholtzMatrix::new(stiffness, mass, k);

    // Assemble RHS for this frequency (parallel over elements)
    let rhs = assemble_rhs_parallel(mesh, sources, frequency, source_width);

    // Create problem struct for solver
    let problem = fem::assembly::HelmholtzProblem {
        matrix: helmholtz,
        rhs,
        stiffness: stiffness.clone(),
        mass: mass.clone(),
    };

    // Solve the system
    let solution = solver::solve(&problem, solver_config).expect("Solver failed");

    // Evaluate pressure at all listening positions
    let spl_values: Vec<f64> = listening_positions
        .iter()
        .zip(lp_elements.iter())
        .map(|(lp, elem_opt)| {
            let pressure =
                evaluate_solution_at_point_interpolated(mesh, &solution.values, *lp, *elem_opt);
            pressure_to_spl(pressure)
        })
        .collect();

    (spl_values, solution.iterations, solution.residual)
}

/// Assemble RHS vector with parallel element processing
fn assemble_rhs_parallel(
    mesh: &Mesh,
    sources: &[Source],
    frequency: f64,
    source_width: f64,
) -> Vec<Complex64> {
    use fem::basis::{Jacobian, evaluate_shape};
    use fem::quadrature::for_mass;

    let n_dofs = mesh.num_nodes();
    let n_elems = mesh.num_elements();

    // Compute element contributions in parallel
    let element_contribs: Vec<Vec<(usize, Complex64)>> = (0..n_elems)
        .into_par_iter()
        .map(|elem_idx| {
            let elem = &mesh.elements[elem_idx];
            let elem_type = elem.element_type;
            let vertices = elem.vertices();
            let n_nodes = vertices.len();

            let quad = for_mass(elem_type, 1); // P1 degree

            let coords: Vec<[f64; 3]> = vertices
                .iter()
                .map(|&v| [mesh.nodes[v].x, mesh.nodes[v].y, mesh.nodes[v].z])
                .collect();

            let mut local_contribs = Vec::with_capacity(n_nodes);

            for qp in quad.iter() {
                let shape = evaluate_shape(
                    elem_type,
                    PolynomialDegree::P1,
                    qp.xi(),
                    qp.eta(),
                    qp.zeta(),
                );
                let jac = Jacobian::from_3d(&shape.gradients, &coords);

                // Physical coordinates at quadrature point
                let x: f64 = shape
                    .values
                    .iter()
                    .zip(&coords)
                    .map(|(n, c)| n * c[0])
                    .sum();
                let y: f64 = shape
                    .values
                    .iter()
                    .zip(&coords)
                    .map(|(n, c)| n * c[1])
                    .sum();
                let z: f64 = shape
                    .values
                    .iter()
                    .zip(&coords)
                    .map(|(n, c)| n * c[2])
                    .sum();

                let f_val = compute_source_term(x, y, z, sources, frequency, source_width);
                let det_j = jac.det.abs();

                for i in 0..n_nodes {
                    let contrib = f_val * Complex64::new(shape.values[i] * det_j * qp.weight, 0.0);
                    if contrib.norm() > 1e-15 {
                        local_contribs.push((vertices[i], contrib));
                    }
                }
            }

            local_contribs
        })
        .collect();

    // Merge contributions into final RHS vector
    let mut rhs = vec![Complex64::new(0.0, 0.0); n_dofs];
    for contribs in element_contribs {
        for (node_idx, val) in contribs {
            rhs[node_idx] += val;
        }
    }

    rhs
}

/// Compute source term at a point from all sources
fn compute_source_term(
    x: f64,
    y: f64,
    z: f64,
    sources: &[Source],
    frequency: f64,
    source_width: f64,
) -> Complex64 {
    let point = Point3D::new(x, y, z);
    let mut total = Complex64::new(0.0, 0.0);

    for source in sources {
        // Gaussian source distribution centered at source position
        let r = source.position.distance_to(&point);

        // Gaussian envelope with configurable width
        let envelope = (-r * r / (2.0 * source_width * source_width)).exp();

        // Get directional amplitude
        let amplitude = source.amplitude_towards(&point, frequency);

        total += Complex64::new(amplitude * envelope, 0.0);
    }

    total
}

/// Find the element containing a point using parallel search
///
/// Uses parallel iteration with early termination via find_map_any
fn find_containing_element_parallel(mesh: &Mesh, point: Point3D) -> Option<usize> {
    // Use parallel search with early termination
    (0..mesh.elements.len())
        .into_par_iter()
        .find_map_any(|elem_idx| {
            let elem = &mesh.elements[elem_idx];
            if elem.element_type != ElementType::Tetrahedron {
                return None;
            }

            let vertices = elem.vertices();
            if vertices.len() != 4 {
                return None;
            }

            let p0 = &mesh.nodes[vertices[0]];
            let p1 = &mesh.nodes[vertices[1]];
            let p2 = &mesh.nodes[vertices[2]];
            let p3 = &mesh.nodes[vertices[3]];

            if compute_barycentric_coords(point, p0, p1, p2, p3).is_some() {
                Some(elem_idx)
            } else {
                None
            }
        })
}

/// Compute barycentric coordinates of a point in a tetrahedron
/// Returns None if point is outside the tetrahedron
fn compute_barycentric_coords(
    p: Point3D,
    v0: &Point,
    v1: &Point,
    v2: &Point,
    v3: &Point,
) -> Option<[f64; 4]> {
    // Vectors from v0
    let v0p = [p.x - v0.x, p.y - v0.y, p.z - v0.z];
    let v01 = [v1.x - v0.x, v1.y - v0.y, v1.z - v0.z];
    let v02 = [v2.x - v0.x, v2.y - v0.y, v2.z - v0.z];
    let v03 = [v3.x - v0.x, v3.y - v0.y, v3.z - v0.z];

    // Compute determinant of [v01, v02, v03]
    let det = v01[0] * (v02[1] * v03[2] - v02[2] * v03[1])
        - v01[1] * (v02[0] * v03[2] - v02[2] * v03[0])
        + v01[2] * (v02[0] * v03[1] - v02[1] * v03[0]);

    if det.abs() < 1e-15 {
        return None; // Degenerate tetrahedron
    }

    let inv_det = 1.0 / det;

    // Solve for barycentric coordinates using Cramer's rule
    // [lambda1, lambda2, lambda3] = inv([v01, v02, v03]) * v0p
    // Then lambda0 = 1 - lambda1 - lambda2 - lambda3

    // Cofactor matrix (transposed for inverse)
    let c00 = v02[1] * v03[2] - v02[2] * v03[1];
    let c01 = -(v02[0] * v03[2] - v02[2] * v03[0]);
    let c02 = v02[0] * v03[1] - v02[1] * v03[0];

    let c10 = -(v01[1] * v03[2] - v01[2] * v03[1]);
    let c11 = v01[0] * v03[2] - v01[2] * v03[0];
    let c12 = -(v01[0] * v03[1] - v01[1] * v03[0]);

    let c20 = v01[1] * v02[2] - v01[2] * v02[1];
    let c21 = -(v01[0] * v02[2] - v01[2] * v02[0]);
    let c22 = v01[0] * v02[1] - v01[1] * v02[0];

    let lambda1 = (c00 * v0p[0] + c10 * v0p[1] + c20 * v0p[2]) * inv_det;
    let lambda2 = (c01 * v0p[0] + c11 * v0p[1] + c21 * v0p[2]) * inv_det;
    let lambda3 = (c02 * v0p[0] + c12 * v0p[1] + c22 * v0p[2]) * inv_det;
    let lambda0 = 1.0 - lambda1 - lambda2 - lambda3;

    // Check if inside (all barycentric coords >= 0, with small tolerance)
    let tol = -1e-10;
    if lambda0 >= tol && lambda1 >= tol && lambda2 >= tol && lambda3 >= tol {
        Some([lambda0, lambda1, lambda2, lambda3])
    } else {
        None
    }
}

/// Evaluate FEM solution at a specific point using shape function interpolation
///
/// If the containing element is known, uses proper barycentric interpolation.
/// Falls back to nearest-neighbor if element lookup fails.
fn evaluate_solution_at_point_interpolated(
    mesh: &Mesh,
    solution: &ndarray::Array1<Complex64>,
    point: Point3D,
    containing_element: Option<usize>,
) -> Complex64 {
    // Try shape function interpolation first
    if let Some(elem_idx) = containing_element {
        let elem = &mesh.elements[elem_idx];
        let vertices = elem.vertices();

        if vertices.len() == 4 {
            let v0 = &mesh.nodes[vertices[0]];
            let v1 = &mesh.nodes[vertices[1]];
            let v2 = &mesh.nodes[vertices[2]];
            let v3 = &mesh.nodes[vertices[3]];

            if let Some(bary) = compute_barycentric_coords(point, v0, v1, v2, v3) {
                // Interpolate using barycentric coordinates (which are the P1 shape functions)
                return solution[vertices[0]] * bary[0]
                    + solution[vertices[1]] * bary[1]
                    + solution[vertices[2]] * bary[2]
                    + solution[vertices[3]] * bary[3];
            }
        }
    }

    // Fallback to nearest-neighbor
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

/// Run hierarchical solve with warm starting
///
/// Strategy:
/// 1. First pass: Solve anchor frequencies (every Nth) in parallel with cold start
/// 2. Second pass: Solve intermediate frequencies with interpolated initial guess
#[allow(clippy::too_many_arguments)]
fn run_hierarchical_solve(
    mesh: &Mesh,
    stiffness: &StiffnessMatrix,
    mass: &MassMatrix,
    solver_config: &SolverConfig,
    sources: &[Source],
    simulation: &RoomSimulation,
    frequencies: &[f64],
    source_width: f64,
    listening_positions: &[Point3D],
    lp_elements: &[Option<usize>],
    anchor_stride: usize,
    progress_counter: &AtomicUsize,
    solve_start: &Instant,
    verbose: bool,
) -> Vec<FrequencyResult> {
    let n_freqs = frequencies.len();

    // Identify anchor indices (every Nth frequency, plus the last one)
    let anchor_indices: Vec<usize> = (0..n_freqs)
        .step_by(anchor_stride)
        .chain(std::iter::once(n_freqs - 1))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .into_iter()
        .collect();
    let mut anchor_indices = anchor_indices;
    anchor_indices.sort();

    let n_anchors = anchor_indices.len();
    let n_intermediates = n_freqs - n_anchors;

    println!(
        "\nWarm-start hierarchical solve ({} anchors + {} intermediates) on {} threads...",
        n_anchors,
        n_intermediates,
        rayon::current_num_threads()
    );

    // ===== PASS 1: Solve anchor frequencies in parallel (cold start) =====
    println!(
        "  Pass 1: Solving {} anchor frequencies (cold start)...",
        n_anchors
    );

    let anchor_results: Vec<FrequencyResultWithSolution> = anchor_indices
        .par_iter()
        .map(|&idx| {
            let freq = frequencies[idx];
            let (solution, spl_values, iterations, residual) =
                solve_single_frequency_returning_solution(
                    mesh,
                    stiffness,
                    mass,
                    solver_config,
                    sources,
                    freq,
                    simulation.wavenumber(freq),
                    source_width,
                    listening_positions,
                    lp_elements,
                    None, // cold start
                );

            // Update progress
            let completed = progress_counter.fetch_add(1, Ordering::Relaxed) + 1;
            if verbose || completed % 10 == 0 {
                let elapsed = solve_start.elapsed().as_secs_f64();
                let rate = completed as f64 / elapsed;
                let remaining = (n_freqs - completed) as f64 / rate;
                print!(
                    "\r  Progress: {}/{} ({:.1}%) - {:.1} freq/s - ETA: {:.0}s    ",
                    completed,
                    n_freqs,
                    100.0 * completed as f64 / n_freqs as f64,
                    rate,
                    remaining
                );
                use std::io::Write;
                std::io::stdout().flush().ok();
            }

            FrequencyResultWithSolution {
                frequency: freq,
                solution,
                spl_values,
                iterations,
                residual,
            }
        })
        .collect();

    // Build a map from frequency index to anchor result for interpolation
    let mut anchor_map: std::collections::HashMap<usize, &FrequencyResultWithSolution> =
        std::collections::HashMap::new();
    for (ai, &idx) in anchor_indices.iter().enumerate() {
        anchor_map.insert(idx, &anchor_results[ai]);
    }

    // ===== PASS 2: Solve intermediate frequencies with warm start =====
    let intermediate_indices: Vec<usize> = (0..n_freqs)
        .filter(|i| !anchor_map.contains_key(i))
        .collect();

    if !intermediate_indices.is_empty() {
        println!(
            "\n  Pass 2: Solving {} intermediate frequencies (warm start)...",
            intermediate_indices.len()
        );
    }

    // For each intermediate, find bounding anchors and interpolate
    let intermediate_results: Vec<FrequencyResult> = intermediate_indices
        .par_iter()
        .map(|&idx| {
            let freq = frequencies[idx];

            // Find bounding anchor indices
            let lower_anchor_idx = anchor_indices
                .iter()
                .filter(|&&ai| ai < idx)
                .max()
                .copied()
                .unwrap_or(0);
            let upper_anchor_idx = anchor_indices
                .iter()
                .filter(|&&ai| ai > idx)
                .min()
                .copied()
                .unwrap_or(n_freqs - 1);

            // Get anchor solutions
            let lower_result = anchor_map.get(&lower_anchor_idx).unwrap();
            let upper_result = anchor_map.get(&upper_anchor_idx).unwrap();

            // Linear interpolation weight
            let t = if upper_anchor_idx != lower_anchor_idx {
                (idx - lower_anchor_idx) as f64 / (upper_anchor_idx - lower_anchor_idx) as f64
            } else {
                0.5
            };

            // Interpolate solution vectors
            let interpolated_guess =
                &lower_result.solution * (1.0 - t) + &upper_result.solution * t;

            // Solve with warm start
            let (_, spl_values, iterations, residual) = solve_single_frequency_returning_solution(
                mesh,
                stiffness,
                mass,
                solver_config,
                sources,
                freq,
                simulation.wavenumber(freq),
                source_width,
                listening_positions,
                lp_elements,
                Some(&interpolated_guess),
            );

            // Update progress
            let completed = progress_counter.fetch_add(1, Ordering::Relaxed) + 1;
            if verbose || completed % 10 == 0 || completed == n_freqs {
                let elapsed = solve_start.elapsed().as_secs_f64();
                let rate = completed as f64 / elapsed;
                let remaining = (n_freqs - completed) as f64 / rate;
                print!(
                    "\r  Progress: {}/{} ({:.1}%) - {:.1} freq/s - ETA: {:.0}s    ",
                    completed,
                    n_freqs,
                    100.0 * completed as f64 / n_freqs as f64,
                    rate,
                    remaining
                );
                use std::io::Write;
                std::io::stdout().flush().ok();
            }

            FrequencyResult {
                frequency: freq,
                spl_values,
                iterations,
                residual,
            }
        })
        .collect();

    // Combine anchor and intermediate results
    let mut all_results: Vec<FrequencyResult> = Vec::with_capacity(n_freqs);

    // Convert anchor results to FrequencyResult (dropping solution vectors)
    for ar in anchor_results {
        all_results.push(FrequencyResult {
            frequency: ar.frequency,
            spl_values: ar.spl_values,
            iterations: ar.iterations,
            residual: ar.residual,
        });
    }
    all_results.extend(intermediate_results);

    // Sort by frequency
    all_results.sort_by(|a, b| a.frequency.partial_cmp(&b.frequency).unwrap());

    // Report warm start statistics
    if verbose {
        let anchor_iters: usize = all_results
            .iter()
            .enumerate()
            .filter(|(i, _)| anchor_indices.contains(i))
            .map(|(_, r)| r.iterations)
            .sum();
        let intermediate_iters: usize = all_results
            .iter()
            .enumerate()
            .filter(|(i, _)| !anchor_indices.contains(i))
            .map(|(_, r)| r.iterations)
            .sum();

        let avg_anchor = if n_anchors > 0 {
            anchor_iters as f64 / n_anchors as f64
        } else {
            0.0
        };
        let avg_intermediate = if n_intermediates > 0 {
            intermediate_iters as f64 / n_intermediates as f64
        } else {
            0.0
        };

        println!(
            "\n  Warm start stats: anchors avg {:.1} iters, intermediates avg {:.1} iters ({:.1}% reduction)",
            avg_anchor,
            avg_intermediate,
            if avg_anchor > 0.0 {
                100.0 * (1.0 - avg_intermediate / avg_anchor)
            } else {
                0.0
            }
        );
    }

    all_results
}

/// Solve for a single frequency, returning the solution vector for warm starting
///
/// Returns (solution, spl_values, iterations, residual)
#[allow(clippy::too_many_arguments)]
fn solve_single_frequency_returning_solution(
    mesh: &Mesh,
    stiffness: &StiffnessMatrix,
    mass: &MassMatrix,
    solver_config: &SolverConfig,
    sources: &[Source],
    frequency: f64,
    wavenumber: f64,
    source_width: f64,
    listening_positions: &[Point3D],
    lp_elements: &[Option<usize>],
    initial_guess: Option<&ndarray::Array1<Complex64>>,
) -> (ndarray::Array1<Complex64>, Vec<f64>, usize, f64) {
    let k = Complex64::new(wavenumber, 0.0);

    // Combine stiffness and mass into Helmholtz matrix: A = K - k²M
    let helmholtz = HelmholtzMatrix::new(stiffness, mass, k);

    // Assemble RHS for this frequency (parallel over elements)
    let rhs = assemble_rhs_parallel(mesh, sources, frequency, source_width);

    // Convert to CSR and solve with optional initial guess
    let csr = helmholtz.to_csr();
    let rhs_array = ndarray::Array1::from(rhs);

    let solution = solver::solve_csr_with_guess(&csr, &rhs_array, initial_guess, solver_config)
        .expect("Solver failed");

    // Evaluate pressure at all listening positions
    let spl_values: Vec<f64> = listening_positions
        .iter()
        .zip(lp_elements.iter())
        .map(|(lp, elem_opt)| {
            let pressure =
                evaluate_solution_at_point_interpolated(mesh, &solution.values, *lp, *elem_opt);
            pressure_to_spl(pressure)
        })
        .collect();

    (
        solution.values,
        spl_values,
        solution.iterations,
        solution.residual,
    )
}
