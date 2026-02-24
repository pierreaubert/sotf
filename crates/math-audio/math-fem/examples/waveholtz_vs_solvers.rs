//! WaveHoltz vs other Helmholtz solvers comparison
//!
//! Benchmarks WaveHoltz against Direct, GMRES+ILU, and Shifted-Laplacian solvers
//! on a 2D unit square problem at various wavenumbers.
//!
//! Run: cargo run -p math-fem --release --example waveholtz_vs_solvers

use math_audio_fem::assembly::{HelmholtzAssembler, HelmholtzProblem};
use math_audio_fem::basis::PolynomialDegree;
use math_audio_fem::mesh::unit_square_triangles;
use math_audio_fem::solver::{SolverConfig, SolverType, solve};
use math_audio_fem::waveholtz::{WaveHoltzConfig, solve_waveholtz};
use ndarray::Array1;
use num_complex::Complex64;
use std::f64::consts::PI;
use std::time::Instant;

fn main() {
    println!("=== WaveHoltz vs Other Helmholtz Solvers ===\n");

    let n = 16; // mesh refinement
    let mesh = unit_square_triangles(n);
    let ndofs = mesh.num_nodes();

    println!("Mesh: {}x{} = {} DOFs\n", n, n, ndofs);

    let source = |x: f64, y: f64, _z: f64| -> Complex64 {
        Complex64::new((PI * x).sin() * (PI * y).sin(), 0.0)
    };

    println!(
        "{:>8} {:>12} {:>8} {:>10} {:>12} {:>8} {:>10}",
        "k", "Solver", "Iters", "Time(ms)", "Residual", "Conv", "RelDiff"
    );
    println!("{}", "-".repeat(75));

    for &k in &[1.0, 3.0, 5.0, 8.0] {
        let omega = k;

        // Direct solver (reference)
        let problem =
            HelmholtzProblem::assemble(&mesh, PolynomialDegree::P1, Complex64::new(k, 0.0), source);

        let direct_start = Instant::now();
        let direct_config = SolverConfig {
            solver_type: SolverType::Direct,
            ..Default::default()
        };
        let direct_sol = solve(&problem, &direct_config).expect("Direct should succeed");
        let direct_time = direct_start.elapsed().as_secs_f64() * 1000.0;
        let direct_norm: f64 = direct_sol.values.iter().map(|c| c.norm()).sum::<f64>();

        println!(
            "{:>8.1} {:>12} {:>8} {:>10.2} {:>12.2e} {:>8} {:>10}",
            k, "Direct", 0, direct_time, direct_sol.residual, "yes", "-"
        );

        // GMRES+ILU
        let ilu_start = Instant::now();
        let ilu_config = SolverConfig {
            solver_type: SolverType::GmresIlu,
            gmres: math_audio_solvers::GmresConfig {
                max_iterations: 500,
                restart: 30,
                tolerance: 1e-8,
                print_interval: 0,
            },
            ..Default::default()
        };
        let ilu_result = solve(&problem, &ilu_config);
        let ilu_time = ilu_start.elapsed().as_secs_f64() * 1000.0;

        match ilu_result {
            Ok(sol) => {
                let diff: f64 = sol
                    .values
                    .iter()
                    .zip(direct_sol.values.iter())
                    .map(|(a, b)| (a - b).norm())
                    .sum::<f64>()
                    / direct_norm.max(1e-15);
                println!(
                    "{:>8} {:>12} {:>8} {:>10.2} {:>12.2e} {:>8} {:>10.2e}",
                    "", "GMRES+ILU", sol.iterations, ilu_time, sol.residual, "yes", diff
                );
            }
            Err(e) => {
                println!(
                    "{:>8} {:>12} {:>8} {:>10.2} {:>12} {:>8} {:>10}",
                    "", "GMRES+ILU", "-", ilu_time, e, "no", "-"
                );
            }
        }

        // WaveHoltz
        let assembler = HelmholtzAssembler::new(&mesh, PolynomialDegree::P1);
        let rhs_real: Array1<f64> = Array1::from_iter(problem.rhs.iter().map(|c| c.re));

        let wh_start = Instant::now();
        let wh_config = WaveHoltzConfig {
            steps_per_period: 20,
            tolerance: 1e-8,
            inner_tolerance: 1e-10,
            dispersion_correction: false,
            ..Default::default()
        };
        let wh_result = solve_waveholtz(&assembler, &rhs_real, omega, &wh_config);
        let wh_time = wh_start.elapsed().as_secs_f64() * 1000.0;

        match wh_result {
            Ok(sol) => {
                let diff: f64 = sol
                    .values
                    .iter()
                    .zip(direct_sol.values.iter())
                    .map(|(a, b)| (a.re - b.re).abs())
                    .sum::<f64>()
                    / direct_norm.max(1e-15);
                println!(
                    "{:>8} {:>12} {:>8} {:>10.2} {:>12.2e} {:>8} {:>10.2e}",
                    "", "WaveHoltz", sol.iterations, wh_time, sol.residual, "yes", diff
                );
            }
            Err(e) => {
                println!(
                    "{:>8} {:>12} {:>8} {:>10.2} {:>12} {:>8} {:>10}",
                    "", "WaveHoltz", "-", wh_time, e, "no", "-"
                );
            }
        }

        println!();
    }
}
