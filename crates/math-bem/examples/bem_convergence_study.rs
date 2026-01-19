//! BEM Convergence Study
//!
//! Studies mesh convergence of the pure Rust BEM solver
//! against analytical Mie series for sphere scattering.
//!
//! # Usage
//! ```bash
//! cargo run --release --example bem_convergence_study --features pure-rust
//! ```

#[cfg(feature = "pure-rust")]
fn main() {
    use math_audio_bem::analytical::sphere_scattering_3d;
    use math_audio_bem::core::{BemProblem, BemSolver};
    use ndarray::Array2;
    use std::f64::consts::PI;

    println!("BEM Convergence Study: Rigid Sphere Scattering");
    println!("==============================================\n");

    let radius = 0.1;
    let speed_of_sound = 343.0;
    let density = 1.21;

    // Test at different ka values
    for ka_target in [0.2, 0.5, 1.0, 2.0] {
        let frequency = ka_target * speed_of_sound / (2.0 * PI * radius);
        let k = 2.0 * PI * frequency / speed_of_sound;
        let ka = k * radius;

        println!("\n=== ka = {:.2} (frequency = {:.1} Hz) ===", ka, frequency);

        // Evaluate at forward direction (theta=0, most sensitive to accuracy)
        let eval_radius = 2.0 * radius;
        let theta = 0.0;
        let analytical = sphere_scattering_3d(k, radius, 50, vec![eval_radius], vec![theta]);
        let analytical_p = analytical.pressure[0].norm();

        let eval_point = Array2::from_shape_vec((1, 3), vec![0.0, 0.0, eval_radius]).unwrap();

        println!("Mesh       DOFs     BEM |p|      Analytical   Error");
        println!("----------------------------------------------------");

        // Test mesh refinement
        for (n_theta, n_phi) in [(4, 8), (6, 12), (8, 16), (10, 20), (12, 24), (16, 32)] {
            let problem = BemProblem::rigid_sphere_scattering_custom(
                radius,
                frequency,
                speed_of_sound,
                density,
                n_theta,
                n_phi,
            );

            let solver = BemSolver::new();
            match solver.solve(&problem) {
                Ok(solution) => {
                    let bem_field = solution.evaluate_pressure_field(&eval_point);
                    let bem_p = bem_field[0].magnitude();
                    let error = (bem_p - analytical_p).abs() / analytical_p * 100.0;

                    println!(
                        "{:2}x{:2}       {:4}    {:10.6}   {:10.6}   {:6.2}%",
                        n_theta,
                        n_phi,
                        solution.num_dofs(),
                        bem_p,
                        analytical_p,
                        error
                    );
                }
                Err(e) => {
                    println!("{:2}x{:2}       FAILED: {}", n_theta, n_phi, e);
                }
            }
        }
    }

    println!("\n\nNote: Non-monotonic convergence may indicate issues with:");
    println!("  - Element normal orientation consistency");
    println!("  - Singular integration accuracy");
    println!("  - Burton-Miller coupling parameter");
    println!("  - Mesh element quality (aspect ratios)");
}

#[cfg(not(feature = "pure-rust"))]
fn main() {
    eprintln!("This example requires the 'pure-rust' feature.");
    eprintln!("Run with: cargo run --example bem_convergence_study --features pure-rust");
}
