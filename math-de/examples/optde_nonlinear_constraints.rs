use autoeq_de::{
    Crossover, DEConfigBuilder, NonlinearConstraintHelper, Strategy, differential_evolution,
};
use ndarray::Array1;
use std::str::FromStr;
use std::sync::Arc;

fn main() {
    // Himmelblau as objective, but with nonlinear constraints to demonstrate helper
    let himmelblau =
        |x: &Array1<f64>| (x[0] * x[0] + x[1] - 11.0).powi(2) + (x[0] + x[1] * x[1] - 7.0).powi(2);

    // Bounds
    let bounds = [(-6.0, 6.0), (-6.0, 6.0)];

    // Nonlinear vector function f(x) with 2 components
    // 1) Circle-ish constraint: x0^2 + x1^2 <= 10  -> f0(x) = x0^2 + x1^2,  lb=-inf, ub=10
    // 2) Sum equality: x0 + x1 = 1  -> f1(x) = x0 + x1,  lb=1, ub=1
    let fun =
        Arc::new(|x: &Array1<f64>| Array1::from(vec![x[0] * x[0] + x[1] * x[1], x[0] + x[1]]));
    let lb = Array1::from(vec![-f64::INFINITY, 1.0]);
    let ub = Array1::from(vec![10.0, 1.0]);
    let nlc = NonlinearConstraintHelper { fun, lb, ub };

    // Strategy parsing from string
    let strategy = Strategy::from_str("best1exp").unwrap_or(Strategy::Best1Exp);

    let mut cfg = DEConfigBuilder::new()
        .seed(456)
        .maxiter(800)
        .popsize(30)
        .strategy(strategy)
        .recombination(0.9)
        .crossover(Crossover::Exponential)
        .build();

    // Apply nonlinear constraints with penalties
    nlc.apply_to(&mut cfg, 1e3, 1e3);

    let rep = differential_evolution(&himmelblau, &bounds, cfg);
    println!(
        "success={} message=\"{}\"\nbest f={:.6e}\nbest x={:?}",
        rep.success, rep.message, rep.fun, rep.x
    );
}
