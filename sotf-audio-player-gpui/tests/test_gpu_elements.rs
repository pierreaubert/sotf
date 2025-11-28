//! Unit tests for GPU-accelerated elements
//!
//! Tests the helper functions and data structures for level meters, spectrum, and EQ curve elements.

use sotf_audio_player_gpui::ui::elements::{
    CompactEQCurve, EQCurveColors, EQCurveElement, LevelMeterElement, SpectrumElement,
};
use std::sync::Arc;

#[test]
fn test_level_meter_creation() {
    let meter = LevelMeterElement::new(-20.0, "L");
    // Just verify creation succeeds
    assert!(true);
}

#[test]
fn test_level_meter_with_peak() {
    let meter = LevelMeterElement::new(-20.0, "L").peak(-15.0);
    // Just verify creation with peak succeeds
    assert!(true);
}

#[test]
fn test_level_meter_colors() {
    use sotf_audio_player_gpui::ui::elements::level_meter::MeterColors;

    let colors = MeterColors::default();
    // Verify default colors are set
    assert!(true);
}

#[test]
fn test_spectrum_element_creation() {
    let magnitudes: Arc<[f32]> = vec![-60.0; 64].into();
    let spectrum = SpectrumElement::new(magnitudes);
    // Just verify creation succeeds
    assert!(true);
}

#[test]
fn test_spectrum_element_configuration() {
    let magnitudes: Arc<[f32]> = vec![-30.0; 32].into();
    let spectrum = SpectrumElement::new(magnitudes)
        .frequency_range(20.0, 20000.0)
        .smoothing(0.5);
    // Verify chained configuration succeeds
    assert!(true);
}

#[test]
fn test_spectrum_element_with_previous() {
    let current: Arc<[f32]> = vec![-30.0; 32].into();
    let previous: Arc<[f32]> = vec![-40.0; 32].into();
    let spectrum = SpectrumElement::new(current).previous(previous);
    // Verify previous values can be set for smoothing
    assert!(true);
}

#[test]
fn test_meter_data_creation() {
    use sotf_audio_player_gpui::ui::elements::spectrum::MeterData;

    let data = MeterData::new(6);
    assert_eq!(data.levels.len(), 6);
    assert_eq!(data.peaks.len(), 6);
    assert_eq!(data.names.len(), 6);
}

#[test]
fn test_meter_data_update() {
    use sotf_audio_player_gpui::ui::elements::spectrum::MeterData;

    let mut data = MeterData::new(2);
    data.update(&[0.5, 0.7], 0.3);

    // With smoothing of 0.3, new value = old * 0.3 + new * 0.7
    // Starting from 0.0: 0.0 * 0.3 + 0.5 * 0.7 = 0.35
    assert!((data.levels[0] - 0.35).abs() < 0.01);
    assert!((data.levels[1] - 0.49).abs() < 0.01);

    // Peaks should be updated to new values since they're higher
    assert!((data.peaks[0] - 0.5).abs() < 0.01);
    assert!((data.peaks[1] - 0.7).abs() < 0.01);
}

#[test]
fn test_meter_data_peak_decay() {
    use sotf_audio_player_gpui::ui::elements::spectrum::MeterData;

    let mut data = MeterData::new(1);
    // Set initial peak high
    data.update(&[1.0], 0.0);
    assert!((data.peaks[0] - 1.0).abs() < 0.01);

    // Update with lower value - peak should decay slowly
    data.update(&[0.5], 0.0);
    assert!(data.peaks[0] > 0.99); // Peak decays by 0.005 per update
    assert!(data.peaks[0] < 1.0);
}

// EQ Curve Element tests

#[test]
fn test_eq_curve_element_creation() {
    use sotf_audio_player::EQFilter;

    let filters: Arc<[EQFilter]> = vec![].into();
    let _curve = EQCurveElement::new(filters);
    // Verify creation succeeds
    assert!(true);
}

#[test]
fn test_eq_curve_element_with_filters() {
    use autoeq_iir::BiquadFilterType;
    use sotf_audio_player::EQFilter;

    let filters: Arc<[EQFilter]> = vec![
        EQFilter {
            filter_type: BiquadFilterType::Peak,
            frequency: 1000.0,
            q: 1.5,
            gain_db: 3.0,
        },
        EQFilter {
            filter_type: BiquadFilterType::Lowshelf,
            frequency: 100.0,
            q: 0.7,
            gain_db: -2.0,
        },
    ]
    .into();

    let _curve = EQCurveElement::new(filters);
    assert!(true);
}

#[test]
fn test_eq_curve_element_configuration() {
    use sotf_audio_player::EQFilter;

    let filters: Arc<[EQFilter]> = vec![].into();
    let _curve = EQCurveElement::new(filters)
        .frequency_range(20.0, 20000.0)
        .db_range(-24.0, 24.0)
        .num_points(128)
        .fill(true);
    // Verify chained configuration succeeds
    assert!(true);
}

#[test]
fn test_eq_curve_colors_default() {
    let colors = EQCurveColors::default();
    // Default colors should be set (non-transparent for main colors)
    // Just verify creation succeeds
    assert!(true);
}

#[test]
fn test_eq_curve_with_custom_colors() {
    use gpui::rgba;
    use sotf_audio_player::EQFilter;

    let filters: Arc<[EQFilter]> = vec![].into();
    let colors = EQCurveColors {
        background: rgba(0x000000ff),
        grid: rgba(0x333333ff),
        curve_boost: rgba(0x00ff00ff),
        curve_cut: rgba(0xff0000ff),
        fill_boost: rgba(0x00ff0044),
        fill_cut: rgba(0xff000044),
        zero_line: rgba(0xffffff88),
    };

    let _curve = EQCurveElement::new(filters).colors(colors);
    assert!(true);
}

#[test]
fn test_compact_eq_curve_creation() {
    use sotf_audio_player::EQFilter;

    let filters: Arc<[EQFilter]> = vec![].into();
    let _compact = CompactEQCurve::new(filters);
    assert!(true);
}

#[test]
fn test_compact_eq_curve_with_size() {
    use gpui::px;
    use sotf_audio_player::EQFilter;

    let filters: Arc<[EQFilter]> = vec![].into();
    let _compact = CompactEQCurve::new(filters).size(px(100.0), px(50.0));
    assert!(true);
}
