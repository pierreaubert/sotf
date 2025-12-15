//! Row Sum Correction Test
//!
//! Compare BEM solution with and without row sum correction against Mie theory.

#[cfg(feature = "pure-rust")]
fn main() {
    use bem::analytical::sphere_scattering_3d;
    use bem::core::assembly::tbem::{apply_row_sum_correction, build_tbem_system};

    use bem::core::incident::IncidentField;
    use bem::core::mesh::generators::generate_icosphere_mesh;
    use bem::core::types::{BoundaryCondition, PhysicsParams};
    use ndarray::{Array1, Array2};
    use num_complex::Complex64;
    use std::f64::consts::PI;

    let radius = 0.1;
    let speed_of_sound = 343.0;
    let density = 1.21;
    let ka_target = 0.2;
    let frequency = ka_target * speed_of_sound / (2.0 * PI * radius);
    let k = 2.0 * PI * frequency / speed_of_sound;

    println!("=== Row Sum Correction Test ===\n");
    println!(
        "ka = {:.2}, k = {:.4}, radius = {}, frequency = {:.2} Hz",
        ka_target, k, radius, frequency
    );

    // Mie theory reference at surface
    let theta_samples: Vec<f64> = (0..=18).map(|i| i as f64 * 10.0 * PI / 180.0).collect();
    let mie = sphere_scattering_3d(k, radius, 50, vec![radius], theta_samples.clone());
    let mie_avg_magnitude: f64 =
        mie.pressure.iter().map(|p| p.norm()).sum::<f64>() / mie.pressure.len() as f64;
    println!(
        "\nMie theory: average |p_total| at surface = {:.4}",
        mie_avg_magnitude
    );

    // Test with different mesh resolutions
    for subdivisions in [1, 2] {
        let mesh = generate_icosphere_mesh(radius, subdivisions);
        let physics = PhysicsParams::new(frequency, speed_of_sound, density, false);
        let beta = physics.burton_miller_beta();

        println!(
            "\n--- Icosphere subdivisions={} ({} elements) ---",
            subdivisions,
            mesh.elements.len()
        );
        println!("    β = {:.4}+{:.4}i", beta.re, beta.im);

        // Prepare elements with velocity BC (rigid scatterer)
        let mut elements = mesh.elements.clone();
        for (i, elem) in elements.iter_mut().enumerate() {
            elem.boundary_condition = BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]);
            elem.dof_addresses = vec![i];
        }

        // Build system WITHOUT correction
        let system_uncorrected = build_tbem_system(&elements, &mesh.nodes, &physics);
        let n = system_uncorrected.num_dofs;

        // Compute row sums before correction
        let mut row_sums_before: Vec<Complex64> = Vec::new();
        for i in 0..n {
            let mut row_sum = Complex64::new(0.0, 0.0);
            for j in 0..n {
                row_sum += system_uncorrected.matrix[[i, j]];
            }
            row_sums_before.push(row_sum);
        }
        let avg_row_sum_before: Complex64 = row_sums_before.iter().sum::<Complex64>() / n as f64;
        println!(
            "    Row sum before correction: avg = {:.4}+{:.4}i (|.|={:.4})",
            avg_row_sum_before.re,
            avg_row_sum_before.im,
            avg_row_sum_before.norm()
        );

        // Build RHS from incident field
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

        // Solve uncorrected system
        let p_uncorrected = solve_system(&system_uncorrected.matrix, &rhs);
        let avg_p_uncorrected: f64 = p_uncorrected.iter().map(|p| p.norm()).sum::<f64>() / n as f64;
        let error_uncorrected =
            (avg_p_uncorrected - mie_avg_magnitude).abs() / mie_avg_magnitude * 100.0;

        println!("    Without correction:");
        println!(
            "      avg |p| = {:.4}, error vs Mie = {:.1}%",
            avg_p_uncorrected, error_uncorrected
        );

        // Build system WITH correction
        let mut system_corrected = build_tbem_system(&elements, &mesh.nodes, &physics);
        let correction_applied = apply_row_sum_correction(&mut system_corrected);

        // Verify row sums after correction
        let mut row_sums_after: Vec<Complex64> = Vec::new();
        for i in 0..n {
            let mut row_sum = Complex64::new(0.0, 0.0);
            for j in 0..n {
                row_sum += system_corrected.matrix[[i, j]];
            }
            row_sums_after.push(row_sum);
        }
        let avg_row_sum_after: Complex64 = row_sums_after.iter().sum::<Complex64>() / n as f64;
        println!(
            "    Row sum after correction: avg = {:.2e}+{:.2e}i (should be ~0)",
            avg_row_sum_after.re, avg_row_sum_after.im
        );

        // Solve corrected system
        let p_corrected = solve_system(&system_corrected.matrix, &rhs);
        let avg_p_corrected: f64 = p_corrected.iter().map(|p| p.norm()).sum::<f64>() / n as f64;
        let error_corrected =
            (avg_p_corrected - mie_avg_magnitude).abs() / mie_avg_magnitude * 100.0;

        println!("    With correction:");
        println!(
            "      avg |p| = {:.4}, error vs Mie = {:.1}%",
            avg_p_corrected, error_corrected
        );

        // Check diagonal change
        let diag_change: f64 = (0..n)
            .map(|i| (system_corrected.matrix[[i, i]] - system_uncorrected.matrix[[i, i]]).norm())
            .sum::<f64>()
            / n as f64;
        println!("    Avg diagonal change: {:.4}", diag_change);
    }

    println!("\n=== Summary ===");
    println!("Row sum correction enforces the theoretical property that");
    println!("A[1] = (c + K[1] + β*E[1]) = 0 for closed surfaces.");
    println!("This removes the bias caused by E[1] integration errors.");
}

/// Solve linear system using LU decomposition
#[cfg(feature = "pure-rust")]
fn solve_system(
    a: &ndarray::Array2<num_complex::Complex64>,
    b: &ndarray::Array1<num_complex::Complex64>,
) -> ndarray::Array1<num_complex::Complex64> {
    use ndarray_linalg::Solve;
    a.solve(b).expect("Failed to solve linear system")
}

#[cfg(not(feature = "pure-rust"))]
fn main() {
    eprintln!("This example requires the 'pure-rust' feature.");
}
