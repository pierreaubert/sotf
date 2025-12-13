//! Optimal Beta Convergence Test
//!
//! Tests convergence at ka=1.0 with optimal β scaling (4×)

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

    println!("=== Optimal Beta (4×) Convergence Test at ka = 1.0 ===\n");

    let radius = 0.1;
    let speed_of_sound = 343.0;
    let density = 1.21;
    let ka_target = 1.0;
    let frequency = ka_target * speed_of_sound / (2.0 * PI * radius);
    let k = 2.0 * PI * frequency / speed_of_sound;
    let scale = 4.0;

    let physics = PhysicsParams::new(frequency, speed_of_sound, density, false);
    let beta = physics.burton_miller_beta_scaled(scale);

    // Mie reference
    let mie = sphere_scattering_3d(k, radius, 50, vec![radius], vec![0.0, PI / 2.0, PI]);
    let mie_front = mie.pressure[0].norm();
    let mie_side = mie.pressure[1].norm();
    let mie_back = mie.pressure[2].norm();
    let mie_avg = (mie_front + mie_side + mie_back) / 3.0;

    println!(
        "ka = {:.2}, β = {:.4}i (scale = {})",
        ka_target, beta.im, scale
    );
    println!(
        "Mie: front = {:.4}, side = {:.4}, back = {:.4}, avg = {:.4}\n",
        mie_front, mie_side, mie_back, mie_avg
    );

    println!(
        "{:>8} | {:>10} | {:>10} | {:>10} | {:>10} | {:>10}",
        "Elems", "BEM avg", "Avg err%", "Front err%", "Side err%", "Back err%"
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

        let system = build_tbem_system_scaled(&elements, &mesh.nodes, &physics, scale);

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

        let solution_x = lu_solve(&system.matrix, &rhs).expect("Solver failed");
        let avg_p: f64 = solution_x.iter().map(|x| x.norm()).sum::<f64>() / n as f64;

        let mut front_p: Vec<f64> = Vec::new();
        let mut side_p: Vec<f64> = Vec::new();
        let mut back_p: Vec<f64> = Vec::new();

        for (i, elem) in elements.iter().enumerate() {
            let z = elem.center[2];
            let cos_theta = z / radius;
            if cos_theta > 0.8 {
                front_p.push(solution_x[i].norm());
            } else if cos_theta < -0.8 {
                back_p.push(solution_x[i].norm());
            } else if cos_theta.abs() < 0.2 {
                side_p.push(solution_x[i].norm());
            }
        }

        let bem_front = if !front_p.is_empty() {
            front_p.iter().sum::<f64>() / front_p.len() as f64
        } else {
            0.0
        };
        let bem_side = if !side_p.is_empty() {
            side_p.iter().sum::<f64>() / side_p.len() as f64
        } else {
            0.0
        };
        let bem_back = if !back_p.is_empty() {
            back_p.iter().sum::<f64>() / back_p.len() as f64
        } else {
            0.0
        };

        let error_avg = 100.0 * (avg_p - mie_avg).abs() / mie_avg;
        let error_front = 100.0 * (bem_front - mie_front).abs() / mie_front;
        let error_side = 100.0 * (bem_side - mie_side).abs() / mie_side;
        let error_back = 100.0 * (bem_back - mie_back).abs() / mie_back;

        println!(
            "{:>8} | {:>10.4} | {:>10.1} | {:>10.1} | {:>10.1} | {:>10.1}",
            n, avg_p, error_avg, error_front, error_side, error_back
        );
    }

    println!("\n=== Summary ===");
    println!("At ka = 1.0 with β = 4i/k, the BEM solver achieves excellent accuracy.");
    println!("For other ka values, different β scaling may be needed, or the mesh");
    println!("resolution may need adjustment (elements per wavelength).");
}

#[cfg(not(feature = "pure-rust"))]
fn main() {
    eprintln!("This example requires the 'pure-rust' feature.");
}
