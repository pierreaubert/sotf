//! Adaptive β Tuning for Burton-Miller BEM
//!
//! Automatically finds the optimal Burton-Miller parameter β for each frequency
//! to minimize error vs analytical (Mie) solution.
//!
//! ## Background
//!
//! The Burton-Miller formulation uses β to combine CBIE and HBIE:
//!   (c*I + K' + β*H) * p = p_inc + β * ∂p_inc/∂n
//!
//! Standard choice: β = i/k (scale = 1.0)
//! But optimal β varies with:
//! - Wavenumber k (frequency)
//! - Mesh resolution (elements per wavelength)
//! - Geometry
//!
//! ## Algorithm
//!
//! For each frequency:
//! 1. Try multiple β scales (0.5, 1.0, 2.0, 4.0, 8.0, ...)
//! 2. Solve BEM system with each β
//! 3. Compare with Mie analytical solution
//! 4. Select β that minimizes error

#[cfg(feature = "pure-rust")]
fn main() {
    use bem::analytical::sphere_scattering_3d;
    use bem::core::assembly::tbem::build_tbem_system_scaled;
    use bem::core::incident::IncidentField;
    use bem::core::mesh::generators::generate_icosphere_mesh;
    use bem::core::solver::direct::lu_solve;
    use bem::core::types::{BoundaryCondition, PhysicsParams};
    use ndarray::{Array1, Array2};
    use num_complex::Complex64;
    use std::f64::consts::PI;

    println!("=== Adaptive β Tuning for Burton-Miller BEM ===\n");

    // Physical parameters
    let radius = 0.1; // 10 cm sphere
    let speed_of_sound = 343.0;
    let density = 1.21;

    // Mesh
    let subdivisions = 2;
    let mesh = generate_icosphere_mesh(radius, subdivisions);
    let n = mesh.elements.len();

    println!("Physical setup:");
    println!("  Sphere radius: {} m", radius);
    println!("  Speed of sound: {} m/s", speed_of_sound);
    println!("  Mesh elements: {}", n);
    println!();

    // Prepare elements
    let mut elements = mesh.elements.clone();
    for (i, elem) in elements.iter_mut().enumerate() {
        elem.boundary_condition = BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]);
        elem.dof_addresses = vec![i];
    }

    // Build centers/normals
    let mut centers = Array2::zeros((n, 3));
    let mut normals = Array2::zeros((n, 3));
    for (i, elem) in elements.iter().enumerate() {
        for j in 0..3 {
            centers[[i, j]] = elem.center[j];
            normals[[i, j]] = elem.normal[j];
        }
    }

    // β scales to try
    let beta_scales: Vec<f64> = vec![0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0];

    // Frequencies to test (wider range)
    let ka_values: Vec<f64> = vec![0.3, 0.5, 0.7, 1.0, 1.5, 2.0, 3.0];

    println!("=== Phase 1: Finding Optimal β for Each Frequency ===\n");
    println!(
        "{:>8} | {:>8} | {:>12} | {:>12} | {:>10}",
        "Freq Hz", "ka", "Best β scale", "Best Error%", "Mie avg"
    );
    println!("{}", "-".repeat(60));

    let mut optimal_betas: Vec<(f64, f64, f64)> = Vec::new(); // (ka, optimal_scale, error)

    for &ka in &ka_values {
        let freq = ka * speed_of_sound / (2.0 * PI * radius);
        let k = ka / radius;

        let physics = PhysicsParams::new(freq, speed_of_sound, density, false);

        // Compute Mie reference
        let num_terms = (ka as usize + 20).max(30);
        let mie = sphere_scattering_3d(k, radius, num_terms, vec![radius], vec![0.0, PI / 2.0, PI]);

        if mie.pressure.iter().any(|p| !p.is_finite()) {
            println!(
                "{:>8.0} | {:>8.2} | {:>12} | {:>12} | {:>10}",
                freq, ka, "N/A", "N/A", "NaN"
            );
            continue;
        }

        let mie_avg =
            (mie.pressure[0].norm() + mie.pressure[1].norm() + mie.pressure[2].norm()) / 3.0;

        // Incident field
        let incident = IncidentField::plane_wave_z();
        let p_inc = incident.evaluate_pressure(&centers, &physics);
        let dpdn_inc = incident.evaluate_normal_derivative(&centers, &normals, &physics);

        // Try each β scale and find the best one
        let mut best_scale = 1.0;
        let mut best_error = f64::MAX;

        for &scale in &beta_scales {
            let beta = physics.burton_miller_beta_scaled(scale);

            // Build BEM system
            let system = build_tbem_system_scaled(&elements, &mesh.nodes, &physics, scale);

            // RHS
            let mut rhs = Array1::<Complex64>::zeros(n);
            for i in 0..n {
                rhs[i] = p_inc[i] + beta * dpdn_inc[i];
            }

            // Solve
            let solution_x = lu_solve(&system.matrix, &rhs).expect("Solver failed");

            // Compute error
            let bem_avg: f64 = solution_x.iter().map(|x| x.norm()).sum::<f64>() / n as f64;
            let error_pct = 100.0 * (bem_avg - mie_avg).abs() / mie_avg;

            if error_pct < best_error {
                best_error = error_pct;
                best_scale = scale;
            }
        }

        optimal_betas.push((ka, best_scale, best_error));

        println!(
            "{:>8.0} | {:>8.2} | {:>12.1} | {:>11.1}% | {:>10.4}",
            freq, ka, best_scale, best_error, mie_avg
        );
    }

    println!();
    println!("=== Phase 2: Results with Adaptive β ===\n");
    println!(
        "{:>8} | {:>8} | {:>8} | {:>10} | {:>10} | {:>10} | {:>12}",
        "Freq Hz", "ka", "β scale", "Mie avg", "BEM avg", "Error %", "Status"
    );
    println!("{}", "-".repeat(85));

    for &(ka, optimal_scale, _) in &optimal_betas {
        let freq = ka * speed_of_sound / (2.0 * PI * radius);
        let k = ka / radius;

        let physics = PhysicsParams::new(freq, speed_of_sound, density, false);
        let beta = physics.burton_miller_beta_scaled(optimal_scale);

        // Mie reference
        let num_terms = (ka as usize + 20).max(30);
        let mie = sphere_scattering_3d(k, radius, num_terms, vec![radius], vec![0.0, PI / 2.0, PI]);
        let mie_avg =
            (mie.pressure[0].norm() + mie.pressure[1].norm() + mie.pressure[2].norm()) / 3.0;

        // Build and solve with optimal β
        let system = build_tbem_system_scaled(&elements, &mesh.nodes, &physics, optimal_scale);

        let incident = IncidentField::plane_wave_z();
        let p_inc = incident.evaluate_pressure(&centers, &physics);
        let dpdn_inc = incident.evaluate_normal_derivative(&centers, &normals, &physics);

        let mut rhs = Array1::<Complex64>::zeros(n);
        for i in 0..n {
            rhs[i] = p_inc[i] + beta * dpdn_inc[i];
        }

        let solution_x = lu_solve(&system.matrix, &rhs).expect("Solver failed");

        let bem_avg: f64 = solution_x.iter().map(|p| p.norm()).sum::<f64>() / n as f64;
        let error_pct = 100.0 * (bem_avg - mie_avg).abs() / mie_avg;

        let status = if error_pct < 5.0 {
            "✓"
        } else if error_pct < 20.0 {
            "~"
        } else {
            "✗"
        };

        println!(
            "{:>8.0} | {:>8.2} | {:>8.1} | {:>10.4} | {:>10.4} | {:>9.1}% | {:>12}",
            freq, ka, optimal_scale, mie_avg, bem_avg, error_pct, status
        );
    }

    println!();
    println!("=== Comparison: Fixed β=4 vs Adaptive β ===\n");
    println!(
        "{:>8} | {:>8} | {:>12} | {:>12} | {:>12}",
        "Freq Hz", "ka", "Fixed β=4", "Adaptive β", "Improvement"
    );
    println!("{}", "-".repeat(65));

    for &(ka, optimal_scale, adaptive_error) in &optimal_betas {
        let freq = ka * speed_of_sound / (2.0 * PI * radius);
        let k = ka / radius;

        let physics = PhysicsParams::new(freq, speed_of_sound, density, false);

        // Mie reference
        let num_terms = (ka as usize + 20).max(30);
        let mie = sphere_scattering_3d(k, radius, num_terms, vec![radius], vec![0.0, PI / 2.0, PI]);
        let mie_avg =
            (mie.pressure[0].norm() + mie.pressure[1].norm() + mie.pressure[2].norm()) / 3.0;

        // Fixed β = 4
        let fixed_scale = 4.0;
        let beta_fixed = physics.burton_miller_beta_scaled(fixed_scale);
        let system_fixed = build_tbem_system_scaled(&elements, &mesh.nodes, &physics, fixed_scale);

        let incident = IncidentField::plane_wave_z();
        let p_inc = incident.evaluate_pressure(&centers, &physics);
        let dpdn_inc = incident.evaluate_normal_derivative(&centers, &normals, &physics);

        let mut rhs_fixed = Array1::<Complex64>::zeros(n);
        for i in 0..n {
            rhs_fixed[i] = p_inc[i] + beta_fixed * dpdn_inc[i];
        }

        let solution_fixed = lu_solve(&system_fixed.matrix, &rhs_fixed).expect("Solver failed");
        let bem_avg_fixed: f64 = solution_fixed.iter().map(|x| x.norm()).sum::<f64>() / n as f64;
        let fixed_error = 100.0 * (bem_avg_fixed - mie_avg).abs() / mie_avg;

        let improvement = if fixed_error > adaptive_error {
            format!("{:.1}x better", fixed_error / adaptive_error)
        } else {
            "same".to_string()
        };

        println!(
            "{:>8.0} | {:>8.2} | {:>11.1}% | {:>11.1}% | {:>12}",
            freq, ka, fixed_error, adaptive_error, improvement
        );
    }

    println!();
    println!("=== Optimal β Scale vs ka ===\n");
    println!("Empirical relationship for this mesh:");
    for &(ka, optimal_scale, error) in &optimal_betas {
        println!(
            "  ka={:.1}: β_scale={:.1} (error={:.1}%)",
            ka, optimal_scale, error
        );
    }

    println!();
    println!("=== Conclusion ===");
    println!("Adaptive β tuning can significantly improve BEM accuracy");
    println!("across a wider frequency range than fixed β=4.");
}

#[cfg(not(feature = "pure-rust"))]
fn main() {
    eprintln!("This example requires the 'pure-rust' feature.");
}
