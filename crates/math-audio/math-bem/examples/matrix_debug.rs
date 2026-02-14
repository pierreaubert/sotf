//! Matrix Debug - Compare system matrices for different mesh types
//!
//! Checks if the BEM matrix assembly produces similar results for different meshes.

#[cfg(feature = "pure-rust")]
fn main() {
    use math_audio_bem::core::assembly::tbem::build_tbem_system;
    use math_audio_bem::core::incident::IncidentField;
    use math_audio_bem::core::mesh::generators::{generate_icosphere_mesh, generate_sphere_mesh};
    use math_audio_bem::core::solver::direct::lu_solve;
    use math_audio_bem::core::types::{BoundaryCondition, PhysicsParams};
    use ndarray::Array2;
    use num_complex::Complex64;
    use std::f64::consts::PI;

    let radius = 0.1;
    let speed_of_sound = 343.0;
    let density = 1.21;
    let ka_target = 0.2;
    let frequency = ka_target * speed_of_sound / (2.0 * PI * radius);

    println!("=== Matrix Debug: ka = {} ===\n", ka_target);

    // UV-sphere
    println!("--- UV-sphere (8, 16) ---");
    let mesh_uv = generate_sphere_mesh(radius, 8, 16);
    analyze_system(&mesh_uv, frequency, speed_of_sound, density);

    // Icosphere
    println!("\n--- Icosphere (subdivisions=2) ---");
    let mesh_ico = generate_icosphere_mesh(radius, 2);
    analyze_system(&mesh_ico, frequency, speed_of_sound, density);
}

#[cfg(feature = "pure-rust")]
fn analyze_system(
    mesh: &math_audio_bem::core::types::Mesh,
    frequency: f64,
    speed_of_sound: f64,
    density: f64,
) {
    use math_audio_bem::core::assembly::tbem::build_tbem_system;
    use math_audio_bem::core::incident::IncidentField;
    use math_audio_bem::core::solver::direct::lu_solve;
    use math_audio_bem::core::types::{BoundaryCondition, PhysicsParams};
    use ndarray::{Array1, Array2};
    use num_complex::Complex64;

    let physics = PhysicsParams::new(frequency, speed_of_sound, density, false);
    let beta = physics.burton_miller_beta();

    // Prepare elements
    let mut elements = mesh.elements.clone();
    for (i, elem) in elements.iter_mut().enumerate() {
        elem.boundary_condition = BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]);
        elem.dof_addresses = vec![i];
    }

    // Build system
    let system = build_tbem_system(&elements, &mesh.nodes, &physics);

    // Matrix statistics
    let n = system.num_dofs;
    let mut diag_sum = Complex64::new(0.0, 0.0);
    let mut off_diag_sum = Complex64::new(0.0, 0.0);

    for i in 0..n {
        diag_sum += system.matrix[[i, i]];
        for j in 0..n {
            if i != j {
                off_diag_sum += system.matrix[[i, j]];
            }
        }
    }

    let diag_avg = diag_sum / n as f64;
    let off_diag_avg = off_diag_sum / ((n * (n - 1)) as f64);

    println!("  {} DOFs", n);
    println!("  β = {:.4} + {:.4}i", beta.re, beta.im);
    println!(
        "  Diagonal avg: {:.4} + {:.4}i (|.| = {:.4})",
        diag_avg.re,
        diag_avg.im,
        diag_avg.norm()
    );
    println!(
        "  Off-diagonal avg: {:.6} + {:.6}i (|.| = {:.6})",
        off_diag_avg.re,
        off_diag_avg.im,
        off_diag_avg.norm()
    );

    // First few diagonal entries
    println!("  First 3 diagonal entries:");
    for i in 0..3.min(n) {
        let d = system.matrix[[i, i]];
        println!("    A[{},{}] = {:.4} + {:.4}i", i, i, d.re, d.im);
    }

    // Incident field RHS
    let incident = IncidentField::plane_wave_z();
    let mut centers = Array2::zeros((n, 3));
    let mut normals = Array2::zeros((n, 3));
    for (i, elem) in elements.iter().enumerate() {
        for j in 0..3 {
            centers[[i, j]] = elem.center[j];
            normals[[i, j]] = elem.normal[j];
        }
    }

    let incident_rhs = incident.compute_rhs(&centers, &normals, &physics, true);

    println!("\n  Incident RHS (first 3):");
    for i in 0..3.min(n) {
        let r = incident_rhs[i];
        println!("    rhs[{}] = {:.4} + {:.4}i", i, r.re, r.im);
    }

    let total_rhs = &system.rhs + &incident_rhs;

    println!("  Total RHS (first 3):");
    for i in 0..3.min(n) {
        let r = total_rhs[i];
        println!("    rhs[{}] = {:.4} + {:.4}i", i, r.re, r.im);
    }

    // Solve
    let solution_x = lu_solve(&system.matrix, &total_rhs).expect("Solver failed");

    // Verify solution
    let ax = system.matrix.dot(&solution_x);

    println!("\n  Solution (first 3):");
    for i in 0..3.min(n) {
        let p = solution_x[i];
        println!(
            "    p[{}] = {:.4} + {:.4}i (|.| = {:.4})",
            i,
            p.re,
            p.im,
            p.norm()
        );
    }

    // Compute residual
    let mut residual = Array1::<Complex64>::zeros(n);
    for i in 0..n {
        residual[i] = total_rhs[i];
        for j in 0..n {
            residual[i] -= system.matrix[[i, j]] * solution_x[j];
        }
    }
    let residual_norm: f64 = residual.iter().map(|r| r.norm_sqr()).sum::<f64>().sqrt();
    let rhs_norm: f64 = total_rhs.iter().map(|r| r.norm_sqr()).sum::<f64>().sqrt();

    println!(
        "\n  Residual ||Ax-b||/||b|| = {:.2e}",
        residual_norm / rhs_norm
    );

    // Solution statistics
    let p_avg: f64 = solution_x.iter().map(|x| x.norm()).sum::<f64>() / n as f64;
    let p_max = solution_x.iter().map(|x| x.norm()).fold(0.0f64, f64::max);
    println!("  Solution |p|: avg={:.4}, max={:.4}", p_avg, p_max);

    // Row sums (total coupling from all elements to each collocation point)
    println!("\n  Row sums (first 5):");
    for i in 0..5.min(n) {
        let mut row_sum = Complex64::new(0.0, 0.0);
        let mut row_sum_off_diag = Complex64::new(0.0, 0.0);
        for j in 0..n {
            row_sum += system.matrix[[i, j]];
            if i != j {
                row_sum_off_diag += system.matrix[[i, j]];
            }
        }
        println!(
            "    Row {}: total={:.4}+{:.4}i (|.|={:.4}), off-diag={:.4}+{:.4}i (|.|={:.4})",
            i,
            row_sum.re,
            row_sum.im,
            row_sum.norm(),
            row_sum_off_diag.re,
            row_sum_off_diag.im,
            row_sum_off_diag.norm()
        );
    }

    // Average row sum of off-diagonal elements
    let mut total_off_diag_row_sum = Complex64::new(0.0, 0.0);
    for i in 0..n {
        for j in 0..n {
            if i != j {
                total_off_diag_row_sum += system.matrix[[i, j]];
            }
        }
    }
    let avg_off_diag_row_sum = total_off_diag_row_sum / n as f64;
    println!(
        "  Avg off-diagonal row sum: {:.4}+{:.4}i (|.|={:.4})",
        avg_off_diag_row_sum.re,
        avg_off_diag_row_sum.im,
        avg_off_diag_row_sum.norm()
    );
}

#[cfg(not(feature = "pure-rust"))]
fn main() {
    eprintln!("This example requires the 'pure-rust' feature.");
}
