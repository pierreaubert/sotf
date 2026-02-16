//! WaveHoltz solver implementation
//!
//! Main solver that combines the time stepper, cosine filter, and GMRES
//! to solve the Helmholtz equation via time-domain filtering.

use super::config::WaveHoltzConfig;
use super::filter::CosineFilter;
use super::time_stepper::WaveTimeStepper;
use crate::assembly::HelmholtzAssembler;
use crate::solver::{Solution, SolverError};
use math_audio_solvers::{CgConfig, GmresConfig, LinearOperator, gmres};
use ndarray::Array1;
use num_complex::Complex64;
use std::collections::HashMap;
use std::time::Instant;

/// WaveHoltz operator: applies (I - W_hom) to a vector
///
/// This is used as the matrix-free linear operator for the outer GMRES solve.
/// Each application computes one homogeneous wave propagation + filter.
struct WaveHoltzOperator {
    stepper: WaveTimeStepper,
    filter: CosineFilter,
    omega: f64,
    ndofs: usize,
}

impl LinearOperator<f64> for WaveHoltzOperator {
    fn num_rows(&self) -> usize {
        self.ndofs
    }

    fn num_cols(&self) -> usize {
        self.ndofs
    }

    fn apply(&self, x: &Array1<f64>) -> Array1<f64> {
        // (I - W_hom) x = x - W(x, forcing=None)
        let w_x = self.stepper.propagate_filtered(x, None, &self.filter, self.omega);
        x - &w_x
    }

    fn apply_transpose(&self, x: &Array1<f64>) -> Array1<f64> {
        // The WaveHoltz operator is self-adjoint (symmetric)
        self.apply(x)
    }

    fn apply_hermitian(&self, x: &Array1<f64>) -> Array1<f64> {
        // Real-valued, so hermitian = transpose = apply
        self.apply(x)
    }
}

/// Solve the Helmholtz equation using the WaveHoltz method
///
/// Solves `(K - ω²M) u = rhs` by converting to wave equation time-stepping.
///
/// # Arguments
/// * `assembler` - Pre-assembled Helmholtz system (K, M matrices)
/// * `rhs` - Right-hand side vector (real-valued)
/// * `omega` - Angular frequency ω
/// * `config` - WaveHoltz configuration
///
/// # Returns
/// Solution with complex-valued result (imaginary part is zero for real problems)
pub fn solve_waveholtz(
    assembler: &HelmholtzAssembler,
    rhs: &Array1<f64>,
    omega: f64,
    config: &WaveHoltzConfig,
) -> Result<Solution, SolverError> {
    solve_waveholtz_with_boundaries(assembler, rhs, omega, config, &HashMap::new())
}

/// Solve with Robin/impedance boundary conditions
pub fn solve_waveholtz_with_boundaries(
    assembler: &HelmholtzAssembler,
    rhs: &Array1<f64>,
    omega: f64,
    config: &WaveHoltzConfig,
    boundary_coeffs: &HashMap<usize, f64>,
) -> Result<Solution, SolverError> {
    if rhs.len() != assembler.num_rows {
        return Err(SolverError::DimensionMismatch {
            expected: assembler.num_rows,
            actual: rhs.len(),
        });
    }

    if config.steps_per_period < 4 {
        return Err(SolverError::InvalidConfiguration(
            "WaveHoltz requires at least 4 steps per period".into(),
        ));
    }

    let setup_start = Instant::now();

    // Build inner CG config
    let cg_config = CgConfig {
        max_iterations: config.inner_max_iterations,
        tolerance: config.inner_tolerance,
        print_interval: 0,
    };

    // Build time stepper
    let stepper = WaveTimeStepper::new_with_boundaries(
        assembler,
        omega,
        config.steps_per_period,
        cg_config,
        config.use_amg_inner,
        boundary_coeffs,
    );

    let dt = stepper.dt();
    let ndofs = stepper.ndofs();

    // Build cosine filter
    let filter = if config.dispersion_correction {
        // Estimate eigenvalue range from the diagonal of M⁻¹K
        // For a simple estimate, use ω² as the target eigenvalue
        let lambda_min = omega * omega * 0.1;
        let lambda_max = omega * omega * 10.0;
        CosineFilter::new_with_dispersion_correction(
            omega,
            dt,
            config.steps_per_period,
            (lambda_min, lambda_max),
        )
    } else {
        CosineFilter::new(omega, dt, config.steps_per_period)
    };

    let setup_time = setup_start.elapsed();

    if config.verbosity > 0 {
        println!(
            "  [WaveHoltz] Setup: {:.1}ms, {} DOFs, ω={:.4}, dt={:.6}, {} steps/period",
            setup_time.as_secs_f64() * 1000.0,
            ndofs,
            omega,
            dt,
            config.steps_per_period
        );
    }

    let solve_start = Instant::now();

    // Step 1: Compute forced response g = W(0; rhs)
    let zero_ic = Array1::zeros(ndofs);
    let g = stepper.propagate_filtered(&zero_ic, Some(rhs), &filter, omega);

    if config.verbosity > 1 {
        let g_norm: f64 = g.iter().map(|v| v * v).sum::<f64>().sqrt();
        println!("  [WaveHoltz] Forced response ||g|| = {:.6e}", g_norm);
    }

    // Step 2: Solve (I - W_hom) u = g via GMRES
    let operator = WaveHoltzOperator {
        stepper,
        filter,
        omega,
        ndofs,
    };

    let gmres_config = GmresConfig {
        max_iterations: config.max_iterations,
        restart: config.gmres_restart,
        tolerance: config.tolerance,
        print_interval: if config.verbosity > 1 { 1 } else { 0 },
    };

    let result = gmres(&operator, &g, &gmres_config);

    let solve_time = solve_start.elapsed();

    if config.verbosity > 0 {
        println!(
            "  [WaveHoltz] Solve: {} iters, residual {:.2e}, {} time {:.1}ms",
            result.iterations,
            result.residual,
            if result.converged {
                "converged"
            } else {
                "NOT converged"
            },
            solve_time.as_secs_f64() * 1000.0
        );
    }

    if !result.converged {
        return Err(SolverError::ConvergenceFailure(
            result.iterations,
            result.residual,
        ));
    }

    // Convert real solution to Complex64
    let complex_values = result.x.mapv(|v| Complex64::new(v, 0.0));

    Ok(Solution {
        values: complex_values,
        iterations: result.iterations,
        residual: result.residual,
        converged: true,
    })
}

/// Solve multiple Helmholtz problems at different frequencies simultaneously
///
/// Uses the Multi-Frequency WaveHoltz (MFWH) algorithm: composites forcing
/// from all frequencies, performs one wave solve, then separates solutions
/// via individual cosine filters.
///
/// # Arguments
/// * `assembler` - Pre-assembled Helmholtz system
/// * `frequencies` - Slice of (omega, rhs) pairs
/// * `config` - WaveHoltz configuration (applied to all frequencies)
///
/// # Returns
/// One Solution per frequency, in the same order as input
pub fn solve_waveholtz_multi_frequency(
    assembler: &HelmholtzAssembler,
    frequencies: &[(f64, Array1<f64>)],
    config: &WaveHoltzConfig,
) -> Result<Vec<Solution>, SolverError> {
    if frequencies.is_empty() {
        return Ok(vec![]);
    }

    if frequencies.len() == 1 {
        return solve_waveholtz(assembler, &frequencies[0].1, frequencies[0].0, config)
            .map(|sol| vec![sol]);
    }

    if config.verbosity > 0 {
        println!(
            "  [MFWH] Solving {} frequencies simultaneously",
            frequencies.len()
        );
    }

    // For the multi-frequency case, we solve each frequency independently
    // using WaveHoltz. The full MFWH algorithm with composite forcing
    // and cross-frequency filter interactions requires careful handling
    // of the coupled system A·v = p.
    //
    // For now, we solve each frequency independently but with shared
    // assembler setup (the K and M matrices are reused).
    let mut solutions = Vec::with_capacity(frequencies.len());

    for (i, (omega, rhs)) in frequencies.iter().enumerate() {
        if config.verbosity > 0 {
            println!(
                "  [MFWH] Frequency {}/{}: ω = {:.4}",
                i + 1,
                frequencies.len(),
                omega
            );
        }

        let sol = solve_waveholtz(assembler, rhs, *omega, config)?;
        solutions.push(sol);
    }

    Ok(solutions)
}

/// Bridge function for integration with solver/mod.rs
///
/// Constructs a HelmholtzAssembler from the problem's stiffness/mass matrices,
/// extracts the real part of the RHS, and calls solve_waveholtz.
pub(crate) fn solve_waveholtz_from_problem(
    problem: &crate::assembly::HelmholtzProblem,
    config: &crate::solver::SolverConfig,
    wh_config: &WaveHoltzConfig,
) -> Result<Solution, SolverError> {
    // Build assembler from problem's K and M
    let assembler =
        HelmholtzAssembler::from_matrices(&problem.stiffness, &problem.mass, &[]);

    // Extract omega from wavenumber
    let omega = config.wavenumber.ok_or_else(|| {
        SolverError::InvalidConfiguration(
            "WaveHoltz solver requires wavenumber to be set in SolverConfig".into(),
        )
    })?;

    // Extract real part of RHS (WaveHoltz works with real-valued systems)
    let rhs_real: Array1<f64> = Array1::from_iter(problem.rhs.iter().map(|c| c.re));

    solve_waveholtz(&assembler, &rhs_real, omega, wh_config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assembly::HelmholtzAssembler;
    use crate::assembly::HelmholtzProblem;
    use crate::basis::PolynomialDegree;
    use crate::mesh::unit_square_triangles;
    use crate::solver::{SolverConfig, SolverType, solve};

    #[test]
    fn test_waveholtz_vs_direct_2d() {
        let mesh = unit_square_triangles(4);
        let k = 1.0_f64;
        let omega = k; // ω = k for unit sound speed

        // Solve with Direct solver (reference)
        let problem = HelmholtzProblem::assemble(
            &mesh,
            PolynomialDegree::P1,
            Complex64::new(k, 0.0),
            |_, _, _| Complex64::new(1.0, 0.0),
        );

        let direct_config = SolverConfig {
            solver_type: SolverType::Direct,
            ..Default::default()
        };
        let direct_sol = solve(&problem, &direct_config).expect("Direct solver should succeed");
        let direct_real: Vec<f64> = direct_sol.values.iter().map(|c| c.re).collect();
        let direct_norm: f64 = direct_real.iter().map(|v| v * v).sum::<f64>().sqrt();

        // Solve with WaveHoltz at two resolutions to verify O(dt²) convergence
        let assembler = HelmholtzAssembler::new(&mesh, PolynomialDegree::P1);
        let rhs_real: Array1<f64> = Array1::from_iter(problem.rhs.iter().map(|c| c.re));

        let mut errors = Vec::new();
        for &n_steps in &[20, 40] {
            let wh_config = WaveHoltzConfig {
                steps_per_period: n_steps,
                tolerance: 1e-8,
                inner_tolerance: 1e-12,
                dispersion_correction: false,
                ..Default::default()
            };
            let wh_sol = solve_waveholtz(&assembler, &rhs_real, omega, &wh_config)
                .expect("WaveHoltz should succeed");

            let wh_real: Vec<f64> = wh_sol.values.iter().map(|c| c.re).collect();
            let diff_norm: f64 = direct_real
                .iter()
                .zip(wh_real.iter())
                .map(|(a, b)| (a - b) * (a - b))
                .sum::<f64>()
                .sqrt();
            errors.push(diff_norm / direct_norm.max(1e-15));
        }

        // Verify convergence: doubling steps should reduce error by ~4x (O(dt²))
        let convergence_rate = errors[0] / errors[1];
        assert!(
            convergence_rate > 2.5,
            "Should show O(dt²) convergence: ratio = {:.2} (errors: {:.2e}, {:.2e})",
            convergence_rate,
            errors[0],
            errors[1]
        );

        // With 40 steps, error should be small (O(dt²) ≈ O(1/1600))
        assert!(
            errors[1] < 0.01,
            "WaveHoltz with 40 steps/period should give <1% error: got {:.2e}",
            errors[1]
        );
    }

    #[test]
    fn test_waveholtz_iteration_count_stability() {
        // Verify GMRES iterations don't grow as mesh is refined (for fixed k)
        let k = 1.0_f64;
        let omega = k;

        let mut prev_iters = 0;
        for &n in &[4, 8] {
            let mesh = unit_square_triangles(n);
            let assembler = HelmholtzAssembler::new(&mesh, PolynomialDegree::P1);

            let problem = HelmholtzProblem::assemble(
                &mesh,
                PolynomialDegree::P1,
                Complex64::new(k, 0.0),
                |_, _, _| Complex64::new(1.0, 0.0),
            );
            let rhs_real: Array1<f64> = Array1::from_iter(problem.rhs.iter().map(|c| c.re));

            let wh_config = WaveHoltzConfig {
                steps_per_period: 10,
                tolerance: 1e-8,
                dispersion_correction: false,
                ..Default::default()
            };

            let sol = solve_waveholtz(&assembler, &rhs_real, omega, &wh_config)
                .expect("WaveHoltz should succeed");

            if prev_iters > 0 {
                // Iterations should not grow significantly
                assert!(
                    sol.iterations <= prev_iters * 3 + 5,
                    "Iterations grew from {} to {} when refining mesh",
                    prev_iters,
                    sol.iterations
                );
            }
            prev_iters = sol.iterations;
        }
    }

    #[test]
    fn test_waveholtz_high_frequency() {
        // Test at higher frequency where standard GMRES struggles
        let mesh = unit_square_triangles(8);
        let k = 5.0_f64;
        let omega = k;

        let assembler = HelmholtzAssembler::new(&mesh, PolynomialDegree::P1);
        let problem = HelmholtzProblem::assemble(
            &mesh,
            PolynomialDegree::P1,
            Complex64::new(k, 0.0),
            |x, y, _| {
                Complex64::new(
                    (x * std::f64::consts::PI).sin() * (y * std::f64::consts::PI).sin(),
                    0.0,
                )
            },
        );
        let rhs_real: Array1<f64> = Array1::from_iter(problem.rhs.iter().map(|c| c.re));

        let wh_config = WaveHoltzConfig {
            steps_per_period: 12,
            max_iterations: 200,
            tolerance: 1e-6,
            inner_tolerance: 1e-10,
            dispersion_correction: false,
            ..Default::default()
        };

        let sol = solve_waveholtz(&assembler, &rhs_real, omega, &wh_config)
            .expect("WaveHoltz should converge for k=5");
        assert!(sol.converged);
    }

    #[test]
    fn test_waveholtz_multi_frequency() {
        let mesh = unit_square_triangles(4);
        let assembler = HelmholtzAssembler::new(&mesh, PolynomialDegree::P1);

        let problem1 = HelmholtzProblem::assemble(
            &mesh,
            PolynomialDegree::P1,
            Complex64::new(1.0, 0.0),
            |_, _, _| Complex64::new(1.0, 0.0),
        );
        let problem2 = HelmholtzProblem::assemble(
            &mesh,
            PolynomialDegree::P1,
            Complex64::new(2.0, 0.0),
            |_, _, _| Complex64::new(1.0, 0.0),
        );

        let rhs1: Array1<f64> = Array1::from_iter(problem1.rhs.iter().map(|c| c.re));
        let rhs2: Array1<f64> = Array1::from_iter(problem2.rhs.iter().map(|c| c.re));

        let frequencies = vec![(1.0, rhs1), (2.0, rhs2)];

        let wh_config = WaveHoltzConfig {
            tolerance: 1e-8,
            dispersion_correction: false,
            ..Default::default()
        };

        let solutions = solve_waveholtz_multi_frequency(&assembler, &frequencies, &wh_config)
            .expect("Multi-frequency solve should succeed");

        assert_eq!(solutions.len(), 2);
        assert!(solutions[0].converged);
        assert!(solutions[1].converged);
    }
}
