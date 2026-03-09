use crate::{DEReport, DifferentialEvolution};
use ndarray::{Array1, Array2, Zip};

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
            Zip::from(&ax)
                .and(&lp.lb)
                .and(&lp.ub)
                .for_each(|&v, &lo, &hi| {
                    if v < lo {
                        let d = lo - v;
                        p += lp.weight * d * d;
                    } else if v > hi {
                        let d = v - hi;
                        p += lp.weight * d * d;
                    }
                });
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
        let polish_cfg = match &self.config.polish {
            Some(cfg) if cfg.enabled => cfg,
            _ => {
                let f = self.energy(x0);
                return (x0.clone(), f, 1);
            }
        };

        let n = x0.len();
        let mut x = x0.clone();
        let mut best_f = self.energy(&x);
        let mut nfev = 1;

        let initial_step = 0.1;
        let min_step = 1e-8;
        let mut step = initial_step;

        let max_eval = polish_cfg.maxeval.min(200 * n);

        while nfev < max_eval && step > min_step {
            let mut improved = false;

            for i in 0..n {
                if nfev >= max_eval {
                    break;
                }

                let original = x[i];
                let bounds_span = self.upper[i] - self.lower[i];
                let dim_step = step * bounds_span.max(1.0);

                for delta in [dim_step, -dim_step] {
                    if nfev >= max_eval {
                        break;
                    }
                    x[i] = (original + delta).clamp(self.lower[i], self.upper[i]);
                    let f = self.energy(&x);
                    nfev += 1;

                    if f < best_f {
                        best_f = f;
                        improved = true;
                        break;
                    }
                    x[i] = original;
                }
            }

            if !improved {
                step *= 0.5;
            }
        }

        (x, best_f, nfev)
    }
}
