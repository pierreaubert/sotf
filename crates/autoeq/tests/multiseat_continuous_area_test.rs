//! Integration tests for `MultiSeatStrategy::ContinuousArea`.

use autoeq::Curve;
use autoeq::roomeq::{
    AreaPriorKind, AreaQuadratureKind, AreaScalarisationKind, ContinuousListeningAreaConfig,
    MultiSeatConfig, MultiSeatMeasurements, MultiSeatStrategy, optimize_multiseat_continuous_area,
};
use ndarray::Array1;

fn create_test_curve(spl_offset: f64, phase_offset: f64) -> Curve {
    let freqs: Vec<f64> = (0..50)
        .map(|i| 20.0 * (200.0 / 20.0_f64).powf(i as f64 / 49.0))
        .collect();
    let spl: Vec<f64> = freqs.iter().map(|_| 90.0 + spl_offset).collect();
    let phase: Vec<f64> = freqs
        .iter()
        .map(|f| -180.0 * f / 100.0 + phase_offset)
        .collect();
    Curve {
        freq: Array1::from(freqs),
        spl: Array1::from(spl),
        phase: Some(Array1::from(phase)),
        ..Default::default()
    }
}

fn make_measurements_2x2() -> MultiSeatMeasurements {
    // 2 subs × 4 seats arranged on a 2D grid.
    let measurements = vec![
        vec![
            create_test_curve(0.0, 0.0),
            create_test_curve(2.0, 10.0),
            create_test_curve(-1.0, 5.0),
            create_test_curve(1.0, 8.0),
        ],
        vec![
            create_test_curve(-1.0, 5.0),
            create_test_curve(1.0, 15.0),
            create_test_curve(0.0, 12.0),
            create_test_curve(0.5, 7.0),
        ],
    ];
    MultiSeatMeasurements::new(measurements).expect("ok")
}

#[test]
fn continuous_area_rejects_when_strategy_set_but_config_missing() {
    let ms = make_measurements_2x2();
    let cfg = MultiSeatConfig {
        enabled: true,
        strategy: MultiSeatStrategy::ContinuousArea,
        continuous_area: None,
        ..Default::default()
    };
    let err = optimize_multiseat_continuous_area(&ms, &cfg, (20.0, 200.0), 48000.0).unwrap_err();
    assert!(
        err.to_string().contains("continuous_area"),
        "unexpected error: {err}"
    );
}

#[test]
fn continuous_area_rejects_unsupported_dimension() {
    let ms = make_measurements_2x2();
    let area = ContinuousListeningAreaConfig {
        dimensions: 4, // unsupported
        bounds: vec![(0.0, 1.0), (0.0, 1.0), (0.0, 1.0), (0.0, 1.0)],
        seat_positions: vec![
            vec![0.0, 0.0, 0.0, 0.0],
            vec![1.0, 0.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0, 0.0],
            vec![1.0, 1.0, 0.0, 0.0],
        ],
        prior: AreaPriorKind::Uniform,
        quadrature: AreaQuadratureKind::Sobol {
            num_points: 8,
            seed: 0,
        },
        scalarisation: AreaScalarisationKind::ExpectedValue,
        idw_power: 2.0,
    };
    let cfg = MultiSeatConfig {
        enabled: true,
        strategy: MultiSeatStrategy::ContinuousArea,
        continuous_area: Some(area),
        ..Default::default()
    };
    let err = optimize_multiseat_continuous_area(&ms, &cfg, (20.0, 200.0), 48000.0).unwrap_err();
    assert!(
        err.to_string().contains("dimensions"),
        "unexpected error: {err}"
    );
}

#[test]
fn continuous_area_2d_uniform_sobol_runs_end_to_end() {
    let ms = make_measurements_2x2();
    let area = ContinuousListeningAreaConfig {
        dimensions: 2,
        bounds: vec![(0.0, 1.0), (0.0, 1.0)],
        seat_positions: vec![
            vec![0.0, 0.0],
            vec![1.0, 0.0],
            vec![0.0, 1.0],
            vec![1.0, 1.0],
        ],
        prior: AreaPriorKind::Uniform,
        quadrature: AreaQuadratureKind::Sobol {
            num_points: 16,
            seed: 0,
        },
        scalarisation: AreaScalarisationKind::ExpectedValue,
        idw_power: 2.0,
    };
    let cfg = MultiSeatConfig {
        enabled: true,
        strategy: MultiSeatStrategy::ContinuousArea,
        continuous_area: Some(area),
        ..Default::default()
    };
    let result = optimize_multiseat_continuous_area(&ms, &cfg, (20.0, 200.0), 48000.0).expect("ok");
    assert_eq!(result.gains.len(), 2);
    assert_eq!(result.delays.len(), 2);
    assert_eq!(result.strategy, MultiSeatStrategy::ContinuousArea);
    assert_eq!(result.objective_name, "continuous_area");
    // Identity sub 0 (gain=0, delay=0) — same convention as discrete MSO.
    assert_eq!(result.gains[0], 0.0);
    assert_eq!(result.delays[0], 0.0);
    // Continuous-area path doesn't compute discrete-seat variance.
    assert_eq!(result.variance_before, 0.0);
    assert_eq!(result.variance_after, 0.0);
    // Improvement is non-negative (or rejected back to identity).
    assert!(
        result.objective_after <= result.objective_before + 1e-9,
        "objective should not regress: before={}, after={}",
        result.objective_before,
        result.objective_after
    );
}

#[test]
fn continuous_area_1d_line_runs_end_to_end() {
    // 1D listening line with 3 calibration points.
    let measurements = vec![
        vec![
            create_test_curve(0.0, 0.0),
            create_test_curve(1.0, 5.0),
            create_test_curve(-1.0, 10.0),
        ],
        vec![
            create_test_curve(-0.5, 3.0),
            create_test_curve(0.5, 8.0),
            create_test_curve(0.0, 12.0),
        ],
    ];
    let ms = MultiSeatMeasurements::new(measurements).expect("ok");

    let area = ContinuousListeningAreaConfig {
        dimensions: 1,
        bounds: vec![(0.0, 2.0)],
        seat_positions: vec![vec![0.0], vec![1.0], vec![2.0]],
        prior: AreaPriorKind::Uniform,
        quadrature: AreaQuadratureKind::GaussLegendre { points_per_axis: 4 },
        scalarisation: AreaScalarisationKind::ExpectedValue,
        idw_power: 2.0,
    };
    let cfg = MultiSeatConfig {
        enabled: true,
        strategy: MultiSeatStrategy::ContinuousArea,
        continuous_area: Some(area),
        ..Default::default()
    };
    let result = optimize_multiseat_continuous_area(&ms, &cfg, (20.0, 200.0), 48000.0).expect("ok");
    assert_eq!(result.gains.len(), 2);
    assert_eq!(result.objective_name, "continuous_area");
}
