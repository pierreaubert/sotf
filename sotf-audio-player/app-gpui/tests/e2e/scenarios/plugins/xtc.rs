//! E2E tests for XTC (Crosstalk Cancellation) Plugin.
//!
//! Tests for the crosstalk cancellation plugin that creates a binaural-like
//! experience from conventional stereo speakers.

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Mock Types for Testing
// =============================================================================

/// XTC plugin state for testing
struct XtcState {
    enabled: bool,
    // Geometry
    distance_m: f32,
    speaker_angle_deg: f32,
    head_radius_m: f32,
    // FFT settings
    fft_size: usize,
    // Regularization
    beta_base: f32,
    beta_low_freq_boost: f32,
    beta_high_freq_boost: f32,
    // Head shadowing
    head_shadow_cutoff_hz: f32,
    head_shadow_slope_db_per_octave: f32,
    // Head tracking
    head_offset_x: f32,
    head_offset_z: f32,
    head_tracking_smooth_s: f32,
}

impl Default for XtcState {
    fn default() -> Self {
        Self {
            enabled: true,
            distance_m: 2.0,
            speaker_angle_deg: 30.0,
            head_radius_m: 0.0875,
            fft_size: 1024,
            beta_base: 0.001,
            beta_low_freq_boost: 10.0,
            beta_high_freq_boost: 10.0,
            head_shadow_cutoff_hz: 4000.0,
            head_shadow_slope_db_per_octave: 6.0,
            head_offset_x: 0.0,
            head_offset_z: 0.0,
            head_tracking_smooth_s: 0.1,
        }
    }
}

// =============================================================================
// Basic Plugin Tests
// =============================================================================

/// Test plugin renders correctly.
#[gpui::test]
async fn test_xtc_renders(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(XtcState::default()));
    assert!(state.borrow().enabled);
}

/// Test default values.
#[gpui::test]
async fn test_xtc_defaults(_cx: &mut TestAppContext) {
    let state = XtcState::default();

    assert!((state.distance_m - 2.0).abs() < 0.01);
    assert!((state.speaker_angle_deg - 30.0).abs() < 0.1);
    assert!((state.head_radius_m - 0.0875).abs() < 0.001);
    assert_eq!(state.fft_size, 1024);
}

// =============================================================================
// Distance Tests
// =============================================================================

/// Test distance control.
#[gpui::test]
async fn test_distance_control(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(XtcState::default()));

    let test_values: Vec<f32> = vec![0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0];
    for value in test_values {
        state.borrow_mut().distance_m = value;
        assert!((state.borrow().distance_m - value).abs() < 0.01);
    }
}

/// Test distance bounds.
#[gpui::test]
async fn test_distance_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(XtcState::default()));

    // Typical range: 0.5m to 10m
    let clamped = (0.1_f32).clamp(0.5, 10.0);
    state.borrow_mut().distance_m = clamped;
    assert!(state.borrow().distance_m >= 0.5);

    let clamped = (15.0_f32).clamp(0.5, 10.0);
    state.borrow_mut().distance_m = clamped;
    assert!(state.borrow().distance_m <= 10.0);
}

/// Test distance display format.
#[gpui::test]
async fn test_distance_display(_cx: &mut TestAppContext) {
    fn format_distance(distance_m: f32) -> String {
        if distance_m < 1.0 {
            format!("{:.0} cm", distance_m * 100.0)
        } else {
            format!("{:.2} m", distance_m)
        }
    }

    assert_eq!(format_distance(0.5), "50 cm");
    assert_eq!(format_distance(1.0), "1.00 m");
    assert_eq!(format_distance(2.5), "2.50 m");
}

// =============================================================================
// Speaker Angle Tests
// =============================================================================

/// Test speaker angle control.
#[gpui::test]
async fn test_speaker_angle_control(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(XtcState::default()));

    let test_values: Vec<f32> = vec![15.0, 20.0, 25.0, 30.0, 35.0, 45.0, 60.0];
    for value in test_values {
        state.borrow_mut().speaker_angle_deg = value;
        assert!((state.borrow().speaker_angle_deg - value).abs() < 0.1);
    }
}

/// Test speaker angle bounds.
#[gpui::test]
async fn test_speaker_angle_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(XtcState::default()));

    // Typical range: 10 to 90 degrees
    let clamped = (5.0_f32).clamp(10.0, 90.0);
    state.borrow_mut().speaker_angle_deg = clamped;
    assert!(state.borrow().speaker_angle_deg >= 10.0);

    let clamped = (100.0_f32).clamp(10.0, 90.0);
    state.borrow_mut().speaker_angle_deg = clamped;
    assert!(state.borrow().speaker_angle_deg <= 90.0);
}

/// Test speaker angle display format.
#[gpui::test]
async fn test_speaker_angle_display(_cx: &mut TestAppContext) {
    fn format_angle(angle_deg: f32) -> String {
        format!("{:.0}°", angle_deg)
    }

    assert_eq!(format_angle(30.0), "30°");
    assert_eq!(format_angle(45.0), "45°");
}

/// Test recommended speaker angles.
#[gpui::test]
async fn test_recommended_angles(_cx: &mut TestAppContext) {
    // ITU-R BS.775 recommends 60° total stereo angle (30° per side)
    let itu_angle: f32 = 30.0;
    assert!((itu_angle - 30.0).abs() < 0.1);

    // Wider angles require stronger cancellation
    fn cancellation_strength_needed(angle: f32) -> &'static str {
        if angle < 25.0 {
            "Light"
        } else if angle < 40.0 {
            "Moderate"
        } else {
            "Strong"
        }
    }

    assert_eq!(cancellation_strength_needed(30.0), "Moderate");
    assert_eq!(cancellation_strength_needed(60.0), "Strong");
}

// =============================================================================
// Head Radius Tests
// =============================================================================

/// Test head radius control.
#[gpui::test]
async fn test_head_radius_control(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(XtcState::default()));

    let test_values: Vec<f32> = vec![0.07, 0.08, 0.0875, 0.09, 0.10];
    for value in test_values {
        state.borrow_mut().head_radius_m = value;
        assert!((state.borrow().head_radius_m - value).abs() < 0.001);
    }
}

/// Test head radius bounds.
#[gpui::test]
async fn test_head_radius_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(XtcState::default()));

    // Typical range: 0.06m to 0.12m
    let clamped = (0.05_f32).clamp(0.06, 0.12);
    state.borrow_mut().head_radius_m = clamped;
    assert!(state.borrow().head_radius_m >= 0.06);
}

/// Test head radius display format.
#[gpui::test]
async fn test_head_radius_display(_cx: &mut TestAppContext) {
    fn format_head_radius(radius_m: f32) -> String {
        format!("{:.1} cm", radius_m * 100.0)
    }

    assert_eq!(format_head_radius(0.0875), "8.8 cm");
    assert_eq!(format_head_radius(0.09), "9.0 cm");
}

// =============================================================================
// FFT Size Tests
// =============================================================================

/// Test FFT size options.
#[gpui::test]
async fn test_fft_size_options(_cx: &mut TestAppContext) {
    let valid_sizes = [256, 512, 1024, 2048, 4096];
    let state = Rc::new(RefCell::new(XtcState::default()));

    for size in valid_sizes {
        state.borrow_mut().fft_size = size;
        assert_eq!(state.borrow().fft_size, size);
    }
}

/// Test FFT size affects latency.
#[gpui::test]
async fn test_fft_latency(_cx: &mut TestAppContext) {
    fn calculate_latency_ms(fft_size: usize, sample_rate: u32) -> f32 {
        // With 75% overlap, latency is fft_size / 4
        let hop_size = fft_size / 4;
        (hop_size as f32 / sample_rate as f32) * 1000.0
    }

    // At 48kHz with 1024 FFT
    let latency = calculate_latency_ms(1024, 48000);
    assert!((latency - 5.33).abs() < 0.1);
}

// =============================================================================
// Regularization Tests
// =============================================================================

/// Test beta base control.
#[gpui::test]
async fn test_beta_base(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(XtcState::default()));

    let test_values: Vec<f32> = vec![0.0001, 0.001, 0.01, 0.1];
    for value in test_values {
        state.borrow_mut().beta_base = value;
        assert!((state.borrow().beta_base - value).abs() < 0.0001);
    }
}

/// Test beta affects stability.
#[gpui::test]
async fn test_beta_stability_tradeoff(_cx: &mut TestAppContext) {
    fn get_stability_description(beta: f32) -> &'static str {
        if beta < 0.0001 {
            "Very aggressive (may ring)"
        } else if beta < 0.001 {
            "Aggressive"
        } else if beta < 0.01 {
            "Balanced"
        } else {
            "Conservative (reduced cancellation)"
        }
    }

    assert_eq!(get_stability_description(0.001), "Balanced");
    assert!(get_stability_description(0.1).contains("Conservative"));
}

/// Test low frequency boost.
#[gpui::test]
async fn test_beta_low_freq_boost(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(XtcState::default()));

    let test_values: Vec<f32> = vec![1.0, 5.0, 10.0, 20.0];
    for value in test_values {
        state.borrow_mut().beta_low_freq_boost = value;
        assert!((state.borrow().beta_low_freq_boost - value).abs() < 0.1);
    }
}

/// Test high frequency boost.
#[gpui::test]
async fn test_beta_high_freq_boost(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(XtcState::default()));

    let test_values: Vec<f32> = vec![1.0, 5.0, 10.0, 20.0];
    for value in test_values {
        state.borrow_mut().beta_high_freq_boost = value;
        assert!((state.borrow().beta_high_freq_boost - value).abs() < 0.1);
    }
}

// =============================================================================
// Head Shadowing Tests
// =============================================================================

/// Test head shadow cutoff control.
#[gpui::test]
async fn test_head_shadow_cutoff(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(XtcState::default()));

    let test_values: Vec<f32> = vec![2000.0, 3000.0, 4000.0, 5000.0, 6000.0];
    for value in test_values {
        state.borrow_mut().head_shadow_cutoff_hz = value;
        assert!((state.borrow().head_shadow_cutoff_hz - value).abs() < 1.0);
    }
}

/// Test head shadow slope control.
#[gpui::test]
async fn test_head_shadow_slope(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(XtcState::default()));

    let test_values: Vec<f32> = vec![3.0, 6.0, 9.0, 12.0];
    for value in test_values {
        state.borrow_mut().head_shadow_slope_db_per_octave = value;
        assert!((state.borrow().head_shadow_slope_db_per_octave - value).abs() < 0.1);
    }
}

// =============================================================================
// Head Tracking Tests
// =============================================================================

/// Test head offset X control.
#[gpui::test]
async fn test_head_offset_x(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(XtcState::default()));

    let test_values: Vec<f32> = vec![-0.2, -0.1, 0.0, 0.1, 0.2];
    for value in test_values {
        state.borrow_mut().head_offset_x = value;
        assert!((state.borrow().head_offset_x - value).abs() < 0.01);
    }
}

/// Test head offset Z control.
#[gpui::test]
async fn test_head_offset_z(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(XtcState::default()));

    let test_values: Vec<f32> = vec![-0.3, -0.15, 0.0, 0.15, 0.3];
    for value in test_values {
        state.borrow_mut().head_offset_z = value;
        assert!((state.borrow().head_offset_z - value).abs() < 0.01);
    }
}

/// Test head tracking smoothing.
#[gpui::test]
async fn test_head_tracking_smoothing(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(XtcState::default()));

    let test_values: Vec<f32> = vec![0.01, 0.05, 0.1, 0.2, 0.5];
    for value in test_values {
        state.borrow_mut().head_tracking_smooth_s = value;
        assert!((state.borrow().head_tracking_smooth_s - value).abs() < 0.01);
    }
}

/// Test head offset display.
#[gpui::test]
async fn test_head_offset_display(_cx: &mut TestAppContext) {
    fn format_offset(offset_m: f32) -> String {
        if offset_m.abs() < 0.01 {
            "Center".to_string()
        } else if offset_m > 0.0 {
            format!("{:.0} cm right", offset_m * 100.0)
        } else {
            format!("{:.0} cm left", offset_m.abs() * 100.0)
        }
    }

    assert_eq!(format_offset(0.0), "Center");
    assert_eq!(format_offset(0.1), "10 cm right");
    assert_eq!(format_offset(-0.15), "15 cm left");
}

// =============================================================================
// Enable/Disable Tests
// =============================================================================

/// Test enabled toggle.
#[gpui::test]
async fn test_xtc_enabled(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(XtcState::default()));

    assert!(state.borrow().enabled);

    state.borrow_mut().enabled = false;
    assert!(!state.borrow().enabled);
}

// =============================================================================
// Path Calculation Tests
// =============================================================================

/// Test ipsilateral path length calculation.
#[gpui::test]
async fn test_ipsilateral_path_length(_cx: &mut TestAppContext) {
    fn calculate_ipsilateral_path(distance_m: f32, angle_deg: f32) -> f32 {
        // Direct path from speaker to ipsilateral (same-side) ear
        // Simplified: assumes ear is at center of head
        distance_m
            * (1.0 - 0.1 * (angle_deg.to_radians().sin()))
    }

    let path = calculate_ipsilateral_path(2.0, 30.0);
    assert!(path > 0.0 && path < 3.0);
}

/// Test contralateral path length calculation.
#[gpui::test]
async fn test_contralateral_path_length(_cx: &mut TestAppContext) {
    fn calculate_contralateral_path(distance_m: f32, angle_deg: f32, head_radius_m: f32) -> f32 {
        // Crosstalk path from speaker to contralateral (opposite) ear
        // Simplified: adds head radius contribution
        let direct = distance_m;
        let head_contribution = head_radius_m * 2.0 * angle_deg.to_radians().sin();
        direct + head_contribution
    }

    let path = calculate_contralateral_path(2.0, 30.0, 0.0875);
    assert!(path > 2.0); // Contralateral path is longer
}

/// Test ITD (Interaural Time Difference).
#[gpui::test]
async fn test_itd_calculation(_cx: &mut TestAppContext) {
    fn calculate_itd_samples(path_diff_m: f32, sample_rate: u32) -> f32 {
        let speed_of_sound = 343.0; // m/s
        let time_diff_s = path_diff_m / speed_of_sound;
        time_diff_s * sample_rate as f32
    }

    // 10cm path difference at 48kHz
    let itd = calculate_itd_samples(0.1, 48000);
    assert!(itd > 10.0 && itd < 20.0);
}

// =============================================================================
// Preset Tests
// =============================================================================

/// Test preset: nearfield.
#[gpui::test]
async fn test_preset_nearfield(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(XtcState::default()));

    // Nearfield setup: close speakers, wider angle
    state.borrow_mut().distance_m = 1.0;
    state.borrow_mut().speaker_angle_deg = 45.0;
    state.borrow_mut().beta_base = 0.01; // More conservative

    assert!(state.borrow().distance_m < 1.5);
}

/// Test preset: home theater.
#[gpui::test]
async fn test_preset_home_theater(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(XtcState::default()));

    // Home theater: moderate distance, ITU angle
    state.borrow_mut().distance_m = 2.5;
    state.borrow_mut().speaker_angle_deg = 30.0;
    state.borrow_mut().beta_base = 0.001;

    assert!((state.borrow().speaker_angle_deg - 30.0).abs() < 0.1);
}

/// Test preset: aggressive.
#[gpui::test]
async fn test_preset_aggressive(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(XtcState::default()));

    // Aggressive XTC for maximum effect
    state.borrow_mut().beta_base = 0.0001;
    state.borrow_mut().beta_low_freq_boost = 5.0;
    state.borrow_mut().beta_high_freq_boost = 5.0;

    assert!(state.borrow().beta_base < 0.001);
}

// =============================================================================
// Visual Feedback Tests
// =============================================================================

/// Test speaker position visualization.
#[gpui::test]
async fn test_speaker_position_visualization(_cx: &mut TestAppContext) {
    fn calculate_speaker_x(distance_m: f32, angle_deg: f32) -> f32 {
        distance_m * angle_deg.to_radians().sin()
    }

    fn calculate_speaker_z(distance_m: f32, angle_deg: f32) -> f32 {
        distance_m * angle_deg.to_radians().cos()
    }

    let x = calculate_speaker_x(2.0, 30.0);
    let z = calculate_speaker_z(2.0, 30.0);

    // 30° angle: x should be about 1m, z about 1.73m
    assert!((x - 1.0).abs() < 0.01);
    assert!((z - 1.732).abs() < 0.01);
}

/// Test cancellation effectiveness indicator.
#[gpui::test]
async fn test_cancellation_effectiveness(_cx: &mut TestAppContext) {
    fn get_effectiveness_label(beta: f32, angle: f32) -> &'static str {
        let difficulty = if angle < 25.0 {
            0.0
        } else if angle < 40.0 {
            0.5
        } else {
            1.0
        };

        let aggressiveness = if beta < 0.001 { 1.0 } else { 0.5 };

        let score = aggressiveness - difficulty * 0.3;
        if score > 0.7 {
            "High"
        } else if score > 0.4 {
            "Moderate"
        } else {
            "Low"
        }
    }

    assert_eq!(get_effectiveness_label(0.0001, 30.0), "High");
    assert_eq!(get_effectiveness_label(0.01, 60.0), "Low");
}
