//! Angular Validation - Compare BEM solution angular distribution against Mie theory
//!
//! This example compares the surface pressure at different angles around a rigid
//! sphere to identify where the BEM solution deviates from analytical Mie series.
//!
//! # Usage
//! ```bash
//! cargo run --release --example angular_validation --features pure-rust
//! ```

#[cfg(feature = "pure-rust")]
fn main() {
    use bem::analytical::sphere_scattering_3d;
    use bem::core::assembly::tbem::{build_tbem_system, build_tbem_system_with_beta};
    use bem::core::incident::IncidentField;
    use bem::core::mesh::generators::generate_sphere_mesh;
    use bem::core::solver::direct::lu_solve;
    use bem::core::types::{BoundaryCondition, PhysicsParams};
    use ndarray::Array2;
    use num_complex::Complex64;
    use std::f64::consts::PI;

    println!("=== Angular Validation: BEM vs Mie Theory ===\n");

    let radius = 0.1;
    let speed_of_sound = 343.0;
    let density = 1.21;

    // Test at ka = 1.0 (Mie regime) with different mesh resolutions
    let ka_target = 1.0;
    let frequency = ka_target * speed_of_sound / (2.0 * PI * radius);
    let k = 2.0 * PI * frequency / speed_of_sound;
    let ka = k * radius;

    println!(
        "Testing ka = {:.2} (f = {:.1} Hz, k = {:.4})",
        ka, frequency, k
    );
    println!(
        "Wavelength λ = {:.4} m, Sphere diameter = {:.4} m",
        2.0 * PI / k,
        2.0 * radius
    );
    println!("Elements per wavelength criterion: ~6-10 needed\n");

    // Test different mesh resolutions
    for (n_theta, n_phi, name) in [(8, 16, "coarse"), (12, 24, "medium"), (16, 32, "fine")] {
        let n_elements = n_theta * n_phi * 2;
        let elements_per_wavelength = (n_elements as f64).sqrt() * 2.0 * PI / (ka * 2.0 * PI);

        println!(
            "=== Mesh: {} ({} elements, ~{:.1} per λ) ===",
            name, n_elements, elements_per_wavelength
        );

        let mesh = generate_sphere_mesh(radius, n_theta, n_phi);
        let physics = PhysicsParams::new(frequency, speed_of_sound, density, false);

        // Prepare elements with velocity BC
        let mut elements = mesh.elements.clone();
        for (i, elem) in elements.iter_mut().enumerate() {
            elem.boundary_condition = BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]);
            elem.dof_addresses = vec![i];
        }

        // Build system with traditional β
        let system = build_tbem_system(&elements, &mesh.nodes, &physics);

        // Build incident field RHS
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

        let incident_rhs = incident.compute_rhs(&centers, &normals, &physics, true);
        let total_rhs = &system.rhs + &incident_rhs;

        // Solve
        let solution = lu_solve(&system.matrix, &total_rhs).expect("Solver failed");

        // Group elements by theta angle (z-axis is the incident direction)
        // theta = 0° is forward (z = +r), theta = 180° is backward (z = -r)
        let mut angle_bins: Vec<(f64, Vec<(usize, f64)>)> = Vec::new();
        for (i, elem) in elements.iter().enumerate() {
            let z = elem.center[2];
            let theta = (z / radius).clamp(-1.0, 1.0).acos() * 180.0 / PI;

            // Find or create bin (10° increments)
            let bin_idx = (theta / 10.0).floor() as usize;
            while angle_bins.len() <= bin_idx {
                angle_bins.push((angle_bins.len() as f64 * 10.0 + 5.0, Vec::new()));
            }
            angle_bins[bin_idx].1.push((i, theta));
        }

        // Get analytical solution at surface for comparison
        // Note: Mie theory gives us pressure at specific angles
        let theta_degrees: Vec<f64> = (0..=18).map(|i| i as f64 * 10.0).collect();
        let theta_radians: Vec<f64> = theta_degrees.iter().map(|t| t * PI / 180.0).collect();
        let mie = sphere_scattering_3d(k, radius, 50, vec![radius], theta_radians.clone());

        println!("\n  θ (deg)  |  BEM |p|  |  Mie |p|  |  Error %  |  # elem");
        println!("  ---------|-----------|-----------|-----------|--------");

        let mut total_error = 0.0;
        let mut count = 0;

        for (bin_theta, elements_in_bin) in &angle_bins {
            if elements_in_bin.is_empty() {
                continue;
            }

            // Average BEM pressure in this angular bin
            let avg_p: Complex64 = elements_in_bin
                .iter()
                .map(|(idx, _)| solution[*idx])
                .sum::<Complex64>()
                / elements_in_bin.len() as f64;
            let bem_mag = avg_p.norm();

            // Get corresponding analytical value
            let bin_idx = (*bin_theta / 10.0).floor() as usize;
            let mie_mag = if bin_idx < mie.pressure.len() {
                mie.pressure[bin_idx].norm()
            } else {
                mie.pressure.last().unwrap().norm()
            };

            let error_pct = if mie_mag > 1e-10 {
                100.0 * (bem_mag - mie_mag).abs() / mie_mag
            } else {
                0.0
            };

            total_error += error_pct;
            count += 1;

            let err_marker = if error_pct > 20.0 { " <--" } else { "" };
            println!(
                "  {:>7.1}° |  {:>7.4}  |  {:>7.4}  |  {:>7.1}%  |  {:>5}{}",
                bin_theta,
                bem_mag,
                mie_mag,
                error_pct,
                elements_in_bin.len(),
                err_marker
            );
        }

        let avg_error = total_error / count as f64;
        println!("\n  Average angular error: {:.1}%\n", avg_error);
    }

    // Also test with floored beta at ka = 2.0 where it helped dramatically
    println!("\n=== Floored β Test at ka = 2.0 ===\n");

    let ka_target = 2.0;
    let frequency = ka_target * speed_of_sound / (2.0 * PI * radius);
    let k = 2.0 * PI * frequency / speed_of_sound;

    let n_theta = 12;
    let n_phi = 24;
    let mesh = generate_sphere_mesh(radius, n_theta, n_phi);
    let physics = PhysicsParams::new(frequency, speed_of_sound, density, false);

    let mut elements = mesh.elements.clone();
    for (i, elem) in elements.iter_mut().enumerate() {
        elem.boundary_condition = BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]);
        elem.dof_addresses = vec![i];
    }

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

    // Traditional β
    let system_trad = build_tbem_system(&elements, &mesh.nodes, &physics);
    let incident_rhs_trad = incident.compute_rhs(&centers, &normals, &physics, true);
    let total_rhs_trad = &system_trad.rhs + &incident_rhs_trad;
    let solution_trad = lu_solve(&system_trad.matrix, &total_rhs_trad).expect("Solver failed");

    // Floored β
    let edge_e_magnitude = 70.0;
    let min_beta_e = 10.0;
    let beta_floored = physics.burton_miller_beta_floored(edge_e_magnitude, min_beta_e);
    let system_floored =
        build_tbem_system_with_beta(&elements, &mesh.nodes, &physics, beta_floored);
    let incident_rhs_floored =
        incident.compute_rhs_with_beta(&centers, &normals, &physics, beta_floored);
    let total_rhs_floored = &system_floored.rhs + &incident_rhs_floored;
    let solution_floored =
        lu_solve(&system_floored.matrix, &total_rhs_floored).expect("Solver failed");

    // Analytical
    let theta_degrees: Vec<f64> = (0..=18).map(|i| i as f64 * 10.0).collect();
    let theta_radians: Vec<f64> = theta_degrees.iter().map(|t| t * PI / 180.0).collect();
    let mie = sphere_scattering_3d(k, radius, 50, vec![radius], theta_radians.clone());

    // Group by angle
    let mut angle_bins: Vec<(f64, Vec<usize>)> = Vec::new();
    for (i, elem) in elements.iter().enumerate() {
        let z = elem.center[2];
        let theta = (z / radius).clamp(-1.0, 1.0).acos() * 180.0 / PI;
        let bin_idx = (theta / 10.0).floor() as usize;
        while angle_bins.len() <= bin_idx {
            angle_bins.push((angle_bins.len() as f64 * 10.0 + 5.0, Vec::new()));
        }
        angle_bins[bin_idx].1.push(i);
    }

    println!(
        "  β_traditional = {:.4}i, β_floored = {:.4}i",
        physics.burton_miller_beta().im,
        beta_floored.im
    );
    println!("\n  θ (deg)  |  BEM(trad) |  BEM(floor)|  Mie |p|  |  Err(t) |  Err(f)");
    println!("  ---------|------------|------------|-----------|---------|--------");

    let mut total_err_trad = 0.0;
    let mut total_err_floor = 0.0;
    let mut count = 0;

    for (bin_theta, indices) in &angle_bins {
        if indices.is_empty() {
            continue;
        }

        let avg_trad: f64 = indices
            .iter()
            .map(|idx| solution_trad[*idx].norm())
            .sum::<f64>()
            / indices.len() as f64;

        let avg_floor: f64 = indices
            .iter()
            .map(|idx| solution_floored[*idx].norm())
            .sum::<f64>()
            / indices.len() as f64;

        let bin_idx = (*bin_theta / 10.0).floor() as usize;
        let mie_mag = if bin_idx < mie.pressure.len() {
            mie.pressure[bin_idx].norm()
        } else {
            mie.pressure.last().unwrap().norm()
        };

        let err_trad = if mie_mag > 1e-10 {
            100.0 * (avg_trad - mie_mag).abs() / mie_mag
        } else {
            0.0
        };

        let err_floor = if mie_mag > 1e-10 {
            100.0 * (avg_floor - mie_mag).abs() / mie_mag
        } else {
            0.0
        };

        total_err_trad += err_trad;
        total_err_floor += err_floor;
        count += 1;

        println!(
            "  {:>7.1}° |  {:>8.4}  |  {:>8.4}  |  {:>7.4}  | {:>6.1}% | {:>6.1}%",
            bin_theta, avg_trad, avg_floor, mie_mag, err_trad, err_floor
        );
    }

    println!(
        "\n  Average error: traditional={:.1}%, floored={:.1}%",
        total_err_trad / count as f64,
        total_err_floor / count as f64
    );
}

#[cfg(not(feature = "pure-rust"))]
fn main() {
    eprintln!("This example requires the 'pure-rust' feature.");
    eprintln!("Run with: cargo run --example angular_validation --features pure-rust");
}
