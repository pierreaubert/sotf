use ndarray::{Array1, Array2};
use rayon::prelude::*;
use std::sync::Arc;

/// Parallel evaluation configuration
#[derive(Debug, Clone)]
pub struct ParallelConfig {
    /// Enable parallel evaluation
    pub enabled: bool,
    /// Number of threads to use (None = use rayon default)
    pub num_threads: Option<usize>,
}

impl Default for ParallelConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            num_threads: None, // Use rayon's default (typically num_cpus)
        }
    }
}

/// Evaluate a population in parallel
///
/// # Arguments
/// * `population` - 2D array where each row is an individual
/// * `eval_fn` - Function to evaluate each individual
/// * `config` - Parallel configuration
///
/// # Returns
/// Array of fitness values for each individual
pub fn evaluate_population_parallel<F>(
    population: &Array2<f64>,
    eval_fn: Arc<F>,
    config: &ParallelConfig,
) -> Array1<f64>
where
    F: Fn(&Array1<f64>) -> f64 + Send + Sync,
{
    let npop = population.nrows();

    if !config.enabled || npop < 4 {
        // Sequential evaluation for small populations or when disabled
        let mut energies = Array1::zeros(npop);
        for i in 0..npop {
            let individual = population.row(i).to_owned();
            energies[i] = eval_fn(&individual);
        }
        return energies;
    }

    // Always use global thread pool (configured once in solver)
    let results = (0..npop)
        .into_par_iter()
        .map(|i| {
            let individual = population.row(i).to_owned();
            eval_fn(&individual)
        })
        .collect::<Vec<f64>>();

    Array1::from_vec(results)
}

/// Evaluate trials in parallel for differential evolution
///
/// This function evaluates multiple trial vectors in parallel, which is useful
/// during the main DE loop where we generate and evaluate one trial per individual.
///
/// # Arguments
/// * `trials` - Vector of trial vectors to evaluate
/// * `eval_fn` - Function to evaluate each trial
/// * `config` - Parallel configuration
///
/// # Returns
/// Vector of fitness values for each trial
pub fn evaluate_trials_parallel<F>(
    trials: Vec<Array1<f64>>,
    eval_fn: Arc<F>,
    config: &ParallelConfig,
) -> Vec<f64>
where
    F: Fn(&Array1<f64>) -> f64 + Send + Sync,
{
    if !config.enabled || trials.len() < 4 {
        // Sequential evaluation for small batches or when disabled
        return trials.iter().map(|trial| eval_fn(trial)).collect();
    }

    // Always use global thread pool (configured once in solver)
    trials.par_iter().map(|trial| eval_fn(trial)).collect()
}

/// Structure to batch evaluate individuals with their indices
pub struct IndexedEvaluation {
    pub index: usize,
    pub individual: Array1<f64>,
    pub fitness: f64,
}

/// Evaluate population with indices preserved for tracking
pub fn evaluate_population_indexed<F>(
    population: &Array2<f64>,
    eval_fn: Arc<F>,
    config: &ParallelConfig,
) -> Vec<IndexedEvaluation>
where
    F: Fn(&Array1<f64>) -> f64 + Send + Sync,
{
    let npop = population.nrows();

    if !config.enabled || npop < 4 {
        // Sequential evaluation
        let mut results = Vec::with_capacity(npop);
        for i in 0..npop {
            let individual = population.row(i).to_owned();
            let fitness = eval_fn(&individual);
            results.push(IndexedEvaluation {
                index: i,
                individual,
                fitness,
            });
        }
        return results;
    }

    // Parallel evaluation (global thread pool)
    (0..npop)
        .into_par_iter()
        .map(|i| {
            let individual = population.row(i).to_owned();
            let fitness = eval_fn(&individual);
            IndexedEvaluation {
                index: i,
                individual,
                fitness,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parallel_evaluation() {
        // Simple quadratic function
        let eval_fn = Arc::new(|x: &Array1<f64>| -> f64 { x.iter().map(|&xi| xi * xi).sum() });

        // Create a small population
        let mut population = Array2::zeros((10, 3));
        for i in 0..10 {
            for j in 0..3 {
                population[[i, j]] = (i as f64) * 0.1 + (j as f64) * 0.01;
            }
        }

        // Test with parallel enabled
        let config = ParallelConfig {
            enabled: true,
            num_threads: Some(2),
        };
        let energies = evaluate_population_parallel(&population, eval_fn.clone(), &config);

        // Verify results
        assert_eq!(energies.len(), 10);
        for i in 0..10 {
            let expected = population.row(i).iter().map(|&x| x * x).sum::<f64>();
            assert!((energies[i] - expected).abs() < 1e-10);
        }

        // Test with parallel disabled
        let config_seq = ParallelConfig {
            enabled: false,
            num_threads: None,
        };
        let energies_seq = evaluate_population_parallel(&population, eval_fn, &config_seq);

        // Results should be identical
        for i in 0..10 {
            assert_eq!(energies[i], energies_seq[i]);
        }
    }

    #[test]
    fn test_indexed_evaluation() {
        let eval_fn = Arc::new(|x: &Array1<f64>| -> f64 { x.iter().sum() });

        let mut population = Array2::zeros((5, 2));
        for i in 0..5 {
            population[[i, 0]] = i as f64;
            population[[i, 1]] = (i * 2) as f64;
        }

        let config = ParallelConfig::default();
        let results = evaluate_population_indexed(&population, eval_fn, &config);

        assert_eq!(results.len(), 5);
        for result in results {
            let expected = population.row(result.index).sum();
            assert_eq!(result.fitness, expected);
        }
    }
}
