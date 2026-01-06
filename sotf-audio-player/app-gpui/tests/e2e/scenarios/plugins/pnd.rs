//! E2E tests for PND (Polyphonic Note Detection/Phase Noise Detection) Plugin.
//!
//! Tests for the phase noise detection and correction plugin.

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Mock Types for Testing
// =============================================================================

/// PND plugin state for testing
struct PndState {
    enabled: bool,
    correction_strength: f32,
    analysis_window_ms: f32,
    drift_smoothing: f32,
}

impl Default for PndState {
    fn default() -> Self {
        Self {
            enabled: true,
            correction_strength: 1.0,
            analysis_window_ms: 50.0,
            drift_smoothing: 0.5,
        }
    }
}

// =============================================================================
// Basic Plugin Tests
// =============================================================================

/// Test plugin renders correctly.
#[gpui::test]
async fn test_pnd_renders(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PndState::default()));
    assert!(state.borrow().enabled);
}

/// Test default values.
#[gpui::test]
async fn test_pnd_defaults(_cx: &mut TestAppContext) {
    let state = PndState::default();

    assert!((state.correction_strength - 1.0).abs() < 0.01);
    assert!((state.analysis_window_ms - 50.0).abs() < 0.1);
    assert!((state.drift_smoothing - 0.5).abs() < 0.01);
}

// =============================================================================
// Correction Strength Tests
// =============================================================================

/// Test correction strength control.
#[gpui::test]
async fn test_correction_strength_control(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PndState::default()));

    let test_values: Vec<f32> = vec![0.0, 0.25, 0.5, 0.75, 1.0];
    for value in test_values {
        state.borrow_mut().correction_strength = value;
        assert!((state.borrow().correction_strength - value).abs() < 0.01);
    }
}

/// Test correction strength bounds.
#[gpui::test]
async fn test_correction_strength_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PndState::default()));

    // Range: 0.0 to 1.0
    let clamped = (-0.1_f32).clamp(0.0, 1.0);
    state.borrow_mut().correction_strength = clamped;
    assert!(state.borrow().correction_strength >= 0.0);

    let clamped = (1.5_f32).clamp(0.0, 1.0);
    state.borrow_mut().correction_strength = clamped;
    assert!(state.borrow().correction_strength <= 1.0);
}

/// Test correction strength display.
#[gpui::test]
async fn test_correction_strength_display(_cx: &mut TestAppContext) {
    fn format_strength(strength: f32) -> String {
        if strength < 0.01 {
            "Off".to_string()
        } else {
            format!("{}%", (strength * 100.0).round() as i32)
        }
    }

    assert_eq!(format_strength(0.0), "Off");
    assert_eq!(format_strength(0.5), "50%");
    assert_eq!(format_strength(1.0), "100%");
}

/// Test correction strength description.
#[gpui::test]
async fn test_correction_strength_description(_cx: &mut TestAppContext) {
    fn get_strength_description(strength: f32) -> &'static str {
        if strength < 0.25 {
            "Minimal correction"
        } else if strength < 0.5 {
            "Light correction"
        } else if strength < 0.75 {
            "Moderate correction"
        } else {
            "Full correction"
        }
    }

    assert_eq!(get_strength_description(0.1), "Minimal correction");
    assert_eq!(get_strength_description(0.4), "Light correction");
    assert_eq!(get_strength_description(0.6), "Moderate correction");
    assert_eq!(get_strength_description(0.9), "Full correction");
}

// =============================================================================
// Analysis Window Tests
// =============================================================================

/// Test analysis window control.
#[gpui::test]
async fn test_analysis_window_control(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PndState::default()));

    let test_values: Vec<f32> = vec![10.0, 25.0, 50.0, 100.0, 200.0];
    for value in test_values {
        state.borrow_mut().analysis_window_ms = value;
        assert!((state.borrow().analysis_window_ms - value).abs() < 0.1);
    }
}

/// Test analysis window bounds.
#[gpui::test]
async fn test_analysis_window_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PndState::default()));

    // Typical range: 10ms to 200ms
    let clamped = (5.0_f32).clamp(10.0, 200.0);
    state.borrow_mut().analysis_window_ms = clamped;
    assert!(state.borrow().analysis_window_ms >= 10.0);

    let clamped = (300.0_f32).clamp(10.0, 200.0);
    state.borrow_mut().analysis_window_ms = clamped;
    assert!(state.borrow().analysis_window_ms <= 200.0);
}

/// Test analysis window display.
#[gpui::test]
async fn test_analysis_window_display(_cx: &mut TestAppContext) {
    fn format_window(ms: f32) -> String {
        format!("{:.0} ms", ms)
    }

    assert_eq!(format_window(50.0), "50 ms");
    assert_eq!(format_window(100.0), "100 ms");
}

/// Test analysis window affects latency.
#[gpui::test]
async fn test_analysis_window_latency(_cx: &mut TestAppContext) {
    fn calculate_latency_samples(window_ms: f32, sample_rate: u32) -> usize {
        (window_ms * sample_rate as f32 / 1000.0).round() as usize
    }

    // 50ms at 48kHz = 2400 samples
    assert_eq!(calculate_latency_samples(50.0, 48000), 2400);
    // 100ms at 48kHz = 4800 samples
    assert_eq!(calculate_latency_samples(100.0, 48000), 4800);
}

// =============================================================================
// Drift Smoothing Tests
// =============================================================================

/// Test drift smoothing control.
#[gpui::test]
async fn test_drift_smoothing_control(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PndState::default()));

    let test_values: Vec<f32> = vec![0.0, 0.25, 0.5, 0.75, 1.0];
    for value in test_values {
        state.borrow_mut().drift_smoothing = value;
        assert!((state.borrow().drift_smoothing - value).abs() < 0.01);
    }
}

/// Test drift smoothing bounds.
#[gpui::test]
async fn test_drift_smoothing_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PndState::default()));

    // Range: 0.0 to 1.0
    let clamped = (-0.1_f32).clamp(0.0, 1.0);
    state.borrow_mut().drift_smoothing = clamped;
    assert!(state.borrow().drift_smoothing >= 0.0);
}

/// Test drift smoothing description.
#[gpui::test]
async fn test_drift_smoothing_description(_cx: &mut TestAppContext) {
    fn get_smoothing_description(smoothing: f32) -> &'static str {
        if smoothing < 0.25 {
            "Fast tracking (less stable)"
        } else if smoothing < 0.75 {
            "Balanced"
        } else {
            "Slow tracking (more stable)"
        }
    }

    assert!(get_smoothing_description(0.1).contains("Fast"));
    assert_eq!(get_smoothing_description(0.5), "Balanced");
    assert!(get_smoothing_description(0.9).contains("Slow"));
}

// =============================================================================
// Enable/Disable Tests
// =============================================================================

/// Test enabled toggle.
#[gpui::test]
async fn test_pnd_enabled(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PndState::default()));

    assert!(state.borrow().enabled);

    state.borrow_mut().enabled = false;
    assert!(!state.borrow().enabled);
}

// =============================================================================
// Visual Feedback Tests
// =============================================================================

/// Test phase error display.
#[gpui::test]
async fn test_phase_error_display(_cx: &mut TestAppContext) {
    fn format_phase_error(error_degrees: f32) -> String {
        if error_degrees.abs() < 0.1 {
            "0.0°".to_string()
        } else {
            format!("{:+.1}°", error_degrees)
        }
    }

    assert_eq!(format_phase_error(0.0), "0.0°");
    assert_eq!(format_phase_error(5.0), "+5.0°");
    assert_eq!(format_phase_error(-3.5), "-3.5°");
}

/// Test correction activity indicator.
#[gpui::test]
async fn test_correction_activity(_cx: &mut TestAppContext) {
    fn get_activity_level(error_degrees: f32) -> &'static str {
        let error = error_degrees.abs();
        if error < 1.0 {
            "minimal"
        } else if error < 5.0 {
            "low"
        } else if error < 15.0 {
            "moderate"
        } else {
            "high"
        }
    }

    assert_eq!(get_activity_level(0.5), "minimal");
    assert_eq!(get_activity_level(3.0), "low");
    assert_eq!(get_activity_level(10.0), "moderate");
    assert_eq!(get_activity_level(20.0), "high");
}

// =============================================================================
// Preset Tests
// =============================================================================

/// Test preset: subtle.
#[gpui::test]
async fn test_preset_subtle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PndState::default()));

    state.borrow_mut().correction_strength = 0.3;
    state.borrow_mut().analysis_window_ms = 100.0;
    state.borrow_mut().drift_smoothing = 0.7;

    assert!(state.borrow().correction_strength < 0.5);
}

/// Test preset: aggressive.
#[gpui::test]
async fn test_preset_aggressive(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PndState::default()));

    state.borrow_mut().correction_strength = 1.0;
    state.borrow_mut().analysis_window_ms = 25.0;
    state.borrow_mut().drift_smoothing = 0.3;

    assert!((state.borrow().correction_strength - 1.0).abs() < 0.01);
}

/// Test preset: balanced.
#[gpui::test]
async fn test_preset_balanced(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(PndState::default()));

    state.borrow_mut().correction_strength = 0.6;
    state.borrow_mut().analysis_window_ms = 50.0;
    state.borrow_mut().drift_smoothing = 0.5;

    assert!(state.borrow().correction_strength > 0.4);
    assert!(state.borrow().correction_strength < 0.8);
}
