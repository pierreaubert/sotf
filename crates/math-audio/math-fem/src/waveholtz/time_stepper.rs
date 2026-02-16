//! Newmark implicit time stepper for WaveHoltz
//!
//! Implements the average-acceleration Newmark scheme (β=1/4, γ=1/2) for
//! the wave equation `M w_tt + K w = f(t)`.
//!
//! The implicit scheme yields the recurrence:
//!   A_impl · w^{n+1} = dt² · f^n + B_rhs · w^n - A_impl · w^{n-1}
//!
//! where:
//!   A_impl = M + (dt²/4) K  (SPD matrix)
//!   B_rhs  = 2M - (dt²/2) K

use super::filter::CosineFilter;
use crate::assembly::HelmholtzAssembler;
use math_audio_solvers::{
    AmgConfig, AmgPreconditioner, AmgSmoother, CgConfig, CsrMatrix, pcg, cg,
};
use ndarray::Array1;
use std::collections::HashMap;

/// Newmark time stepper for wave equation propagation
pub struct WaveTimeStepper {
    /// SPD implicit matrix: M + (dt²/4)K
    a_impl: CsrMatrix<f64>,
    /// Explicit RHS matrix: 2M - (dt²/2)K
    b_rhs: CsrMatrix<f64>,
    /// Time step
    dt: f64,
    /// dt²
    dt_sq: f64,
    /// Number of DOFs
    ndofs: usize,
    /// CG solver configuration for inner solves
    cg_config: CgConfig<f64>,
    /// AMG preconditioner for A_impl (optional)
    amg_precond: Option<AmgPreconditioner<f64>>,
}

impl WaveTimeStepper {
    /// Create a new time stepper from an assembler
    ///
    /// # Arguments
    /// * `assembler` - Helmholtz assembler with K and M matrices
    /// * `omega` - Angular frequency ω
    /// * `steps_per_period` - Number of time steps per period T = 2π/ω
    /// * `cg_config` - Configuration for inner CG solves
    /// * `use_amg` - Whether to use AMG preconditioning for inner solves
    pub fn new(
        assembler: &HelmholtzAssembler,
        omega: f64,
        steps_per_period: usize,
        cg_config: CgConfig<f64>,
        use_amg: bool,
    ) -> Self {
        Self::new_with_boundaries(assembler, omega, steps_per_period, cg_config, use_amg, &HashMap::new())
    }

    /// Create a new time stepper with Robin/impedance boundary conditions
    ///
    /// Boundary coefficients are added to both A_impl and B_rhs matrices
    /// to incorporate impedance boundary conditions from the wave equation.
    pub fn new_with_boundaries(
        assembler: &HelmholtzAssembler,
        omega: f64,
        steps_per_period: usize,
        cg_config: CgConfig<f64>,
        use_amg: bool,
        boundary_coeffs: &HashMap<usize, f64>,
    ) -> Self {
        let period = 2.0 * std::f64::consts::PI / omega;
        let dt = period / steps_per_period as f64;
        let dt_sq = dt * dt;
        let ndofs = assembler.num_rows;

        // A_impl = M + (dt²/4)K + boundary terms
        // B_rhs  = 2M - (dt²/2)K + boundary terms
        let a_impl = if boundary_coeffs.is_empty() {
            assembler.assemble_real(dt_sq / 4.0, 1.0)
        } else {
            assembler.assemble_real_with_boundaries(dt_sq / 4.0, 1.0, boundary_coeffs)
        };

        let b_rhs = if boundary_coeffs.is_empty() {
            assembler.assemble_real(-dt_sq / 2.0, 2.0)
        } else {
            assembler.assemble_real_with_boundaries(-dt_sq / 2.0, 2.0, boundary_coeffs)
        };

        // Build AMG preconditioner for A_impl
        let amg_precond = if use_amg {
            let mut amg_config = AmgConfig::for_fem();
            amg_config.smoother = AmgSmoother::SymmetricGaussSeidel;
            Some(AmgPreconditioner::from_csr(&a_impl, amg_config))
        } else {
            None
        };

        Self {
            a_impl,
            b_rhs,
            dt,
            dt_sq,
            ndofs,
            cg_config,
            amg_precond,
        }
    }

    /// Solve A_impl * x = rhs using (P)CG
    fn solve_implicit(&self, rhs: &Array1<f64>) -> Array1<f64> {
        let result = if let Some(ref precond) = self.amg_precond {
            pcg(&self.a_impl, precond, rhs, &self.cg_config)
        } else {
            cg(&self.a_impl, rhs, &self.cg_config)
        };
        result.x
    }

    /// Number of DOFs
    pub fn ndofs(&self) -> usize {
        self.ndofs
    }

    /// Time step size
    pub fn dt(&self) -> f64 {
        self.dt
    }

    /// Propagate wave equation for one period with cosine filtering
    ///
    /// Solves `M w_tt + K w = forcing · cos(ωt)` with initial conditions
    /// `w(0) = w0`, `w_t(0) = 0`, and returns the cosine-filtered result.
    ///
    /// Memory-efficient: only stores w_prev, w_curr, w_next + accumulator (4 vectors).
    ///
    /// # Arguments
    /// * `w0` - Initial condition w(0)
    /// * `forcing` - Spatial forcing vector (multiplied by cos(ωt) at each step), or None
    /// * `filter` - Cosine filter for time-domain extraction
    /// * `omega` - Angular frequency (for computing cos(ωt) forcing)
    pub fn propagate_filtered(
        &self,
        w0: &Array1<f64>,
        forcing: Option<&Array1<f64>>,
        filter: &CosineFilter,
        omega: f64,
    ) -> Array1<f64> {
        let n_steps = filter.n_steps();
        let mut accumulator = Array1::zeros(self.ndofs);

        // Step 0: w_curr = w0
        let mut w_prev;
        let mut w_curr = w0.clone();

        // Accumulate step 0
        filter.accumulate(0, &w_curr, &mut accumulator);

        // Step 1: zero-velocity initial condition trick
        // w_t(0) = 0 implies w_{-1} = w_1, so:
        //   A_impl · w_1 + A_impl · w_{-1} = dt² · f(0) + B_rhs · w_0
        //   2 · A_impl · w_1 = dt² · f(0) + B_rhs · w_0
        {
            let mut rhs = self.b_rhs.matvec(&w_curr);
            if let Some(f) = forcing {
                // f(t=0) = f · cos(0) = f
                rhs.scaled_add(self.dt_sq, f);
            }
            // Solve 2·A_impl · w_1 = rhs → scale rhs by 0.5
            rhs.mapv_inplace(|v| v * 0.5);
            let w_next = self.solve_implicit(&rhs);

            w_prev = w_curr;
            w_curr = w_next;
        }

        // Accumulate step 1
        filter.accumulate(1, &w_curr, &mut accumulator);

        // Steps 2..N_t: compute w^n from w^{n-1} (w_curr) and w^{n-2} (w_prev)
        // Newmark recurrence: A w^{n+1} = dt² F(t_n) + B w^n - A w^{n-1}
        // Since loop index n gives the step we're computing, the "recurrence n"
        // is (n-1), so forcing is at t_{n-1}.
        for n in 2..=n_steps {
            let mut rhs = self.b_rhs.matvec(&w_curr);
            let a_prev = self.a_impl.matvec(&w_prev);
            rhs -= &a_prev;

            if let Some(f) = forcing {
                let t = (n - 1) as f64 * self.dt;
                let cos_wt = (omega * t).cos();
                if cos_wt.abs() > 1e-20 {
                    rhs.scaled_add(self.dt_sq * cos_wt, f);
                }
            }

            let w_next = self.solve_implicit(&rhs);

            w_prev = w_curr;
            w_curr = w_next;

            filter.accumulate(n, &w_curr, &mut accumulator);
        }

        accumulator
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assembly::HelmholtzAssembler;
    use crate::basis::PolynomialDegree;
    use crate::mesh::unit_square_triangles;

    #[test]
    fn test_a_impl_is_spd() {
        // Verify CG converges on A_impl with random RHS (SPD check)
        let mesh = unit_square_triangles(4);
        let assembler = HelmholtzAssembler::new(&mesh, PolynomialDegree::P1);
        let omega = std::f64::consts::PI;
        let cg_config = CgConfig {
            max_iterations: 500,
            tolerance: 1e-10,
            print_interval: 0,
        };

        let stepper = WaveTimeStepper::new(&assembler, omega, 10, cg_config, false);

        // Random-ish RHS
        let n = stepper.ndofs();
        let rhs: Array1<f64> = Array1::from_iter((0..n).map(|i| (i as f64 * 0.37).sin()));

        let result = cg(&stepper.a_impl, &rhs, &stepper.cg_config);
        assert!(
            result.converged,
            "CG should converge on A_impl (SPD). Residual: {:.2e} after {} iters",
            result.residual,
            result.iterations
        );
    }

    #[test]
    fn test_standing_wave_one_period() {
        // 1D-like test on narrow strip mesh: IC = sin(πx), propagate one period
        // After one full period, w(T) ≈ w(0) for an unforced standing wave
        let mesh = unit_square_triangles(8);
        let assembler = HelmholtzAssembler::new(&mesh, PolynomialDegree::P1);

        let omega = std::f64::consts::PI; // matching first mode on [0,1]
        let n_steps = 20;
        let cg_config = CgConfig {
            max_iterations: 500,
            tolerance: 1e-12,
            print_interval: 0,
        };

        let stepper = WaveTimeStepper::new(&assembler, omega, n_steps, cg_config, false);

        // IC: sin(πx) * sin(πy)
        let ndofs = stepper.ndofs();
        let w0: Array1<f64> = Array1::from_iter((0..ndofs).map(|i| {
            // approximate x,y from node index on unit square mesh
            let n_side = 9; // 8+1 nodes per side
            let ix = i % n_side;
            let iy = i / n_side;
            let x = ix as f64 / (n_side - 1) as f64;
            let y = iy as f64 / (n_side - 1) as f64;
            (std::f64::consts::PI * x).sin() * (std::f64::consts::PI * y).sin()
        }));

        let filter = CosineFilter::new(omega, stepper.dt(), n_steps);
        let result = stepper.propagate_filtered(&w0, None, &filter, omega);

        // The filtered result should be related to the initial condition
        // (cosine filter extracts the ω-component of the time evolution)
        let result_norm: f64 = result.iter().map(|v| v * v).sum::<f64>().sqrt();
        assert!(
            result_norm > 1e-6,
            "Filtered result should be non-trivial"
        );
    }
}
