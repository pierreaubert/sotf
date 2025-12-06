//! Bounded Beta Test
//!
//! Test if using a bounded Burton-Miller β improves solution accuracy.
//! The standard β = i/k can cause ill-conditioning at low frequencies because
//! E[1] errors get amplified. Using β = i/(k + k_ref) where k_ref ~ 1/element_size
//! reduces the β magnitude and thus the E[1] impact.

#[cfg(feature = "pure-rust")]
fn main() {
    use bem::analytical::sphere_scattering_3d;
    use bem::core::assembly::tbem::build_tbem_system_with_beta;
    use bem::core::incident::IncidentField;
    use bem::core::mesh::generators::generate_icosphere_mesh;
    use bem::core::types::{BoundaryCondition, PhysicsParams};
    use ndarray::{Array1, Array2};
    use num_complex::Complex64;
    use std::f64::consts::PI;

    let radius = 0.1;
    let speed_of_sound = 343.0;
    let density = 1.21;

    println!("=== Bounded Beta Test ===\n");

    for ka_target in [0.2, 0.5, 1.0] {
        let frequency = ka_target * speed_of_sound / (2.0 * PI * radius);
        let k = 2.0 * PI * frequency / speed_of_sound;

        println!("=== ka = {:.2} ===\n", ka_target);

        // Mie reference
        let mie = sphere_scattering_3d(k, radius, 50, vec![radius], vec![0.0, PI]);
        let mie_avg = (mie.pressure[0].norm() + mie.pressure[1].norm()) / 2.0;

        let mesh = generate_icosphere_mesh(radius, 2);
        let physics = PhysicsParams::new(frequency, speed_of_sound, density, false);
        let avg_elem_size = mesh.average_element_area().sqrt();

        // Prepare elements
        let mut elements = mesh.elements.clone();
        for (i, elem) in elements.iter_mut().enumerate() {
            elem.boundary_condition = BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]);
            elem.dof_addresses = vec![i];
        }

        let n = elements.len();

        // Build RHS
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

        println!(
            "Testing different β values (element size = {:.4})",
            avg_elem_size
        );
        println!("Standard β = i/k = i*{:.4}", 1.0 / k);
        println!("Mie reference: avg |p| = {:.4}\n", mie_avg);

        // Test different beta scalings
        let beta_factors = [0.1, 0.2, 0.5, 1.0, 2.0];

        for &factor in &beta_factors {
            // Scale β relative to standard value
            let eta = factor / k;
            let beta = Complex64::new(0.0, eta);

            // Build RHS with this beta
            let mut rhs = Array1::<Complex64>::zeros(n);
            for i in 0..n {
                rhs[i] = p_inc[i] + beta * dpdn_inc[i];
            }

            // Build system
            let system = build_tbem_system_with_beta(&elements, &mesh.nodes, &physics, beta);

            // Solve
            let p = solve_system(&system.matrix, &rhs);
            let avg_p: f64 = p.iter().map(|x| x.norm()).sum::<f64>() / n as f64;
            let error = (avg_p - mie_avg).abs() / mie_avg * 100.0;

            // Check row sum
            let mut avg_row_sum = Complex64::new(0.0, 0.0);
            for i in 0..n {
                let mut row_sum = Complex64::new(0.0, 0.0);
                for j in 0..n {
                    row_sum += system.matrix[[i, j]];
                }
                avg_row_sum += row_sum;
            }
            avg_row_sum /= n as f64;

            println!(
                "  β = {:.2}×(i/k): η = {:.4}, |p| = {:.4}, error = {:.1}%, row_sum = {:.4}+{:.4}i",
                factor, eta, avg_p, error, avg_row_sum.re, avg_row_sum.im
            );
        }

        // Also test optimal bounded beta
        let k_ref = 1.0 / avg_elem_size;
        let eta_bounded = 1.0 / (k + k_ref);
        let beta_bounded = Complex64::new(0.0, eta_bounded);

        let mut rhs_bounded = Array1::<Complex64>::zeros(n);
        for i in 0..n {
            rhs_bounded[i] = p_inc[i] + beta_bounded * dpdn_inc[i];
        }

        let system_bounded =
            build_tbem_system_with_beta(&elements, &mesh.nodes, &physics, beta_bounded);
        let p_bounded = solve_system(&system_bounded.matrix, &rhs_bounded);
        let avg_p_bounded: f64 = p_bounded.iter().map(|x| x.norm()).sum::<f64>() / n as f64;
        let error_bounded = (avg_p_bounded - mie_avg).abs() / mie_avg * 100.0;

        let mut avg_row_sum_bounded = Complex64::new(0.0, 0.0);
        for i in 0..n {
            let mut row_sum = Complex64::new(0.0, 0.0);
            for j in 0..n {
                row_sum += system_bounded.matrix[[i, j]];
            }
            avg_row_sum_bounded += row_sum;
        }
        avg_row_sum_bounded /= n as f64;

        println!(
            "\n  Bounded β = i/(k+1/h): η = {:.4}, |p| = {:.4}, error = {:.1}%, row_sum = {:.4}+{:.4}i",
            eta_bounded,
            avg_p_bounded,
            error_bounded,
            avg_row_sum_bounded.re,
            avg_row_sum_bounded.im
        );
        println!();
    }
}

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
