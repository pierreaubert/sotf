//! 3D Helmholtz solver scaling analysis
//!
//! Prints a detailed table showing how the solver scales with mesh size
//! and thread count.
//!
//! Run with different thread counts:
//!   RAYON_NUM_THREADS=1 cargo run -p math-fem --example scaling_3d --release
//!   RAYON_NUM_THREADS=2 cargo run -p math-fem --example scaling_3d --release
//!   RAYON_NUM_THREADS=4 cargo run -p math-fem --example scaling_3d --release
//!   RAYON_NUM_THREADS=8 cargo run -p math-fem --example scaling_3d --release

use fem::assembly::HelmholtzProblem;
use fem::basis::PolynomialDegree;
use fem::boundary::{DirichletBC, apply_dirichlet};
use fem::mesh::unit_cube_tetrahedra;
use ndarray::Array1;
use num_complex::Complex64;
use solvers::{CsrMatrix, GmresConfig, gmres};
use std::f64::consts::PI;
use std::time::Instant;

/// Convert HelmholtzMatrix to CsrMatrix for solver
fn to_csr_matrix(problem: &HelmholtzProblem) -> CsrMatrix<Complex64> {
    let compressed = problem.matrix.to_compressed();
    let n = compressed.dim;

    let triplets: Vec<(usize, usize, Complex64)> = (0..compressed.nnz())
        .map(|i| (compressed.rows[i], compressed.cols[i], compressed.values[i]))
        .collect();

    CsrMatrix::from_triplets(n, n, triplets)
}

fn main() {
    println!();
    println!("=== 3D Helmholtz Solver Scaling Analysis ===");
    println!();

    #[cfg(feature = "parallel")]
    {
        let num_threads = rayon::current_num_threads();
        println!("Parallelism: ENABLED ({} threads)", num_threads);
    }

    #[cfg(not(feature = "parallel"))]
    {
        println!("Parallelism: DISABLED (single-threaded)");
    }

    println!();
    println!("Problem: -∆u - k²u = f in [0,1]³ with Dirichlet BCs");
    println!("MMS solution: u = sin(πx) sin(πy) sin(πz)");
    println!("Wavenumber: k = 1.0");
    println!();

    let k = Complex64::new(1.0, 0.0);

    // MMS: u = sin(πx) * sin(πy) * sin(πz)
    // Laplacian: ∆u = -3π²u
    // f = -∆u - k²u = (3π² - k²)u
    let coef = 3.0 * PI * PI - 1.0;
    let source = move |x: f64, y: f64, z: f64| {
        Complex64::new(coef * (PI * x).sin() * (PI * y).sin() * (PI * z).sin(), 0.0)
    };

    let exact_u = |x: f64, y: f64, z: f64| {
        Complex64::new((PI * x).sin() * (PI * y).sin() * (PI * z).sin(), 0.0)
    };

    let config = GmresConfig {
        max_iterations: 1000,
        restart: 50,
        tolerance: 1e-8,
        print_interval: 0,
    };

    // Header
    println!(
        "{:>4} {:>8} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>12}",
        "n",
        "DOFs",
        "Elements",
        "Mesh(ms)",
        "Asm(ms)",
        "BC(ms)",
        "Solve(ms)",
        "Total(ms)",
        "Status"
    );
    println!("{}", "-".repeat(100));

    // Run tests with increasing mesh sizes
    for n in [4, 6, 8, 10, 12, 14] {
        // 1. Mesh generation
        let start_mesh = Instant::now();
        let mesh = unit_cube_tetrahedra(n);
        let mesh_time = start_mesh.elapsed();

        let num_dofs = mesh.num_nodes();
        let num_elements = mesh.num_elements();

        // 2. Assembly
        let start_asm = Instant::now();
        let mut problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k, source);
        let asm_time = start_asm.elapsed();

        // 3. Boundary conditions
        let start_bc = Instant::now();
        let bcs: Vec<DirichletBC> = (1..=6).map(|tag| DirichletBC::new(tag, exact_u)).collect();
        apply_dirichlet(&mut problem, &mesh, &bcs);
        let bc_time = start_bc.elapsed();

        // 4. Convert to CSR and solve
        let matrix = to_csr_matrix(&problem);
        let rhs = Array1::from_vec(problem.rhs.clone());

        let start_solve = Instant::now();
        let solution = gmres(&matrix, &rhs, &config);
        let solve_time = start_solve.elapsed();

        let total_time = mesh_time + asm_time + bc_time + solve_time;

        // Status
        let status = if solution.converged {
            format!("{} iters", solution.iterations)
        } else {
            "FAILED".to_string()
        };

        println!(
            "{:>4} {:>8} {:>10} {:>10.2} {:>10.2} {:>10.2} {:>10.2} {:>10.2} {:>12}",
            n,
            num_dofs,
            num_elements,
            mesh_time.as_secs_f64() * 1000.0,
            asm_time.as_secs_f64() * 1000.0,
            bc_time.as_secs_f64() * 1000.0,
            solve_time.as_secs_f64() * 1000.0,
            total_time.as_secs_f64() * 1000.0,
            status
        );
    }

    println!();
    println!("Notes:");
    println!("  - DOFs = (n+1)³ nodes");
    println!("  - Elements = 6n³ tetrahedra");
    println!("  - Mesh: structured tetrahedral mesh generation");
    println!("  - Asm: Helmholtz matrix assembly (stiffness + mass)");
    println!("  - BC: Dirichlet boundary condition application");
    println!("  - Solve: GMRES iterative solver");
    println!();

    // Thread scaling summary
    println!("=== Thread Scaling Summary ===");
    println!();
    println!("Run with different thread counts to measure parallel speedup:");
    println!("  RAYON_NUM_THREADS=1 cargo run -p math-fem --example scaling_3d --release");
    println!("  RAYON_NUM_THREADS=2 cargo run -p math-fem --example scaling_3d --release");
    println!("  RAYON_NUM_THREADS=4 cargo run -p math-fem --example scaling_3d --release");
    println!("  RAYON_NUM_THREADS=8 cargo run -p math-fem --example scaling_3d --release");
    println!();
}
