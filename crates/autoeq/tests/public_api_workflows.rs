//! Integration tests for AutoEQ public API workflows.
//!
//! These tests call into the `autoeq` crate as an external consumer would,
//! exercising the public optimize/read/plot/x2peq functions directly rather
//! than going through the CLI binary. They use synthetic curves and fixtures
//! from `data_tests/` so they can run from a workspace checkout.

use std::path::PathBuf;
#[cfg(feature = "plotly")]
use std::sync::Arc;
#[cfg(feature = "plotly")]
use std::task::{Context, Poll, Wake};

use autoeq::Curve;
use autoeq::cli::{Args, PeqModel};
use autoeq::iir::{Biquad, BiquadFilterType};
use autoeq::loss::{CrossoverType, DriverMeasurement, DriversLossData};
use autoeq::read::{
    PsychoacousticSmoothingConfig, create_log_frequency_grid, normalize_and_interpolate_response,
    read_curve_from_csv, smooth_psychoacoustic,
};
use autoeq::workflow::{
    compute_visualization_curves, optimize_drivers_crossover, optimize_headphone, optimize_multisub,
};
use autoeq::x2peq::{peq2x, x2peq};
use ndarray::Array1;

/// Minimal spin-lock executor for the one `async` public function we exercise.
/// The futures involved are effectively synchronous (they do not rely on a
/// reactor), so busy-polling until `Ready` is sufficient for testing.
#[cfg(feature = "plotly")]
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    struct DummyWaker;
    impl Wake for DummyWaker {
        fn wake(self: Arc<Self>) {}
    }

    let waker = Arc::new(DummyWaker).into();
    let mut context = Context::from_waker(&waker);
    let mut pinned = Box::pin(fut);

    loop {
        match pinned.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

/// Build a headphone-style measurement with a mild bass bump.
fn synthetic_headphone_curve() -> Curve {
    let freq = Array1::from_vec(vec![
        20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0,
    ]);
    let spl = Array1::from_vec(vec![5.0, 4.0, 3.0, 1.0, 0.0, 0.0, -1.0, -2.0, -3.0, -4.0]);
    Curve {
        freq,
        spl,
        ..Default::default()
    }
}

/// Write a `Curve` to disk as a two-column CSV that `read_curve_from_csv` accepts.
fn write_csv(path: &PathBuf, curve: &Curve) {
    let mut content = "frequency,spl\n".to_string();
    for (f, s) in curve.freq.iter().zip(curve.spl.iter()) {
        content.push_str(&format!("{},{}\n", f, s));
    }
    std::fs::write(path, content).unwrap();
}

#[test]
fn test_optimize_headphone_happy_path() {
    let tmp = tempfile::TempDir::new().unwrap();
    let csv_path = tmp.path().join("hp.csv");
    write_csv(&csv_path, &synthetic_headphone_curve());

    let grid = create_log_frequency_grid(100, 20.0, 20000.0);
    let target = Curve {
        freq: grid.clone(),
        spl: Array1::zeros(grid.len()),
        ..Default::default()
    };

    let mut args = Args::headphone_defaults();
    args.num_filters = 3;
    args.loss = autoeq::LossType::HeadphoneFlat;
    args.maxeval = 500;
    args.population = 40;
    args.seed = Some(42);
    args.min_freq = 20.0;
    args.max_freq = 16000.0;
    args.min_db = 1.0;
    args.max_db = 6.0;

    let result = optimize_headphone(&csv_path, &target, &args, None, None::<fn(&_) -> _>)
        .expect("headphone optimization should succeed");

    assert!(
        !result.biquads.is_empty(),
        "optimization should produce at least one biquad"
    );
    assert_eq!(
        result.biquads.len(),
        args.num_filters,
        "biquad count should match requested filter count"
    );
    // `optimize_headphone` always normalizes to a hard-coded 200-point grid.
    assert_eq!(
        result.curves.frequencies.len(),
        200,
        "visualization curves should cover the standard 200-point grid"
    );
    assert!(
        result.curves.corrected_curve.iter().all(|v| v.is_finite()),
        "corrected curve must be finite"
    );
    // Loss should not increase over the optimization history when history is present.
    if result.history.len() >= 2 {
        assert!(
            result.final_loss <= result.initial_loss,
            "final loss ({}) should not exceed initial loss ({})",
            result.final_loss,
            result.initial_loss
        );
    }
}

#[test]
fn test_optimize_headphone_missing_file_error_path() {
    let missing = PathBuf::from("/nonexistent/autoeq/measurement.csv");
    let target = Curve {
        freq: create_log_frequency_grid(50, 20.0, 20000.0),
        spl: Array1::zeros(50),
        ..Default::default()
    };
    let args = Args::headphone_defaults();

    let result = optimize_headphone(&missing, &target, &args, None, None::<fn(&_) -> _>);
    assert!(
        result.is_err(),
        "optimizing a missing measurement should return an error"
    );
    let err_string = format!("{}", result.unwrap_err());
    assert!(
        err_string.contains("No such file")
            || err_string.contains("not found")
            || err_string.contains("NoValidData"),
        "unexpected error message: {}",
        err_string
    );
}

#[test]
fn test_read_curve_from_csv_roundtrip() {
    let tmp = tempfile::TempDir::new().unwrap();
    let csv_path = tmp.path().join("curve.csv");

    let csv = "frequency,spl,phase\n\
        20.0,0.0,-1.0\n\
        50.0,-0.5,-3.0\n\
        200.0,-2.0,-10.0\n\
        1000.0,-6.0,-30.0\n\
        4000.0,-12.0,-60.0\n\
        10000.0,-20.0,-85.0\n";
    std::fs::write(&csv_path, csv).unwrap();

    let curve = read_curve_from_csv(&csv_path).expect("CSV should parse");
    assert_eq!(curve.freq.len(), 6);
    assert_eq!(curve.spl.len(), 6);
    assert!(
        curve.phase.is_some(),
        "phase column should be parsed when present"
    );
    // The loader decomposes phase into minimum-phase / excess-phase / delay caches.
    assert!(
        curve.min_phase.is_some(),
        "min_phase cache should be populated"
    );
    assert!(
        curve.excess_phase.is_some(),
        "excess_phase cache should be populated"
    );
    assert!(
        curve.excess_delay_ms.is_some(),
        "excess_delay_ms should be populated"
    );
}

#[test]
fn test_read_curve_from_csv_invalid_input_errors() {
    let tmp = tempfile::TempDir::new().unwrap();
    let csv_path = tmp.path().join("garbage.csv");
    std::fs::write(&csv_path, "this is not valid measurement data\n").unwrap();

    let result = read_curve_from_csv(&csv_path);
    assert!(result.is_err(), "invalid CSV should fail to parse");
}

#[test]
fn test_normalize_and_interpolate_response() {
    let grid = create_log_frequency_grid(64, 20.0, 20000.0);
    let curve = synthetic_headphone_curve();

    let normalized = normalize_and_interpolate_response(&grid, &curve);
    assert_eq!(
        normalized.freq.len(),
        grid.len(),
        "normalized curve should use the target grid"
    );
    assert_eq!(
        normalized.spl.len(),
        grid.len(),
        "normalized SPL should use the target grid"
    );
    assert!(
        normalized.spl.iter().all(|v| v.is_finite()),
        "all normalized SPL values must be finite"
    );
}

#[test]
fn test_smooth_psychoacoustic_preserves_length() {
    let n = 128;
    let freq = Array1::from_vec(
        (0..n)
            .map(|i| 20.0 * (1000.0_f64).powf(i as f64 / n as f64))
            .collect(),
    );
    let spl = Array1::from_vec(
        freq.iter()
            .map(|f| 2.0 * (f / 1000.0).sin())
            .collect::<Vec<_>>(),
    );
    let curve = Curve {
        freq,
        spl,
        ..Default::default()
    };

    let smoothed = smooth_psychoacoustic(&curve, &PsychoacousticSmoothingConfig::default());
    assert_eq!(smoothed.freq.len(), n);
    assert_eq!(smoothed.spl.len(), n);
    assert!(
        smoothed.spl.iter().all(|v| v.is_finite()),
        "smoothed curve must be finite"
    );
}

#[test]
fn test_compute_visualization_curves() {
    let grid = create_log_frequency_grid(100, 20.0, 20000.0);
    let input = Curve {
        freq: grid.clone(),
        spl: Array1::from_elem(grid.len(), 0.0),
        ..Default::default()
    };
    let target = Curve {
        freq: grid.clone(),
        spl: Array1::from_elem(grid.len(), 0.0),
        ..Default::default()
    };
    let biquads = vec![
        Biquad::new(BiquadFilterType::Peak, 250.0, 48000.0, 1.5, 3.0),
        Biquad::new(BiquadFilterType::Peak, 4000.0, 48000.0, 2.0, -4.0),
    ];

    let curves = compute_visualization_curves(&grid.to_vec(), &input, &target, &biquads);

    assert_eq!(curves.frequencies.len(), grid.len());
    assert_eq!(curves.input_curve.len(), grid.len());
    assert_eq!(curves.target_curve.len(), grid.len());
    assert_eq!(curves.deviation_curve.len(), grid.len());
    assert_eq!(curves.filter_response.len(), grid.len());
    assert_eq!(curves.error_curve.len(), grid.len());
    assert_eq!(curves.corrected_curve.len(), grid.len());
    assert_eq!(
        curves.individual_filter_responses.len(),
        biquads.len(),
        "there should be one individual response per filter"
    );
    for response in &curves.individual_filter_responses {
        assert_eq!(response.len(), grid.len());
    }
}

#[test]
fn test_x2peq_peq2x_roundtrip_pk() {
    let x_original = vec![200.0_f64.log10(), 1.0, 3.0, 4000.0_f64.log10(), 2.0, -4.0];

    let peq = x2peq(&x_original, 48000.0, PeqModel::Pk);
    assert_eq!(peq.len(), 2, "PK model should yield two filters");

    let x_recovered = peq2x(&peq, PeqModel::Pk);
    assert_eq!(
        x_recovered.len(),
        x_original.len(),
        "recovered parameter vector length should match original"
    );
    for (i, (orig, rec)) in x_original.iter().zip(x_recovered.iter()).enumerate() {
        assert!(
            (orig - rec).abs() < 1e-9,
            "parameter {} differs: {} vs {}",
            i,
            orig,
            rec
        );
    }
}

#[test]
fn test_optimize_drivers_crossover_happy_path() {
    let woofer = DriverMeasurement::new(
        Array1::from_vec(vec![20.0, 50.0, 100.0, 200.0, 500.0, 1000.0]),
        Array1::from_vec(vec![80.0, 82.0, 81.0, 79.0, 76.0, 72.0]),
        None,
    );
    let tweeter = DriverMeasurement::new(
        Array1::from_vec(vec![500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0]),
        Array1::from_vec(vec![68.0, 72.0, 75.0, 76.0, 77.0, 78.0]),
        None,
    );
    let drivers_data = DriversLossData::new(vec![woofer, tweeter], CrossoverType::LinkwitzRiley4);

    let result = optimize_drivers_crossover(
        drivers_data,
        100.0,
        10000.0,
        48000.0,
        "autoeq:de",
        200,
        30,
        -12.0,
        12.0,
        None,
        Some(42),
    )
    .expect("driver crossover optimization should succeed");

    assert_eq!(result.gains.len(), 2);
    assert_eq!(result.delays.len(), 2);
    assert_eq!(result.crossover_freqs.len(), 1);
    assert!(
        result.crossover_freqs[0] > 100.0 && result.crossover_freqs[0] < 5000.0,
        "crossover frequency should lie inside the evaluation band"
    );
    assert!(
        result.post_objective.is_finite(),
        "post-optimization objective must be finite"
    );
}

#[test]
fn test_optimize_drivers_crossover_fixed_frequencies() {
    let woofer = DriverMeasurement::new(
        Array1::from_vec(vec![20.0, 100.0, 500.0, 1000.0]),
        Array1::from_vec(vec![80.0, 81.0, 78.0, 74.0]),
        None,
    );
    let tweeter = DriverMeasurement::new(
        Array1::from_vec(vec![1000.0, 5000.0, 10000.0, 20000.0]),
        Array1::from_vec(vec![70.0, 75.0, 76.0, 77.0]),
        None,
    );
    let drivers_data = DriversLossData::new(vec![woofer, tweeter], CrossoverType::LinkwitzRiley2);

    let result = optimize_drivers_crossover(
        drivers_data,
        100.0,
        10000.0,
        48000.0,
        "autoeq:de",
        150,
        20,
        -12.0,
        12.0,
        Some(vec![1500.0]),
        Some(7),
    )
    .expect("driver optimization with fixed crossover should succeed");

    assert!(
        (result.crossover_freqs[0] - 1500.0).abs() < 1e-6,
        "fixed crossover frequency should be preserved"
    );
}

#[test]
fn test_optimize_multisub_happy_path() {
    let sub1 = DriverMeasurement::new(
        Array1::from_vec(vec![20.0, 30.0, 50.0, 80.0, 120.0, 200.0]),
        Array1::from_vec(vec![82.0, 84.0, 85.0, 83.0, 80.0, 76.0]),
        None,
    );
    let sub2 = DriverMeasurement::new(
        Array1::from_vec(vec![20.0, 30.0, 50.0, 80.0, 120.0, 200.0]),
        Array1::from_vec(vec![80.0, 82.0, 84.0, 85.0, 82.0, 78.0]),
        None,
    );
    let drivers_data = DriversLossData::new(vec![sub1, sub2], CrossoverType::None);

    let result = optimize_multisub(
        drivers_data,
        20.0,
        200.0,
        48000.0,
        "autoeq:de",
        150,
        20,
        -12.0,
        12.0,
        Some(13),
    )
    .expect("multisub optimization should succeed");

    assert_eq!(result.gains.len(), 2);
    assert_eq!(result.delays.len(), 2);
    assert!(
        result.crossover_freqs.is_empty(),
        "multisub has no crossover frequencies"
    );
    assert!(
        result.post_objective <= result.pre_objective,
        "multisub optimization should not increase the objective"
    );
}

#[cfg(feature = "plotly")]
#[test]
fn test_plot_results_writes_html() {
    use std::collections::HashMap;

    let tmp = tempfile::TempDir::new().unwrap();
    let output_path = tmp.path().join("report.png");

    let grid = create_log_frequency_grid(60, 20.0, 20000.0);
    let input = Curve {
        freq: grid.clone(),
        spl: Array1::from_elem(grid.len(), 2.0),
        ..Default::default()
    };
    let target = Curve {
        freq: grid.clone(),
        spl: Array1::from_elem(grid.len(), 0.0),
        ..Default::default()
    };
    let deviation = Curve {
        freq: grid.clone(),
        spl: &target.spl - &input.spl,
        ..Default::default()
    };

    let mut args = Args::speaker_defaults();
    args.num_filters = 3;
    args.sample_rate = 48000.0;
    args.min_freq = 20.0;
    args.max_freq = 20000.0;

    // Three PK filters: 200 Hz +3 dB, 1 kHz -4 dB, 5 kHz +2 dB.
    let optimized_params = vec![
        200.0_f64.log10(),
        1.0,
        3.0,
        1000.0_f64.log10(),
        1.5,
        -4.0,
        5000.0_f64.log10(),
        1.0,
        2.0,
    ];

    let cea2034_curves: Option<HashMap<String, Curve>> = None;

    let result = block_on(autoeq::plot::plot_results(
        &args,
        &optimized_params,
        &input,
        &target,
        &deviation,
        &cea2034_curves,
        &output_path,
    ));

    assert!(result.is_ok(), "plot_results failed: {:?}", result.err());
    let html_path = output_path.with_extension("html");
    assert!(
        html_path.exists(),
        "HTML report should be written at {:?}",
        html_path
    );
    let html = std::fs::read_to_string(&html_path).unwrap();
    assert!(
        html.contains("IIR Filter Optimization Results"),
        "HTML report should contain the expected title"
    );
}
