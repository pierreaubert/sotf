//! Mesh Convergence Study: Finding optimal mesh density per ka
//!
//! Tests different subdivision levels (2, 3, 4) across the ka range
//! to determine minimum mesh density for target accuracy.
//!
//! Key insight: Different ka values may require different mesh densities.
//! Low ka (long wavelength) → coarser mesh OK
//! High ka (short wavelength) → finer mesh needed

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

    println!("=== Mesh Convergence Study ===\n");

    let radius = 0.1;
    let speed_of_sound = 343.0;
    let density = 1.21;

    // Test ka values focusing on problematic ones
    let ka_values: Vec<f64> = vec![0.3, 0.5, 0.7, 0.8, 0.9, 1.0, 1.5, 2.0, 3.0];

    // Test subdivisions 2, 3, and optionally 4
    let subdivisions = vec![2, 3];

    // Precompute meshes
    let meshes: Vec<_> = subdivisions
        .iter()
        .map(|&s| {
            let mesh = generate_icosphere_mesh(radius, s);
            let n = mesh.elements.len();
            let sphere_area = 4.0 * PI * radius * radius;
            let avg_element_size = (sphere_area / n as f64).sqrt();
            println!(
                "Subdivision {}: {} elements, avg size {:.4}m",
                s, n, avg_element_size
            );
            (s, mesh, avg_element_size)
        })
        .collect();

    println!();
    println!(
        "{:>6} | {:>8} | {:>12} | {:>12} | {:>10}",
        "ka", "Freq Hz", "Sub2 (320)", "Sub3 (1280)", "Best"
    );
    println!("{}", "-".repeat(65));

    for &ka in &ka_values {
        let freq = ka * speed_of_sound / (2.0 * PI * radius);
        let k = ka / radius;
        let wavelength = 2.0 * PI / k;

        // Mie reference
        let num_terms = (ka as usize + 30).max(50);
        let mie = sphere_scattering_3d(k, radius, num_terms, vec![radius], vec![0.0, PI / 2.0, PI]);

        if mie.pressure.iter().any(|p| !p.is_finite()) {
            println!("{:>6.1} | {:>8.0} | Mie error", ka, freq);
            continue;
        }

        let mie_avg =
            (mie.pressure[0].norm() + mie.pressure[1].norm() + mie.pressure[2].norm()) / 3.0;

        let mut errors: Vec<(usize, f64, f64)> = Vec::new(); // (subdivision, error%, epw)

        for (subdivision, mesh, avg_element_size) in &meshes {
            let n = mesh.elements.len();
            let epw = wavelength / avg_element_size; // elements per wavelength

            // Prepare elements
            let mut elements = mesh.elements.clone();
            for (i, elem) in elements.iter_mut().enumerate() {
                elem.boundary_condition =
                    BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]);
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

            // Physics with adaptive beta
            let physics = PhysicsParams::new(freq, speed_of_sound, density, false);
            let (beta, scale) = physics.burton_miller_beta_adaptive(radius);

            // Build system
            let system = build_tbem_system_scaled(&elements, &mesh.nodes, &physics, scale);

            // Incident field
            let incident = IncidentField::plane_wave_z();
            let p_inc = incident.evaluate_pressure(&centers, &physics);
            let dpdn_inc = incident.evaluate_normal_derivative(&centers, &normals, &physics);

            // RHS
            let mut rhs = Array1::<Complex64>::zeros(n);
            for i in 0..n {
                rhs[i] = p_inc[i] + beta * dpdn_inc[i];
            }

            // Solve
            let solution_x = lu_solve(&system.matrix, &rhs).expect("Solver failed");
            let bem_avg: f64 = solution_x.iter().map(|x| x.norm()).sum::<f64>() / n as f64;
            let error_pct = 100.0 * (bem_avg - mie_avg).abs() / mie_avg;

            errors.push((*subdivision, error_pct, epw));
        }

        // Find best
        let best = errors
            .iter()
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap();

        let sub2_err = errors
            .iter()
            .find(|e| e.0 == 2)
            .map(|e| e.1)
            .unwrap_or(f64::NAN);
        let sub3_err = errors
            .iter()
            .find(|e| e.0 == 3)
            .map(|e| e.1)
            .unwrap_or(f64::NAN);

        let sub2_str = format!("{:>5.1}%", sub2_err);
        let sub3_str = format!("{:>5.1}%", sub3_err);

        let best_str = if best.1 < 5.0 {
            format!("Sub{} ✓", best.0)
        } else if best.1 < 10.0 {
            format!("Sub{} ~", best.0)
        } else {
            format!("Sub{} ✗", best.0)
        };

        println!(
            "{:>6.1} | {:>8.0} | {:>12} | {:>12} | {:>10}",
            ka, freq, sub2_str, sub3_str, best_str
        );
    }

    println!();
    println!("=== Analysis ===");
    println!();
    println!("Elements per wavelength (EPW) at each subdivision:");
    for (subdivision, mesh, avg_element_size) in &meshes {
        println!(
            "  Subdivision {}: {} elements",
            subdivision,
            mesh.elements.len()
        );
        for &ka in &[0.5, 1.0, 2.0, 3.0] {
            let k = ka / radius;
            let wavelength = 2.0 * PI / k;
            let epw = wavelength / avg_element_size;
            println!("    ka={:.1}: {:.1} elements/wavelength", ka, epw);
        }
    }

    println!();
    println!("Recommendation:");
    println!("  NumCalc recommends 6-8 elements per wavelength.");
    println!("  Use subdivision 2 (320 elem) for ka < 1.0");
    println!("  Use subdivision 3 (1280 elem) for ka >= 1.0");
}

#[cfg(not(feature = "pure-rust"))]
fn main() {
    eprintln!("This example requires the 'pure-rust' feature.");
}
