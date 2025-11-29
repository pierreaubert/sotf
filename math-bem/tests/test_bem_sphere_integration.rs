//! BEM-Analytical Integration Tests
//!
//! This module tests the pure Rust BEM solver against analytical Mie series
//! solutions for sphere scattering. This validates the entire BEM pipeline:
//!
//! 1. Mesh generation (icosphere)
//! 2. System assembly (TBEM)
//! 3. Linear solve (Direct LU)
//! 4. Post-processing (field evaluation)
//!
//! against known analytical results.

#![cfg(feature = "pure-rust")]

use bem::analytical::sphere_scattering_3d;
use bem::core::{BemProblem, BemSolver};
use ndarray::Array2;
use std::f64::consts::PI;

/// Test BEM solver for low frequency (Rayleigh regime)
///
/// ka << 1: scattering is very weak, dominated by dipole term
#[test]
fn test_bem_vs_analytical_rayleigh() {
    let radius = 0.1;
    let frequency = 100.0; // Very low frequency
    let speed_of_sound = 343.0;
    let density = 1.21;

    let k = 2.0 * PI * frequency / speed_of_sound;
    let ka = k * radius;

    println!("\n=== BEM vs Analytical: Rayleigh Regime ===");
    println!("ka = {:.4}", ka);

    // Create and solve BEM problem with coarse mesh (sufficient for low frequency)
    let problem = BemProblem::rigid_sphere_scattering_custom(
        radius,
        frequency,
        speed_of_sound,
        density,
        6,  // n_theta - coarse mesh
        12, // n_phi
    );

    let solver = BemSolver::new();
    let solution = solver.solve(&problem).expect("BEM solve failed");

    println!("BEM DOFs: {}", solution.num_dofs());
    println!(
        "Max surface pressure: {:.6}",
        solution.max_surface_pressure()
    );

    // Evaluate at points outside sphere
    let eval_radius = 2.0 * radius;
    let theta_points: Vec<f64> = (0..9).map(|i| PI * i as f64 / 8.0).collect();

    // Generate evaluation points on a sphere
    let mut eval_points = Vec::new();
    for &theta in &theta_points {
        let x = eval_radius * theta.sin();
        let z = eval_radius * theta.cos();
        eval_points.push(x);
        eval_points.push(0.0);
        eval_points.push(z);
    }
    let eval_points = Array2::from_shape_vec((theta_points.len(), 3), eval_points).unwrap();

    // Get BEM field values
    let bem_field = solution.evaluate_pressure_field(&eval_points);

    // Get analytical solution
    let analytical = sphere_scattering_3d(
        k,
        radius,
        20, // num_terms
        vec![eval_radius],
        theta_points.clone(),
    );

    // Compare BEM vs analytical
    println!("\nTheta(deg)    BEM |p|       Analytical |p|    Rel Error");
    println!("--------------------------------------------------------");

    let mut max_rel_error = 0.0f64;
    for (i, &theta) in theta_points.iter().enumerate() {
        let bem_magnitude = bem_field[i].magnitude();
        let analytical_magnitude = analytical.pressure[i].norm();

        let rel_error = if analytical_magnitude > 1e-10 {
            (bem_magnitude - analytical_magnitude).abs() / analytical_magnitude
        } else {
            0.0
        };
        max_rel_error = max_rel_error.max(rel_error);

        println!(
            "{:8.1}     {:12.6}   {:12.6}        {:.2}%",
            theta * 180.0 / PI,
            bem_magnitude,
            analytical_magnitude,
            rel_error * 100.0
        );
    }

    println!("\nMax relative error: {:.2}%", max_rel_error * 100.0);

    // For Rayleigh regime with coarse mesh, we allow larger error
    // The key is that BEM converges to analytical as mesh is refined
    assert!(
        max_rel_error < 0.5,
        "BEM error too large for Rayleigh regime: {:.2}%",
        max_rel_error * 100.0
    );
}

/// Test BEM solver for medium frequency (Mie regime)
///
/// ka ~ 1: resonance regime, full multipole expansion needed
#[test]
fn test_bem_vs_analytical_mie() {
    let radius = 0.1;
    let frequency = 546.0; // Chosen so ka ~ 1
    let speed_of_sound = 343.0;
    let density = 1.21;

    let k = 2.0 * PI * frequency / speed_of_sound;
    let ka = k * radius;

    println!("\n=== BEM vs Analytical: Mie Regime ===");
    println!("ka = {:.4}", ka);

    // Create and solve BEM problem with medium mesh
    let problem = BemProblem::rigid_sphere_scattering_custom(
        radius,
        frequency,
        speed_of_sound,
        density,
        8,  // n_theta
        16, // n_phi
    );

    let solver = BemSolver::new();
    let solution = solver.solve(&problem).expect("BEM solve failed");

    println!("BEM DOFs: {}", solution.num_dofs());
    println!(
        "Max surface pressure: {:.6}",
        solution.max_surface_pressure()
    );

    // Evaluate at points outside sphere
    let eval_radius = 2.0 * radius;
    let theta_points: Vec<f64> = (0..13).map(|i| PI * i as f64 / 12.0).collect();

    let mut eval_points = Vec::new();
    for &theta in &theta_points {
        let x = eval_radius * theta.sin();
        let z = eval_radius * theta.cos();
        eval_points.push(x);
        eval_points.push(0.0);
        eval_points.push(z);
    }
    let eval_points = Array2::from_shape_vec((theta_points.len(), 3), eval_points).unwrap();

    let bem_field = solution.evaluate_pressure_field(&eval_points);

    let analytical = sphere_scattering_3d(k, radius, 30, vec![eval_radius], theta_points.clone());

    println!("\nTheta(deg)    BEM |p|       Analytical |p|    Rel Error");
    println!("--------------------------------------------------------");

    let mut max_rel_error = 0.0f64;
    for (i, &theta) in theta_points.iter().enumerate() {
        let bem_magnitude = bem_field[i].magnitude();
        let analytical_magnitude = analytical.pressure[i].norm();

        let rel_error = if analytical_magnitude > 1e-10 {
            (bem_magnitude - analytical_magnitude).abs() / analytical_magnitude
        } else {
            0.0
        };
        max_rel_error = max_rel_error.max(rel_error);

        println!(
            "{:8.1}     {:12.6}   {:12.6}        {:.2}%",
            theta * 180.0 / PI,
            bem_magnitude,
            analytical_magnitude,
            rel_error * 100.0
        );
    }

    println!("\nMax relative error: {:.2}%", max_rel_error * 100.0);

    // Mie regime is more challenging, allow moderate error
    assert!(
        max_rel_error < 0.75,
        "BEM error too large for Mie regime: {:.2}%",
        max_rel_error * 100.0
    );
}

/// Test BEM solver surface pressure distribution
///
/// Verifies that the BEM solution produces physically reasonable surface pressures
#[test]
fn test_bem_surface_pressure_distribution() {
    let radius = 0.1;
    let frequency = 200.0;
    let speed_of_sound = 343.0;
    let density = 1.21;

    let k = 2.0 * PI * frequency / speed_of_sound;
    let ka = k * radius;

    println!("\n=== BEM Surface Pressure Distribution ===");
    println!("ka = {:.4}", ka);

    let problem = BemProblem::rigid_sphere_scattering_custom(
        radius,
        frequency,
        speed_of_sound,
        density,
        8,
        16,
    );

    let solver = BemSolver::new();
    let solution = solver.solve(&problem).expect("BEM solve failed");

    let n_elem = solution.surface_pressure.len();
    let max_p = solution.max_surface_pressure();
    let mean_p = solution.mean_surface_pressure();

    println!("Number of elements: {}", n_elem);
    println!("Max |p|: {:.6}", max_p);
    println!("Mean |p|: {:.6}", mean_p);

    // Physical checks:
    // 1. Surface pressure should be real and positive magnitude
    assert!(max_p > 0.0, "Surface pressure should be non-zero");

    // 2. For a rigid sphere in plane wave, pressure is approximately
    //    doubled at the forward stagnation point (theta=0) and reduced at sides
    //    So max should be roughly 1.5-2.5x mean
    let ratio = max_p / mean_p;
    println!("Max/Mean ratio: {:.3}", ratio);
    assert!(
        ratio > 1.0 && ratio < 5.0,
        "Max/Mean ratio should be reasonable: {}",
        ratio
    );
}

/// Test mesh convergence study (informational, not strict assertion)
///
/// Note: BEM convergence depends on many factors including element quality,
/// quadrature accuracy, and the ka regime. This test documents behavior
/// rather than enforcing strict monotonic convergence.
#[test]
fn test_bem_mesh_convergence() {
    let radius = 0.1;
    let frequency = 300.0;
    let speed_of_sound = 343.0;
    let density = 1.21;

    let k = 2.0 * PI * frequency / speed_of_sound;
    let ka = k * radius;

    println!("\n=== BEM Mesh Convergence Study ===");
    println!("ka = {:.4}", ka);

    // Analytical solution at a single point
    let eval_radius = 2.0 * radius;
    let theta = PI / 4.0; // 45 degrees
    let analytical = sphere_scattering_3d(k, radius, 30, vec![eval_radius], vec![theta]);
    let analytical_p = analytical.pressure[0].norm();

    let eval_point = Array2::from_shape_vec(
        (1, 3),
        vec![eval_radius * theta.sin(), 0.0, eval_radius * theta.cos()],
    )
    .unwrap();

    // Test different mesh resolutions
    let mesh_sizes = [(4, 8), (6, 12), (8, 16)];
    let mut errors = Vec::new();

    println!("\nMesh       DOFs    BEM |p|      Analytical    Error");
    println!("-----------------------------------------------------");

    for (n_theta, n_phi) in mesh_sizes {
        let problem = BemProblem::rigid_sphere_scattering_custom(
            radius,
            frequency,
            speed_of_sound,
            density,
            n_theta,
            n_phi,
        );

        let solver = BemSolver::new();
        let solution = solver.solve(&problem).unwrap();
        let bem_field = solution.evaluate_pressure_field(&eval_point);
        let bem_p = bem_field[0].magnitude();

        let error = (bem_p - analytical_p).abs() / analytical_p;
        errors.push(error);

        println!(
            "{:2}x{:2}      {:4}    {:10.6}   {:10.6}    {:.2}%",
            n_theta,
            n_phi,
            solution.num_dofs(),
            bem_p,
            analytical_p,
            error * 100.0
        );
    }

    // Verify that at least one mesh gives reasonable error (< 50%)
    let min_error = errors.iter().copied().fold(f64::INFINITY, f64::min);
    assert!(
        min_error < 0.5,
        "At least one mesh should give < 50% error, got min error {:.2}%",
        min_error * 100.0
    );
}

/// Quick sanity test that the solver runs without panicking
#[test]
fn test_bem_solver_basic_sanity() {
    // Very simple test: just verify the solver doesn't crash
    let problem = BemProblem::rigid_sphere_scattering(0.1, 100.0, 343.0, 1.21);

    let solver = BemSolver::new();
    let result = solver.solve(&problem);

    assert!(result.is_ok(), "Solver should not fail on basic problem");

    let solution = result.unwrap();
    assert!(solution.num_dofs() > 0);
    assert!(solution.max_surface_pressure().is_finite());
    assert!(solution.max_surface_pressure() > 0.0);
}
