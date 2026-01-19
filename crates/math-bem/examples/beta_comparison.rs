//! Beta Comparison - Find optimal β scaling for different ka
//!
//! Tests different β scalings to find what works best at different frequencies

#[cfg(feature = "pure-rust")]
fn main() {
    use math_audio_bem::analytical::sphere_scattering_3d;
    use math_audio_bem::core::assembly::tbem::build_tbem_system_with_beta;
    use math_audio_bem::core::incident::IncidentField;
    use math_audio_bem::core::mesh::generators::generate_icosphere_mesh;
    use math_audio_bem::core::solver::direct::lu_solve;
    use math_audio_bem::core::types::{BoundaryCondition, PhysicsParams};
    use ndarray::{Array1, Array2};
    use num_complex::Complex64;
    use std::f64::consts::PI;

    println!("=== Beta Scaling Comparison ===\n");

    let radius = 0.1;
    let speed_of_sound = 343.0;
    let density = 1.21;

    let mesh = generate_icosphere_mesh(radius, 2);
    let n = mesh.elements.len();

    println!("Testing with {} elements\n", n);

    for &ka_target in &[0.3, 0.5, 1.0, 2.0] {
        let frequency = ka_target * speed_of_sound / (2.0 * PI * radius);
        let k = 2.0 * PI * frequency / speed_of_sound;

        let physics = PhysicsParams::new(frequency, speed_of_sound, density, false);

        // Mie reference
        let mie = sphere_scattering_3d(k, radius, 50, vec![radius], vec![0.0, PI / 2.0, PI]);
        let mie_avg =
            (mie.pressure[0].norm() + mie.pressure[1].norm() + mie.pressure[2].norm()) / 3.0;

        println!("=== ka = {:.1} === (Mie avg = {:.4})", ka_target, mie_avg);
        println!(
            "{:>10} | {:>10} | {:>10} | {:>10}",
            "β scale", "β value", "BEM avg", "Error%"
        );
        println!("{}", "-".repeat(50));

        for &scale in &[0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 5.0] {
            let beta = physics.burton_miller_beta_scaled(scale);

            let mut elements = mesh.elements.clone();
            for (i, elem) in elements.iter_mut().enumerate() {
                elem.boundary_condition =
                    BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]);
                elem.dof_addresses = vec![i];
            }

            let system = build_tbem_system_with_beta(&elements, &mesh.nodes, &physics, beta);

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

            let solution_x = lu_solve(&system.matrix, &rhs).expect("Linear solver failed");

            let avg_p: f64 = solution_x.iter().map(|p| p.norm()).sum::<f64>() / n as f64;
            let error = 100.0 * (avg_p - mie_avg).abs() / mie_avg;

            let marker = if error < 20.0 {
                " *"
            } else if error < 50.0 {
                " ."
            } else {
                ""
            };
            println!(
                "{:>10.1} | {:>10.4}i | {:>10.4} | {:>10.1}{}",
                scale, beta.im, avg_p, error, marker
            );
        }
        println!();
    }

    println!("Legend: * = error < 20%, . = error < 50%");
}

#[cfg(not(feature = "pure-rust"))]
fn main() {
    eprintln!("This example requires the 'pure-rust' feature.");
}
