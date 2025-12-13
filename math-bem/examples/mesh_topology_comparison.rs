//! Mesh Topology Comparison: UV sphere vs Icosphere
//!
//! Tests whether UV sphere topology avoids the resonance issues
//! seen in icosphere at ka=0.7-0.8 and ka=2.0

#[cfg(feature = "pure-rust")]
fn main() {
    use bem::analytical::sphere_scattering_3d;
    use bem::core::assembly::tbem::build_tbem_system_scaled;
    use bem::core::incident::IncidentField;
    use bem::core::mesh::generators::{generate_icosphere_mesh, generate_sphere_mesh};
    use bem::core::solver::direct::lu_solve;
    use bem::core::types::{BoundaryCondition, Mesh, PhysicsParams};
    use ndarray::{Array1, Array2};
    use num_complex::Complex64;
    use std::f64::consts::PI;

    println!("=== Mesh Topology Comparison: UV vs Icosphere ===\n");

    let radius = 0.1;
    let speed_of_sound = 343.0;
    let density = 1.21;

    // Focus on problematic ka values
    let ka_values: Vec<f64> = vec![0.5, 0.7, 0.8, 0.9, 1.0, 2.0, 3.0];

    // Generate comparable meshes (~300-400 elements each)
    let ico_mesh = generate_icosphere_mesh(radius, 2); // 320 elements
    let uv_mesh = generate_sphere_mesh(radius, 12, 24); // ~576 elements (close enough)

    println!("Icosphere: {} elements", ico_mesh.elements.len());
    println!("UV sphere: {} elements\n", uv_mesh.elements.len());

    println!(
        "{:>6} | {:>12} | {:>12} | {:>10}",
        "ka", "Icosphere", "UV Sphere", "Better"
    );
    println!("{}", "-".repeat(50));

    fn solve_for_mesh(mesh: &Mesh, ka: f64, radius: f64, speed_of_sound: f64, density: f64) -> f64 {
        let freq = ka * speed_of_sound / (2.0 * PI * radius);
        let k = ka / radius;
        let n = mesh.elements.len();

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

        // Physics with adaptive beta
        let physics = PhysicsParams::new(freq, speed_of_sound, density, false);
        let (beta, scale) = physics.burton_miller_beta_adaptive(radius);

        // Build and solve
        let system = build_tbem_system_scaled(&elements, &mesh.nodes, &physics, scale);
        let incident = IncidentField::plane_wave_z();
        let p_inc = incident.evaluate_pressure(&centers, &physics);
        let dpdn_inc = incident.evaluate_normal_derivative(&centers, &normals, &physics);

        let mut rhs = Array1::<Complex64>::zeros(n);
        for i in 0..n {
            rhs[i] = p_inc[i] + beta * dpdn_inc[i];
        }

        let solution_x = lu_solve(&system.matrix, &rhs).expect("Solver failed");
        let bem_avg: f64 = solution_x.iter().map(|x| x.norm()).sum::<f64>() / n as f64;

        // Mie reference
        let num_terms = (ka as usize + 30).max(50);
        let mie = sphere_scattering_3d(k, radius, num_terms, vec![radius], vec![0.0, PI / 2.0, PI]);
        let mie_avg =
            (mie.pressure[0].norm() + mie.pressure[1].norm() + mie.pressure[2].norm()) / 3.0;

        100.0 * (bem_avg - mie_avg).abs() / mie_avg
    }

    for &ka in &ka_values {
        let ico_err = solve_for_mesh(&ico_mesh, ka, radius, speed_of_sound, density);
        let uv_err = solve_for_mesh(&uv_mesh, ka, radius, speed_of_sound, density);

        let better = if ico_err < uv_err { "Ico" } else { "UV" };
        let marker = if ico_err.min(uv_err) < 5.0 {
            "✓"
        } else if ico_err.min(uv_err) < 10.0 {
            "~"
        } else {
            "✗"
        };

        println!(
            "{:>6.1} | {:>10.1}% | {:>10.1}% | {:>6} {}",
            ka, ico_err, uv_err, better, marker
        );
    }

    println!("\n=== Conclusion ===");
    println!("If UV sphere performs better at ka=0.7-0.8 and ka=2.0,");
    println!("the icosphere resonance is a mesh topology issue.");
}

#[cfg(not(feature = "pure-rust"))]
fn main() {
    eprintln!("This example requires the 'pure-rust' feature.");
}
