use ndarray::{Array1, Array2};
use rand::Rng;
use rand::seq::SliceRandom;

use crate::mutant_rand1::mutant_rand1;

/// Adaptive mutation based on Self-Adaptive Mutation (SAM) from the paper
/// Uses linearly decreasing weight w to select from top individuals
pub(crate) fn mutant_adaptive<R: Rng + ?Sized>(
    i: usize,
    pop: &Array2<f64>,
    sorted_indices: &[usize],
    w: f64,
    f: f64,
    rng: &mut R,
) -> Array1<f64> {
    // Calculate w% of population size for adaptive selection
    let w_size = ((w * pop.nrows() as f64) as usize)
        .max(1)
        .min(pop.nrows() - 1);

    // Select gr_better from top w% individuals randomly
    let top_indices = &sorted_indices[0..w_size];
    let gr_better_idx = top_indices[rng.random_range(0..w_size)];
    // Get two distinct random indices different from i and gr_better_idx
    let mut available: Vec<usize> = (0..pop.nrows())
        .filter(|&idx| idx != i && idx != gr_better_idx)
        .collect();
    available.shuffle(rng);

    if available.len() < 2 {
        // Fallback to standard rand1 if not enough individuals
        return mutant_rand1(i, pop, f, rng);
    }

    let r1 = available[0];
    let r2 = available[1];

    // Adaptive mutation: x_i + F * (x_gr_better - x_i + x_r1 - x_r2)
    // This is the SAM approach from equation (18) in the paper
    pop.row(i).to_owned()
        + &((pop.row(gr_better_idx).to_owned() - pop.row(i).to_owned() + pop.row(r1).to_owned()
            - pop.row(r2).to_owned())
            * f)
}

/// Tests for adaptive differential evolution strategies

#[cfg(test)]
mod tests {
    use crate::{AdaptiveConfig, DEConfigBuilder, Mutation, Strategy, differential_evolution};
    use autoeq_testfunctions::quadratic;

    #[test]
    fn test_adaptive_basic() {
        // Test basic adaptive DE functionality
        let bounds = [(-5.0, 5.0), (-5.0, 5.0)];

        // Configure adaptive DE with SAM approach
        let adaptive_config = AdaptiveConfig {
            adaptive_mutation: true,
            wls_enabled: false, // Start with mutation only
            w_max: 0.9,
            w_min: 0.1,
            ..AdaptiveConfig::default()
        };

        let config = DEConfigBuilder::new()
            .seed(42)
            .maxiter(100)
            .popsize(30)
            .strategy(Strategy::AdaptiveBin)
            .mutation(Mutation::Adaptive { initial_f: 0.8 })
            .adaptive(adaptive_config)
            .build();

        let result = differential_evolution(&quadratic, &bounds, config);

        // Should converge to global minimum at (0, 0)
        assert!(
            result.fun < 1e-3,
            "Adaptive DE should converge: f={}",
            result.fun
        );

        // Check that solution is close to expected optimum
        for &xi in result.x.iter() {
            assert!(
                xi.abs() < 0.5,
                "Solution component should be close to 0: {}",
                xi
            );
        }
    }

    #[test]
    fn test_adaptive_with_wls() {
        // Test adaptive DE with Wrapper Local Search
        let bounds = [(-5.0, 5.0), (-5.0, 5.0)];

        let adaptive_config = AdaptiveConfig {
            adaptive_mutation: true,
            wls_enabled: true,
            wls_prob: 0.2, // Apply WLS to 20% of population
            wls_scale: 0.1,
            ..AdaptiveConfig::default()
        };

        let config = DEConfigBuilder::new()
            .seed(123)
            .maxiter(200)
            .popsize(40)
            .strategy(Strategy::AdaptiveExp)
            .adaptive(adaptive_config)
            .build();

        let result = differential_evolution(&quadratic, &bounds, config);

        // Should converge even better with WLS
        assert!(
            result.fun < 1e-4,
            "Adaptive DE with WLS should converge well: f={}",
            result.fun
        );

        for &xi in result.x.iter() {
            assert!(
                xi.abs() < 0.2,
                "Solution should be very close to 0 with WLS: {}",
                xi
            );
        }
    }
}
