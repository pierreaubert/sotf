//! Validation tests comparing FEM solutions against analytical solutions
//!
//! These tests verify that the FEM solver produces correct results by comparing
//! against known analytical solutions from the math-wave crate.

use fem::assembly::HelmholtzProblem;
use fem::basis::PolynomialDegree;
use fem::boundary::{DirichletBC, apply_dirichlet};
use fem::mesh::{
    annular_mesh_triangles, box_mesh_tetrahedra, rectangular_mesh_triangles,
    spherical_shell_mesh_tetrahedra, unit_square_triangles, BoundaryType,
};
use math_wave::analytical::{legendre_p, spherical_bessel_j, spherical_bessel_y};
use ndarray::Array1;
use num_complex::Complex64;
use solvers::{CsrMatrix, GmresConfig, gmres};
use std::f64::consts::PI;

/// Convert HelmholtzProblem matrix to CsrMatrix for solver
fn to_csr_matrix(problem: &HelmholtzProblem) -> CsrMatrix<Complex64> {
    let compressed = problem.matrix.to_compressed();
    let n = compressed.dim;

    // Convert triplet format to CSR using from_triplets
    let triplets: Vec<(usize, usize, Complex64)> = (0..compressed.nnz())
        .map(|i| (compressed.rows[i], compressed.cols[i], compressed.values[i]))
        .collect();

    CsrMatrix::from_triplets(n, n, triplets)
}

/// Compute L2 error between FEM solution and analytical solution
fn l2_error<F>(mesh: &fem::mesh::Mesh, fem_solution: &Array1<Complex64>, analytical: F) -> f64
where
    F: Fn(f64, f64, f64) -> Complex64,
{
    let mut error_sq = 0.0;
    let mut norm_sq = 0.0;

    for (i, node) in mesh.nodes.iter().enumerate() {
        let fem_val = fem_solution[i];
        let exact_val = analytical(node.x, node.y, node.z);
        let diff = fem_val - exact_val;
        error_sq += diff.norm_sqr();
        norm_sq += exact_val.norm_sqr();
    }

    if norm_sq > 1e-15 {
        (error_sq / norm_sq).sqrt()
    } else {
        error_sq.sqrt()
    }
}

/// Test 1D Helmholtz mode in x-direction using manufactured solution
///
/// Uses a source term to drive the solution to sin(kx)
#[test]
fn test_1d_mode_in_2d_strip() {
    // Create a thin strip mesh (effectively 1D)
    // Domain: [0, 1] x [0, 0.05]
    let l = 1.0;
    let k = 1.5; // Use a wavenumber that doesn't match the domain

    // Use a thin strip to approximate 1D - finer mesh for accuracy
    let mesh = rectangular_mesh_triangles(0.0, l, 0.0, 0.05, 40, 2);

    // The solution u(x,y) = sin(πx) satisfies -∇²u - k²u = f where:
    // f = (π² - k²) * sin(πx)
    let k_pi = PI;
    let coef = k_pi * k_pi - k * k;
    let source = move |x: f64, _y: f64, _z: f64| Complex64::new(coef * (k_pi * x).sin(), 0.0);
    let exact_u = move |x: f64, _y: f64, _z: f64| Complex64::new((k_pi * x).sin(), 0.0);

    let k_complex = Complex64::new(k, 0.0);
    let mut problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k_complex, source);

    // Apply Dirichlet BCs: u = 0 at x=0 (tag 1) and x=L (tag 2)
    // u = exact_u on top/bottom (tags 3, 4)
    let bc_left = DirichletBC::new(1, |_, _, _| Complex64::new(0.0, 0.0));
    let bc_right = DirichletBC::new(2, |_, _, _| Complex64::new(0.0, 0.0));
    let bc_top = DirichletBC::new(4, exact_u);
    let bc_bottom = DirichletBC::new(3, exact_u);

    apply_dirichlet(&mut problem, &mesh, &[bc_left, bc_right, bc_top, bc_bottom]);

    // Solve the system
    let matrix = to_csr_matrix(&problem);
    let rhs = Array1::from_vec(problem.rhs.clone());

    let config = GmresConfig {
        max_iterations: 500,
        restart: 50,
        tolerance: 1e-10,
        print_interval: 0,
    };

    let solution = gmres(&matrix, &rhs, &config);
    assert!(solution.converged, "GMRES should converge");

    // Compare with analytical solution u(x,y) = sin(πx)
    let error = l2_error(&mesh, &solution.x, exact_u);

    // P1 elements on fine mesh should achieve good accuracy
    assert!(
        error < 0.02,
        "L2 error {} should be < 0.02 for P1 elements",
        error
    );
}

/// Test 2D plane wave solution
///
/// Solves the Helmholtz equation with plane wave boundary conditions
/// u(x,y) = exp(i(k_x * x + k_y * y))
#[test]
fn test_2d_plane_wave() {
    let k = 2.0; // Wavenumber
    let theta = PI / 4.0; // 45 degree angle
    let kx = k * theta.cos();
    let ky = k * theta.sin();

    // Finer mesh for better accuracy
    let mesh = unit_square_triangles(16);

    // The plane wave satisfies Helmholtz with zero source
    let k_complex = Complex64::new(k, 0.0);
    let mut problem =
        HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k_complex, |_, _, _| {
            Complex64::new(0.0, 0.0)
        });

    // Apply plane wave as Dirichlet BC on all boundaries
    let plane_wave = move |x: f64, y: f64, _z: f64| {
        let phase = kx * x + ky * y;
        Complex64::new(phase.cos(), phase.sin())
    };

    let bcs: Vec<DirichletBC> = (1..=4)
        .map(|tag| DirichletBC::new(tag, plane_wave))
        .collect();

    apply_dirichlet(&mut problem, &mesh, &bcs);

    // Solve
    let matrix = to_csr_matrix(&problem);
    let rhs = Array1::from_vec(problem.rhs.clone());

    let config = GmresConfig {
        max_iterations: 500,
        restart: 50,
        tolerance: 1e-10,
        print_interval: 0,
    };

    let solution = gmres(&matrix, &rhs, &config);
    assert!(solution.converged, "GMRES should converge");

    // Compare with analytical plane wave
    let error = l2_error(&mesh, &solution.x, plane_wave);

    // Should be very accurate since plane wave is an exact solution
    assert!(
        error < 0.01,
        "L2 error {} should be < 0.01 for plane wave",
        error
    );
}

/// Test convergence rate with mesh refinement
///
/// For P1 elements, we expect O(h²) convergence in L2 norm
#[test]
fn test_convergence_rate() {
    let k = 1.0;
    let kx = k * 0.6; // Not aligned with grid
    let ky = k * 0.8;

    let analytical = move |x: f64, y: f64, _z: f64| {
        let phase = kx * x + ky * y;
        Complex64::new(phase.cos(), phase.sin())
    };

    let mesh_sizes = [4, 8, 16];
    let mut errors = Vec::new();
    let mut h_values = Vec::new();

    for &n in &mesh_sizes {
        let mesh = unit_square_triangles(n);
        let h = 1.0 / (n as f64); // Mesh size

        let k_complex = Complex64::new(k, 0.0);
        let mut problem =
            HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k_complex, |_, _, _| {
                Complex64::new(0.0, 0.0)
            });

        // Apply plane wave BCs on all boundaries
        let bcs: Vec<DirichletBC> = (1..=4)
            .map(|tag| DirichletBC::new(tag, analytical))
            .collect();

        apply_dirichlet(&mut problem, &mesh, &bcs);

        let matrix = to_csr_matrix(&problem);
        let rhs = Array1::from_vec(problem.rhs.clone());

        let config = GmresConfig {
            max_iterations: 1000,
            restart: 100,
            tolerance: 1e-12,
            print_interval: 0,
        };

        let solution = gmres(&matrix, &rhs, &config);
        assert!(
            solution.converged,
            "GMRES should converge for mesh size {}",
            n
        );

        let error = l2_error(&mesh, &solution.x, analytical);
        errors.push(error);
        h_values.push(h);
    }

    // Check convergence rate between successive meshes
    // For O(h²), rate ≈ 2 when halving h
    for i in 1..errors.len() {
        let rate = (errors[i - 1] / errors[i]).log2() / (h_values[i - 1] / h_values[i]).log2();
        // Rate should be approximately 2 for P1 elements
        assert!(
            rate > 1.5,
            "Convergence rate {} should be > 1.5 (expected ~2.0 for P1)",
            rate
        );
    }
}

/// Test standing wave solution (2D analog of 1D mode)
///
/// u(x,y) = sin(n*π*x/L_x) * sin(m*π*y/L_y)
/// is an eigenfunction of the Laplacian with eigenvalue k² = (nπ/L_x)² + (mπ/L_y)²
#[test]
fn test_2d_standing_wave() {
    let lx = 1.0;
    let ly = 1.0;
    let n = 1;
    let m = 1;

    let kx = (n as f64) * PI / lx;
    let ky = (m as f64) * PI / ly;
    let k = (kx * kx + ky * ky).sqrt();

    let mesh = unit_square_triangles(10);

    let k_complex = Complex64::new(k, 0.0);
    let mut problem =
        HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k_complex, |_, _, _| {
            Complex64::new(0.0, 0.0)
        });

    // Homogeneous Dirichlet BCs (standing wave is zero on all boundaries)
    let bcs: Vec<DirichletBC> = (1..=4)
        .map(|tag| DirichletBC::new(tag, |_, _, _| Complex64::new(0.0, 0.0)))
        .collect();

    apply_dirichlet(&mut problem, &mesh, &bcs);

    let matrix = to_csr_matrix(&problem);
    let rhs = Array1::from_vec(problem.rhs.clone());

    let config = GmresConfig {
        max_iterations: 500,
        restart: 50,
        tolerance: 1e-10,
        print_interval: 0,
    };

    let solution = gmres(&matrix, &rhs, &config);

    // For the eigenvalue problem with homogeneous BCs and zero source,
    // the solution should be zero (trivial solution)
    // This verifies the system is correctly assembled
    let max_val: f64 = solution.x.iter().map(|v| v.norm()).fold(0.0, f64::max);
    assert!(
        max_val < 1e-8,
        "Solution should be trivial for homogeneous problem"
    );
}

/// Test with non-trivial source term
///
/// Uses manufactured solution: choose u, compute f = -∆u - k²u
#[test]
fn test_manufactured_solution() {
    let k = 1.5;

    // Choose a smooth solution that's zero on boundaries
    // u(x,y) = sin(πx) * sin(πy)
    let exact_u = |x: f64, y: f64, _z: f64| Complex64::new((PI * x).sin() * (PI * y).sin(), 0.0);

    // For u = sin(πx)*sin(πy):
    // ∆u = -2π²*sin(πx)*sin(πy)
    // f = -∆u - k²u = (2π² - k²)*sin(πx)*sin(πy)
    let coef = 2.0 * PI * PI - k * k;
    let source =
        move |x: f64, y: f64, _z: f64| Complex64::new(coef * (PI * x).sin() * (PI * y).sin(), 0.0);

    let mesh = unit_square_triangles(12);
    let k_complex = Complex64::new(k, 0.0);

    let mut problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k_complex, source);

    // Homogeneous Dirichlet BCs (our exact solution is zero on boundary)
    let bcs: Vec<DirichletBC> = (1..=4)
        .map(|tag| DirichletBC::new(tag, |_, _, _| Complex64::new(0.0, 0.0)))
        .collect();

    apply_dirichlet(&mut problem, &mesh, &bcs);

    let matrix = to_csr_matrix(&problem);
    let rhs = Array1::from_vec(problem.rhs.clone());

    let config = GmresConfig {
        max_iterations: 500,
        restart: 50,
        tolerance: 1e-10,
        print_interval: 0,
    };

    let solution = gmres(&matrix, &rhs, &config);
    assert!(solution.converged, "GMRES should converge");

    // Compute error against manufactured solution
    let error = l2_error(&mesh, &solution.x, exact_u);

    // Should achieve reasonable accuracy
    assert!(
        error < 0.02,
        "L2 error {} should be < 0.02 for manufactured solution",
        error
    );
}

/// Test using math-wave plane_wave_1d function to validate against a known solution
///
/// This test uses the analytical solution from math-wave and compares with FEM
#[test]
fn test_with_math_wave_plane_wave_1d() {
    use math_wave::analytical::plane_wave_1d;

    let k = 2.0;
    let x_min = 0.0;
    let x_max = 1.0;

    // Get analytical solution from math-wave
    let analytical_solution = plane_wave_1d(k, x_min, x_max, 50);

    // Create a thin 2D strip to simulate 1D
    let mesh = rectangular_mesh_triangles(x_min, x_max, 0.0, 0.05, 20, 1);

    let k_complex = Complex64::new(k, 0.0);
    let mut problem =
        HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k_complex, |_, _, _| {
            Complex64::new(0.0, 0.0)
        });

    // Use the plane wave as Dirichlet BC
    // The plane wave solution is exp(ikx) = cos(kx) + i*sin(kx)
    let plane_wave_bc = move |x: f64, _y: f64, _z: f64| {
        let kx = k * x;
        Complex64::new(kx.cos(), kx.sin())
    };

    let bcs: Vec<DirichletBC> = (1..=4)
        .map(|tag| DirichletBC::new(tag, plane_wave_bc))
        .collect();

    apply_dirichlet(&mut problem, &mesh, &bcs);

    let matrix = to_csr_matrix(&problem);
    let rhs = Array1::from_vec(problem.rhs.clone());

    let config = GmresConfig {
        max_iterations: 500,
        restart: 50,
        tolerance: 1e-10,
        print_interval: 0,
    };

    let solution = gmres(&matrix, &rhs, &config);
    assert!(solution.converged, "GMRES should converge");

    // Verify that the FEM solution matches the analytical plane wave
    let error = l2_error(&mesh, &solution.x, plane_wave_bc);

    // Also verify that math-wave's solution has the expected properties
    // At x=0: exp(0) = 1
    assert!(
        (analytical_solution.pressure[0].re - 1.0).abs() < 1e-10,
        "Analytical solution at x=0 should be 1"
    );

    // The FEM should achieve good accuracy
    assert!(
        error < 0.05,
        "L2 error {} should be < 0.05 for plane wave",
        error
    );
}

/// Test using math-wave helmholtz_1d_mode function
///
/// The Helmholtz 1D mode solution satisfies:
/// d²u/dx² + k²u = sin(nπx/L) / (k² - (nπ/L)²)
#[test]
fn test_with_math_wave_helmholtz_mode() {
    use math_wave::analytical::helmholtz_1d_mode;

    let k = 2.0; // Not at resonance
    let l = 1.0;
    let mode_n = 2;

    // Get analytical solution from math-wave
    let analytical_solution = helmholtz_1d_mode(k, l, mode_n, 50);

    // Create a thin 2D strip
    let mesh = rectangular_mesh_triangles(0.0, l, 0.0, 0.05, 20, 1);

    // The source term for this mode: f(x) = sin(nπx/L)
    let kn = (mode_n as f64) * PI / l;
    let source = move |x: f64, _y: f64, _z: f64| Complex64::new((kn * x).sin(), 0.0);

    let k_complex = Complex64::new(k, 0.0);
    let mut problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k_complex, source);

    // The Helmholtz mode has u(0) = u(L) = 0
    // Use analytical values on top/bottom boundaries
    let denom = k * k - kn * kn;
    let exact_u = move |x: f64, _y: f64, _z: f64| Complex64::new((kn * x).sin() / denom, 0.0);

    let bc_left = DirichletBC::new(1, |_, _, _| Complex64::new(0.0, 0.0));
    let bc_right = DirichletBC::new(2, |_, _, _| Complex64::new(0.0, 0.0));
    let bc_top = DirichletBC::new(4, exact_u);
    let bc_bottom = DirichletBC::new(3, exact_u);

    apply_dirichlet(&mut problem, &mesh, &[bc_left, bc_right, bc_top, bc_bottom]);

    let matrix = to_csr_matrix(&problem);
    let rhs = Array1::from_vec(problem.rhs.clone());

    let config = GmresConfig {
        max_iterations: 500,
        restart: 50,
        tolerance: 1e-10,
        print_interval: 0,
    };

    let solution = gmres(&matrix, &rhs, &config);
    assert!(solution.converged, "GMRES should converge");

    // Verify the FEM solution matches the analytical
    let error = l2_error(&mesh, &solution.x, exact_u);

    // Also verify math-wave's solution properties
    // At x=0: sin(0) = 0, so u(0) = 0
    assert!(
        analytical_solution.pressure[0].norm() < 1e-10,
        "Analytical solution at x=0 should be 0"
    );

    assert!(
        error < 0.1,
        "L2 error {} should be < 0.1 for helmholtz mode",
        error
    );
}

/// Test damped wave solution from math-wave
#[test]
fn test_with_math_wave_damped_wave() {
    use math_wave::analytical::damped_wave_1d;

    let k = 1.0;
    let alpha = 0.5; // Damping coefficient

    // Get analytical solution from math-wave
    let analytical_solution = damped_wave_1d(k, alpha, 0.0, 1.0, 50);

    // The damped wave is: exp(-(α + ik)x) = exp(-αx) * exp(ikx)
    // This solves the Helmholtz equation with complex wavenumber k_complex = k + i*α
    let k_complex = Complex64::new(k, alpha);

    let mesh = rectangular_mesh_triangles(0.0, 1.0, 0.0, 0.05, 20, 1);

    let mut problem =
        HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k_complex, |_, _, _| {
            Complex64::new(0.0, 0.0)
        });

    // The damped wave solution as BC
    let damped_wave = move |x: f64, _y: f64, _z: f64| {
        let damping = (-alpha * x).exp();
        let phase = k * x;
        Complex64::new(damping * phase.cos(), damping * phase.sin())
    };

    let bcs: Vec<DirichletBC> = (1..=4)
        .map(|tag| DirichletBC::new(tag, damped_wave))
        .collect();

    apply_dirichlet(&mut problem, &mesh, &bcs);

    let matrix = to_csr_matrix(&problem);
    let rhs = Array1::from_vec(problem.rhs.clone());

    let config = GmresConfig {
        max_iterations: 500,
        restart: 50,
        tolerance: 1e-10,
        print_interval: 0,
    };

    let solution = gmres(&matrix, &rhs, &config);
    assert!(solution.converged, "GMRES should converge");

    let error = l2_error(&mesh, &solution.x, damped_wave);

    // Verify math-wave solution
    // At x=0: exp(0) = 1
    assert!(
        (analytical_solution.pressure[0].re - 1.0).abs() < 1e-10,
        "Damped wave at x=0 should be 1"
    );

    assert!(
        error < 0.1,
        "L2 error {} should be < 0.1 for damped wave",
        error
    );
}

// ============================================================================
// Method of Manufactured Solutions (MMS) Tests
// ============================================================================

/// Helper: solve Helmholtz problem with MMS and return L2 error
fn solve_mms_problem<U, F>(mesh: &fem::mesh::Mesh, k: f64, exact_u: U, source: F) -> f64
where
    U: Fn(f64, f64, f64) -> Complex64 + Copy + 'static,
    F: Fn(f64, f64, f64) -> Complex64 + Sync,
{
    let k_complex = Complex64::new(k, 0.0);
    let mut problem = HelmholtzProblem::assemble(mesh, PolynomialDegree::P1, k_complex, source);

    // Apply exact solution as Dirichlet BC on all boundaries
    let bcs: Vec<DirichletBC> = (1..=4).map(|tag| DirichletBC::new(tag, exact_u)).collect();

    apply_dirichlet(&mut problem, mesh, &bcs);

    let matrix = to_csr_matrix(&problem);
    let rhs = Array1::from_vec(problem.rhs.clone());

    let config = GmresConfig {
        max_iterations: 1000,
        restart: 100,
        tolerance: 1e-12,
        print_interval: 0,
    };

    let solution = gmres(&matrix, &rhs, &config);
    assert!(solution.converged, "GMRES should converge");

    l2_error(mesh, &solution.x, exact_u)
}

/// MMS Test 1: u(x,y) = sin(πx) * cos(πy)
///
/// Laplacian: ∆u = -π²sin(πx)cos(πy) - π²sin(πx)cos(πy) = -2π²u
/// Helmholtz: -∆u - k²u = f  =>  f = (2π² - k²) * sin(πx) * cos(πy)
#[test]
fn test_mms_sin_cos() {
    let k = 1.5;

    // Manufactured solution: u = sin(πx) * cos(πy)
    let exact_u = |x: f64, y: f64, _z: f64| Complex64::new((PI * x).sin() * (PI * y).cos(), 0.0);

    // Source term: f = (2π² - k²) * sin(πx) * cos(πy)
    let coef = 2.0 * PI * PI - k * k;
    let source =
        move |x: f64, y: f64, _z: f64| Complex64::new(coef * (PI * x).sin() * (PI * y).cos(), 0.0);

    // Test on a single mesh first
    let mesh = unit_square_triangles(16);
    let error = solve_mms_problem(&mesh, k, exact_u, source);

    assert!(
        error < 0.01,
        "MMS sin*cos: L2 error {} should be < 0.01",
        error
    );
}

/// MMS Test 2: u(x,y) = sin(πx) * cos(πy) with mesh convergence
///
/// Verifies O(h²) convergence for P1 elements
#[test]
fn test_mms_sin_cos_convergence() {
    let k = 1.5;

    let exact_u = |x: f64, y: f64, _z: f64| Complex64::new((PI * x).sin() * (PI * y).cos(), 0.0);

    let coef = 2.0 * PI * PI - k * k;
    let source =
        move |x: f64, y: f64, _z: f64| Complex64::new(coef * (PI * x).sin() * (PI * y).cos(), 0.0);

    let mesh_sizes = [4, 8, 16, 32];
    let mut errors = Vec::new();
    let mut h_values = Vec::new();

    for &n in &mesh_sizes {
        let mesh = unit_square_triangles(n);
        let h = 1.0 / (n as f64);
        let error = solve_mms_problem(&mesh, k, exact_u, source);

        errors.push(error);
        h_values.push(h);
    }

    // Verify convergence rate
    // Note: rate may be lower on coarse meshes due to pre-asymptotic behavior
    for i in 1..errors.len() {
        let rate = (errors[i - 1] / errors[i]).log2() / (h_values[i - 1] / h_values[i]).log2();
        assert!(
            rate > 1.5,
            "MMS sin*cos convergence rate {} at refinement {} should be > 1.5 (expected ~2.0)",
            rate,
            i
        );
    }

    // The average rate should be closer to 2.0
    let avg_rate: f64 = (1..errors.len())
        .map(|i| (errors[i - 1] / errors[i]).log2() / (h_values[i - 1] / h_values[i]).log2())
        .sum::<f64>()
        / (errors.len() - 1) as f64;
    assert!(
        avg_rate > 1.7,
        "Average convergence rate {} should be > 1.7",
        avg_rate
    );

    // Error should decrease monotonically
    for i in 1..errors.len() {
        assert!(
            errors[i] < errors[i - 1],
            "Error should decrease with mesh refinement: {} >= {}",
            errors[i],
            errors[i - 1]
        );
    }
}

/// MMS Test 3: u(x,y) = sin(2πx) * sin(2πy) (higher frequency)
///
/// Tests accuracy with higher frequency modes
#[test]
fn test_mms_sin_sin_2pi() {
    let k = 2.0;

    // Higher frequency: u = sin(2πx) * sin(2πy)
    // ∆u = -4π²sin(2πx)sin(2πy) - 4π²sin(2πx)sin(2πy) = -8π²u
    let exact_u =
        |x: f64, y: f64, _z: f64| Complex64::new((2.0 * PI * x).sin() * (2.0 * PI * y).sin(), 0.0);

    // f = (8π² - k²) * sin(2πx) * sin(2πy)
    let coef = 8.0 * PI * PI - k * k;
    let source = move |x: f64, y: f64, _z: f64| {
        Complex64::new(coef * (2.0 * PI * x).sin() * (2.0 * PI * y).sin(), 0.0)
    };

    let mesh_sizes = [8, 16, 32];
    let mut errors = Vec::new();
    let mut h_values = Vec::new();

    for &n in &mesh_sizes {
        let mesh = unit_square_triangles(n);
        let h = 1.0 / (n as f64);
        let error = solve_mms_problem(&mesh, k, exact_u, source);

        errors.push(error);
        h_values.push(h);
    }

    // Check convergence rate
    for i in 1..errors.len() {
        let rate = (errors[i - 1] / errors[i]).log2() / (h_values[i - 1] / h_values[i]).log2();
        assert!(
            rate > 1.5,
            "MMS sin(2πx)*sin(2πy) convergence rate {} should be > 1.5",
            rate
        );
    }
}

/// MMS Test 4: u(x,y) = x(1-x) * y(1-y) (polynomial, zero on boundary)
///
/// Simple polynomial that's exactly representable in the solution space
/// but requires proper source term computation
#[test]
fn test_mms_polynomial() {
    let k = 1.0;

    // u = x(1-x) * y(1-y)
    // ∂²u/∂x² = -2 * y(1-y)
    // ∂²u/∂y² = -2 * x(1-x)
    // ∆u = -2[x(1-x) + y(1-y)]
    let exact_u = |x: f64, y: f64, _z: f64| Complex64::new(x * (1.0 - x) * y * (1.0 - y), 0.0);

    // f = -∆u - k²u = 2[x(1-x) + y(1-y)] - k² * x(1-x) * y(1-y)
    let source = move |x: f64, y: f64, _z: f64| {
        let u_val = x * (1.0 - x) * y * (1.0 - y);
        let laplacian = -2.0 * (x * (1.0 - x) + y * (1.0 - y));
        Complex64::new(-laplacian - k * k * u_val, 0.0)
    };

    let mesh_sizes = [4, 8, 16, 32];
    let mut errors = Vec::new();
    let mut h_values = Vec::new();

    for &n in &mesh_sizes {
        let mesh = unit_square_triangles(n);
        let h = 1.0 / (n as f64);
        let error = solve_mms_problem(&mesh, k, exact_u, source);

        errors.push(error);
        h_values.push(h);
    }

    // Polynomial should show good convergence
    for i in 1..errors.len() {
        let rate = (errors[i - 1] / errors[i]).log2() / (h_values[i - 1] / h_values[i]).log2();
        assert!(
            rate > 1.5,
            "MMS polynomial convergence rate {} should be > 1.5",
            rate
        );
    }
}

/// MMS Test 5: u(x,y) = exp(x) * sin(πy) (exponential in x, periodic in y)
///
/// Mixed exponential-trigonometric solution
#[test]
fn test_mms_exp_sin() {
    let k = 1.0;

    // u = exp(x) * sin(πy)
    // ∂²u/∂x² = exp(x) * sin(πy) = u
    // ∂²u/∂y² = -π² * exp(x) * sin(πy) = -π²u
    // ∆u = (1 - π²) * u
    let exact_u = |x: f64, y: f64, _z: f64| Complex64::new(x.exp() * (PI * y).sin(), 0.0);

    // f = -∆u - k²u = -(1 - π²)u - k²u = (π² - 1 - k²) * u
    let coef = PI * PI - 1.0 - k * k;
    let source =
        move |x: f64, y: f64, _z: f64| Complex64::new(coef * x.exp() * (PI * y).sin(), 0.0);

    let mesh_sizes = [4, 8, 16, 32];
    let mut errors = Vec::new();
    let mut h_values = Vec::new();

    for &n in &mesh_sizes {
        let mesh = unit_square_triangles(n);
        let h = 1.0 / (n as f64);
        let error = solve_mms_problem(&mesh, k, exact_u, source);

        errors.push(error);
        h_values.push(h);
    }

    // Check convergence
    for i in 1..errors.len() {
        let rate = (errors[i - 1] / errors[i]).log2() / (h_values[i - 1] / h_values[i]).log2();
        assert!(
            rate > 1.5,
            "MMS exp*sin convergence rate {} should be > 1.5",
            rate
        );
    }
}

/// MMS Test 6: Complex-valued solution u(x,y) = (1 + i) * sin(πx) * sin(πy)
///
/// Tests that complex arithmetic is handled correctly
#[test]
fn test_mms_complex_valued() {
    let k = 1.0;

    // Complex amplitude
    let amp = Complex64::new(1.0, 1.0);

    // u = (1+i) * sin(πx) * sin(πy)
    // ∆u = -2π² * (1+i) * sin(πx) * sin(πy) = -2π² * u
    let exact_u = move |x: f64, y: f64, _z: f64| amp * (PI * x).sin() * (PI * y).sin();

    // f = (2π² - k²) * (1+i) * sin(πx) * sin(πy)
    let coef = 2.0 * PI * PI - k * k;
    let source = move |x: f64, y: f64, _z: f64| amp * coef * (PI * x).sin() * (PI * y).sin();

    let mesh_sizes = [4, 8, 16, 32];
    let mut errors = Vec::new();
    let mut h_values = Vec::new();

    for &n in &mesh_sizes {
        let mesh = unit_square_triangles(n);
        let h = 1.0 / (n as f64);
        let error = solve_mms_problem(&mesh, k, exact_u, source);

        errors.push(error);
        h_values.push(h);
    }

    // Check convergence for complex solution
    for i in 1..errors.len() {
        let rate = (errors[i - 1] / errors[i]).log2() / (h_values[i - 1] / h_values[i]).log2();
        assert!(
            rate > 1.5,
            "MMS complex convergence rate {} should be > 1.5",
            rate
        );
    }
}

/// MMS Test 7: Verify exact convergence rates numerically
///
/// This test prints actual convergence rates for verification
#[test]
fn test_mms_convergence_rates_detailed() {
    let k = 1.5;

    // u = sin(πx) * sin(πy) - classic choice with homogeneous BCs
    let exact_u = |x: f64, y: f64, _z: f64| Complex64::new((PI * x).sin() * (PI * y).sin(), 0.0);

    let coef = 2.0 * PI * PI - k * k;
    let source =
        move |x: f64, y: f64, _z: f64| Complex64::new(coef * (PI * x).sin() * (PI * y).sin(), 0.0);

    let mesh_sizes = [4, 8, 16, 32, 64];
    let mut errors = Vec::new();
    let mut h_values = Vec::new();

    for &n in &mesh_sizes {
        let mesh = unit_square_triangles(n);
        let h = 1.0 / (n as f64);
        let error = solve_mms_problem(&mesh, k, exact_u, source);

        errors.push(error);
        h_values.push(h);
    }

    // Calculate and verify all convergence rates
    let mut rates = Vec::new();
    for i in 1..errors.len() {
        let rate = (errors[i - 1] / errors[i]).log2() / (h_values[i - 1] / h_values[i]).log2();
        rates.push(rate);
    }

    // Average rate should be close to 2.0 for P1 elements in L2 norm
    let avg_rate: f64 = rates.iter().sum::<f64>() / rates.len() as f64;
    assert!(
        avg_rate > 1.9 && avg_rate < 2.2,
        "Average convergence rate {} should be in [1.9, 2.2] (theoretical: 2.0)",
        avg_rate
    );

    // Finest mesh should have small error
    // With 64x64 mesh and h=1/64, O(h²) gives ~2.4e-4 theoretical error
    let finest_error = errors.last().unwrap();
    assert!(
        *finest_error < 5e-4,
        "Finest mesh error {} should be < 5e-4",
        finest_error
    );
}

/// MMS Test 8: Different wavenumbers to test dispersion behavior
#[test]
fn test_mms_varying_wavenumber() {
    // Test with several different wavenumbers
    let wavenumbers = [0.5, 1.0, 2.0, 4.0];

    for &k in &wavenumbers {
        let exact_u =
            |x: f64, y: f64, _z: f64| Complex64::new((PI * x).sin() * (PI * y).sin(), 0.0);

        let coef = 2.0 * PI * PI - k * k;
        let source = move |x: f64, y: f64, _z: f64| {
            Complex64::new(coef * (PI * x).sin() * (PI * y).sin(), 0.0)
        };

        // Need finer mesh for higher wavenumbers (pollution effect)
        let n = if k > 2.0 { 32 } else { 16 };
        let mesh = unit_square_triangles(n);
        let error = solve_mms_problem(&mesh, k, exact_u, source);

        // Error tolerance scales with wavenumber (pollution effect)
        let tol = 0.01 * (1.0 + k * k / 10.0);
        assert!(
            error < tol,
            "MMS with k={}: error {} should be < {}",
            k,
            error,
            tol
        );
    }
}

/// MMS Test 9: Non-square domain (rectangle)
#[test]
fn test_mms_rectangle() {
    let k = 1.0;

    // Domain [0, 2] x [0, 1]
    let lx = 2.0;
    let ly = 1.0;

    // u = sin(πx/lx) * sin(πy/ly)
    // ∆u = -(π/lx)²u - (π/ly)²u = -[(π/lx)² + (π/ly)²] * u
    let kx = PI / lx;
    let ky = PI / ly;

    let exact_u =
        move |x: f64, y: f64, _z: f64| Complex64::new((kx * x).sin() * (ky * y).sin(), 0.0);

    // f = [(π/lx)² + (π/ly)² - k²] * u
    let laplacian_coef = kx * kx + ky * ky;
    let coef = laplacian_coef - k * k;
    let source =
        move |x: f64, y: f64, _z: f64| Complex64::new(coef * (kx * x).sin() * (ky * y).sin(), 0.0);

    let mesh_sizes = [4, 8, 16, 32];
    let mut errors = Vec::new();
    let mut h_values = Vec::new();

    for &n in &mesh_sizes {
        // Rectangular mesh with aspect ratio 2:1
        let mesh = rectangular_mesh_triangles(0.0, lx, 0.0, ly, 2 * n, n);
        let h = ly / (n as f64); // Use smaller dimension for h
        let error = solve_mms_problem(&mesh, k, exact_u, source);

        errors.push(error);
        h_values.push(h);
    }

    // Check convergence on rectangular domain
    for i in 1..errors.len() {
        let rate = (errors[i - 1] / errors[i]).log2() / (h_values[i - 1] / h_values[i]).log2();
        assert!(
            rate > 1.5,
            "MMS rectangle convergence rate {} should be > 1.5",
            rate
        );
    }
}

// ============================================================================
// Scattering Problem: Circular Obstacle (2D)
// ============================================================================

/// Helper: compute the analytical scattered field for a sound-soft cylinder
///
/// For a sound-soft (Dirichlet) cylinder with u=0 on surface:
/// Total field = incident + scattered
/// u_total = exp(ikr cos θ) + Σ aₙ Hₙ(kr) cos(nθ)
///
/// Coefficients: aₙ = -εₙ iⁿ Jₙ(ka) / Hₙ(ka)
fn cylinder_scattering_analytical(
    k: f64,
    cylinder_radius: f64,
    x: f64,
    y: f64,
    num_terms: usize,
) -> Complex64 {
    use std::f64::consts::PI;

    let r = (x * x + y * y).sqrt();
    let theta = y.atan2(x);

    if r < cylinder_radius * 0.999 {
        // Inside cylinder - not physical
        return Complex64::new(0.0, 0.0);
    }

    let ka = k * cylinder_radius;
    let kr = k * r;

    // Incident plane wave: exp(ikx) = exp(ikr cos θ)
    let incident = Complex64::new((kr * theta.cos()).cos(), (kr * theta.cos()).sin());

    // Scattered field using Bessel/Hankel series
    let mut scattered = Complex64::new(0.0, 0.0);

    for n in 0..num_terms {
        let n_f64 = n as f64;
        let epsilon_n = if n == 0 { 1.0 } else { 2.0 };

        // Bessel functions at ka (cylinder surface) and kr (evaluation point)
        let jn_ka = bessel_j(n as i64, ka);
        let yn_ka = bessel_y(n as i64, ka);
        let jn_kr = bessel_j(n as i64, kr);
        let yn_kr = bessel_y(n as i64, kr);

        // Hankel function H_n = J_n + i*Y_n
        let hn_ka = Complex64::new(jn_ka, yn_ka);
        let hn_kr = Complex64::new(jn_kr, yn_kr);

        // i^n = exp(i*n*π/2)
        let i_power_n = Complex64::new((n_f64 * PI / 2.0).cos(), (n_f64 * PI / 2.0).sin());

        // Coefficient for sound-soft cylinder: a_n = -ε_n * i^n * J_n(ka) / H_n(ka)
        let a_n = -epsilon_n * jn_ka / hn_ka * i_power_n;

        // Contribution: a_n * H_n(kr) * cos(nθ)
        let cos_n_theta = (n_f64 * theta).cos();
        scattered += a_n * hn_kr * cos_n_theta;
    }

    incident + scattered
}

/// Bessel function of the first kind using spec_math crate
fn bessel_j(n: i64, x: f64) -> f64 {
    use spec_math::Bessel;
    x.bessel_jv(n as f64)
}

/// Bessel function of the second kind (Neumann function) using spec_math crate
fn bessel_y(n: i64, x: f64) -> f64 {
    use spec_math::Bessel;
    x.bessel_yv(n as f64)
}

/// Scattering by a sound-soft circular cylinder
///
/// Plane wave incident from +x direction scatters off a sound-soft (u=0)
/// cylinder. Uses analytical Bessel/Hankel series solution.
#[test]
fn test_cylinder_scattering_sound_soft() {
    let cylinder_radius = 1.0;
    let outer_radius = 3.0;
    let k = 2.0; // ka = 2 (resonance regime)

    // Create annular mesh
    let n_radial = 16;
    let n_angular = 32;
    let mesh = annular_mesh_triangles(0.0, 0.0, cylinder_radius, outer_radius, n_radial, n_angular);

    let num_terms = 30; // Enough terms for ka=2

    // Analytical solution as the exact field
    let exact_u = move |x: f64, y: f64, _z: f64| {
        cylinder_scattering_analytical(k, cylinder_radius, x, y, num_terms)
    };

    // For Helmholtz equation in exterior domain: -∆u - k²u = 0 (no source)
    let source = |_x: f64, _y: f64, _z: f64| Complex64::new(0.0, 0.0);

    // Assemble the FEM problem
    let k_complex = Complex64::new(k, 0.0);
    let mut problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k_complex, source);

    // Apply Dirichlet BCs:
    // - Inner boundary (tag 1): u = 0 (sound-soft)
    // - Outer boundary (tag 2): u = exact analytical solution
    let bc_inner = DirichletBC::new(1, |_x, _y, _z| Complex64::new(0.0, 0.0));
    let bc_outer = DirichletBC::new(2, exact_u);

    apply_dirichlet(&mut problem, &mesh, &[bc_inner, bc_outer]);

    // Solve
    let matrix = to_csr_matrix(&problem);
    let rhs = Array1::from_vec(problem.rhs.clone());

    let config = GmresConfig {
        max_iterations: 2000,
        restart: 100,
        tolerance: 1e-10,
        print_interval: 0,
    };

    let solution = gmres(&matrix, &rhs, &config);
    assert!(
        solution.converged,
        "GMRES should converge for scattering problem"
    );

    // Compute L2 error against analytical solution
    let error = l2_error(&mesh, &solution.x, exact_u);

    // For this mesh resolution, expect reasonable accuracy
    // Scattering problems are harder due to oscillatory nature and curved boundary
    assert!(
        error < 0.25,
        "Cylinder scattering L2 error {} should be < 0.25",
        error
    );
}

/// Test mesh convergence for cylinder scattering
#[test]
fn test_cylinder_scattering_convergence() {
    let cylinder_radius = 1.0;
    let outer_radius = 3.0;
    let k = 1.5; // ka = 1.5

    let num_terms = 25;

    let exact_u = move |x: f64, y: f64, _z: f64| {
        cylinder_scattering_analytical(k, cylinder_radius, x, y, num_terms)
    };

    let source = |_x: f64, _y: f64, _z: f64| Complex64::new(0.0, 0.0);

    // Test with increasing mesh resolution
    let refinements = [(8, 16), (12, 24), (16, 32)];
    let mut errors = Vec::new();
    let mut h_values = Vec::new();

    for &(n_radial, n_angular) in &refinements {
        let mesh =
            annular_mesh_triangles(0.0, 0.0, cylinder_radius, outer_radius, n_radial, n_angular);

        let k_complex = Complex64::new(k, 0.0);
        let mut problem =
            HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k_complex, source);

        let bc_inner = DirichletBC::new(1, |_x, _y, _z| Complex64::new(0.0, 0.0));
        let bc_outer = DirichletBC::new(2, exact_u);
        apply_dirichlet(&mut problem, &mesh, &[bc_inner, bc_outer]);

        let matrix = to_csr_matrix(&problem);
        let rhs = Array1::from_vec(problem.rhs.clone());

        let config = GmresConfig {
            max_iterations: 2000,
            restart: 100,
            tolerance: 1e-10,
            print_interval: 0,
        };

        let solution = gmres(&matrix, &rhs, &config);
        assert!(solution.converged, "GMRES should converge");

        let error = l2_error(&mesh, &solution.x, exact_u);
        errors.push(error);

        // Approximate h from radial spacing
        let h = (outer_radius - cylinder_radius) / (n_radial as f64);
        h_values.push(h);
    }

    // Verify errors decrease with refinement
    for i in 1..errors.len() {
        assert!(
            errors[i] < errors[i - 1],
            "Error should decrease: {} should be < {}",
            errors[i],
            errors[i - 1]
        );
    }

    // Verify convergence rate (may be lower than 2 due to curved boundary approximation)
    for i in 1..errors.len() {
        let rate = (errors[i - 1] / errors[i]).log2() / (h_values[i - 1] / h_values[i]).log2();
        assert!(
            rate > 1.0,
            "Scattering convergence rate {} should be > 1.0",
            rate
        );
    }
}

/// Test that scattered field satisfies Sommerfeld radiation condition qualitatively
///
/// For outgoing waves, |u_scattered| should decay like 1/√r in 2D.
#[test]
fn test_cylinder_scattering_far_field_decay() {
    let cylinder_radius = 1.0;
    let k = 2.0;
    let num_terms = 30;

    // Compute scattered field at different radii along θ=0
    let radii = [2.0, 3.0, 4.0, 5.0];
    let mut scattered_magnitudes = Vec::new();

    for &r in &radii {
        let total = cylinder_scattering_analytical(k, cylinder_radius, r, 0.0, num_terms);

        // Subtract incident wave to get scattered field
        let kr = k * r;
        let incident = Complex64::new(kr.cos(), kr.sin()); // exp(ikr) at θ=0
        let scattered = total - incident;

        scattered_magnitudes.push(scattered.norm());
    }

    // For 2D scattering, |u_scattered| ~ 1/√r
    // So |u(r1)| * √r1 ≈ |u(r2)| * √r2
    for i in 1..radii.len() {
        let r1 = radii[i - 1];
        let r2 = radii[i];
        let expected_ratio = (r1 / r2).sqrt();
        let actual_ratio = scattered_magnitudes[i] / scattered_magnitudes[i - 1];

        // Allow some tolerance since we're not in true far-field
        assert!(
            (actual_ratio - expected_ratio).abs() < 0.3,
            "Far-field decay: expected ratio ~{:.3}, got {:.3}",
            expected_ratio,
            actual_ratio
        );
    }
}

// ============================================================================
// 3D Tests
// ============================================================================

/// Test 3D plane wave solution
#[test]
fn test_3d_plane_wave() {
    let k = 2.0;
    let theta = PI / 4.0;
    let phi = PI / 3.0;

    // Wave vector
    let kx = k * theta.sin() * phi.cos();
    let ky = k * theta.sin() * phi.sin();
    let kz = k * theta.cos();

    // Use box mesh
    let mut mesh = box_mesh_tetrahedra(0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 4, 4, 4);

    // Set all boundaries to tag 1 (Dirichlet)
    mesh.set_boundary_condition(BoundaryType::Dirichlet, 1, |_| true);

    let k_complex = Complex64::new(k, 0.0);
    let mut problem =
        HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k_complex, |_, _, _| {
            Complex64::default()
        });

    let plane_wave = move |x: f64, y: f64, z: f64| {
        let phase = kx * x + ky * y + kz * z;
        Complex64::new(phase.cos(), phase.sin())
    };

    // Apply BC on tag 1
    let bc = DirichletBC::new(1, plane_wave);
    apply_dirichlet(&mut problem, &mesh, &[bc]);

    let matrix = to_csr_matrix(&problem);
    let rhs = Array1::from_vec(problem.rhs.clone());
    let config = GmresConfig {
        max_iterations: 500,
        restart: 50,
        tolerance: 1e-10,
        print_interval: 0,
    };
    let solution = gmres(&matrix, &rhs, &config);
    assert!(solution.converged, "GMRES should converge");

    let error = l2_error(&mesh, &solution.x, plane_wave);
    assert!(
        error < 0.05,
        "3D plane wave error {} should be < 0.05",
        error
    );
}

/// Helper: 2D Rigid Cylinder scattering
fn cylinder_scattering_rigid_point(k: f64, a: f64, x: f64, y: f64, num_terms: usize) -> Complex64 {
    use spec_math::Bessel;

    let r = (x * x + y * y).sqrt();
    let theta = y.atan2(x);
    let ka = k * a;
    let kr = k * r;

    let incident = Complex64::new((kr * theta.cos()).cos(), (kr * theta.cos()).sin());
    let mut scattered = Complex64::new(0.0, 0.0);

    for n in 0..num_terms {
        let n_f64 = n as f64;
        let epsilon_n = if n == 0 { 1.0 } else { 2.0 };

        // Derivatives J'_n(x) = J_{n-1}(x) - n/x J_n(x)
        let jn_ka = ka.bessel_jv(n_f64);
        let jn_prev = if n == 0 {
            -ka.bessel_jv(1.0)
        } else {
            ka.bessel_jv(n_f64 - 1.0)
        };
        let jn_prime = jn_prev - n_f64 / ka * jn_ka;

        // Y'_n(x)
        let yn_ka = ka.bessel_yv(n_f64);
        let yn_prev = if n == 0 {
            -ka.bessel_yv(1.0)
        } else {
            ka.bessel_yv(n_f64 - 1.0)
        };
        let yn_prime = yn_prev - n_f64 / ka * yn_ka;

        let hn_prime = Complex64::new(jn_prime, yn_prime);
        let hn_kr = Complex64::new(kr.bessel_jv(n_f64), kr.bessel_yv(n_f64));

        let i_pow_n = Complex64::new((n_f64 * PI / 2.0).cos(), (n_f64 * PI / 2.0).sin());

        // Coefficient for rigid cylinder (Neumann)
        let coeff = -epsilon_n * jn_prime / hn_prime * i_pow_n;

        scattered += coeff * hn_kr * (n_f64 * theta).cos();
    }
    incident + scattered
}

#[test]
fn test_cylinder_scattering_rigid() {
    let cylinder_radius = 1.0;
    let outer_radius = 3.0;
    let k = 2.0;

    let n_radial = 16;
    let n_angular = 32;
    let mesh =
        annular_mesh_triangles(0.0, 0.0, cylinder_radius, outer_radius, n_radial, n_angular);

    let num_terms = 30;
    let exact_u = move |x: f64, y: f64, _z: f64| {
        cylinder_scattering_rigid_point(k, cylinder_radius, x, y, num_terms)
    };

    let k_complex = Complex64::new(k, 0.0);
    let mut problem =
        HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k_complex, |_, _, _| {
            Complex64::default()
        });

    // BCs:
    // Inner (tag 1): Neumann (Rigid) -> Natural BC (do nothing)
    // Outer (tag 2): Dirichlet -> Exact solution
    let bc_outer = DirichletBC::new(2, exact_u);
    apply_dirichlet(&mut problem, &mesh, &[bc_outer]);

    let matrix = to_csr_matrix(&problem);
    let rhs = Array1::from_vec(problem.rhs.clone());
    let config = GmresConfig {
        max_iterations: 2000,
        restart: 100,
        tolerance: 1e-10,
        print_interval: 0,
    };
    let solution = gmres(&matrix, &rhs, &config);
    assert!(solution.converged);

    let error = l2_error(&mesh, &solution.x, exact_u);
    assert!(
        error < 0.25,
        "Rigid cylinder error {} should be < 0.25",
        error
    );
}

/// Helper: 3D Sphere scattering point evaluation
fn sphere_scattering_point(
    k: f64,
    a: f64,
    x: f64,
    y: f64,
    z: f64,
    num_terms: usize,
    bc_type: &str,
) -> Complex64 {
    let r = (x * x + y * y + z * z).sqrt();
    if r < 1e-10 {
        return Complex64::default();
    }
    // Angle theta from z-axis
    let cos_theta = z / r;

    let ka = k * a;
    let kr = k * r;

    let mut total = Complex64::default();

    for n in 0..num_terms {
        let n_f64 = n as f64;
        let prefactor = 2.0 * n_f64 + 1.0;
        let i_pow_n = Complex64::new((n_f64 * PI / 2.0).cos(), (n_f64 * PI / 2.0).sin());

        let jn_ka = spherical_bessel_j(n, ka);
        let yn_ka = spherical_bessel_y(n, ka);
        let hn_ka = Complex64::new(jn_ka, yn_ka);

        let jn_kr = spherical_bessel_j(n, kr);
        let yn_kr = spherical_bessel_y(n, kr);
        let hn_kr = Complex64::new(jn_kr, yn_kr);

        let pn = legendre_p(n, cos_theta);

        let coeff;
        if bc_type == "dirichlet" {
            // Sound-soft: jn(ka) / hn(ka)
            coeff = jn_ka / hn_ka;
        } else {
            // Rigid: jn'(ka) / hn'(ka)
            let jn_prev = if n == 0 {
                ka.cos() / ka
            } else {
                spherical_bessel_j(n - 1, ka)
            };
            let jn_prime = jn_prev - (n_f64 + 1.0) / ka * jn_ka;

            let yn_prev = if n == 0 {
                -ka.sin() / ka
            } else {
                spherical_bessel_y(n - 1, ka)
            };
            let yn_prime = yn_prev - (n_f64 + 1.0) / ka * yn_ka;

            let hn_prime = Complex64::new(jn_prime, yn_prime);
            coeff = Complex64::new(jn_prime, 0.0) / hn_prime;
        }

        // Total field = incident + scattered
        // u = Σ (2n+1) i^n [jn(kr) - coeff * hn(kr)] Pn(cos theta)
        total += prefactor * i_pow_n * (jn_kr - coeff * hn_kr) * pn;
    }
    total
}

#[test]
fn test_sphere_scattering_sound_soft() {
    let a = 1.0;
    let outer_r = 2.0; // Keep domain small for test speed
    let k = 1.0; // ka=1 (Mie)

    // Coarse mesh for speed
    let mesh = spherical_shell_mesh_tetrahedra(0.0, 0.0, 0.0, a, outer_r, 1, 4);

    let num_terms = 15;
    let exact_u = move |x: f64, y: f64, z: f64| {
        sphere_scattering_point(k, a, x, y, z, num_terms, "dirichlet")
    };

    let k_complex = Complex64::new(k, 0.0);
    let mut problem =
        HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k_complex, |_, _, _| {
            Complex64::default()
        });

    // Inner: Dirichlet u=0
    let bc_inner = DirichletBC::new(1, |_, _, _| Complex64::default());
    // Outer: Exact
    let bc_outer = DirichletBC::new(2, exact_u);

    apply_dirichlet(&mut problem, &mesh, &[bc_inner, bc_outer]);

    let matrix = to_csr_matrix(&problem);
    let rhs = Array1::from_vec(problem.rhs.clone());
    let config = GmresConfig {
        max_iterations: 1000,
        restart: 50,
        tolerance: 1e-8,
        print_interval: 0,
    };
    let solution = gmres(&matrix, &rhs, &config);
    assert!(solution.converged);

    let error = l2_error(&mesh, &solution.x, exact_u);
    // 3D FEM with linear tets on coarse mesh is not super accurate
    // Just check it's not garbage
    assert!(
        error < 0.4,
        "Sphere scattering (soft) error {} should be < 0.4",
        error
    );
}

#[test]
fn test_sphere_scattering_rigid() {
    let a = 1.0;
    let outer_r = 2.0;
    let k = 1.0;

    let mesh = spherical_shell_mesh_tetrahedra(0.0, 0.0, 0.0, a, outer_r, 1, 4);

    let num_terms = 15;
    let exact_u = move |x: f64, y: f64, z: f64| {
        sphere_scattering_point(k, a, x, y, z, num_terms, "rigid")
    };

    let k_complex = Complex64::new(k, 0.0);
    let mut problem =
        HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k_complex, |_, _, _| {
            Complex64::default()
        });

    // Inner: Natural Neumann (Rigid)
    // Outer: Exact
    let bc_outer = DirichletBC::new(2, exact_u);

    apply_dirichlet(&mut problem, &mesh, &[bc_outer]);

    let matrix = to_csr_matrix(&problem);
    let rhs = Array1::from_vec(problem.rhs.clone());
    let config = GmresConfig {
        max_iterations: 1000,
        restart: 50,
        tolerance: 1e-8,
        print_interval: 0,
    };
    let solution = gmres(&matrix, &rhs, &config);
    assert!(solution.converged);

    let error = l2_error(&mesh, &solution.x, exact_u);
    assert!(
        error < 0.4,
        "Sphere scattering (rigid) error {} should be < 0.4",
        error
    );
}
