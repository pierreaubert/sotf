//! Shared bounded scalar-objective optimizer dispatch.
//!
//! RoomEQ has a few optimisation problems that are not PEQ filter fitting:
//! GD-Opt v2, multi-sub all-pass, and other small bounded black-box searches.
//! They should still honor the user-selected optimizer algorithm without each
//! caller wiring algorithm-specific code.

use crate::optim::registry;
use math_audio_optimisation::cobyla::{CobylaRhoBegin, cobyla};
use math_audio_optimisation::{
    CmaEsConfig, CobylaConfig, CobylaStopTols, DEConfigBuilder, Init, IsresConfig, Mutation,
    Strategy, cma_es, differential_evolution, isres,
};
use ndarray::Array1;
use std::str::FromStr;

/// Options for bounded scalar minimization.
#[derive(Debug, Clone)]
pub struct ScalarOptimConfig {
    /// User-facing optimizer name, e.g. `"autoeq:cmaes"` or `"autoeq:de"`.
    pub algorithm: String,
    /// Maximum iteration/evaluation budget. DE interprets this as generations;
    /// evaluation-budget optimizers interpret it as objective evaluations.
    pub max_iter: usize,
    /// Population size or population multiplier, depending on backend.
    pub population: usize,
    /// Relative convergence tolerance.
    pub tolerance: f64,
    /// Absolute convergence tolerance.
    pub atolerance: f64,
    /// DE mutation strategy. Ignored by non-DE backends.
    pub strategy: String,
    /// Optional deterministic seed.
    pub seed: Option<u64>,
}

impl Default for ScalarOptimConfig {
    fn default() -> Self {
        Self {
            algorithm: "autoeq:cmaes".to_string(),
            max_iter: 10_000,
            population: 20,
            tolerance: 1e-8,
            atolerance: 1e-8,
            strategy: "lshade".to_string(),
            seed: None,
        }
    }
}

/// Result of a bounded scalar minimization.
#[derive(Debug, Clone)]
pub struct ScalarOptimResult {
    /// Best parameter vector found.
    pub x: Vec<f64>,
    /// Objective value at [`Self::x`].
    pub fun: f64,
    /// Canonical resolved algorithm name.
    pub algorithm: String,
    /// Whether the backend reported convergence.
    pub success: bool,
    /// Human-readable backend status.
    pub message: String,
}

/// Minimize `objective` over box bounds using a configured AutoEQ optimizer.
pub fn optimize_bounded_scalar<F>(
    bounds: &[(f64, f64)],
    initial: &[f64],
    config: &ScalarOptimConfig,
    objective: F,
) -> Result<ScalarOptimResult, String>
where
    F: Fn(&[f64]) -> f64 + Sync,
{
    validate_problem(bounds, initial)?;

    let backend = registry::resolve(&config.algorithm)
        .ok_or_else(|| format!("Unknown algorithm: {}", config.algorithm))?;
    let canonical = backend.name().to_string();

    let f = |x: &Array1<f64>| objective(x.as_slice().unwrap());
    let x0 = clamp_initial(initial, bounds);

    match canonical.as_str() {
        "autoeq:cmaes" => optimize_cmaes(&canonical, bounds, x0, config, &f),
        "autoeq:de" => optimize_de(&canonical, bounds, x0, config, &f),
        "autoeq:cobyla" => optimize_cobyla(&canonical, bounds, x0, config, &f),
        "autoeq:isres" => optimize_isres(&canonical, bounds, x0, config, &f),
        other => Err(format!(
            "Algorithm '{}' is registered for PEQ filter optimization but is not supported for bounded scalar RoomEQ objectives",
            other
        )),
    }
}

fn validate_problem(bounds: &[(f64, f64)], initial: &[f64]) -> Result<(), String> {
    if bounds.is_empty() {
        return Err("scalar optimizer requires at least one parameter".to_string());
    }
    if bounds.len() != initial.len() {
        return Err(format!(
            "scalar optimizer dimension mismatch: bounds={}, initial={}",
            bounds.len(),
            initial.len()
        ));
    }
    for (idx, (lo, hi)) in bounds.iter().enumerate() {
        if lo > hi {
            return Err(format!(
                "invalid scalar optimizer bounds at {}: lower {} > upper {}",
                idx, lo, hi
            ));
        }
    }
    Ok(())
}

fn clamp_initial(initial: &[f64], bounds: &[(f64, f64)]) -> Array1<f64> {
    Array1::from(
        initial
            .iter()
            .zip(bounds.iter())
            .map(|(&x, (lo, hi))| x.clamp(*lo, *hi))
            .collect::<Vec<_>>(),
    )
}

fn optimize_cmaes<F>(
    canonical: &str,
    bounds: &[(f64, f64)],
    x0: Array1<f64>,
    config: &ScalarOptimConfig,
    f: &F,
) -> Result<ScalarOptimResult, String>
where
    F: Fn(&Array1<f64>) -> f64 + Sync,
{
    let lambda = config.population.max(4);
    let report = cma_es(
        f,
        CmaEsConfig {
            bounds: bounds.to_vec(),
            x0: Some(x0),
            sigma0: Some(0.20),
            lambda,
            mu: 0,
            maxeval: config.max_iter.max(lambda + 1),
            seed: config.seed,
            f_tol: config.atolerance.max(1e-12),
            stagnation_window: 80,
            ..Default::default()
        },
    )
    .map_err(|e| format!("CMA-ES failed: {e:?}"))?;

    Ok(ScalarOptimResult {
        x: report.x.to_vec(),
        fun: report.fun,
        algorithm: canonical.to_string(),
        success: report.success,
        message: report.message,
    })
}

fn optimize_de<F>(
    canonical: &str,
    bounds: &[(f64, f64)],
    x0: Array1<f64>,
    config: &ScalarOptimConfig,
    f: &F,
) -> Result<ScalarOptimResult, String>
where
    F: Fn(&Array1<f64>) -> f64 + Sync,
{
    let strategy = Strategy::from_str(&config.strategy).unwrap_or(Strategy::LShadeBin);
    let mut builder = DEConfigBuilder::new()
        .maxiter(config.max_iter)
        .popsize(config.population.max(4))
        .tol(config.tolerance)
        .atol(config.atolerance)
        .strategy(strategy)
        .mutation(Mutation::Range { min: 0.4, max: 1.2 })
        .init(Init::LatinHypercube)
        .x0(x0)
        .disp(false);
    if let Some(seed) = config.seed {
        builder = builder.seed(seed);
    }

    let de_config = builder
        .build()
        .map_err(|e| format!("DE config error: {e:?}"))?;
    let report =
        differential_evolution(f, bounds, de_config).map_err(|e| format!("DE failed: {e:?}"))?;

    Ok(ScalarOptimResult {
        x: report.x.to_vec(),
        fun: report.fun,
        algorithm: canonical.to_string(),
        success: report.success,
        message: report.message,
    })
}

fn optimize_cobyla<F>(
    canonical: &str,
    bounds: &[(f64, f64)],
    x0: Array1<f64>,
    config: &ScalarOptimConfig,
    f: &F,
) -> Result<ScalarOptimResult, String>
where
    F: Fn(&Array1<f64>) -> f64 + Sync,
{
    let report = cobyla(
        f,
        &[],
        CobylaConfig {
            x0,
            bounds: bounds.to_vec(),
            rho_begin: CobylaRhoBegin::All(0.2),
            maxeval: config.max_iter,
            stop_tol: CobylaStopTols {
                ftol_abs: config.atolerance,
                ftol_rel: config.tolerance,
                ..Default::default()
            },
        },
    )
    .map_err(|e| format!("COBYLA failed: {e:?}"))?;

    Ok(ScalarOptimResult {
        x: report.x.to_vec(),
        fun: report.fun,
        algorithm: canonical.to_string(),
        success: report.success,
        message: report.message,
    })
}

fn optimize_isres<F>(
    canonical: &str,
    bounds: &[(f64, f64)],
    x0: Array1<f64>,
    config: &ScalarOptimConfig,
    f: &F,
) -> Result<ScalarOptimResult, String>
where
    F: Fn(&Array1<f64>) -> f64 + Sync,
{
    let mu = config.population.max(2);
    let report = isres(
        f,
        &[],
        IsresConfig {
            bounds: bounds.to_vec(),
            x0: Some(x0),
            mu,
            lambda: 0,
            maxeval: config.max_iter.max(mu * 7),
            seed: config.seed,
            f_tol: config.atolerance.max(1e-12),
            ..Default::default()
        },
    )
    .map_err(|e| format!("ISRES failed: {e:?}"))?;

    Ok(ScalarOptimResult {
        x: report.x.to_vec(),
        fun: report.fun,
        algorithm: canonical.to_string(),
        success: report.success,
        message: report.message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quadratic(x: &[f64]) -> f64 {
        (x[0] - 0.25).powi(2) + (x[1] + 0.5).powi(2)
    }

    fn assert_solves_quadratic(algo: &str) {
        let result = optimize_bounded_scalar(
            &[(-2.0, 2.0), (-2.0, 2.0)],
            &[1.5, 1.5],
            &ScalarOptimConfig {
                algorithm: algo.to_string(),
                max_iter: 400,
                population: 12,
                seed: Some(7),
                ..Default::default()
            },
            quadratic,
        )
        .expect("optimizer should run");

        assert!(result.fun < 1e-2, "{algo} fun={}", result.fun);
        assert!(result.algorithm.starts_with("autoeq:"));
    }

    #[test]
    fn cmaes_solves_bounded_scalar_quadratic() {
        assert_solves_quadratic("autoeq:cmaes");
    }

    #[test]
    fn cmaes_alias_resolves_for_bounded_scalar() {
        let result = optimize_bounded_scalar(
            &[(-1.0, 1.0)],
            &[0.9],
            &ScalarOptimConfig {
                algorithm: "cma-es".to_string(),
                max_iter: 200,
                population: 8,
                seed: Some(3),
                ..Default::default()
            },
            |x| (x[0] + 0.2).powi(2),
        )
        .expect("optimizer should run");

        assert_eq!(result.algorithm, "autoeq:cmaes");
        assert!(result.fun < 1e-2, "fun={}", result.fun);
    }

    #[test]
    fn de_solves_bounded_scalar_quadratic() {
        assert_solves_quadratic("autoeq:de");
    }
}
