//! Residual Check - Verify solver is working correctly
//!
//! Checks ||Ap - b|| to ensure the linear system is being solved correctly.
//! If residual is small but solution is wrong, the issue is in formulation or RHS.

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

    println!("=== Residual Check ===");
    println!("Verifying ||Ap - b|| to ensure solver works correctly\n");

    let radius = 0.1;
    let speed_of_sound = 343.0;
    let density = 1.21;
    let ka_target = 1.0;
    let frequency = ka_target * speed_of_sound / (2.0 * PI * radius);
    let k = 2.0 * PI * frequency / speed_of_sound;

    let physics = PhysicsParams::new(frequency, speed_of_sound, density, false);
    let beta = physics.burton_miller_beta_scaled(2.0);

    let mesh = generate_icosphere_mesh(radius, 2);
    let n = mesh.elements.len();

    let mut elements = mesh.elements.clone();
    for (i, elem) in elements.iter_mut().enumerate() {
        elem.boundary_condition = BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]);
        elem.dof_addresses = vec![i];
    }

    // Build system
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

    let mut rhs = Array1::<Complex64>::zeros(n);
    for i in 0..n {
        rhs[i] = p_inc[i] + beta * dpdn_inc[i];
    }

    // Solve
    let solution_x = lu_solve(&system.matrix, &rhs).expect("Solver failed");

    // Verify residual
    let ax = system.matrix.dot(&solution_x);

    // Compute residual Ap - b
    let mut residual = Array1::<Complex64>::zeros(n);
    for i in 0..n {
        let mut ax_i = Complex64::new(0.0, 0.0);
        for j in 0..n {
            ax_i += system.matrix[[i, j]] * solution_x[j];
        }
        residual[i] = ax_i - rhs[i];
    }

    // Compute norms
    let residual_norm: f64 = residual.iter().map(|x| x.norm()).sum::<f64>() / n as f64;
    let rhs_norm: f64 = rhs.iter().map(|x| x.norm()).sum::<f64>() / n as f64;
    let solution_norm: f64 = solution_x.iter().map(|x| x.norm()).sum::<f64>() / n as f64;

    println!("ka = {:.2}, {} elements, β = {:.4}i", ka_target, n, beta.im);
    println!();
    println!("||b|| / n = {:.6} (average RHS magnitude)", rhs_norm);
    println!(
        "||x|| / n = {:.6} (average solution magnitude)",
        solution_norm
    );
    println!("||Ax - b|| / n = {:.10} (average residual)", residual_norm);
    println!("Relative residual = {:.2e}", residual_norm / rhs_norm);

    // Check some individual elements
    println!("\nSample residuals:");
    for i in [0, n / 4, n / 2, 3 * n / 4, n - 1] {
        println!(
            "  elem[{}]: residual = {:.2e} + {:.2e}i, |rhs| = {:.4}, |x| = {:.4}",
            i,
            residual[i].re,
            residual[i].im,
            rhs[i].norm(),
            solution_x[i].norm()
        );
    }

    // Mie reference
    let theta_refs: Vec<f64> = (0..=18).map(|i| i as f64 * 10.0 * PI / 180.0).collect();
    let mie = sphere_scattering_3d(k, radius, 50, vec![radius], theta_refs.clone());
    let mie_avg: f64 =
        mie.pressure.iter().map(|p| p.norm()).sum::<f64>() / mie.pressure.len() as f64;

    println!("\n=== Comparison with Mie ===");
    println!("Mie average |p| = {:.4}", mie_avg);
    println!("BEM average |p| = {:.4}", solution_norm);
    println!(
        "Error = {:.1}%",
        100.0 * (solution_norm - mie_avg).abs() / mie_avg
    );

    // Check front/back
    let mut front_bem: Vec<f64> = Vec::new();
    let mut back_bem: Vec<f64> = Vec::new();
    for (i, elem) in elements.iter().enumerate() {
        let z = elem.center[2];
        if z > 0.05 * radius {
            front_bem.push(solution_x[i].norm());
        } else if z < -0.05 * radius {
            back_bem.push(solution_x[i].norm());
        }
    }
    let front_avg = front_bem.iter().sum::<f64>() / front_bem.len() as f64;
    let back_avg = back_bem.iter().sum::<f64>() / back_bem.len() as f64;

    println!(
        "\nFront (z > 0): BEM = {:.4}, Mie @ 0° = {:.4}, error = {:.1}%",
        front_avg,
        mie.pressure[0].norm(),
        100.0 * (front_avg - mie.pressure[0].norm()).abs() / mie.pressure[0].norm()
    );
    println!(
        "Back (z < 0): BEM = {:.4}, Mie @ 180° = {:.4}, error = {:.1}%",
        back_avg,
        mie.pressure[18].norm(),
        100.0 * (back_avg - mie.pressure[18].norm()).abs() / mie.pressure[18].norm()
    );

    // Verify the incident field RHS values
    println!("\n=== Incident Field Values ===");
    println!("Front element sample:");
    for (i, elem) in elements.iter().enumerate() {
        if elem.center[2] > 0.08 {
            println!(
                "  elem[{}]: z = {:.4}, |p_inc| = {:.4}, |dpdn| = {:.4}",
                i,
                elem.center[2],
                p_inc[i].norm(),
                dpdn_inc[i].norm()
            );
            println!("           |rhs| = {:.4} = |p_inc + β*dpdn|", rhs[i].norm());
            break;
        }
    }

    println!("\nBack element sample:");
    for (i, elem) in elements.iter().enumerate() {
        if elem.center[2] < -0.08 {
            println!(
                "  elem[{}]: z = {:.4}, |p_inc| = {:.4}, |dpdn| = {:.4}",
                i,
                elem.center[2],
                p_inc[i].norm(),
                dpdn_inc[i].norm()
            );
            println!("           |rhs| = {:.4} = |p_inc + β*dpdn|", rhs[i].norm());
            break;
        }
    }

    if residual_norm / rhs_norm < 1e-8 {
        println!("\n✓ Solver is working correctly (relative residual < 1e-8)");
        println!("  The issue is in the BEM formulation or RHS computation.");
    } else {
        println!(
            "\n✗ Solver may have issues (relative residual = {:.2e})",
            residual_norm / rhs_norm
        );
    }
}

#[cfg(not(feature = "pure-rust"))]
fn main() {
    eprintln!("This example requires the 'pure-rust' feature.");
}
