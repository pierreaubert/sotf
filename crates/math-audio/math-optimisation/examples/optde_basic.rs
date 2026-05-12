use math_audio_optimisation::{
    CallbackAction, Crossover, DEConfig, Mutation, PolishConfig, Strategy, differential_evolution,
};
use ndarray::Array1;
use std::sync::Arc;

fn main() {
    // Ackley function (2D)
    let ackley = |x: &Array1<f64>| {
        let x0 = x[0];
        let x1 = x[1];
        let s = 0.5 * (x0 * x0 + x1 * x1);
        let c = 0.5
            * ((2.0 * std::f64::consts::PI * x0).cos() + (2.0 * std::f64::consts::PI * x1).cos());
        -20.0 * (-0.2 * s.sqrt()).exp() - c.exp() + 20.0 + std::f64::consts::E
    };

    let bounds = [(-5.0, 5.0), (-5.0, 5.0)];

    let mut cfg = DEConfig {
        maxiter: 300,
        popsize: 20,
        strategy: Strategy::Best1Bin,
        crossover: Crossover::Exponential, // demonstrate exponential crossover
        mutation: Mutation::Range { min: 0.5, max: 1.0 }, // dithering
        recombination: 0.9,
        seed: Some(42),
        ..Default::default()
    };

    // Penalty examples (here just a dummy inequality fc(x) <= 0):
    // Circle of radius 3: x0^2 + x1^2 - 9 <= 0
    cfg.penalty_ineq.push((
        Arc::new(|x: &Array1<f64>| x[0] * x[0] + x[1] * x[1] - 9.0),
        1e3,
    ));

    // Callback every generation: stop early when convergence small enough
    let mut iter_log = 0usize;
    cfg.callback = Some(Box::new(move |inter| {
        if iter_log.is_multiple_of(25) {
            eprintln!(
                "iter {:4}  best_f={:.6e}  conv(stdE)={:.3e}",
                inter.iter, inter.fun, inter.convergence
            );
        }
        iter_log += 1;
        if inter.convergence < 1e-6 {
            CallbackAction::Stop
        } else {
            CallbackAction::Continue
        }
    }));

    // Optional polishing with a local optimizer
    cfg.polish = Some(PolishConfig {
        enabled: true,
        maxeval: 400,
    });

    let report = differential_evolution(&ackley, &bounds, cfg).expect("optimization failed");

    println!(
        "success={} message=\"{}\"\nbest f={:.6e}\nbest x={:?}",
        report.success, report.message, report.fun, report.x
    );
}
