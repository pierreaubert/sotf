//! Benchmark: 3D Helmholtz solver scaling
//!
//! Tests how the FEM solver scales with:
//! 1. Mesh size (number of elements/DOFs)
//! 2. Number of threads (parallel assembly and solving)
//!
//! Problem: 3D Helmholtz equation in unit cube [0,1]³
//! -∆u - k²u = f with Dirichlet BCs
//!
//! Run with:
//!   cargo bench -p math-fem --bench helmholtz_3d_scaling
//!
//! For detailed output with thread scaling:
//!   RAYON_NUM_THREADS=1 cargo bench -p math-fem --bench helmholtz_3d_scaling -- --verbose
//!   RAYON_NUM_THREADS=4 cargo bench -p math-fem --bench helmholtz_3d_scaling -- --verbose

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use fem::assembly::HelmholtzProblem;
use fem::basis::PolynomialDegree;
use fem::boundary::{DirichletBC, apply_dirichlet};
use fem::mesh::unit_cube_tetrahedra;
use ndarray::Array1;
use num_complex::Complex64;
use solvers::{CsrMatrix, GmresConfig, gmres};
use std::f64::consts::PI;
use std::time::Duration;

/// Convert HelmholtzMatrix to CsrMatrix for solver
fn to_csr_matrix(problem: &HelmholtzProblem) -> CsrMatrix<Complex64> {
    let compressed = problem.matrix.to_compressed();
    let n = compressed.dim;

    let triplets: Vec<(usize, usize, Complex64)> = (0..compressed.nnz())
        .map(|i| (compressed.rows[i], compressed.cols[i], compressed.values[i]))
        .collect();

    CsrMatrix::from_triplets(n, n, triplets)
}

/// Benchmark mesh generation for 3D unit cube
fn bench_mesh_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("mesh_generation_3d");
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(5));

    // Test different mesh sizes: n³ cells → 6n³ tetrahedra
    for &n in &[4, 6, 8, 10, 12] {
        let num_elements = 6 * n * n * n;
        group.throughput(Throughput::Elements(num_elements as u64));

        group.bench_with_input(BenchmarkId::new("unit_cube", n), &n, |b, &n| {
            b.iter(|| {
                let mesh = unit_cube_tetrahedra(n);
                black_box(mesh)
            });
        });
    }

    group.finish();
}

/// Benchmark FEM assembly (stiffness + mass matrices)
fn bench_assembly(c: &mut Criterion) {
    let mut group = c.benchmark_group("assembly_3d");
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(10));

    let k = Complex64::new(2.0, 0.0);

    // Source term: f = 1
    let source = |_x: f64, _y: f64, _z: f64| Complex64::new(1.0, 0.0);

    for &n in &[4, 6, 8, 10] {
        let mesh = unit_cube_tetrahedra(n);
        let num_dofs = mesh.num_nodes();

        group.throughput(Throughput::Elements(num_dofs as u64));

        group.bench_with_input(BenchmarkId::new("helmholtz", n), &mesh, |b, mesh| {
            b.iter(|| {
                let problem = HelmholtzProblem::assemble(mesh, PolynomialDegree::P1, k, source);
                black_box(problem)
            });
        });
    }

    group.finish();
}

/// Benchmark Dirichlet BC application
fn bench_dirichlet_bc(c: &mut Criterion) {
    let mut group = c.benchmark_group("dirichlet_bc_3d");
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(5));

    let k = Complex64::new(2.0, 0.0);
    let source = |_x: f64, _y: f64, _z: f64| Complex64::new(1.0, 0.0);

    // Exact solution for BC: u = sin(πx) * sin(πy) * sin(πz)
    let exact_u = |x: f64, y: f64, z: f64| {
        Complex64::new((PI * x).sin() * (PI * y).sin() * (PI * z).sin(), 0.0)
    };

    for &n in &[4, 6, 8, 10] {
        let mesh = unit_cube_tetrahedra(n);

        group.bench_with_input(BenchmarkId::new("apply", n), &mesh, |b, mesh| {
            b.iter_batched(
                || {
                    let problem = HelmholtzProblem::assemble(mesh, PolynomialDegree::P1, k, source);
                    problem
                },
                |mut problem| {
                    // Apply Dirichlet on all 6 faces
                    let bcs: Vec<DirichletBC> =
                        (1..=6).map(|tag| DirichletBC::new(tag, exact_u)).collect();
                    apply_dirichlet(&mut problem, mesh, &bcs);
                    black_box(problem)
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

/// Benchmark GMRES solver
fn bench_gmres_solve(c: &mut Criterion) {
    let mut group = c.benchmark_group("gmres_solve_3d");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(20); // Fewer samples for expensive solves

    // Lower k for faster convergence
    // MMS: u = sin(πx) * sin(πy) * sin(πz)
    // f = (3π² - k²) * sin(πx) * sin(πy) * sin(πz)
    let k_val = 1.0;
    let coef = 3.0 * PI * PI - k_val * k_val;
    let source = move |x: f64, y: f64, z: f64| {
        Complex64::new(coef * (PI * x).sin() * (PI * y).sin() * (PI * z).sin(), 0.0)
    };

    let exact_u = |x: f64, y: f64, z: f64| {
        Complex64::new((PI * x).sin() * (PI * y).sin() * (PI * z).sin(), 0.0)
    };

    let config = GmresConfig {
        max_iterations: 500,
        restart: 50,
        tolerance: 1e-8,
        print_interval: 0,
    };

    for &n in &[4, 6, 8] {
        let mesh = unit_cube_tetrahedra(n);
        let num_dofs = mesh.num_nodes();

        // Pre-assemble the problem
        let k = Complex64::new(k_val, 0.0);
        let mut problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k, source);

        let bcs: Vec<DirichletBC> = (1..=6).map(|tag| DirichletBC::new(tag, exact_u)).collect();
        apply_dirichlet(&mut problem, &mesh, &bcs);

        let matrix = to_csr_matrix(&problem);
        let rhs = Array1::from_vec(problem.rhs.clone());

        group.throughput(Throughput::Elements(num_dofs as u64));

        group.bench_with_input(
            BenchmarkId::new("solve", format!("{}^3", n)),
            &(&matrix, &rhs),
            |b, (matrix, rhs)| {
                b.iter(|| {
                    let solution = gmres(*matrix, *rhs, &config);
                    black_box(solution)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark complete solve pipeline (assembly + BC + solve)
fn bench_full_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_pipeline_3d");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(20));
    group.sample_size(10); // Few samples for expensive full solves

    let k = Complex64::new(1.0, 0.0);

    // MMS problem
    let coef = 3.0 * PI * PI - 1.0;
    let source = move |x: f64, y: f64, z: f64| {
        Complex64::new(coef * (PI * x).sin() * (PI * y).sin() * (PI * z).sin(), 0.0)
    };

    let exact_u = |x: f64, y: f64, z: f64| {
        Complex64::new((PI * x).sin() * (PI * y).sin() * (PI * z).sin(), 0.0)
    };

    let config = GmresConfig {
        max_iterations: 500,
        restart: 50,
        tolerance: 1e-8,
        print_interval: 0,
    };

    for &n in &[4, 6, 8] {
        let num_dofs = (n + 1) * (n + 1) * (n + 1);
        group.throughput(Throughput::Elements(num_dofs as u64));

        group.bench_with_input(
            BenchmarkId::new("complete", format!("{}^3", n)),
            &n,
            |b, &n| {
                b.iter(|| {
                    // 1. Generate mesh
                    let mesh = unit_cube_tetrahedra(n);

                    // 2. Assemble
                    let mut problem =
                        HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k, source);

                    // 3. Apply BCs
                    let bcs: Vec<DirichletBC> =
                        (1..=6).map(|tag| DirichletBC::new(tag, exact_u)).collect();
                    apply_dirichlet(&mut problem, &mesh, &bcs);

                    // 4. Convert and solve
                    let matrix = to_csr_matrix(&problem);
                    let rhs = Array1::from_vec(problem.rhs.clone());
                    let solution = gmres(&matrix, &rhs, &config);

                    black_box(solution)
                });
            },
        );
    }

    group.finish();
}

/// Print scaling information (run separately, not as part of criterion)
#[allow(dead_code)]
fn print_scaling_info() {
    use std::time::Instant;

    println!("\n=== 3D Helmholtz Solver Scaling Analysis ===\n");

    let num_threads = rayon::current_num_threads();
    println!("Number of threads: {}", num_threads);
    println!();

    let k = Complex64::new(1.0, 0.0);
    let coef = 3.0 * PI * PI - 1.0;
    let source = move |x: f64, y: f64, z: f64| {
        Complex64::new(coef * (PI * x).sin() * (PI * y).sin() * (PI * z).sin(), 0.0)
    };
    let exact_u = |x: f64, y: f64, z: f64| {
        Complex64::new((PI * x).sin() * (PI * y).sin() * (PI * z).sin(), 0.0)
    };

    let config = GmresConfig {
        max_iterations: 500,
        restart: 50,
        tolerance: 1e-8,
        print_interval: 0,
    };

    println!(
        "{:>6} {:>10} {:>12} {:>12} {:>12} {:>12} {:>10}",
        "n", "DOFs", "Mesh(ms)", "Asm(ms)", "BC(ms)", "Solve(ms)", "Total(ms)"
    );
    println!("{}", "-".repeat(80));

    for n in [4, 6, 8, 10, 12] {
        let start_mesh = Instant::now();
        let mesh = unit_cube_tetrahedra(n);
        let mesh_time = start_mesh.elapsed();

        let num_dofs = mesh.num_nodes();

        let start_asm = Instant::now();
        let mut problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k, source);
        let asm_time = start_asm.elapsed();

        let start_bc = Instant::now();
        let bcs: Vec<DirichletBC> = (1..=6).map(|tag| DirichletBC::new(tag, exact_u)).collect();
        apply_dirichlet(&mut problem, &mesh, &bcs);
        let bc_time = start_bc.elapsed();

        let matrix = to_csr_matrix(&problem);
        let rhs = Array1::from_vec(problem.rhs.clone());

        let start_solve = Instant::now();
        let solution = gmres(&matrix, &rhs, &config);
        let solve_time = start_solve.elapsed();

        let total_time = mesh_time + asm_time + bc_time + solve_time;

        println!(
            "{:>6} {:>10} {:>12.2} {:>12.2} {:>12.2} {:>12.2} {:>10.2}  {}",
            n,
            num_dofs,
            mesh_time.as_secs_f64() * 1000.0,
            asm_time.as_secs_f64() * 1000.0,
            bc_time.as_secs_f64() * 1000.0,
            solve_time.as_secs_f64() * 1000.0,
            total_time.as_secs_f64() * 1000.0,
            if solution.converged {
                format!("({} iters)", solution.iterations)
            } else {
                "FAILED".to_string()
            }
        );
    }

    println!();
}

criterion_group!(
    benches,
    bench_mesh_generation,
    bench_assembly,
    bench_dirichlet_bc,
    bench_gmres_solve,
    bench_full_pipeline,
);

criterion_main!(benches);
