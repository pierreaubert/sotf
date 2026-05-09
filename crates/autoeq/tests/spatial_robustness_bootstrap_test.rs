//! Integration tests for measurement-uncertainty-aware robust optimization
//! (`MultiMeasurementStrategy::MinimaxUncertainty`).

use autoeq::Curve;
use autoeq::roomeq::spatial_robustness::{BootstrapBand, BootstrapConfig, bootstrap_band};
use ndarray::Array1;

fn make_curve(freq: Vec<f64>, spl: Vec<f64>) -> Curve {
    Curve {
        freq: Array1::from_vec(freq),
        spl: Array1::from_vec(spl),
        phase: None,
        ..Default::default()
    }
}

#[test]
fn bootstrap_band_widens_with_more_input_variability() {
    // Two scenarios:
    //  A) Three nearly-identical curves → tight band.
    //  B) Three highly-divergent curves → much wider band.
    // The wider input variance must produce a wider bootstrap band.
    let freqs = vec![100.0, 1000.0, 5000.0];

    let tight = vec![
        make_curve(freqs.clone(), vec![80.0, 85.0, 75.0]),
        make_curve(freqs.clone(), vec![80.1, 85.1, 75.0]),
        make_curve(freqs.clone(), vec![79.9, 84.9, 75.1]),
    ];
    let wide = vec![
        make_curve(freqs.clone(), vec![70.0, 80.0, 65.0]),
        make_curve(freqs.clone(), vec![85.0, 90.0, 80.0]),
        make_curve(freqs.clone(), vec![90.0, 95.0, 85.0]),
    ];

    let cfg = BootstrapConfig {
        num_resamples: 500,
        alpha: 0.10,
        seed: 1,
    };
    let tight_band = bootstrap_band(&tight, &cfg, None).expect("ok");
    let wide_band = bootstrap_band(&wide, &cfg, None).expect("ok");

    for bin in 0..3 {
        let tight_w = tight_band.upper.spl[bin] - tight_band.lower.spl[bin];
        let wide_w = wide_band.upper.spl[bin] - wide_band.lower.spl[bin];
        assert!(
            wide_w > tight_w,
            "bin {}: wider input ({} dB) should produce wider bootstrap band than tight ({} dB)",
            bin,
            wide_w,
            tight_w
        );
    }
}

#[test]
fn bootstrap_band_per_bin_std_consistency() {
    // The per-bin std field should equal the sample std of the resampled means.
    // We don't see the raw resamples, but we can check that for identical curves
    // the std is zero, and for two-curve disjoint inputs it's non-trivial.
    let identical = vec![
        make_curve(vec![100.0], vec![80.0]),
        make_curve(vec![100.0], vec![80.0]),
    ];
    let cfg = BootstrapConfig {
        num_resamples: 200,
        alpha: 0.10,
        seed: 42,
    };
    let band: BootstrapBand = bootstrap_band(&identical, &cfg, None).expect("ok");
    assert_eq!(band.per_bin_std.len(), 1);
    assert!(band.per_bin_std[0] < 1e-9);

    let disjoint = vec![
        make_curve(vec![100.0], vec![70.0]),
        make_curve(vec![100.0], vec![90.0]),
    ];
    let band2 = bootstrap_band(&disjoint, &cfg, None).expect("ok");
    assert!(band2.per_bin_std[0] > 0.0 && band2.per_bin_std[0].is_finite());
}

#[test]
fn bootstrap_band_median_brackets_input_mean_in_db_power() {
    // For N ≥ 2 i.i.d. resamples in dB SPL, the median of the resampled
    // RMS-averaged means should lie within [min_input_spl, max_input_spl] (dB).
    // RMS power-domain averaging of [70, 80, 90] gives ≈ 85.04 dB; the median
    // of 500 resamples should be near that value.
    let curves = vec![
        make_curve(vec![100.0], vec![70.0]),
        make_curve(vec![100.0], vec![80.0]),
        make_curve(vec![100.0], vec![90.0]),
    ];
    let cfg = BootstrapConfig {
        num_resamples: 500,
        alpha: 0.10,
        seed: 7,
    };
    let band = bootstrap_band(&curves, &cfg, None).expect("ok");
    assert!(band.median.spl[0] > 70.0 && band.median.spl[0] < 90.0);
    // RMS-power mean of the input set is ≈ 85.04 dB; the median should be in
    // the same neighborhood, since each resample is itself an RMS-power mean
    // of three draws-with-replacement from the input.
    assert!(
        (band.median.spl[0] - 85.0).abs() < 4.0,
        "median {} should be near the RMS-power mean ~85 dB",
        band.median.spl[0]
    );
}
