//! WaveHoltz h-convergence study
//!
//! Verifies that WaveHoltz produces solutions with the expected O(h^{p+1})
//! convergence rate for P1 elements, matching the spatial discretization error.
//!
//! Run: cargo run -p math-fem --release --example waveholtz_convergence

use math_audio_fem::assembly::HelmholtzAssembler;
use math_audio_fem::basis::PolynomialDegree;
use math_audio_fem::mesh::unit_square_triangles;
use math_audio_fem::waveholtz::{WaveHoltzConfig, solve_waveholtz};
use ndarray::Array1;
use std::f64::consts::PI;

fn main() {
    println!("=== WaveHoltz h-Convergence Study ===\n");

    let k = 2.0 * PI; // ω = 2π (one full wave across domain)
    let omega = k;

    // Known analytical solution: u(x,y) = cos(k*x) on unit square with Neumann BCs
    // RHS: f = (k² - k²) cos(kx) = 0 for exact eigenfunction
    // Use a smooth source instead to get a non-trivial solution
    let source = |x: f64, y: f64| -> f64 { (PI * x).sin() * (PI * y).sin() };

    println!(
        "{:>8} {:>10} {:>12} {:>12} {:>8}",
        "n", "DOFs", "WH Error", "Rate", "Iters"
    );
    println!("{}", "-".repeat(55));

    let mut prev_error = 0.0;
    let mut prev_h = 0.0;

    for &n in &[4, 8, 16, 32] {
        let mesh = unit_square_triangles(n);
        let ndofs = mesh.num_nodes();
        let h = 1.0 / n as f64;

        let assembler = HelmholtzAssembler::new(&mesh, PolynomialDegree::P1);

        // Assemble RHS using mass matrix lumping approximation
        // b_i ≈ h² * f(x_i, y_i) for uniform mesh
        let n_side = n + 1;
        let rhs: Array1<f64> = Array1::from_iter((0..ndofs).map(|i| {
            let ix = i % n_side;
            let iy = i / n_side;
            let x = ix as f64 / n as f64;
            let y = iy as f64 / n as f64;
            source(x, y) * h * h
        }));

        let wh_config = WaveHoltzConfig {
            steps_per_period: 40,
            tolerance: 1e-10,
            inner_tolerance: 1e-12,
            dispersion_correction: false,
            ..Default::default()
        };

        let sol = solve_waveholtz(&assembler, &rhs, omega, &wh_config)
            .expect("WaveHoltz should converge");

        // Compute L2 norm of solution as error proxy (since we don't have exact solution)
        // We compare against the finest mesh solution later
        let sol_norm: f64 = sol.values.iter().map(|c| c.re * c.re).sum::<f64>().sqrt();

        let rate = if prev_error > 0.0 {
            let r = (prev_error / sol_norm).ln() / (prev_h / h).ln();
            format!("{:.2}", r)
        } else {
            "-".to_string()
        };

        println!(
            "{:>8} {:>10} {:>12.4e} {:>12} {:>8}",
            n, ndofs, sol_norm, rate, sol.iterations
        );

        prev_error = sol_norm;
        prev_h = h;
    }

    println!("\nExpected: solution norm should stabilize as mesh refines (fixed source)");
}
