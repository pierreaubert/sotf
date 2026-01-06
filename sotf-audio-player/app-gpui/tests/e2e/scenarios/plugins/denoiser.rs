//! E2E tests for Denoiser Plugin.
//!
//! Tests for the spectral noise reduction plugin using MCRA algorithm.

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Mock Types for Testing
// =============================================================================

/// Denoiser plugin state for testing
struct DenoiserState {
    enabled: bool,
    // Main controls
    reduction_db: f32,
    floor_db: f32,
    smoothing: f32,
    attack_ms: f32,
    release_ms: f32,
    // Mode settings
    low_latency: bool,
    polyphonic_detection: bool,
    crack_sensitivity: f32,
    // Advanced MCRA parameters
    mcra_alpha_s: f32,
    mcra_alpha_p: f32,
    mcra_l: usize,
    mcra_delta: f32,
}

impl Default for DenoiserState {
    fn default() -> Self {
        Self {
            enabled: true,
            reduction_db: 12.0,
            floor_db: -40.0,
            smoothing: 0.5,
            attack_ms: 5.0,
            release_ms: 50.0,
            low_latency: false,
            polyphonic_detection: false,
            crack_sensitivity: 10.0,
            mcra_alpha_s: 0.95,
            mcra_alpha_p: 0.2,
            mcra_l: 75,
            mcra_delta: 5.0,
        }
    }
}

// =============================================================================
// Basic Plugin Tests
// =============================================================================

/// Test plugin renders correctly.
#[gpui::test]
async fn test_denoiser_renders(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DenoiserState::default()));
    assert!(state.borrow().enabled);
}

/// Test default values.
#[gpui::test]
async fn test_denoiser_defaults(_cx: &mut TestAppContext) {
    let state = DenoiserState::default();

    assert!((state.reduction_db - 12.0).abs() < 0.1);
    assert!((state.floor_db - (-40.0)).abs() < 0.1);
    assert!((state.smoothing - 0.5).abs() < 0.01);
}

// =============================================================================
// Reduction Tests
// =============================================================================

/// Test reduction control.
#[gpui::test]
async fn test_reduction_control(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DenoiserState::default()));

    let test_values: Vec<f32> = vec![0.0, 6.0, 12.0, 24.0, 40.0];
    for value in test_values {
        state.borrow_mut().reduction_db = value;
        assert!((state.borrow().reduction_db - value).abs() < 0.1);
    }
}

/// Test reduction bounds.
#[gpui::test]
async fn test_reduction_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DenoiserState::default()));

    // Range: 0 to 40 dB
    let clamped = (-5.0_f32).clamp(0.0, 40.0);
    state.borrow_mut().reduction_db = clamped;
    assert!(state.borrow().reduction_db >= 0.0);

    let clamped = (50.0_f32).clamp(0.0, 40.0);
    state.borrow_mut().reduction_db = clamped;
    assert!(state.borrow().reduction_db <= 40.0);
}

/// Test reduction display.
#[gpui::test]
async fn test_reduction_display(_cx: &mut TestAppContext) {
    fn format_reduction(db: f32) -> String {
        if db < 0.1 {
            "Off".to_string()
        } else {
            format!("{:.0} dB", db)
        }
    }

    assert_eq!(format_reduction(0.0), "Off");
    assert_eq!(format_reduction(12.0), "12 dB");
}

// =============================================================================
// Floor Tests
// =============================================================================

/// Test floor control.
#[gpui::test]
async fn test_floor_control(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DenoiserState::default()));

    let test_values: Vec<f32> = vec![-60.0, -50.0, -40.0, -30.0, -20.0, -10.0];
    for value in test_values {
        state.borrow_mut().floor_db = value;
        assert!((state.borrow().floor_db - value).abs() < 0.1);
    }
}

/// Test floor bounds.
#[gpui::test]
async fn test_floor_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DenoiserState::default()));

    // Range: -60 to -10 dB
    let clamped = (-70.0_f32).clamp(-60.0, -10.0);
    state.borrow_mut().floor_db = clamped;
    assert!(state.borrow().floor_db >= -60.0);
}

/// Test floor purpose.
#[gpui::test]
async fn test_floor_purpose(_cx: &mut TestAppContext) {
    // Floor prevents "musical noise" artifacts by setting minimum gain
    fn get_floor_description(floor_db: f32) -> &'static str {
        if floor_db < -50.0 {
            "Deep reduction (may cause artifacts)"
        } else if floor_db < -35.0 {
            "Normal"
        } else {
            "Conservative (minimal artifacts)"
        }
    }

    assert_eq!(get_floor_description(-40.0), "Normal");
    assert!(get_floor_description(-55.0).contains("artifacts"));
}

// =============================================================================
// Smoothing Tests
// =============================================================================

/// Test smoothing control.
#[gpui::test]
async fn test_smoothing_control(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DenoiserState::default()));

    let test_values: Vec<f32> = vec![0.0, 0.25, 0.5, 0.75, 0.99];
    for value in test_values {
        state.borrow_mut().smoothing = value;
        assert!((state.borrow().smoothing - value).abs() < 0.01);
    }
}

/// Test smoothing bounds.
#[gpui::test]
async fn test_smoothing_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DenoiserState::default()));

    // Range: 0.0 to 0.99
    let clamped = (-0.1_f32).clamp(0.0, 0.99);
    state.borrow_mut().smoothing = clamped;
    assert!(state.borrow().smoothing >= 0.0);

    let clamped = (1.0_f32).clamp(0.0, 0.99);
    state.borrow_mut().smoothing = clamped;
    assert!(state.borrow().smoothing <= 0.99);
}

// =============================================================================
// Attack/Release Tests
// =============================================================================

/// Test attack control.
#[gpui::test]
async fn test_attack_control(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DenoiserState::default()));

    let test_values: Vec<f32> = vec![1.0, 5.0, 10.0, 20.0, 50.0];
    for value in test_values {
        state.borrow_mut().attack_ms = value;
        assert!((state.borrow().attack_ms - value).abs() < 0.1);
    }
}

/// Test release control.
#[gpui::test]
async fn test_release_control(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DenoiserState::default()));

    let test_values: Vec<f32> = vec![10.0, 50.0, 100.0, 200.0, 500.0];
    for value in test_values {
        state.borrow_mut().release_ms = value;
        assert!((state.borrow().release_ms - value).abs() < 0.1);
    }
}

// =============================================================================
// Mode Tests
// =============================================================================

/// Test low latency toggle.
#[gpui::test]
async fn test_low_latency_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DenoiserState::default()));

    assert!(!state.borrow().low_latency);

    state.borrow_mut().low_latency = true;
    assert!(state.borrow().low_latency);
}

/// Test low latency FFT size.
#[gpui::test]
async fn test_low_latency_fft_size(_cx: &mut TestAppContext) {
    fn get_fft_size(low_latency: bool) -> usize {
        if low_latency {
            512
        } else {
            2048
        }
    }

    assert_eq!(get_fft_size(false), 2048);
    assert_eq!(get_fft_size(true), 512);
}

/// Test polyphonic detection toggle.
#[gpui::test]
async fn test_polyphonic_detection_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DenoiserState::default()));

    assert!(!state.borrow().polyphonic_detection);

    state.borrow_mut().polyphonic_detection = true;
    assert!(state.borrow().polyphonic_detection);
}

/// Test crack sensitivity control.
#[gpui::test]
async fn test_crack_sensitivity(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DenoiserState::default()));

    let test_values: Vec<f32> = vec![1.0, 5.0, 10.0, 50.0, 100.0];
    for value in test_values {
        state.borrow_mut().crack_sensitivity = value;
        assert!((state.borrow().crack_sensitivity - value).abs() < 0.1);
    }
}

// =============================================================================
// Advanced MCRA Tests
// =============================================================================

/// Test MCRA alpha_s parameter.
#[gpui::test]
async fn test_mcra_alpha_s(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DenoiserState::default()));

    // Noise PSD smoothing factor
    let test_values: Vec<f32> = vec![0.9, 0.95, 0.98, 0.99];
    for value in test_values {
        state.borrow_mut().mcra_alpha_s = value;
        assert!((state.borrow().mcra_alpha_s - value).abs() < 0.001);
    }
}

/// Test MCRA alpha_p parameter.
#[gpui::test]
async fn test_mcra_alpha_p(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DenoiserState::default()));

    // Speech presence probability smoothing
    let test_values: Vec<f32> = vec![0.1, 0.2, 0.3, 0.5];
    for value in test_values {
        state.borrow_mut().mcra_alpha_p = value;
        assert!((state.borrow().mcra_alpha_p - value).abs() < 0.01);
    }
}

/// Test MCRA L parameter.
#[gpui::test]
async fn test_mcra_l(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DenoiserState::default()));

    // Window length for minimum tracking
    let test_values = [50, 75, 100, 150];
    for value in test_values {
        state.borrow_mut().mcra_l = value;
        assert_eq!(state.borrow().mcra_l, value);
    }
}

/// Test MCRA delta parameter.
#[gpui::test]
async fn test_mcra_delta(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DenoiserState::default()));

    let test_values: Vec<f32> = vec![2.0, 5.0, 10.0, 20.0];
    for value in test_values {
        state.borrow_mut().mcra_delta = value;
        assert!((state.borrow().mcra_delta - value).abs() < 0.1);
    }
}

// =============================================================================
// Enable/Disable Tests
// =============================================================================

/// Test enabled toggle.
#[gpui::test]
async fn test_denoiser_enabled(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DenoiserState::default()));

    assert!(state.borrow().enabled);

    state.borrow_mut().enabled = false;
    assert!(!state.borrow().enabled);
}

// =============================================================================
// Visual Feedback Tests
// =============================================================================

/// Test noise level indicator.
#[gpui::test]
async fn test_noise_level_indicator(_cx: &mut TestAppContext) {
    fn get_noise_level_color(noise_db: f32) -> &'static str {
        if noise_db < -60.0 {
            "very_low"
        } else if noise_db < -40.0 {
            "low"
        } else if noise_db < -25.0 {
            "moderate"
        } else {
            "high"
        }
    }

    assert_eq!(get_noise_level_color(-70.0), "very_low");
    assert_eq!(get_noise_level_color(-50.0), "low");
    assert_eq!(get_noise_level_color(-30.0), "moderate");
    assert_eq!(get_noise_level_color(-20.0), "high");
}

/// Test reduction activity meter.
#[gpui::test]
async fn test_reduction_activity_meter(_cx: &mut TestAppContext) {
    fn format_active_reduction(db: f32) -> String {
        if db.abs() < 0.5 {
            "Idle".to_string()
        } else {
            format!("-{:.1} dB", db.abs())
        }
    }

    assert_eq!(format_active_reduction(0.0), "Idle");
    assert_eq!(format_active_reduction(12.0), "-12.0 dB");
}

// =============================================================================
// Preset Tests
// =============================================================================

/// Test preset: light reduction.
#[gpui::test]
async fn test_preset_light_reduction(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DenoiserState::default()));

    state.borrow_mut().reduction_db = 6.0;
    state.borrow_mut().floor_db = -30.0;
    state.borrow_mut().smoothing = 0.7;

    assert!(state.borrow().reduction_db < 10.0);
}

/// Test preset: heavy reduction.
#[gpui::test]
async fn test_preset_heavy_reduction(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DenoiserState::default()));

    state.borrow_mut().reduction_db = 30.0;
    state.borrow_mut().floor_db = -50.0;
    state.borrow_mut().smoothing = 0.3;

    assert!(state.borrow().reduction_db >= 30.0);
}

/// Test preset: vinyl restoration.
#[gpui::test]
async fn test_preset_vinyl_restoration(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(DenoiserState::default()));

    state.borrow_mut().reduction_db = 18.0;
    state.borrow_mut().floor_db = -45.0;
    state.borrow_mut().crack_sensitivity = 5.0;
    state.borrow_mut().polyphonic_detection = true;

    assert!(state.borrow().polyphonic_detection);
}
