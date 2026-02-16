//! WaveHoltz O(N) scaling verification
//!
//! Measures solve time vs DOFs for fixed wavenumber. The key claim of WaveHoltz
//! is O(N) total cost: the number of outer iterations stays constant while
//! inner CG+AMG solves cost O(N) each.
//!
//! Run: cargo run -p math-fem --release --example waveholtz_scaling

use math_audio_fem::assembly::{HelmholtzAssembler, HelmholtzProblem};
use math_audio_fem::basis::PolynomialDegree;
use math_audio_fem::mesh::unit_square_triangles;
use math_audio_fem::waveholtz::{WaveHoltzConfig, solve_waveholtz};
use ndarray::Array1;
use num_complex::Complex64;
use std::f64::consts::PI;
use std::time::Instant;

fn main() {
    println!("=== WaveHoltz O(N) Scaling Verification ===\n");
    println!("Fixed wavenumber k = 1.0, increasing mesh refinement\n");

    let k = 1.0_f64;
    let omega = k;

    println!(
        "{:>8} {:>10} {:>12} {:>12} {:>8} {:>14}",
        "n", "DOFs", "Setup(ms)", "Solve(ms)", "Iters", "Time/DOF(μs)"
    );
    println!("{}", "-".repeat(70));

    for &n in &[4, 8, 16, 32, 64] {
        let mesh = unit_square_triangles(n);
        let ndofs = mesh.num_nodes();

        let setup_start = Instant::now();
        let assembler = HelmholtzAssembler::new(&mesh, PolynomialDegree::P1);
        let setup_time = setup_start.elapsed().as_secs_f64() * 1000.0;

        let problem = HelmholtzProblem::assemble(
            &mesh,
            PolynomialDegree::P1,
            Complex64::new(k, 0.0),
            |x, y, _| Complex64::new((PI * x).sin() * (PI * y).sin(), 0.0),
        );
        let rhs_real: Array1<f64> = Array1::from_iter(problem.rhs.iter().map(|c| c.re));

        let wh_config = WaveHoltzConfig {
            steps_per_period: 20,
            tolerance: 1e-8,
            inner_tolerance: 1e-10,
            dispersion_correction: false,
            ..Default::default()
        };

        let solve_start = Instant::now();
        let sol = solve_waveholtz(&assembler, &rhs_real, omega, &wh_config)
            .expect("WaveHoltz should converge");
        let solve_time = solve_start.elapsed().as_secs_f64() * 1000.0;
        let time_per_dof = solve_time * 1000.0 / ndofs as f64; // μs per DOF

        println!(
            "{:>8} {:>10} {:>12.2} {:>12.2} {:>8} {:>14.2}",
            n, ndofs, setup_time, solve_time, sol.iterations, time_per_dof
        );
    }

    println!("\nExpected: time/DOF should stay roughly constant (O(N) total cost)");
    println!("Note: iterations should stay constant for fixed k as N grows");
}
