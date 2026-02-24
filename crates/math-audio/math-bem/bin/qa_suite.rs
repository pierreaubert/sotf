//! QA Suite for Math-BEM
//!
//! Comprehensive validation suite for BEM solvers.
//!
//! ## Tests
//! 1. Rigid Sphere Scattering (Rayleigh, Mie, Geometric regimes)
//! 2. Pulsating Sphere Radiation (Monopole)
//!
//! ## Solver Selection
//! | Problem Size | Solver Method |
//! //! |--------------|---------------|
//! //! | N < 1000 | Direct (LU) |
//! //! | N < 5000 | GMRES+ILU |
//! //! | N < 20000 | FMM+GMRES+ILU |
//! //! | N > 20000 | FMM+Batched |
//!
//! ## Bug Fixes Applied (Feb 2026)
//! - Parallel assembly free term sign correction (tbem.rs)
//! - Quad post-processing shape functions (pressure.rs)
//! - Near-cluster symmetry in FMM (slfmm.rs)
//! - Subelement indexing for singular integration (singular.rs)
//! - Element size estimation via sqrt(area) (singular.rs)
//! - Characteristic radius from bounding box (tbem.rs)
//! - Beta scale threshold consistency (types.rs)
//!
//! Usage:
//!     cargo run --release --bin qa-suite
//!     cargo run --release --bin qa-suite -- --debug
//!     cargo run --release --bin qa-suite -- --skip-radiation

use clap::Parser;
use math_audio_bem::analytical::sphere_scattering_3d;
use math_audio_bem::core::assembly::tbem::build_tbem_system_with_beta;
use math_audio_bem::core::incident::IncidentField;
use math_audio_bem::core::mesh::generators::generate_icosphere_mesh;
use math_audio_bem::core::solver::{
    BiCgstabConfig, CgsConfig, DenseOperator, GmresConfig, direct::lu_solve,
    gmres_solve_tbem_with_ilu, solve_bicgstab, solve_cgs,
};
use math_audio_bem::core::types::{BoundaryCondition, PhysicsParams};
use math_audio_bem::testing::ValidationResult;
use math_audio_wave::analytical::{AnalyticalSolution, Point};
use num_complex::Complex64;
use std::f64::consts::PI;
use std::path::Path;

#[derive(Parser, Debug)]
#[command(name = "qa-suite")]
#[command(about = "Quality assurance tests for Math-BEM", long_about = None)]
struct Args {
    /// Enable verbose debug output for diagnostics
    #[arg(short, long)]
    debug: bool,

    /// Skip radiation (pulsating sphere) tests
    #[arg(long)]
    skip_radiation: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
enum SolverType {
    Lu,
    GmresIlu,
    Bicgstab,
    Cgs,
}

impl std::fmt::Display for SolverType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SolverType::Lu => write!(f, "LU"),
            SolverType::GmresIlu => write!(f, "GMRES+ILU"),
            SolverType::Bicgstab => write!(f, "BiCGStab"),
            SolverType::Cgs => write!(f, "CGS"),
        }
    }
}

/// Returns the best solver type based on problem size (number of DOFs)
#[allow(dead_code)]
fn best_solver_for_size(n_dofs: usize) -> SolverType {
    match n_dofs {
        n if n < 1000 => SolverType::Lu, // Direct LU for small problems
        n if n < 5000 => SolverType::GmresIlu, // GMRES+ILU for medium problems
        _ => SolverType::GmresIlu,       // GMRES+ILU scales better than BiCGStab/CGS
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Initialize logging
    env_logger::init();

    println!("Starting Math-BEM QA Suite...");
    println!("=============================");

    if args.debug {
        println!("Debug mode enabled");
    }

    let mut results = Vec::new();
    let radius = 0.1;
    let speed_of_sound = 343.0;
    let density = 1.21;

    let solvers = [SolverType::Lu, SolverType::GmresIlu];

    // 1. Rigid Sphere Scattering Tests
    println!("\nRunning Rigid Sphere Scattering Tests...");

    // Rayleigh regime (ka = 0.2) - small problem
    for solver in &solvers {
        results.push(run_scattering_test(
            &format!("Scattering (Rayleigh, ka=0.2) [{}]", solver),
            radius,
            0.2,
            speed_of_sound,
            density,
            *solver,
            args.debug,
        )?);
    }

    // Mie regime (ka = 1.0) - medium problem
    for solver in &solvers {
        results.push(run_scattering_test(
            &format!("Scattering (Mie, ka=1.0) [{}]", solver),
            radius,
            1.0,
            speed_of_sound,
            density,
            *solver,
            args.debug,
        )?);
    }

    // Geometric regime (ka = 3.0) - larger problem
    for solver in &solvers {
        results.push(run_scattering_test(
            &format!("Scattering (Geometric, ka=3.0) [{}]", solver),
            radius,
            3.0,
            speed_of_sound,
            density,
            *solver,
            args.debug,
        )?);
    }

    // 2. Best Solver Tests - auto-select best solver for each regime
    println!("\nRunning Best Solver Selection Tests...");

    // Rayleigh: Small problem, use Direct LU
    results.push(run_scattering_test(
        "Scattering (Rayleigh, ka=0.2) [Best: LU]",
        radius,
        0.2,
        speed_of_sound,
        density,
        SolverType::Lu,
        args.debug,
    )?);

    // Mie: Medium problem, use GMRES+ILU
    results.push(run_scattering_test(
        "Scattering (Mie, ka=1.0) [Best: GMRES+ILU]",
        radius,
        1.0,
        speed_of_sound,
        density,
        SolverType::GmresIlu,
        args.debug,
    )?);

    // Geometric: Larger problem, use GMRES+ILU
    results.push(run_scattering_test(
        "Scattering (Geometric, ka=3.0) [Best: GMRES+ILU]",
        radius,
        3.0,
        speed_of_sound,
        density,
        SolverType::GmresIlu,
        args.debug,
    )?);

    // 3. Pulsating Sphere Radiation Test
    if !args.skip_radiation {
        println!("\nRunning Pulsating Sphere Radiation Tests...");
        for solver in &solvers {
            results.push(run_pulsating_sphere_test(
                &format!("Radiation (Monopole, ka=1.0) [{}]", solver),
                radius,
                1.0,
                speed_of_sound,
                density,
                *solver,
                args.debug,
            )?);
        }
    } else {
        println!("\nSkipping Pulsating Sphere Radiation Tests (--skip-radiation)");
    }

    // Summary
    print_summary(&results);

    // Save results
    let output_path = "qa_results.json";
    save_results(&results, output_path)?;
    println!("\nFull results saved to: {}", output_path);

    // Check strict pass/fail
    let mut failed = false;
    for res in &results {
        // Rayleigh (low freq) should be very accurate after bug fixes
        // Resonance regimes (Mie/Geometric) are harder for constant elements
        let tolerance = if res.parameters.dimensionless_param >= 1.0 {
            0.30
        } else {
            0.02 // Tightened from 0.05 after bug fixes
        };

        if !res.passed(tolerance) {
            eprintln!(
                "TEST FAILED: {} (Error: {:.2}%)",
                res.test_name,
                res.errors.l2_relative * 100.0
            );
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

fn run_scattering_test(
    name: &str,
    radius: f64,
    ka: f64,
    c: f64,
    rho: f64,
    solver_type: SolverType,
    debug: bool,
) -> anyhow::Result<ValidationResult> {
    println!("  Executing: {}...", name);
    let start_time = std::time::Instant::now();

    let k = ka / radius;
    let freq = k * c / (2.0 * PI);
    let physics = PhysicsParams::new(freq, c, rho, false);

    // Mesh generation
    // Use finer mesh for Mie/Geometric to improve accuracy
    let subdivisions = if ka >= 1.0 { 3 } else { 2 };
    let mesh = generate_icosphere_mesh(radius, subdivisions);

    // Setup Problem: Rigid Sphere (v=0)
    let mut elements = mesh.elements.clone();
    for (i, elem) in elements.iter_mut().enumerate() {
        elem.boundary_condition = BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]);
        elem.dof_addresses = vec![i];
    }

    // Compute characteristic radius for diagnostics
    let mut min_coord = [f64::MAX, f64::MAX, f64::MAX];
    let mut max_coord = [f64::MIN, f64::MIN, f64::MIN];
    for elem in &elements {
        for d in 0..3 {
            min_coord[d] = min_coord[d].min(elem.center[d]);
            max_coord[d] = max_coord[d].max(elem.center[d]);
        }
    }
    let characteristic_radius = 0.5
        * (max_coord[0] - min_coord[0])
            .max(max_coord[1] - min_coord[1])
            .max(max_coord[2] - min_coord[2]);
    let computed_ka = k * characteristic_radius;
    let beta_scale = PhysicsParams::optimal_beta_scale(computed_ka);

    if debug {
        println!("    Diagnostics:");
        println!("      Characteristic radius: {:.4}m", characteristic_radius);
        println!("      Computed ka: {:.3}", computed_ka);
        println!("      Beta scale: {}", beta_scale);
        println!("      Mesh subdivisions: {}", subdivisions);
        println!("      Elements: {}", elements.len());
    }

    // Solve
    let (beta, _scale) = physics.burton_miller_beta_adaptive(radius);
    let system = build_tbem_system_with_beta(&elements, &mesh.nodes, &physics, beta);

    // Incident Field (Plane Wave +z)
    let incident = IncidentField::plane_wave_z();

    // Compute RHS
    let n_elem = elements.len();
    let mut centers = ndarray::Array2::zeros((n_elem, 3));
    let mut normals = ndarray::Array2::zeros((n_elem, 3));
    for (i, elem) in elements.iter().enumerate() {
        for j in 0..3 {
            centers[[i, j]] = elem.center[j];
            normals[[i, j]] = elem.normal[j];
        }
    }

    let rhs = incident.compute_rhs_with_beta(&centers, &normals, &physics, beta);

    // Total RHS (v=0 implies system.rhs is 0)
    let total_rhs = &system.rhs + &rhs;

    let p_bem = match solver_type {
        SolverType::Lu => lu_solve(&system.matrix, &total_rhs).map_err(|e| anyhow::anyhow!(e))?,
        SolverType::GmresIlu => {
            let config = GmresConfig {
                max_iterations: 1000,
                restart: 50,
                tolerance: 1e-6,
                print_interval: 0,
            };
            let solution = gmres_solve_tbem_with_ilu(&system.matrix, &total_rhs, &config);
            if !solution.converged {
                eprintln!("GMRES+ILU failed to converge");
            }
            solution.x
        }
        SolverType::Bicgstab => {
            let config = BiCgstabConfig {
                max_iterations: 1000,
                tolerance: 1e-6,
                print_interval: 0,
            };
            let op = DenseOperator::new(system.matrix.clone());
            let solution = solve_bicgstab(&op, &total_rhs, &config);
            if !solution.converged {
                eprintln!("BiCGStab failed to converge");
            }
            solution.x
        }
        SolverType::Cgs => {
            let config = CgsConfig {
                max_iterations: 1000,
                tolerance: 1e-6,
                print_interval: 0,
            };
            let op = DenseOperator::new(system.matrix.clone());
            let solution = solve_cgs(&op, &total_rhs, &config);
            if !solution.converged {
                eprintln!("CGS failed to converge");
            }
            solution.x
        }
    };

    // Analytical Solution (Surface Pressure)
    let mut p_analytical = Vec::with_capacity(n_elem);
    let mut positions = Vec::with_capacity(n_elem);

    for elem in &elements {
        let center = &elem.center;
        positions.push(Point {
            x: center[0],
            y: center[1],
            z: center[2],
        });

        // Theta angle from z-axis
        let r = (center[0] * center[0] + center[1] * center[1] + center[2] * center[2]).sqrt();
        let theta = (center[2] / r).acos();

        // Evaluate Mie series
        let mie = sphere_scattering_3d(k, radius, 50, vec![r], vec![theta]);
        p_analytical.push(mie.pressure[0]);
    }

    let mut analytical_sol = AnalyticalSolution::new(name, 3, positions, p_analytical, k, freq);
    analytical_sol.metadata =
        serde_json::json!({ "ka": ka, "radius": radius, "solver": solver_type.to_string() });

    let duration = start_time.elapsed().as_millis() as u64;

    Ok(ValidationResult::new(
        name,
        &analytical_sol,
        p_bem.to_vec(),
        duration,
        0.0,
    ))
}

fn run_pulsating_sphere_test(
    name: &str,
    radius: f64,
    ka: f64,
    c: f64,
    rho: f64,
    solver_type: SolverType,
    debug: bool,
) -> anyhow::Result<ValidationResult> {
    println!("  Executing: {}...", name);
    let start_time = std::time::Instant::now();

    let k = ka / radius;
    let freq = k * c / (2.0 * PI);
    let physics = PhysicsParams::new(freq, c, rho, false);

    let mesh = generate_icosphere_mesh(radius, 2);

    // Setup Problem: Pulsating Sphere (v = 1.0 m/s outwards)
    let v0 = Complex64::new(1.0, 0.0);
    let mut elements = mesh.elements.clone();
    for (i, elem) in elements.iter_mut().enumerate() {
        elem.boundary_condition = BoundaryCondition::Velocity(vec![v0]);
        elem.dof_addresses = vec![i];
    }

    if debug {
        println!("    Diagnostics:");
        println!("      Elements: {}", elements.len());
        println!("      Velocity BC: v0 = 1.0 m/s");
        println!(
            "      Analytical surface pressure: {:.6}",
            Complex64::new(0.0, 1.0) * ka * rho * c * v0 / (Complex64::new(0.0, 1.0) * ka - 1.0)
        );
    }

    let beta = physics.burton_miller_beta();
    // Use build_tbem_system_with_beta directly for radiation
    let system = build_tbem_system_with_beta(&elements, &mesh.nodes, &physics, beta);

    // No incident field, only BC excitation (which is in system.rhs)
    let p_bem = match solver_type {
        SolverType::Lu => lu_solve(&system.matrix, &system.rhs).map_err(|e| anyhow::anyhow!(e))?,
        SolverType::GmresIlu => {
            let config = GmresConfig {
                max_iterations: 1000,
                restart: 50,
                tolerance: 1e-6,
                print_interval: 0,
            };
            let solution = gmres_solve_tbem_with_ilu(&system.matrix, &system.rhs, &config);
            if !solution.converged {
                eprintln!("GMRES+ILU failed to converge");
            }
            solution.x
        }
        SolverType::Bicgstab => {
            let config = BiCgstabConfig {
                max_iterations: 1000,
                tolerance: 1e-6,
                print_interval: 0,
            };
            let op = DenseOperator::new(system.matrix.clone());
            let solution = solve_bicgstab(&op, &system.rhs, &config);
            if !solution.converged {
                eprintln!("BiCGStab failed to converge");
            }
            solution.x
        }
        SolverType::Cgs => {
            let config = CgsConfig {
                max_iterations: 1000,
                tolerance: 1e-6,
                print_interval: 0,
            };
            let op = DenseOperator::new(system.matrix.clone());
            let solution = solve_cgs(&op, &system.rhs, &config);
            if !solution.converged {
                eprintln!("CGS failed to converge");
            }
            solution.x
        }
    };

    // Analytical Solution for Monopole at Surface
    let numerator = Complex64::new(0.0, 1.0) * ka * rho * c * v0;
    let denominator = Complex64::new(0.0, 1.0) * ka - 1.0;
    let p_surf_analytical = numerator / denominator;

    let n_elem = elements.len();
    let mut p_analytical = Vec::with_capacity(n_elem);
    let mut positions = Vec::with_capacity(n_elem);

    for elem in &elements {
        let center = &elem.center;
        positions.push(Point {
            x: center[0],
            y: center[1],
            z: center[2],
        });
        p_analytical.push(p_surf_analytical); // Constant on surface
    }

    let mut analytical_sol = AnalyticalSolution::new(name, 3, positions, p_analytical, k, freq);
    analytical_sol.metadata =
        serde_json::json!({ "ka": ka, "radius": radius, "solver": solver_type.to_string() });

    let duration = start_time.elapsed().as_millis() as u64;

    Ok(ValidationResult::new(
        name,
        &analytical_sol,
        p_bem.to_vec(),
        duration,
        0.0,
    ))
}

fn print_summary(results: &[ValidationResult]) {
    println!("\nQA Summary:");
    println!(
        "{:<35} | {:<10} | {:<10} | {:<10}",
        "Test Name", "L2 Error%", "Max Err%", "Status"
    );
    println!("{:-<35}-|-{:-<10}-|-{:-<10}-|-{:-<10}", "", "", "", "");

    for res in results {
        let l2_err = res.errors.l2_relative * 100.0;
        let max_err = res.errors.max_relative * 100.0;

        // Tightened Rayleigh tolerance after bug fixes
        let tolerance = if res.parameters.dimensionless_param >= 1.0 {
            0.30
        } else {
            0.02
        };
        let status = if res.passed(tolerance) {
            "PASS"
        } else {
            "FAIL"
        };

        println!(
            "{:<35} | {:6.2}%    | {:6.2}%    | {}",
            res.test_name, l2_err, max_err, status
        );
    }
}

fn save_results(results: &[ValidationResult], path: impl AsRef<Path>) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(results)?;
    std::fs::write(path, json)?;
    Ok(())
}
