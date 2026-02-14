use clap::{Parser, ValueEnum};
use math_audio_fem::assembly::HelmholtzProblem;
use math_audio_fem::basis::PolynomialDegree;
use math_audio_fem::boundary::{DirichletBC, apply_dirichlet};
use math_audio_fem::mesh::unit_cube_tetrahedra;
use math_audio_fem::solver::{ShiftedLaplacianConfig, SolverConfig, SolverType, solve};
use num_complex::Complex64;
use std::f64::consts::PI;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(name = "scaling_3d")]
#[command(about = "3D Helmholtz Solver Scaling Analysis")]
struct Args {
    /// Mesh size (n) to test
    #[arg(short, long, default_value = "40")]
    n: usize,

    /// Thread counts to test (comma-separated, e.g., "1,2,4,8,14")
    #[arg(short, long, default_value = "1,2,4,8,14")]
    threads: String,

    /// Solver type to use
    #[arg(short, long, value_enum, default_value = "pipelined-amg")]
    solver: CliSolverType,

    /// Wavenumber k
    #[arg(short, long, default_value = "1.0")]
    k: f64,

    /// Max iterations for iterative solver
    #[arg(long, default_value = "2000")]
    max_iters: usize,

    /// Tolerance for iterative solver
    #[arg(long, default_value = "1e-8")]
    tolerance: f64,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliSolverType {
    Direct,
    Gmres,
    GmresIlu,
    GmresAmg,
    Pipelined,
    PipelinedIlu,
    PipelinedAmg,
    Jacobi,
    Schwarz,
    ShiftedLaplacian,
}

impl From<CliSolverType> for SolverType {
    fn from(cli: CliSolverType) -> Self {
        match cli {
            CliSolverType::Direct => SolverType::Direct,
            CliSolverType::Gmres => SolverType::Gmres,
            CliSolverType::GmresIlu => SolverType::GmresIlu,
            CliSolverType::GmresAmg => SolverType::GmresAmg,
            CliSolverType::Pipelined => SolverType::GmresPipelined,
            CliSolverType::PipelinedIlu => SolverType::GmresPipelinedIlu,
            CliSolverType::PipelinedAmg => SolverType::GmresPipelinedAmg,
            CliSolverType::Jacobi => SolverType::GmresJacobi,
            CliSolverType::Schwarz => SolverType::GmresSchwarz,
            CliSolverType::ShiftedLaplacian => SolverType::GmresShiftedLaplacian,
        }
    }
}

fn run_benchmark(n: usize, threads: usize, args: &Args) {
    let k_val = Complex64::new(args.k, 0.0);
    let coef = 3.0 * PI * PI - args.k * args.k;
    let source = move |x: f64, y: f64, z: f64| {
        Complex64::new(coef * (PI * x).sin() * (PI * y).sin() * (PI * z).sin(), 0.0)
    };
    let exact_u = |x: f64, y: f64, z: f64| {
        Complex64::new((PI * x).sin() * (PI * y).sin() * (PI * z).sin(), 0.0)
    };

    let mut solver_config = SolverConfig::default();
    solver_config.solver_type = args.solver.into();
    solver_config.gmres.max_iterations = args.max_iters;
    solver_config.gmres.tolerance = args.tolerance;

    solver_config.wavenumber = Some(args.k);

    // Configure shifted-Laplacian if that solver is selected
    if matches!(args.solver, CliSolverType::ShiftedLaplacian) {
        solver_config.shifted_laplacian = Some(ShiftedLaplacianConfig::for_wavenumber(args.k));
    }

    // Force thread count
    let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();

    pool.install(|| {
        let start_total = Instant::now();

        let mesh = unit_cube_tetrahedra(n);
        let mesh_time = start_total.elapsed();

        let start_asm = Instant::now();
        let mut problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k_val, source);
        let asm_time = start_asm.elapsed();

        let start_bc = Instant::now();
        let bcs: Vec<DirichletBC> = (1..=6).map(|tag| DirichletBC::new(tag, exact_u)).collect();
        apply_dirichlet(&mut problem, &mesh, &bcs);
        let bc_time = start_bc.elapsed();

        let start_solve = Instant::now();
        let solution = solve(&problem, &solver_config).expect("Solver failed");
        let solve_time = start_solve.elapsed();

        let total_time = start_total.elapsed();

        println!(
            "{:>8} {:>10} {:>10.2} {:>10.2} {:>10.2} {:>10.2} {:>10.2}  {} iters",
            threads,
            problem.num_dofs(),
            mesh_time.as_secs_f64() * 1000.0,
            asm_time.as_secs_f64() * 1000.0,
            bc_time.as_secs_f64() * 1000.0,
            solve_time.as_secs_f64() * 1000.0,
            total_time.as_secs_f64() * 1000.0,
            solution.iterations
        );
    });
}

fn main() {
    let args = Args::parse();

    println!("\n=== 3D Helmholtz Solver Thread Scaling (n={}) ===\n", args.n);
    println!("Solver: {:?}", args.solver);
    println!("k: {}", args.k);
    println!();

    println!(
        "{:>8} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10}  {}",
        "Threads", "DOFs", "Mesh(ms)", "Asm(ms)", "BC(ms)", "Solve(ms)", "Total(ms)", "Status"
    );
    println!("{}", "-".repeat(90));

    let thread_counts: Vec<usize> = args.threads
        .split(',')
        .filter_map(|s: &str| s.trim().parse().ok())
        .collect();

    for threads in thread_counts {
        run_benchmark(args.n, threads, &args);
    }
}
