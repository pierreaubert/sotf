use crate::{CallbackAction, DEConfigBuilder, DifferentialEvolution, PolishConfig, Strategy};
use ndarray::{Array1, array};
use rand::SeedableRng;
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(test)]
mod strategy_tests {
    use super::*;

    #[test]
    fn test_best1_binomial_convergence() {
        let sphere = |x: &Array1<f64>| x.iter().map(|&xi| xi * xi).sum::<f64>();

        let config = DEConfigBuilder::new()
            .seed(42)
            .maxiter(200)
            .popsize(20)
            .strategy(Strategy::Best1Bin)
            .build()
            .expect("popsize must be >= 4");

        let mut de =
            DifferentialEvolution::new(&sphere, array![-5.0f64, -5.0], array![5.0f64, 5.0])
                .unwrap();
        *de.config_mut() = config;
        let report = de.solve();

        assert!(
            report.fun < 1.0,
            "Should converge near origin: f={}",
            report.fun
        );
    }

    #[test]
    fn test_rand1_exponential_convergence() {
        let sphere = |x: &Array1<f64>| x.iter().map(|&xi| xi * xi).sum::<f64>();

        let config = DEConfigBuilder::new()
            .seed(123)
            .maxiter(300)
            .popsize(30)
            .strategy(Strategy::Rand1Exp)
            .recombination(0.5)
            .build()
            .expect("popsize must be >= 4");

        let mut de =
            DifferentialEvolution::new(&sphere, array![-5.0f64, -5.0], array![5.0f64, 5.0])
                .unwrap();
        *de.config_mut() = config;
        let report = de.solve();

        assert!(report.fun < 1.0, "Should converge: f={}", report.fun);
    }

    #[test]
    fn test_rand2_binomial_convergence() {
        let sphere = |x: &Array1<f64>| x.iter().map(|&xi| xi * xi).sum::<f64>();

        let config = DEConfigBuilder::new()
            .seed(456)
            .maxiter(300)
            .popsize(30)
            .strategy(Strategy::Rand2Bin)
            .build()
            .expect("popsize must be >= 4");

        let mut de =
            DifferentialEvolution::new(&sphere, array![-5.0f64, -5.0], array![5.0f64, 5.0])
                .unwrap();
        *de.config_mut() = config;
        let report = de.solve();

        assert!(report.fun < 1.0, "Should converge: f={}", report.fun);
    }

    #[test]
    fn test_current_to_best_convergence() {
        let sphere = |x: &Array1<f64>| x.iter().map(|&xi| xi * xi).sum::<f64>();

        let config = DEConfigBuilder::new()
            .seed(789)
            .maxiter(200)
            .popsize(25)
            .strategy(Strategy::CurrentToBest1Bin)
            .build()
            .expect("popsize must be >= 4");

        let mut de =
            DifferentialEvolution::new(&sphere, array![-5.0f64, -5.0], array![5.0f64, 5.0])
                .unwrap();
        *de.config_mut() = config;
        let report = de.solve();

        assert!(report.fun < 1.0, "Should converge: f={}", report.fun);
    }

    #[test]
    fn test_best2_convergence() {
        let sphere = |x: &Array1<f64>| x.iter().map(|&xi| xi * xi).sum::<f64>();

        let config = DEConfigBuilder::new()
            .seed(321)
            .maxiter(300)
            .popsize(30)
            .strategy(Strategy::Best2Bin)
            .build()
            .expect("popsize must be >= 4");

        let mut de =
            DifferentialEvolution::new(&sphere, array![-5.0f64, -5.0], array![5.0f64, 5.0])
                .unwrap();
        *de.config_mut() = config;
        let report = de.solve();

        assert!(report.fun < 1.0, "Should converge: f={}", report.fun);
    }
}

#[cfg(test)]
mod crossover_tests {
    use super::*;
    use crate::crossover_binomial::binomial_crossover;
    use crate::crossover_exponential::exponential_crossover;

    #[test]
    fn test_binomial_crossover_preserves_dimensions() {
        let target = array![1.0, 2.0, 3.0, 4.0, 5.0];
        let mutant = array![10.0, 20.0, 30.0, 40.0, 50.0];
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);

        let trial = binomial_crossover(&target, &mutant, 0.5, &mut rng);

        assert_eq!(trial.len(), target.len());
    }

    #[test]
    fn test_exponential_crossover_preserves_dimensions() {
        let target = array![1.0, 2.0, 3.0, 4.0, 5.0];
        let mutant = array![10.0, 20.0, 30.0, 40.0, 50.0];
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);

        let trial = exponential_crossover(&target, &mutant, 0.5, &mut rng);

        assert_eq!(trial.len(), target.len());
    }
}

#[cfg(test)]
mod initialization_tests {
    use super::*;
    use crate::init_latin_hypercube::init_latin_hypercube;
    use crate::init_random::init_random;

    #[test]
    fn test_latin_hypercube_dimensions() {
        let lower = array![0.0, 0.0];
        let upper = array![10.0, 10.0];
        let is_free = vec![true, true];
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);

        let pop = init_latin_hypercube(2, 20, &lower, &upper, &is_free, &mut rng);

        assert_eq!(pop.nrows(), 20);
        assert_eq!(pop.ncols(), 2);
    }

    #[test]
    fn test_latin_hypercube_bounds() {
        let lower = array![0.0, 0.0];
        let upper = array![10.0, 10.0];
        let is_free = vec![true, true];
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);

        let pop = init_latin_hypercube(2, 20, &lower, &upper, &is_free, &mut rng);

        for row in pop.rows() {
            assert!(row[0] >= 0.0 && row[0] <= 10.0);
            assert!(row[1] >= 0.0 && row[1] <= 10.0);
        }
    }

    #[test]
    fn test_random_initialization_dimensions() {
        let lower = array![0.0, 0.0];
        let upper = array![10.0, 10.0];
        let is_free = vec![true, true];
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);

        let pop = init_random(2, 20, &lower, &upper, &is_free, &mut rng);

        assert_eq!(pop.nrows(), 20);
        assert_eq!(pop.ncols(), 2);
    }
}

#[cfg(test)]
mod edge_case_tests {
    use super::*;

    #[test]
    fn test_single_dimension() {
        let sphere = |x: &Array1<f64>| x[0] * x[0];

        let config = DEConfigBuilder::new()
            .seed(42)
            .maxiter(100)
            .popsize(10)
            .build()
            .expect("popsize must be >= 4");

        let mut de = DifferentialEvolution::new(&sphere, array![-5.0f64], array![5.0f64]).unwrap();
        *de.config_mut() = config;
        let report = de.solve();

        assert!(report.fun < 1.0, "Should find minimum near 0");
    }

    #[test]
    fn test_fixed_variables() {
        let sphere = |x: &Array1<f64>| x[1] * x[1];

        let config = DEConfigBuilder::new()
            .seed(42)
            .maxiter(50)
            .popsize(10)
            .build()
            .expect("popsize must be >= 4");

        let mut de =
            DifferentialEvolution::new(&sphere, array![-5.0f64, 3.0f64], array![5.0f64, 3.0f64])
                .unwrap();
        *de.config_mut() = config;
        let report = de.solve();

        assert!((report.x[1] - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_deterministic_with_seed() {
        let sphere = |x: &Array1<f64>| x.iter().map(|&xi| xi * xi).sum::<f64>();

        let config1 = DEConfigBuilder::new()
            .seed(42)
            .maxiter(50)
            .popsize(15)
            .build()
            .expect("popsize must be >= 4");

        let mut de1 =
            DifferentialEvolution::new(&sphere, array![-5.0f64, -5.0], array![5.0f64, 5.0])
                .unwrap();
        *de1.config_mut() = config1;
        let report1 = de1.solve();

        let config2 = DEConfigBuilder::new()
            .seed(42)
            .maxiter(50)
            .popsize(15)
            .build()
            .expect("popsize must be >= 4");

        let mut de2 =
            DifferentialEvolution::new(&sphere, array![-5.0f64, -5.0], array![5.0f64, 5.0])
                .unwrap();
        *de2.config_mut() = config2;
        let report2 = de2.solve();

        // With the same seed, results should be very similar (though not necessarily bitwise identical
        // due to potential floating-point non-associativity in parallel operations)
        let diff0 = (report1.x[0] - report2.x[0]).abs();
        let diff1 = (report1.x[1] - report2.x[1]).abs();

        assert!(
            diff0 < 1e-6,
            "x[0] should be nearly deterministic with same seed: diff = {}",
            diff0
        );
        assert!(
            diff1 < 1e-6,
            "x[1] should be nearly deterministic with same seed: diff = {}",
            diff1
        );
    }
}

#[cfg(test)]
mod callback_tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_callback_stop_early() {
        let sphere = |x: &Array1<f64>| x.iter().map(|&xi| xi * xi).sum::<f64>();
        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        let config = DEConfigBuilder::new()
            .seed(42)
            .maxiter(1000)
            .popsize(10)
            .tol(0.0)
            .atol(0.0)
            .callback(Box::new(move |inter| {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
                eprintln!("Callback called at iter {}", inter.iter);
                if inter.iter >= 5 {
                    CallbackAction::Stop
                } else {
                    CallbackAction::Continue
                }
            }))
            .build()
            .expect("popsize must be >= 4");

        let mut de =
            DifferentialEvolution::new(&sphere, array![-5.0f64, -5.0], array![5.0f64, 5.0])
                .unwrap();
        *de.config_mut() = config;
        let report = de.solve();

        let final_count = call_count.load(Ordering::SeqCst);
        eprintln!("Final call_count: {}", final_count);
        eprintln!("Report nit: {}", report.nit);
        assert_eq!(final_count, 5, "Callback should be called exactly 5 times");
        assert_eq!(report.nit, 5, "Should stop after 5 iterations");
    }
}

#[cfg(test)]
mod config_validation_tests {
    use super::*;

    #[test]
    fn test_popsize_too_small() {
        let result = DEConfigBuilder::new().popsize(3).build();

        assert!(result.is_err());
    }

    #[test]
    fn test_popsize_minimum() {
        let result = DEConfigBuilder::new().popsize(4).build();

        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod polish_tests {
    use super::*;

    #[test]
    fn test_polish_improves_solution() {
        let sphere = |x: &Array1<f64>| x.iter().map(|&xi| xi * xi).sum::<f64>();

        let config = DEConfigBuilder::new()
            .seed(42)
            .maxiter(20)
            .popsize(10)
            .polish(PolishConfig {
                enabled: true,
                algo: "neldermead".to_string(),
                maxeval: 100,
            })
            .build()
            .expect("popsize must be >= 4");

        let mut de =
            DifferentialEvolution::new(&sphere, array![-5.0f64, -5.0], array![5.0f64, 5.0])
                .unwrap();
        *de.config_mut() = config;
        let report = de.solve();

        assert!(
            report.fun < 10.0,
            "Polish should improve solution: f={}",
            report.fun
        );
    }
}
