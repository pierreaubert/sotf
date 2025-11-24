use autoeq_de::{DEConfigBuilder, Mutation, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::{de_jong_step2, dejong_f5_foxholes};
use ndarray::Array1;

#[test]
fn test_de_dejong_sphere() {
    let b10 = vec![(-5.12, 5.12); 10];
    let c = DEConfigBuilder::new()
        .seed(34)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.9)
        .build();
    // f1 sphere 10D
    let f1 = |x: &Array1<f64>| x.iter().map(|v| v * v).sum::<f64>();
    {
        let result = run_recorded_differential_evolution("dejong_sphere", f1, &b10, c);
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(report.fun < 1e-3)
    };
}

#[test]
fn test_de_dejong_step() {
    let b10 = vec![(-5.12, 5.12); 10];
    let c = DEConfigBuilder::new()
        .seed(34)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.9)
        .build();
    // f3 step 10D
    {
        let result = run_recorded_differential_evolution("dejong_step", de_jong_step2, &b10, c);
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(report.fun <= 10.0)
    };
}

#[test]
fn test_de_dejong_foxholes() {
    let bfox = vec![(-65.536, 65.536), (-65.536, 65.536)];
    let cfgf5 = DEConfigBuilder::new()
        .seed(35)
        .maxiter(1500)
        .popsize(60)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .mutation(Mutation::Range { min: 0.4, max: 1.2 })
        .build();
    {
        let result = run_recorded_differential_evolution(
            "dejong_foxholes",
            dejong_f5_foxholes,
            &bfox,
            cfgf5,
        );
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(report.fun < 1.0)
    };
}
