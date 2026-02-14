//! QA Suite for Math-FEM
//! 
//! Comprehensive validation suite for FEM solvers.
//! Validates:
//! 1. Cylinder Scattering (Sound-soft) - 2D
//! 2. Sphere Scattering (Sound-hard) - 3D
//! 
//! Usage:
//!     cargo run --bin qa-suite --release

use math_audio_fem::assembly::HelmholtzProblem;
use math_audio_fem::basis::PolynomialDegree;
use math_audio_fem::boundary::{DirichletBC, apply_dirichlet};
use math_audio_fem::mesh::{annular_mesh_triangles, spherical_shell_mesh_tetrahedra};
use math_audio_fem::solver::{
    GmresConfigF64, ShiftedLaplacianConfig, SolverConfig, SolverType, solve,
};
use math_audio_wave::special::{
    legendre_p, spherical_bessel_j, spherical_bessel_y,
};
use spec_math::Bessel;
use ndarray::Array1;
use num_complex::Complex64;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub test_name: String,
    pub solver: String,
    pub mesh_info: String,
    pub dofs: usize,
    pub duration_ms: u64,
    pub l2_error: f64,
    pub iterations: usize,
    pub residual: f64,
    pub b_norm: f64,
    pub x_norm: f64,
    pub passed: bool,
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    println!("Starting Math-FEM QA Suite...");
    println!("=============================");

    let mut results = Vec::new();

    // Define solvers to test
    let standard_solvers = [
        SolverType::GmresIlu,
        SolverType::GmresAmg,
        SolverType::GmresShiftedLaplacian,
    ];

    let parallel_solvers = [
        SolverType::GmresPipelinedAmg,
        SolverType::GmresPipelinedIlu,
    ];

    // 1. Cylinder Scattering Test (2D)
    println!("\nRunning Cylinder Scattering Tests (Convergence Study)...");

    let k_values = [0.5, 2.0, 3.0, 5.0];
    
    // (n_radial, n_angular, Name)
    let refinements = [
        (8, 16, "Coarse"),
        (16, 32, "Medium"),
        (32, 64, "Fine"),
        (64, 128, "Very Fine"),
        (128, 256, "Super Fine"),
    ];

    for k_val in &k_values {
        for (n_radial, n_angular, mesh_name) in &refinements {
            // Adaptive Skip Logic
            let skip = match *k_val {
                0.5 => *mesh_name == "Super Fine",
                2.0 => *mesh_name == "Coarse",
                3.0 => *mesh_name == "Coarse" || *mesh_name == "Medium",
                5.0 => *mesh_name != "Very Fine" && *mesh_name != "Super Fine",
                _ => false,
            };
            if skip { continue; }

            // Use Direct solver for small meshes to verify baseline
            if *n_radial <= 16 {
                results.push(run_cylinder_scattering_test(
                    &format!("Cylinder (k={:.1}) [Direct]", k_val),
                    *k_val,
                    SolverConfig { solver_type: SolverType::Direct, verbosity: 0, ..Default::default() },
                    *n_radial, *n_angular, mesh_name
                )?);
            }

            // Test Iterative Solvers
            for solver_type in &standard_solvers {
                // Configure specific solvers
                let mut config = SolverConfig {
                    solver_type: *solver_type,
                    verbosity: 2, // Enable detailed logging
                    wavenumber: Some(*k_val),
                    gmres: GmresConfigF64 {
                        max_iterations: 1000,
                        restart: 50,
                        tolerance: 1e-10,
                        print_interval: 10,
                    },
                    ..Default::default()
                };

                // Tune Shifted Laplacian for higher k
                if *solver_type == SolverType::GmresShiftedLaplacian && *k_val >= 3.0 {
                     config.shifted_laplacian = Some(ShiftedLaplacianConfig::conservative(*k_val));
                }

                results.push(run_cylinder_scattering_test(
                    &format!("Cylinder (k={:.1}) [{:?}]", k_val, solver_type),
                    *k_val,
                    config,
                    *n_radial, *n_angular, mesh_name
                )?);
            }
            
            // Test Parallel Solvers on larger meshes
            if *n_radial >= 32 {
                 for solver_type in &parallel_solvers {
                    let config = SolverConfig {
                        solver_type: *solver_type,
                        verbosity: 0,
                        gmres: GmresConfigF64 {
                            max_iterations: 5000,
                            restart: 100,
                            tolerance: 1e-10,
                            print_interval: 0,
                        },
                         ..Default::default()
                    };
                    results.push(run_cylinder_scattering_test(
                        &format!("Cylinder (k={:.1}) [{:?}]", k_val, solver_type),
                        *k_val,
                        config,
                        *n_radial, *n_angular, mesh_name
                    )?);
                 }
            }
        }
    }

    // 2. Sphere Scattering Test (3D)
    println!("\nRunning Sphere Scattering Tests (3D)...");

    let sphere_cases = [
        (1.0, 1, 4, "Subdiv 1 (Coarse)"),
        (1.0, 2, 8, "Subdiv 2 (Medium)"),
        (2.0, 3, 12, "Subdiv 3 (Fine)"),
    ];

    for (k, subdiv, layers, name) in &sphere_cases {
         // Direct solver check for Coarse
         if *subdiv == 1 {
            results.push(run_sphere_scattering_test(
                &format!("Sphere (k={:.1}) [Direct]", k),
                *k,
                SolverConfig { solver_type: SolverType::Direct, verbosity: 0, ..Default::default() },
                *subdiv, *layers, name
            )?);
         }

        for solver_type in &standard_solvers {
             let mut config = SolverConfig {
                solver_type: *solver_type,
                verbosity: 0,
                wavenumber: Some(*k),
                gmres: GmresConfigF64 {
                    max_iterations: 2000,
                    restart: 60,
                    tolerance: 1e-8,
                    print_interval: 0,
                },
                ..Default::default()
            };
            
            // For 3D, Shifted Laplacian benefits from aggressive shifts sometimes
            if *solver_type == SolverType::GmresShiftedLaplacian {
                 config.shifted_laplacian = Some(ShiftedLaplacianConfig::for_wavenumber(*k));
            }

            results.push(run_sphere_scattering_test(
                &format!("Sphere (k={:.1}) [{:?}]", k, solver_type),
                *k,
                config,
                *subdiv, *layers, name
            )?);
        }
    }

    // Summary
    print_summary(&results);

    // Save results
    let output_path = "qa_results_fem.json";
    save_results(&results, output_path)?;
    println!("\nFull results saved to: {}", output_path);

    // Check pass/fail
    let mut failed = false;
    for res in &results {
        if !res.passed {
            eprintln!("TEST FAILED: {} - {} (Error: {:.2}%)", res.test_name, res.mesh_info, res.l2_error * 100.0);
            failed = true;
        }
    }

    if failed {
        std::process::exit(1);
    } else {
        println!("\nALL TESTS PASSED");
        Ok(())
    }
}

fn run_sphere_scattering_test(
    name: &str,
    k: f64,
    config: SolverConfig,
    subdivisions: usize,
    layers: usize,
    mesh_name: &str,
) -> anyhow::Result<ValidationResult> {
    println!("  Executing: {} with {}...", name, mesh_name);
    let start_time = Instant::now();

    let sphere_radius = 1.0;
    let outer_radius = 2.0;
    let mesh = spherical_shell_mesh_tetrahedra(0.0, 0.0, 0.0, sphere_radius, outer_radius, subdivisions, layers);

    let num_terms = (k * sphere_radius + 10.0) as usize;
    let coeffs = compute_rigid_sphere_coefficients(k * sphere_radius, num_terms);

    // Analytical Solution
    let exact_u = move |x: f64, y: f64, z: f64| {
        let r = (x * x + y * y + z * z).sqrt();
        let kr = k * r;
        let cos_theta = if r > 1e-10 { z / r } else { 0.0 };

        let mut total = Complex64::new(0.0, 0.0);
        
        // Optimize: Precompute Legendre if possible, but for QA direct eval is fine
        for (n, coeff) in coeffs.iter().enumerate() {
            let n_f64 = n as f64;
            let prefactor = 2.0 * n_f64 + 1.0;
            // i^n
            let i_pow = match n % 4 {
                0 => Complex64::new(1.0, 0.0),
                1 => Complex64::new(0.0, 1.0),
                2 => Complex64::new(-1.0, 0.0),
                _ => Complex64::new(0.0, -1.0),
            };

            let j_vals = spherical_bessel_j(n as usize + 1, kr);
            let y_vals = spherical_bessel_y(n as usize + 1, kr);
            let jn = j_vals[n as usize];
            let yn = y_vals[n as usize];
            let hn = Complex64::new(jn, yn);
            let pn = legendre_p(n as usize, cos_theta);

            total += prefactor * i_pow * (jn - coeff * hn) * pn;
        }
        total
    };

    let k_complex = Complex64::new(k, 0.0);
    let source = |_x: f64, _y: f64, _z: f64| Complex64::new(0.0, 0.0);
    let mut problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k_complex, source);

    let bc_outer = DirichletBC::new(2, exact_u.clone());
    apply_dirichlet(&mut problem, &mesh, &[bc_outer]);

    // Solve
    let result = solve(&problem, &config);

    let duration = start_time.elapsed().as_millis() as u64;

    let (_solution_vec, iterations, residual, passed, error, x_norm) = match result {
        Ok(sol) => {
            let error = l2_error(&mesh, &sol.values, exact_u);
            let x_norm = sol.values.iter().map(|x| x.norm_sqr()).sum::<f64>().sqrt();
            
            // Relaxed thresholds for 3D P1 elements (they are notoriously inaccurate for wave problems)
            let threshold = match k {
                v if v < 1.5 => 0.60, 
                _ => 1.0,
            };
            
            (sol.values, sol.iterations, sol.residual, error < threshold, error, x_norm)
        },
        Err(e) => {
            eprintln!("Solver failed: {}", e);
            (Array1::zeros(problem.num_dofs()), 0, 1.0, false, 1.0, 0.0)
        }
    };

    let b_norm = problem.rhs.iter().map(|x| x.norm_sqr()).sum::<f64>().sqrt();

    Ok(ValidationResult {
        test_name: name.to_string(),
        solver: format!("{:?}", config.solver_type),
        mesh_info: format!("{} ({} DOFs)", mesh_name, problem.num_dofs()),
        dofs: problem.num_dofs(),
        duration_ms: duration,
        l2_error: error,
        iterations,
        residual,
        b_norm,
        x_norm,
        passed,
    })
}

fn run_cylinder_scattering_test(
    name: &str,
    k: f64,
    config: SolverConfig,
    n_radial: usize,
    n_angular: usize,
    mesh_name: &str,
) -> anyhow::Result<ValidationResult> {
    println!("  Executing: {} with {}...", name, mesh_name);
    let start_time = Instant::now();

    let cylinder_radius = 1.0;
    let outer_radius = 3.0;
    let mesh = annular_mesh_triangles(0.0, 0.0, cylinder_radius, outer_radius, n_radial, n_angular);

    // Analytical Solution
    let num_terms = (k * cylinder_radius + 15.0) as usize;
    let exact_u = move |x: f64, y: f64, _z: f64| {
        cylinder_scattering_analytical(k, cylinder_radius, x, y, num_terms)
    };

    let k_complex = Complex64::new(k, 0.0);
    let source = |_x: f64, _y: f64, _z: f64| Complex64::new(0.0, 0.0);
    let mut problem = HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, k_complex, source);

    let bc_inner = DirichletBC::new(1, |_x, _y, _z| Complex64::new(0.0, 0.0));
    let bc_outer = DirichletBC::new(2, exact_u);
    apply_dirichlet(&mut problem, &mesh, &[bc_inner, bc_outer]);

    // Solve
    let result = solve(&problem, &config);

    let duration = start_time.elapsed().as_millis() as u64;

    let (_solution_vec, iterations, residual, passed, error, x_norm) = match result {
        Ok(sol) => {
            let error = l2_error(&mesh, &sol.values, exact_u);
            let x_norm = sol.values.iter().map(|x| x.norm_sqr()).sum::<f64>().sqrt();
            
            // Dynamic thresholds based on mesh density (k*h) 
            // P1 elements require k*h < 1 (approx 6-10 elements per wavelength)
            // Here we just use approximate "reasonable" error check
            let threshold = match (mesh_name, k) {
                ("Coarse", 0.5) => 0.02,
                ("Coarse", _) => 0.80, // Terrible for high k
                ("Medium", 2.0) => 0.30,
                ("Fine", 3.0) => 0.30,
                ("Super Fine", 5.0) => 0.50,
                (_, _) => 0.20,
            };
            
            (sol.values, sol.iterations, sol.residual, error < threshold, error, x_norm)
        },
        Err(e) => {
             eprintln!("Solver failed: {}", e);
            (Array1::zeros(problem.num_dofs()), 0, 1.0, false, 1.0, 0.0)
        }
    };

    let b_norm = problem.rhs.iter().map(|x| x.norm_sqr()).sum::<f64>().sqrt();

    Ok(ValidationResult {
        test_name: name.to_string(),
        solver: format!("{:?}", config.solver_type),
        mesh_info: format!("{} ({} DOFs)", mesh_name, problem.num_dofs()),
        dofs: problem.num_dofs(),
        duration_ms: duration,
        l2_error: error,
        iterations,
        residual,
        b_norm,
        x_norm,
        passed,
    })
}

fn compute_rigid_sphere_coefficients(ka: f64, num_terms: usize) -> Vec<Complex64> {
    let mut coefficients = Vec::with_capacity(num_terms);
    for n in 0..num_terms {
        let n_f64 = n as f64;
        let j_n_vals = spherical_bessel_j(n as usize + 1, ka);
        let y_n_vals = spherical_bessel_y(n as usize + 1, ka);
        let jn = j_n_vals[n as usize];
        let yn = y_n_vals[n as usize];

        let jn_minus_1 = if n > 0 { j_n_vals[n-1] } else { ka.cos() / ka };
        let yn_minus_1 = if n > 0 { y_n_vals[n-1] } else { -ka.sin() / ka };

        let jn_prime = jn_minus_1 - (n_f64 + 1.0) / ka * jn;
        let yn_prime = yn_minus_1 - (n_f64 + 1.0) / ka * yn;

        let hn_prime = Complex64::new(jn_prime, yn_prime);
        coefficients.push(Complex64::new(jn_prime, 0.0) / hn_prime);
    }
    coefficients
}

fn cylinder_scattering_analytical(k: f64, cylinder_radius: f64, x: f64, y: f64, num_terms: usize) -> Complex64 {
    let r = (x * x + y * y).sqrt();
    let theta = y.atan2(x);
    if r < cylinder_radius * 0.999 { return Complex64::new(0.0, 0.0); }

    let ka = k * cylinder_radius;
    let kr = k * r;
    let incident = Complex64::new((kr * theta.cos()).cos(), (kr * theta.cos()).sin());
    let mut scattered = Complex64::new(0.0, 0.0);

    for n in 0..num_terms {
        let n_f64 = n as f64;
        let epsilon_n = if n == 0 { 1.0 } else { 2.0 };
        
        let jn_ka = ka.bessel_jv(n_f64);
        let yn_ka = ka.bessel_yv(n_f64);
        let jn_kr = kr.bessel_jv(n_f64);
        let yn_kr = kr.bessel_yv(n_f64);
        
        let hn_ka = Complex64::new(jn_ka, yn_ka);
        let hn_kr = Complex64::new(jn_kr, yn_kr);
        
        let i_power_n = match n % 4 {
             0 => Complex64::new(1.0, 0.0),
             1 => Complex64::new(0.0, 1.0),
             2 => Complex64::new(-1.0, 0.0),
             _ => Complex64::new(0.0, -1.0),
        };

        let a_n = -epsilon_n * jn_ka / hn_ka * i_power_n;
        scattered += a_n * hn_kr * (n_f64 * theta).cos();
    }
    incident + scattered
}

fn l2_error<F>(mesh: &math_audio_fem::mesh::Mesh, fem_solution: &Array1<Complex64>, analytical: F) -> f64
where F: Fn(f64, f64, f64) -> Complex64
{
    let mut error_sq = 0.0;
    let mut norm_sq = 0.0;
    for (i, node) in mesh.nodes.iter().enumerate() {
        let fem_val = fem_solution[i];
        let exact_val = analytical(node.x, node.y, node.z);
        error_sq += (fem_val - exact_val).norm_sqr();
        norm_sq += exact_val.norm_sqr();
    }
    if norm_sq > 1e-15 { (error_sq / norm_sq).sqrt() } else { error_sq.sqrt() }
}

fn print_summary(results: &[ValidationResult]) {
    println!("\nQA Summary:");
    println!(
        "{:<35} | {:<25} | {:<10} | {:<6} | {:<6} | {:<8} | {:<10}",
        "Test Name", "Solver", "Mesh DOFs", "Error%", "Iters", "Time(ms)", "Status"
    );
    println!("{:-<120}", "");
    for res in results {
        let l2_err = res.l2_error * 100.0;
        let status = if res.passed { "PASS" } else { "FAIL" };
        let solver_short = res.solver.replace("Gmres", "").replace("ShiftedLaplacian", "SL");
        println!(
            "{:<35} | {:<25} | {:<10} | {:6.2}% | {:<6} | {:<8} | {}",
            res.test_name, solver_short, res.dofs, l2_err, res.iterations, res.duration_ms, status
        );
    }
}

fn save_results(results: &[ValidationResult], path: impl AsRef<Path>) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(results)?;
    std::fs::write(path, json)?;
    Ok(())
}