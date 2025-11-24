use crate::{DEReport, DifferentialEvolution};
use ndarray::{Array1, Array2};

// ------------------------------ Internal helpers ------------------------------

impl<'a, F> DifferentialEvolution<'a, F>
where
    F: Fn(&Array1<f64>) -> f64 + Sync,
{
    pub(crate) fn energy(&self, x: &Array1<f64>) -> f64 {
        let base = (self.func)(x);
        base + self.penalty(x)
    }

    pub(crate) fn penalty(&self, x: &Array1<f64>) -> f64 {
        let mut p = 0.0;
        // Nonlinear ineq: fc(x) <= 0 feasible
        for (f, w) in &self.config.penalty_ineq {
            let v = f(x);
            let viol = v.max(0.0);
            p += w * viol * viol;
        }
        // Nonlinear eq: h(x) = 0
        for (h, w) in &self.config.penalty_eq {
            let v = h(x);
            p += w * v * v;
        }
        // Linear penalties: lb <= A x <= ub
        if let Some(lp) = &self.config.linear_penalty {
            let ax = lp.a.dot(&x.view());
            for i in 0..ax.len() {
                let v = ax[i];
                let lo = lp.lb[i];
                let hi = lp.ub[i];
                if v < lo {
                    let d = lo - v;
                    p += lp.weight * d * d;
                }
                if v > hi {
                    let d = v - hi;
                    p += lp.weight * d * d;
                }
            }
        }
        p
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn finish_report(
        &self,
        pop: Array2<f64>,
        energies: Array1<f64>,
        x: Array1<f64>,
        fun: f64,
        success: bool,
        message: String,
        nit: usize,
        nfev: usize,
    ) -> DEReport {
        DEReport {
            x,
            fun,
            success,
            message,
            nit,
            nfev,
            population: pop,
            population_energies: energies,
        }
    }

    pub(crate) fn polish(&self, x0: &Array1<f64>) -> (Array1<f64>, f64, usize) {
        // Simple polish: just return the input solution (polishing disabled)
        // In a full implementation, this would use a local optimizer like Nelder-Mead
        let f = self.energy(x0);
        (x0.clone(), f, 1)
    }
}
