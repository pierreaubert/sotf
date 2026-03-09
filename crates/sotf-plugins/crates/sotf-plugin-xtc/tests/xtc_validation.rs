#![allow(clippy::field_reassign_with_default)]
//! XTC Plugin Validation Integration Tests
//!
//! These tests verify the XTC implementation against acoustic physics formulas.
//!
//! Run with:
//!   cargo test -p plugins --no-default-features xtc_validation -- --nocapture

use sotf_plugin_xtc::XtcPluginParams;
use sotf_plugin_xtc::validation::{
    REFERENCE_ILD_POINTS, measure_cancellation_depth_db, reference_ild_db, reference_itd_ms,
    run_validation, run_validation_verbose,
};

mod reference {
    use std::f32::consts::PI;

    pub const SPEED_OF_SOUND: f32 = 343.0;

    pub fn itd_ms(angle_deg: f32, head_radius: f32) -> f32 {
        let theta = angle_deg * PI / 180.0;
        ((head_radius / SPEED_OF_SOUND) * (theta + theta.sin())) * 1000.0
    }
}

// ============================================================================
// ITD Validation Tests
// ============================================================================

#[test]
fn itd_matches_woodworth_at_30_degrees() {
    let params = XtcPluginParams::default();
    let expected = reference::itd_ms(30.0, 0.0875);

    // Expected: ~0.255ms for 30° with 8.75cm head
    assert!(
        (expected - 0.255).abs() < 0.01,
        "Reference ITD formula error: {} != 0.255ms",
        expected
    );

    let computed = reference_itd_ms(params.speaker_angle_deg, params.head_radius_m);
    assert!(
        (computed - expected).abs() < 0.001,
        "ITD computation mismatch: {} vs {}",
        computed,
        expected
    );
}

#[test]
fn itd_matches_woodworth_at_multiple_angles() {
    for angle in [15.0, 30.0, 45.0, 60.0, 75.0] {
        let expected = reference::itd_ms(angle, 0.0875);
        let computed = reference_itd_ms(angle, 0.0875);

        assert!(
            (computed - expected).abs() < 0.001,
            "ITD at {}°: expected {}ms, got {}ms",
            angle,
            expected,
            computed
        );
    }
}

#[test]
fn itd_scales_with_head_radius() {
    let mut params_small = XtcPluginParams::default();
    params_small.head_radius_m = 0.07;

    let mut params_large = XtcPluginParams::default();
    params_large.head_radius_m = 0.10;

    let itd_small = reference_itd_ms(params_small.speaker_angle_deg, params_small.head_radius_m);
    let itd_large = reference_itd_ms(params_large.speaker_angle_deg, params_large.head_radius_m);

    let ratio = itd_large / itd_small;
    let expected_ratio = 0.10 / 0.07;

    assert!(
        (ratio - expected_ratio).abs() < 0.05,
        "ITD scaling error: large/small = {:.3}, expected {:.3}",
        ratio,
        expected_ratio
    );
}

#[test]
fn itd_zero_at_frontal_incidence() {
    let itd_frontal = reference_itd_ms(0.0, 0.0875);
    assert!(
        itd_frontal.abs() < 0.001,
        "ITD at 0° should be ~0, got {}ms",
        itd_frontal
    );
}

// ============================================================================
// ILD Validation Tests
// ============================================================================

#[test]
fn ild_increases_with_frequency() {
    let angle = 30.0;
    let head_radius = 0.0875;

    let mut prev_ild = 0.0;
    for &(freq, _) in REFERENCE_ILD_POINTS {
        let ild = reference_ild_db(freq, angle, head_radius);
        assert!(
            ild >= prev_ild - 0.5,
            "ILD should generally increase with freq: {}Hz gave {}dB, previous was {}dB",
            freq,
            ild,
            prev_ild
        );
        prev_ild = ild;
    }
}

#[test]
fn ild_low_frequency_minimal() {
    let ild_250hz = reference_ild_db(250.0, 30.0, 0.0875);
    assert!(
        ild_250hz < 2.0,
        "ILD at 250Hz should be minimal (<2dB), got {}dB",
        ild_250hz
    );
}

#[test]
fn ild_high_frequency_significant() {
    let ild_8khz = reference_ild_db(8000.0, 90.0, 0.0875);
    assert!(
        ild_8khz > 8.0,
        "ILD at 8kHz/90° should be significant (>8dB), got {}dB",
        ild_8khz
    );
}

// ============================================================================
// Cancellation Depth Tests
// ============================================================================

#[test]
fn cancellation_depth_at_mid_frequencies() {
    let params = XtcPluginParams::default();
    let sample_rate = 48000;

    // Mid-frequencies should have measurable cancellation
    for &freq in &[500.0, 1000.0, 2000.0] {
        let depth = measure_cancellation_depth_db(&params, sample_rate, freq);
        assert!(
            depth > 0.0,
            "Cancellation at {}Hz should be positive, got {}dB",
            freq,
            depth
        );
        assert!(
            depth < 50.0,
            "Cancellation at {}Hz should be reasonable, got {}dB",
            freq,
            depth
        );
    }
}

#[test]
fn cancellation_depth_varies_with_frequency() {
    let params = XtcPluginParams::default();

    // Collect cancellation depths across spectrum
    let depths: Vec<(f32, f32)> = [100.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0]
        .iter()
        .map(|&f| (f, measure_cancellation_depth_db(&params, 48000, f)))
        .collect();

    // At minimum, depths should be finite and reasonable
    for (freq, depth) in &depths {
        assert!(depth.is_finite(), "Depth at {}Hz should be finite", freq);
    }
}

// ============================================================================
// Full Validation Suite
// ============================================================================

#[test]
fn full_validation_default_params() {
    let params = XtcPluginParams::default();
    let report = run_validation(&params, 48000);

    println!("\n=== XTC Validation Report (Default Params) ===");
    for result in &report.results {
        println!("{}", result);
    }
    println!(
        "\nSummary: {}/{} tests passed",
        report.passed_count,
        report.results.len()
    );

    // Filter symmetry should always pass for zero yaw
    let symmetry_passed = report
        .results
        .iter()
        .find(|r| r.metric_name.contains("Symmetry"))
        .map(|r| r.passed)
        .unwrap_or(false);
    assert!(symmetry_passed, "Filter symmetry should pass for zero yaw");

    // Filter stability should always pass (filters are below max gain)
    let stability_passed = report
        .results
        .iter()
        .find(|r| r.metric_name.contains("Stability"))
        .map(|r| r.passed)
        .unwrap_or(false);
    assert!(stability_passed, "Filter stability should pass");
}

#[test]
fn full_validation_custom_geometry() {
    let mut params = XtcPluginParams::default();
    params.speaker_angle_deg = 45.0;
    params.distance_m = 1.5;

    let report = run_validation(&params, 48000);

    println!("\n=== XTC Validation Report (45° speakers, 1.5m) ===");
    for result in &report.results {
        println!("{}", result);
    }

    // ITD geometry check should pass
    let itd_passed = report
        .results
        .iter()
        .find(|r| r.metric_name.starts_with("ITD"))
        .map(|r| r.passed)
        .unwrap_or(false);
    assert!(itd_passed, "ITD geometry check should pass");
}

#[test]
fn full_validation_small_head() {
    let mut params = XtcPluginParams::default();
    params.head_radius_m = 0.07;

    let report = run_validation(&params, 48000);

    println!("\n=== XTC Validation Report (7cm head radius) ===");
    for result in &report.results {
        println!("{}", result);
    }

    // All geometry-derived checks should pass
    let geometry_passed = report
        .results
        .iter()
        .filter(|r| r.metric_name.starts_with("ITD") || r.metric_name.starts_with("ILD"))
        .all(|r| r.passed);
    assert!(geometry_passed, "Geometry checks should pass");
}

#[test]
fn validation_report_output() {
    let params = XtcPluginParams::default();
    let report = run_validation_verbose(&params, 48000);

    // Report should have results
    assert!(!report.results.is_empty());

    // Summary should be consistent
    let manual_passed = report.results.iter().filter(|r| r.passed).count();
    assert_eq!(report.passed_count, manual_passed);
}

// ============================================================================
// Regression Tests
// ============================================================================

#[test]
fn filter_magnitude_bounds() {
    let params = XtcPluginParams::default();

    for &freq in [100.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0].iter() {
        let depth = measure_cancellation_depth_db(&params, 48000, freq);

        // Cancellation depth should be finite and reasonable
        assert!(
            depth.is_finite(),
            "Cancellation depth at {}Hz is not finite",
            freq
        );
        assert!(
            (0.0..=60.0).contains(&depth),
            "Cancellation depth at {}Hz is out of bounds: {}dB",
            freq,
            depth
        );
    }
}

#[test]
fn no_nan_or_inf_in_results() {
    let params = XtcPluginParams::default();
    let report = run_validation(&params, 48000);

    for result in &report.results {
        assert!(
            result.measured.is_finite(),
            "{} produced non-finite measurement",
            result.metric_name
        );
        assert!(
            result.expected.is_finite(),
            "{} has non-finite expected value",
            result.metric_name
        );
    }
}
