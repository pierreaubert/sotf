//! Mesh Refinement Test - Check convergence with finer meshes
//!
//! Tests if BEM accuracy improves with mesh refinement

#[cfg(feature = "pure-rust")]
fn main() {
    use math_audio_bem::analytical::sphere_scattering_3d;
    use math_audio_bem::core::assembly::tbem::build_tbem_system_scaled;
    use math_audio_bem::core::incident::IncidentField;
    use math_audio_bem::core::mesh::generators::generate_icosphere_mesh;
    use math_audio_bem::core::solver::direct::lu_solve;
    use math_audio_bem::core::types::{BoundaryCondition, PhysicsParams};
    use ndarray::Array2;
    use num_complex::Complex64;
    use std::f64::consts::PI;

    println!("=== Mesh Refinement Test ===");
    println!("Testing convergence with scaled β = 2i/k\n");

    let radius = 0.1;
    let speed_of_sound = 343.0;
    let density = 1.21;
    let ka_target = 1.0;
    let frequency = ka_target * speed_of_sound / (2.0 * PI * radius);
    let k = 2.0 * PI * frequency / speed_of_sound;

    let physics = PhysicsParams::new(frequency, speed_of_sound, density, false);
    let beta = physics.burton_miller_beta_scaled(2.0);

    // Mie reference
    let theta_refs: Vec<f64> = (0..=18).map(|i| i as f64 * 10.0 * PI / 180.0).collect();
    let mie = sphere_scattering_3d(k, radius, 50, vec![radius], theta_refs.clone());
    let mie_front = mie.pressure[0].norm();
    let mie_side = mie.pressure[9].norm(); // 90 degrees
    let mie_back = mie.pressure[18].norm();

    println!("ka = {:.2}, Mie reference:", ka_target);
    println!("  Front (θ=0°):   |p| = {:.4}", mie_front);
    println!("  Side (θ=90°):   |p| = {:.4}", mie_side);
    println!("  Back (θ=180°):  |p| = {:.4}", mie_back);
    println!();

    println!(
        "{:>12} | {:>6} | {:>10} | {:>10} | {:>10} | {:>10}",
        "Subdivisions", "Elems", "Front err%", "Side err%", "Back err%", "Avg err%"
    );
    println!("{}", "-".repeat(75));

    // Test different subdivision levels
    for subdivisions in [1, 2, 3, 4] {
        let mesh = generate_icosphere_mesh(radius, subdivisions);
        let n = mesh.elements.len();

        // Prepare elements
        let mut elements = mesh.elements.clone();
        for (i, elem) in elements.iter_mut().enumerate() {
            elem.boundary_condition = BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]);
            elem.dof_addresses = vec![i];
        }

        // Build system with scaled beta
        let system = build_tbem_system_scaled(&elements, &mesh.nodes, &physics, 2.0);

        // Compute RHS
        let incident = IncidentField::plane_wave_z();
        let mut centers = Array2::zeros((n, 3));
        let mut normals = Array2::zeros((n, 3));
        for (i, elem) in elements.iter().enumerate() {
            for j in 0..3 {
                centers[[i, j]] = elem.center[j];
                normals[[i, j]] = elem.normal[j];
            }
        }
        let p_inc = incident.evaluate_pressure(&centers, &physics);
        let dpdn_inc = incident.evaluate_normal_derivative(&centers, &normals, &physics);

        let mut rhs = ndarray::Array1::<Complex64>::zeros(n);
        for i in 0..n {
            rhs[i] = p_inc[i] + beta * dpdn_inc[i];
        }

        // Solve
        let solution_x = lu_solve(&system.matrix, &rhs).expect("Solver failed");
        let bem_avg: f64 = solution_x.iter().map(|x| x.norm()).sum::<f64>() / n as f64;

        // Find elements near key angles
        let mut front_p: Vec<f64> = Vec::new();
        let mut side_p: Vec<f64> = Vec::new();
        let mut back_p: Vec<f64> = Vec::new();

        for (i, elem) in elements.iter().enumerate() {
            let z = elem.center[2];
            let cos_theta = z / radius;

            // Front: cos(theta) > 0.9 (within ~25 degrees of front)
            if cos_theta > 0.9 {
                front_p.push(solution_x[i].norm());
            }
            // Side: |cos(theta)| < 0.2 (within ~10 degrees of equator)
            else if cos_theta.abs() < 0.2 {
                side_p.push(solution_x[i].norm());
            }
            // Back: cos(theta) < -0.9 (within ~25 degrees of back)
            else if cos_theta < -0.9 {
                back_p.push(solution_x[i].norm());
            }
        }

        let bem_front = front_p.iter().sum::<f64>() / front_p.len() as f64;
        let bem_side = side_p.iter().sum::<f64>() / side_p.len() as f64;
        let bem_back = back_p.iter().sum::<f64>() / back_p.len() as f64;

        let err_front = 100.0 * (bem_front - mie_front).abs() / mie_front;
        let err_side = 100.0 * (bem_side - mie_side).abs() / mie_side;
        let err_back = 100.0 * (bem_back - mie_back).abs() / mie_back;
        let err_avg = (err_front + err_side + err_back) / 3.0;

        println!(
            "{:>12} | {:>6} | {:>10.1}% | {:>10.1}% | {:>10.1}% | {:>10.1}%",
            subdivisions, n, err_front, err_side, err_back, err_avg
        );
    }

    // Also test at ka = 0.5 where resolution should be adequate
    println!("\n\n=== Testing at ka = 0.5 (better resolution) ===\n");

    let ka_target = 0.5;
    let frequency = ka_target * speed_of_sound / (2.0 * PI * radius);
    let k = 2.0 * PI * frequency / speed_of_sound;

    let physics = PhysicsParams::new(frequency, speed_of_sound, density, false);
    let beta = physics.burton_miller_beta_scaled(2.0);

    let mie = sphere_scattering_3d(k, radius, 50, vec![radius], theta_refs.clone());
    let mie_front = mie.pressure[0].norm();
    let mie_side = mie.pressure[9].norm();
    let mie_back = mie.pressure[18].norm();

    println!("ka = {:.2}, Mie reference:", ka_target);
    println!("  Front (θ=0°):   |p| = {:.4}", mie_front);
    println!("  Side (θ=90°):   |p| = {:.4}", mie_side);
    println!("  Back (θ=180°):  |p| = {:.4}", mie_back);
    println!();

    println!(
        "{:>12} | {:>6} | {:>10} | {:>10} | {:>10} | {:>10}",
        "Subdivisions", "Elems", "Front err%", "Side err%", "Back err%", "Avg err%"
    );
    println!("{}", "-".repeat(75));

    for subdivisions in [1, 2, 3, 4] {
        let mesh = generate_icosphere_mesh(radius, subdivisions);
        let n = mesh.elements.len();

        let mut elements = mesh.elements.clone();
        for (i, elem) in elements.iter_mut().enumerate() {
            elem.boundary_condition = BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]);
            elem.dof_addresses = vec![i];
        }

        let system = build_tbem_system_scaled(&elements, &mesh.nodes, &physics, 2.0);

        let incident = IncidentField::plane_wave_z();
        let mut centers = Array2::zeros((n, 3));
        let mut normals = Array2::zeros((n, 3));
        for (i, elem) in elements.iter().enumerate() {
            for j in 0..3 {
                centers[[i, j]] = elem.center[j];
                normals[[i, j]] = elem.normal[j];
            }
        }
        let p_inc = incident.evaluate_pressure(&centers, &physics);
        let dpdn_inc = incident.evaluate_normal_derivative(&centers, &normals, &physics);

        let mut rhs = ndarray::Array1::<Complex64>::zeros(n);
        for i in 0..n {
            rhs[i] = p_inc[i] + beta * dpdn_inc[i];
        }

        let solution_x = lu_solve(&system.matrix, &rhs).expect("Solver failed");

        let avg_p: f64 = solution_x.iter().map(|x| x.norm()).sum::<f64>() / n as f64;

        let mut front_p: Vec<f64> = Vec::new();
        let mut side_p: Vec<f64> = Vec::new();
        let mut back_p: Vec<f64> = Vec::new();

        for (i, elem) in elements.iter().enumerate() {
            let z = elem.center[2];
            let cos_theta = z / radius;

            if cos_theta > 0.9 {
                front_p.push(solution_x[i].norm());
            } else if cos_theta.abs() < 0.2 {
                side_p.push(solution_x[i].norm());
            } else if cos_theta < -0.9 {
                back_p.push(solution_x[i].norm());
            }
        }

        let bem_front = front_p.iter().sum::<f64>() / front_p.len() as f64;
        let bem_side = side_p.iter().sum::<f64>() / side_p.len() as f64;
        let bem_back = back_p.iter().sum::<f64>() / back_p.len() as f64;

        let err_front = 100.0 * (bem_front - mie_front).abs() / mie_front;
        let err_side = 100.0 * (bem_side - mie_side).abs() / mie_side;
        let err_back = 100.0 * (bem_back - mie_back).abs() / mie_back;
        let err_avg = (err_front + err_side + err_back) / 3.0;

        println!(
            "{:>12} | {:>6} | {:>10.1}% | {:>10.1}% | {:>10.1}% | {:>10.1}%",
            subdivisions, n, err_front, err_side, err_back, err_avg
        );
    }
}

#[cfg(not(feature = "pure-rust"))]
fn main() {
    eprintln!("This example requires the 'pure-rust' feature.");
}
