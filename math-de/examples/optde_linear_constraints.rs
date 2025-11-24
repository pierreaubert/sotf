use autoeq_de::{
    Crossover, DEConfigBuilder, LinearConstraintHelper, Mutation, Strategy, differential_evolution,
};
use ndarray::{Array1, Array2};
use std::str::FromStr;

fn main() {
    // Objective: sphere in 2D
    let sphere = |x: &Array1<f64>| x.iter().map(|v| v * v).sum::<f64>();

    // Bounds
    let bounds = [(-5.0, 5.0), (-5.0, 5.0)];

    // Linear constraint example: lb <= A x <= ub
    // 1) x0 + x1 <= 1.0
    // 2) 0.2 <= x0 - x1 <= 0.4
    let a = Array2::from_shape_vec((2, 2), vec![1.0, 1.0, 1.0, -1.0]).unwrap();
    let lb = Array1::from(vec![-f64::INFINITY, 0.2]);
    let ub = Array1::from(vec![1.0, 0.4]);
    let lc = LinearConstraintHelper { a, lb, ub };

    // Strategy parsing from string (mirrors SciPy names)
    let strategy = Strategy::from_str("randtobest1exp").unwrap_or(Strategy::RandToBest1Exp);

    // Build config using the fluent builder
    let mut cfg = DEConfigBuilder::new()
        .seed(123)
        .maxiter(600)
        .popsize(30)
        .strategy(strategy)
        .recombination(0.9)
        .mutation(Mutation::Range { min: 0.4, max: 1.0 })
        .crossover(Crossover::Exponential)
        .build();

    // Apply linear constraints with a penalty weight
    lc.apply_to(&mut cfg, 1e3);

    let rep = differential_evolution(&sphere, &bounds, cfg);
    println!(
        "success={} message=\"{}\"\nbest f={:.6e}\nbest x={:?}",
        rep.success, rep.message, rep.fun, rep.x
    );
}
