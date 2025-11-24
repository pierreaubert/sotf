use crate::{DEConfig, DEReport, DifferentialEvolution};
use ndarray::Array1;

/// Convenience function mirroring SciPy's API shape (simplified):
/// - `func`: objective function mapping x -> f(x)
/// - `bounds`: vector of (lower, upper) pairs
/// - `config`: DE configuration
pub fn differential_evolution<F>(func: &F, bounds: &[(f64, f64)], config: DEConfig) -> DEReport
where
    F: Fn(&Array1<f64>) -> f64 + Sync,
{
    let n = bounds.len();
    let mut lower = Array1::<f64>::zeros(n);
    let mut upper = Array1::<f64>::zeros(n);
    for (i, (lo, hi)) in bounds.iter().enumerate() {
        lower[i] = *lo;
        upper[i] = *hi;
        assert!(hi >= lo, "bound[{}] has upper < lower", i);
    }
    let mut de = DifferentialEvolution::new(func, lower, upper);
    *de.config_mut() = config;
    de.solve()
}
