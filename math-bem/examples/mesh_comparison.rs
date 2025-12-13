//! Mesh Comparison Test - Compare UV-sphere vs Icosphere meshes
//!
//! The UV-sphere has non-uniform element sizes (small near poles, large at equator)
//! while the icosphere has more uniform elements. This tests if mesh uniformity
//! affects BEM accuracy.
//!
//! Also compares standard β = i/k vs scaled β = 2i/k for conditioning improvement.

#[cfg(feature = "pure-rust")]
fn main() {
    use bem::analytical::sphere_scattering_3d;
    use bem::core::assembly::tbem::{build_tbem_system, build_tbem_system_scaled};
    use bem::core::incident::IncidentField;
    use bem::core::mesh::generators::{generate_icosphere_mesh, generate_sphere_mesh};
    use bem::core::solver::direct::lu_solve;
    use bem::core::types::{BoundaryCondition, PhysicsParams};
    use ndarray::Array2;
    use num_complex::Complex64;
    use std::f64::consts::PI;

    println!("=== Mesh Comparison: UV-sphere vs Icosphere ===");
    println!("=== Comparing standard β vs scaled β (2×) ===\n");

    let radius = 0.1;
    let speed_of_sound = 343.0;
    let density = 1.21;

    // Test at different ka values
    for ka_target in [0.2, 0.5, 1.0] {
        let frequency = ka_target * speed_of_sound / (2.0 * PI * radius);
        let k = 2.0 * PI * frequency / speed_of_sound;

        println!("\n{}", "=".repeat(60));
        println!(
            "=== ka = {:.2} (f = {:.1} Hz, k = {:.2}) ===",
            ka_target, frequency, k
        );
        println!("{}", "=".repeat(60));

        // Test UV-sphere with both beta values
        println!("\n--- UV-sphere mesh (n_theta=8, n_phi=16) ---");
        let mesh_uv = generate_sphere_mesh(radius, 8, 16);
        println!("\n  Standard β = i/k:");
        test_mesh(
            &mesh_uv,
            frequency,
            speed_of_sound,
            density,
            k,
            radius,
            "UV-sphere",
            None,
        );
        println!("\n  Scaled β = 2i/k:");
        test_mesh(
            &mesh_uv,
            frequency,
            speed_of_sound,
            density,
            k,
            radius,
            "UV-sphere",
            Some(2.0),
        );

        // Test Icosphere with both beta values
        println!("\n--- Icosphere mesh (subdivisions=2, ~320 elements) ---");
        let mesh_ico = generate_icosphere_mesh(radius, 2);
        println!("\n  Standard β = i/k:");
        test_mesh(
            &mesh_ico,
            frequency,
            speed_of_sound,
            density,
            k,
            radius,
            "Icosphere",
            None,
        );
        println!("\n  Scaled β = 2i/k:");
        test_mesh(
            &mesh_ico,
            frequency,
            speed_of_sound,
            density,
            k,
            radius,
            "Icosphere",
            Some(2.0),
        );
    }
}

#[cfg(feature = "pure-rust")]
fn test_mesh(
    mesh: &bem::core::types::Mesh,
    frequency: f64,
    speed_of_sound: f64,
    density: f64,
    k: f64,
    radius: f64,
    _name: &str,
    beta_scale: Option<f64>,
) {
    use bem::analytical::sphere_scattering_3d;
    use bem::core::assembly::tbem::{build_tbem_system, build_tbem_system_scaled};
    use bem::core::incident::IncidentField;
    use bem::core::solver::direct::lu_solve;
    use bem::core::types::{BoundaryCondition, PhysicsParams};
    use ndarray::Array2;
    use num_complex::Complex64;
    use std::f64::consts::PI;

    let physics = PhysicsParams::new(frequency, speed_of_sound, density, false);

    // Prepare elements
    let mut elements = mesh.elements.clone();
    for (i, elem) in elements.iter_mut().enumerate() {
        elem.boundary_condition = BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]);
        elem.dof_addresses = vec![i];
    }

    // Build system with standard or scaled beta
    let system = match beta_scale {
        Some(scale) => build_tbem_system_scaled(&elements, &mesh.nodes, &physics, scale),
        None => build_tbem_system(&elements, &mesh.nodes, &physics),
    };

    // Compute RHS with corresponding beta
    let incident = IncidentField::plane_wave_z();
    let n = elements.len();
    let mut centers = Array2::zeros((n, 3));
    let mut normals = Array2::zeros((n, 3));
    for (i, elem) in elements.iter().enumerate() {
        for j in 0..3 {
            centers[[i, j]] = elem.center[j];
            normals[[i, j]] = elem.normal[j];
        }
    }

    // RHS must use the same beta as the matrix!
    let beta = match beta_scale {
        Some(scale) => physics.burton_miller_beta_scaled(scale),
        None => physics.burton_miller_beta(),
    };
    let p_inc = incident.evaluate_pressure(&centers, &physics);
    let dpdn_inc = incident.evaluate_normal_derivative(&centers, &normals, &physics);

    // RHS = p_inc + β * ∂p_inc/∂n (for velocity BC with v=0)
    let mut total_rhs = ndarray::Array1::<Complex64>::zeros(n);
    for i in 0..n {
        total_rhs[i] = p_inc[i] + beta * dpdn_inc[i];
    }

    // Solve
    let solution_x = lu_solve(&system.matrix, &total_rhs).expect("Solver failed");

    // Statistics
    let p_avg: f64 = solution_x.iter().map(|x| x.norm()).sum::<f64>() / n as f64;
    let p_max = solution_x.iter().map(|x| x.norm()).fold(0.0f64, f64::max);

    // Get analytical reference
    let theta_refs: Vec<f64> = (0..=18).map(|i| i as f64 * 10.0 * PI / 180.0).collect();
    let mie = sphere_scattering_3d(k, radius, 50, vec![radius], theta_refs.clone());
    let mie_avg: f64 =
        mie.pressure.iter().map(|p| p.norm()).sum::<f64>() / mie.pressure.len() as f64;

    println!(
        "  {} elements, avg |p| = {:.4}, Mie avg = {:.4}",
        n, p_avg, mie_avg
    );

    // Angular comparison at key angles
    let test_angles = [0.0, 30.0, 60.0, 90.0, 120.0, 150.0, 180.0];

    println!("  θ (deg)  |  BEM |p|  |  Mie |p|  | Error %");
    println!("  ---------|-----------|-----------|--------");

    for &theta_deg in &test_angles {
        let theta_rad = theta_deg * PI / 180.0;
        let cos_theta_target = theta_rad.cos();

        // Find elements near this angle
        let mut matching_elements: Vec<usize> = Vec::new();
        for (i, elem) in elements.iter().enumerate() {
            let z = elem.center[2];
            let cos_theta = z / radius;
            // Within 10 degrees
            if (cos_theta - cos_theta_target).abs() < 0.17 {
                matching_elements.push(i);
            }
        }

        if matching_elements.is_empty() {
            continue;
        }

        let bem_avg: f64 = matching_elements
            .iter()
            .map(|&i| solution_x[i].norm())
            .sum::<f64>()
            / matching_elements.len() as f64;

        // Mie at this angle
        let mie_idx = (theta_deg / 10.0).round() as usize;
        let mie_val = if mie_idx < mie.pressure.len() {
            mie.pressure[mie_idx].norm()
        } else {
            mie.pressure.last().unwrap().norm()
        };

        let error = if mie_val > 1e-10 {
            100.0 * (bem_avg - mie_val).abs() / mie_val
        } else {
            0.0
        };

        let marker = if error > 20.0 { " <--" } else { "" };
        println!(
            "  {:>7.1}° |  {:>7.4}  |  {:>7.4}  | {:>6.1}%{}",
            theta_deg, bem_avg, mie_val, error, marker
        );
    }
}

#[cfg(not(feature = "pure-rust"))]
fn main() {
    eprintln!("This example requires the 'pure-rust' feature.");
}
