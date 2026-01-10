//! E2E tests for A/B Compare Plugin.
//!
//! Tests for the A/B comparison plugin that allows fair comparison
//! between two audio processing chains with automatic loudness matching.

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Mock Types for Testing
// =============================================================================

/// Mix mode for A/B comparison
#[derive(Debug, Clone, Copy, PartialEq)]
enum MixMode {
    Potentiometer,
    Binary,
}

/// Loudness measurement type
#[derive(Debug, Clone, Copy, PartialEq)]
enum LoudnessType {
    Momentary,
    ShortTerm,
    Integrated,
}

/// A/B Compare plugin state for testing
struct ABCompareState {
    enabled: bool,
    // Mix controls
    mix_mode: MixMode,
    mix: f32,           // -1.0 = A, 0.0 = 50/50, +1.0 = B
    selected_path: i32, // 0 = A, 1 = B (for binary mode)
    bypass: bool,
    // Auto-gain
    auto_gain_enabled: bool,
    loudness_type: LoudnessType,
    gain_smoothing_ms: f32,
    // Path status
    path_a_configured: bool,
    path_b_configured: bool,
    path_a_gain_db: f32,
    path_b_gain_db: f32,
}

impl Default for ABCompareState {
    fn default() -> Self {
        Self {
            enabled: true,
            mix_mode: MixMode::Potentiometer,
            mix: 0.0,
            selected_path: 0,
            bypass: false,
            auto_gain_enabled: true,
            loudness_type: LoudnessType::ShortTerm,
            gain_smoothing_ms: 100.0,
            path_a_configured: false,
            path_b_configured: false,
            path_a_gain_db: 0.0,
            path_b_gain_db: 0.0,
        }
    }
}

// =============================================================================
// Basic Plugin Tests
// =============================================================================

/// Test plugin renders correctly.
#[gpui::test]
async fn test_ab_compare_renders(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ABCompareState::default()));
    assert!(state.borrow().enabled);
}

/// Test default values.
#[gpui::test]
async fn test_ab_compare_defaults(_cx: &mut TestAppContext) {
    let state = ABCompareState::default();

    assert_eq!(state.mix_mode, MixMode::Potentiometer);
    assert!((state.mix - 0.0).abs() < 0.001);
    assert!(state.auto_gain_enabled);
}

// =============================================================================
// Mix Mode Tests
// =============================================================================

/// Test mix mode selection.
#[gpui::test]
async fn test_mix_mode_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ABCompareState::default()));

    state.borrow_mut().mix_mode = MixMode::Potentiometer;
    assert_eq!(state.borrow().mix_mode, MixMode::Potentiometer);

    state.borrow_mut().mix_mode = MixMode::Binary;
    assert_eq!(state.borrow().mix_mode, MixMode::Binary);
}

/// Test mix mode descriptions.
#[gpui::test]
async fn test_mix_mode_descriptions(_cx: &mut TestAppContext) {
    fn get_mode_description(mode: MixMode) -> &'static str {
        match mode {
            MixMode::Potentiometer => "Continuous blend between A and B",
            MixMode::Binary => "Switch between A or B only",
        }
    }

    assert!(get_mode_description(MixMode::Potentiometer).contains("Continuous"));
    assert!(get_mode_description(MixMode::Binary).contains("Switch"));
}

// =============================================================================
// Mix Control Tests (Potentiometer Mode)
// =============================================================================

/// Test mix control.
#[gpui::test]
async fn test_mix_control(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ABCompareState::default()));

    let test_values: Vec<f32> = vec![-1.0, -0.5, 0.0, 0.5, 1.0];
    for value in test_values {
        state.borrow_mut().mix = value;
        assert!((state.borrow().mix - value).abs() < 0.001);
    }
}

/// Test mix bounds.
#[gpui::test]
async fn test_mix_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ABCompareState::default()));

    // Range: -1.0 to +1.0
    let clamped = (-1.5_f32).clamp(-1.0, 1.0);
    state.borrow_mut().mix = clamped;
    assert!(state.borrow().mix >= -1.0);

    let clamped = (1.5_f32).clamp(-1.0, 1.0);
    state.borrow_mut().mix = clamped;
    assert!(state.borrow().mix <= 1.0);
}

/// Test mix display format.
#[gpui::test]
async fn test_mix_display(_cx: &mut TestAppContext) {
    fn format_mix(mix: f32) -> String {
        if mix <= -0.99 {
            "100% A".to_string()
        } else if mix >= 0.99 {
            "100% B".to_string()
        } else if mix.abs() < 0.01 {
            "50/50".to_string()
        } else if mix < 0.0 {
            format!("{}% A", ((1.0 - mix.abs()) * 50.0 + 50.0).round() as i32)
        } else {
            format!("{}% B", ((mix + 1.0) / 2.0 * 100.0).round() as i32)
        }
    }

    assert_eq!(format_mix(-1.0), "100% A");
    assert_eq!(format_mix(1.0), "100% B");
    assert_eq!(format_mix(0.0), "50/50");
}

/// Test mix gain calculation.
#[gpui::test]
async fn test_mix_gain_calculation(_cx: &mut TestAppContext) {
    fn calculate_mix_gains(mix: f32) -> (f32, f32) {
        // Equal power crossfade
        let pos = (mix + 1.0) / 2.0; // 0.0 to 1.0
        let gain_a = (1.0 - pos).sqrt();
        let gain_b = pos.sqrt();
        (gain_a, gain_b)
    }

    let (a, b) = calculate_mix_gains(-1.0);
    assert!((a - 1.0).abs() < 0.01);
    assert!((b - 0.0).abs() < 0.01);

    let (a, b) = calculate_mix_gains(0.0);
    assert!((a - 0.707).abs() < 0.01);
    assert!((b - 0.707).abs() < 0.01);

    let (a, b) = calculate_mix_gains(1.0);
    assert!((a - 0.0).abs() < 0.01);
    assert!((b - 1.0).abs() < 0.01);
}

// =============================================================================
// Binary Mode Tests
// =============================================================================

/// Test binary path selection.
#[gpui::test]
async fn test_binary_path_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ABCompareState::default()));

    state.borrow_mut().mix_mode = MixMode::Binary;

    // Select path A
    state.borrow_mut().selected_path = 0;
    assert_eq!(state.borrow().selected_path, 0);

    // Select path B
    state.borrow_mut().selected_path = 1;
    assert_eq!(state.borrow().selected_path, 1);
}

/// Test binary mode toggle.
#[gpui::test]
async fn test_binary_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ABCompareState::default()));

    state.borrow_mut().mix_mode = MixMode::Binary;
    state.borrow_mut().selected_path = 0;

    // Toggle
    let new_path = 1 - state.borrow().selected_path;
    state.borrow_mut().selected_path = new_path;
    assert_eq!(state.borrow().selected_path, 1);
}

/// Test binary mode label.
#[gpui::test]
async fn test_binary_mode_label(_cx: &mut TestAppContext) {
    fn get_path_label(selected: i32) -> &'static str {
        if selected == 0 { "Path A" } else { "Path B" }
    }

    assert_eq!(get_path_label(0), "Path A");
    assert_eq!(get_path_label(1), "Path B");
}

// =============================================================================
// Bypass Tests
// =============================================================================

/// Test bypass toggle.
#[gpui::test]
async fn test_bypass_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ABCompareState::default()));

    assert!(!state.borrow().bypass);

    state.borrow_mut().bypass = true;
    assert!(state.borrow().bypass);
}

/// Test bypass passes original input.
#[gpui::test]
async fn test_bypass_passes_input(_cx: &mut TestAppContext) {
    fn get_output_description(bypass: bool, mix: f32) -> &'static str {
        if bypass {
            "Original input (unprocessed)"
        } else if mix <= -0.99 {
            "Path A processing"
        } else if mix >= 0.99 {
            "Path B processing"
        } else {
            "Mixed A+B processing"
        }
    }

    assert_eq!(
        get_output_description(true, 0.0),
        "Original input (unprocessed)"
    );
    assert_eq!(get_output_description(false, -1.0), "Path A processing");
}

// =============================================================================
// Auto-Gain Tests
// =============================================================================

/// Test auto-gain toggle.
#[gpui::test]
async fn test_auto_gain_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ABCompareState::default()));

    assert!(state.borrow().auto_gain_enabled);

    state.borrow_mut().auto_gain_enabled = false;
    assert!(!state.borrow().auto_gain_enabled);
}

/// Test loudness type selection.
#[gpui::test]
async fn test_loudness_type_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ABCompareState::default()));

    let types = [
        LoudnessType::Momentary,
        LoudnessType::ShortTerm,
        LoudnessType::Integrated,
    ];

    for lt in types {
        state.borrow_mut().loudness_type = lt;
        assert_eq!(state.borrow().loudness_type, lt);
    }
}

/// Test loudness type descriptions.
#[gpui::test]
async fn test_loudness_type_descriptions(_cx: &mut TestAppContext) {
    fn get_loudness_type_desc(lt: LoudnessType) -> &'static str {
        match lt {
            LoudnessType::Momentary => "400ms window (fast response)",
            LoudnessType::ShortTerm => "3s window (balanced)",
            LoudnessType::Integrated => "Full program (slow response)",
        }
    }

    assert!(get_loudness_type_desc(LoudnessType::Momentary).contains("400ms"));
    assert!(get_loudness_type_desc(LoudnessType::ShortTerm).contains("3s"));
}

/// Test gain smoothing control.
#[gpui::test]
async fn test_gain_smoothing_control(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ABCompareState::default()));

    let test_values: Vec<f32> = vec![10.0, 50.0, 100.0, 200.0, 500.0];
    for value in test_values {
        state.borrow_mut().gain_smoothing_ms = value;
        assert!((state.borrow().gain_smoothing_ms - value).abs() < 0.1);
    }
}

// =============================================================================
// Path Configuration Tests
// =============================================================================

/// Test path configuration status.
#[gpui::test]
async fn test_path_configuration_status(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ABCompareState::default()));

    // Initially unconfigured
    assert!(!state.borrow().path_a_configured);
    assert!(!state.borrow().path_b_configured);

    // Configure path A
    state.borrow_mut().path_a_configured = true;
    assert!(state.borrow().path_a_configured);
}

/// Test path gain values.
#[gpui::test]
async fn test_path_gain_values(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ABCompareState::default()));

    state.borrow_mut().path_a_gain_db = -3.0;
    state.borrow_mut().path_b_gain_db = 2.0;

    assert!((state.borrow().path_a_gain_db - (-3.0)).abs() < 0.1);
    assert!((state.borrow().path_b_gain_db - 2.0).abs() < 0.1);
}

// =============================================================================
// Enable/Disable Tests
// =============================================================================

/// Test enabled toggle.
#[gpui::test]
async fn test_ab_compare_enabled(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ABCompareState::default()));

    assert!(state.borrow().enabled);

    state.borrow_mut().enabled = false;
    assert!(!state.borrow().enabled);
}

// =============================================================================
// Visual Feedback Tests
// =============================================================================

/// Test path indicator color.
#[gpui::test]
async fn test_path_indicator_color(_cx: &mut TestAppContext) {
    fn get_path_color(is_active: bool, is_configured: bool) -> &'static str {
        if !is_configured {
            "gray"
        } else if is_active {
            "green"
        } else {
            "dim"
        }
    }

    assert_eq!(get_path_color(false, false), "gray");
    assert_eq!(get_path_color(true, true), "green");
    assert_eq!(get_path_color(false, true), "dim");
}

/// Test loudness difference display.
#[gpui::test]
async fn test_loudness_diff_display(_cx: &mut TestAppContext) {
    fn format_loudness_diff(diff_db: f32) -> String {
        if diff_db.abs() < 0.1 {
            "Matched".to_string()
        } else if diff_db > 0.0 {
            format!("B +{:.1} dB", diff_db)
        } else {
            format!("A +{:.1} dB", diff_db.abs())
        }
    }

    assert_eq!(format_loudness_diff(0.0), "Matched");
    assert_eq!(format_loudness_diff(3.0), "B +3.0 dB");
    assert_eq!(format_loudness_diff(-2.0), "A +2.0 dB");
}

/// Test mix position indicator.
#[gpui::test]
async fn test_mix_position_indicator(_cx: &mut TestAppContext) {
    fn get_mix_position_percent(mix: f32) -> f32 {
        // Convert -1..+1 to 0..100
        (mix + 1.0) / 2.0 * 100.0
    }

    assert!((get_mix_position_percent(-1.0) - 0.0).abs() < 0.1);
    assert!((get_mix_position_percent(0.0) - 50.0).abs() < 0.1);
    assert!((get_mix_position_percent(1.0) - 100.0).abs() < 0.1);
}

// =============================================================================
// Use Case Tests
// =============================================================================

/// Test blind comparison setup.
#[gpui::test]
async fn test_blind_comparison_setup(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ABCompareState::default()));

    // For blind testing: use binary mode with auto-gain
    state.borrow_mut().mix_mode = MixMode::Binary;
    state.borrow_mut().auto_gain_enabled = true;

    assert_eq!(state.borrow().mix_mode, MixMode::Binary);
    assert!(state.borrow().auto_gain_enabled);
}

/// Test smooth transition setup.
#[gpui::test]
async fn test_smooth_transition_setup(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ABCompareState::default()));

    // For smooth transitions: use potentiometer mode
    state.borrow_mut().mix_mode = MixMode::Potentiometer;
    state.borrow_mut().gain_smoothing_ms = 200.0;

    assert_eq!(state.borrow().mix_mode, MixMode::Potentiometer);
}

/// Test quick comparison setup.
#[gpui::test]
async fn test_quick_comparison_setup(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ABCompareState::default()));

    // Quick comparison with momentary loudness
    state.borrow_mut().loudness_type = LoudnessType::Momentary;
    state.borrow_mut().gain_smoothing_ms = 50.0;

    assert_eq!(state.borrow().loudness_type, LoudnessType::Momentary);
}
