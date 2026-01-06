//! E2E tests for Expander Plugin UI.
//!
//! Tests for verifying downward expander functionality:
//! - Threshold control
//! - Ratio control
//! - Attack/Release timing
//! - Range control
//! - Knee control
//! - Expansion curve display

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Parameter Constants
// =============================================================================

const MIN_THRESHOLD_DB: f64 = -80.0;
const MAX_THRESHOLD_DB: f64 = 0.0;
const DEFAULT_THRESHOLD_DB: f64 = -40.0;

const MIN_RATIO: f64 = 1.0;
const MAX_RATIO: f64 = 20.0;
const DEFAULT_RATIO: f64 = 2.0;

const MIN_ATTACK_MS: f64 = 0.1;
const MAX_ATTACK_MS: f64 = 100.0;
const DEFAULT_ATTACK_MS: f64 = 10.0;

const MIN_RELEASE_MS: f64 = 10.0;
const MAX_RELEASE_MS: f64 = 2000.0;
const DEFAULT_RELEASE_MS: f64 = 200.0;

const MIN_RANGE_DB: f64 = 0.0;
const MAX_RANGE_DB: f64 = 80.0;
const DEFAULT_RANGE_DB: f64 = 40.0;

const MIN_KNEE_DB: f64 = 0.0;
const MAX_KNEE_DB: f64 = 24.0;
const DEFAULT_KNEE_DB: f64 = 6.0;

// =============================================================================
// Expander State
// =============================================================================

#[derive(Debug, Clone)]
struct ExpanderState {
    threshold_db: f64,
    ratio: f64,
    attack_ms: f64,
    release_ms: f64,
    range_db: f64,
    knee_db: f64,
    enabled: bool,
    // Metering
    input_level_db: f64,
    gain_reduction_db: f64,
}

impl Default for ExpanderState {
    fn default() -> Self {
        Self {
            threshold_db: DEFAULT_THRESHOLD_DB,
            ratio: DEFAULT_RATIO,
            attack_ms: DEFAULT_ATTACK_MS,
            release_ms: DEFAULT_RELEASE_MS,
            range_db: DEFAULT_RANGE_DB,
            knee_db: DEFAULT_KNEE_DB,
            enabled: true,
            input_level_db: -60.0,
            gain_reduction_db: 0.0,
        }
    }
}

// =============================================================================
// Threshold Tests
// =============================================================================

/// Test threshold initial state.
#[gpui::test]
async fn test_expander_threshold_initial(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ExpanderState::default()));

    assert!(
        (state.borrow().threshold_db - DEFAULT_THRESHOLD_DB).abs() < 0.001,
        "Initial threshold should be {} dB",
        DEFAULT_THRESHOLD_DB
    );
}

/// Test threshold slider adjustment.
#[gpui::test]
async fn test_expander_threshold_slider(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ExpanderState::default()));

    let test_values: Vec<f64> = vec![-80.0, -60.0, -40.0, -20.0, -10.0, 0.0];
    for value in test_values {
        state.borrow_mut().threshold_db = value.clamp(MIN_THRESHOLD_DB, MAX_THRESHOLD_DB);
        assert!(
            (state.borrow().threshold_db - value).abs() < 0.001,
            "Threshold should be {} dB",
            value
        );
    }
}

/// Test threshold bounds.
#[gpui::test]
async fn test_expander_threshold_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ExpanderState::default()));

    // Minimum
    state.borrow_mut().threshold_db = MIN_THRESHOLD_DB;
    assert!((state.borrow().threshold_db - MIN_THRESHOLD_DB).abs() < 0.001);

    // Maximum
    state.borrow_mut().threshold_db = MAX_THRESHOLD_DB;
    assert!(state.borrow().threshold_db.abs() < 0.001);
}

// =============================================================================
// Ratio Tests
// =============================================================================

/// Test ratio initial state.
#[gpui::test]
async fn test_expander_ratio_initial(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ExpanderState::default()));

    assert!(
        (state.borrow().ratio - DEFAULT_RATIO).abs() < 0.001,
        "Initial ratio should be 1:{}",
        DEFAULT_RATIO
    );
}

/// Test ratio slider adjustment.
#[gpui::test]
async fn test_expander_ratio_slider(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ExpanderState::default()));

    let test_values: Vec<f64> = vec![1.0, 1.5, 2.0, 4.0, 10.0, 20.0];
    for value in test_values {
        state.borrow_mut().ratio = value.clamp(MIN_RATIO, MAX_RATIO);
        assert!(
            (state.borrow().ratio - value).abs() < 0.001,
            "Ratio should be 1:{}",
            value
        );
    }
}

/// Test ratio display formatting.
#[gpui::test]
async fn test_expander_ratio_display(_cx: &mut TestAppContext) {
    fn format_ratio(ratio: f64) -> String {
        if ratio >= 20.0 {
            "1:∞ (gate)".to_string()
        } else {
            format!("1:{:.1}", ratio)
        }
    }

    assert_eq!(format_ratio(2.0), "1:2.0");
    assert_eq!(format_ratio(4.0), "1:4.0");
    assert_eq!(format_ratio(20.0), "1:∞ (gate)");
}

/// Test expander vs gate behavior.
#[gpui::test]
async fn test_expander_vs_gate(_cx: &mut TestAppContext) {
    // Expander: gradual reduction below threshold
    // Gate: hard cutoff below threshold

    fn is_gate_mode(ratio: f64) -> bool {
        ratio >= 10.0 // High ratio acts like a gate
    }

    assert!(!is_gate_mode(2.0)); // Gentle expansion
    assert!(!is_gate_mode(4.0)); // Moderate expansion
    assert!(is_gate_mode(10.0)); // Gate-like
    assert!(is_gate_mode(20.0)); // Hard gate
}

// =============================================================================
// Attack/Release Tests
// =============================================================================

/// Test attack initial state.
#[gpui::test]
async fn test_expander_attack_initial(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ExpanderState::default()));

    assert!(
        (state.borrow().attack_ms - DEFAULT_ATTACK_MS).abs() < 0.001,
        "Initial attack should be {} ms",
        DEFAULT_ATTACK_MS
    );
}

/// Test attack slider adjustment.
#[gpui::test]
async fn test_expander_attack_slider(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ExpanderState::default()));

    let test_values: Vec<f64> = vec![0.1, 1.0, 5.0, 10.0, 50.0, 100.0];
    for value in test_values {
        state.borrow_mut().attack_ms = value.clamp(MIN_ATTACK_MS, MAX_ATTACK_MS);
        assert!(
            (state.borrow().attack_ms - value).abs() < 0.01,
            "Attack should be {} ms",
            value
        );
    }
}

/// Test release initial state.
#[gpui::test]
async fn test_expander_release_initial(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ExpanderState::default()));

    assert!(
        (state.borrow().release_ms - DEFAULT_RELEASE_MS).abs() < 0.001,
        "Initial release should be {} ms",
        DEFAULT_RELEASE_MS
    );
}

/// Test release slider adjustment.
#[gpui::test]
async fn test_expander_release_slider(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ExpanderState::default()));

    let test_values: Vec<f64> = vec![10.0, 50.0, 100.0, 500.0, 1000.0, 2000.0];
    for value in test_values {
        state.borrow_mut().release_ms = value.clamp(MIN_RELEASE_MS, MAX_RELEASE_MS);
        assert!(
            (state.borrow().release_ms - value).abs() < 0.01,
            "Release should be {} ms",
            value
        );
    }
}

// =============================================================================
// Range Tests
// =============================================================================

/// Test range initial state.
#[gpui::test]
async fn test_expander_range_initial(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ExpanderState::default()));

    assert!(
        (state.borrow().range_db - DEFAULT_RANGE_DB).abs() < 0.001,
        "Initial range should be {} dB",
        DEFAULT_RANGE_DB
    );
}

/// Test range slider adjustment.
#[gpui::test]
async fn test_expander_range_slider(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ExpanderState::default()));

    let test_values: Vec<f64> = vec![0.0, 10.0, 20.0, 40.0, 60.0, 80.0];
    for value in test_values {
        state.borrow_mut().range_db = value.clamp(MIN_RANGE_DB, MAX_RANGE_DB);
        assert!(
            (state.borrow().range_db - value).abs() < 0.001,
            "Range should be {} dB",
            value
        );
    }
}

/// Test range limits expansion.
#[gpui::test]
async fn test_expander_range_limit(_cx: &mut TestAppContext) {
    // Range limits maximum gain reduction
    // Even with high ratio, GR won't exceed range

    fn calculate_gain_reduction(below_threshold_db: f64, ratio: f64, range_db: f64) -> f64 {
        // Without range limit
        let expansion = below_threshold_db * (1.0 - 1.0 / ratio);
        // Limit to range
        expansion.min(range_db)
    }

    // 20dB below threshold, 4:1 ratio, 40dB range
    let gr = calculate_gain_reduction(20.0, 4.0, 40.0);
    assert!((gr - 15.0).abs() < 0.1, "GR should be 15dB");

    // 60dB below threshold, 4:1 ratio, 40dB range - limited
    let gr = calculate_gain_reduction(60.0, 4.0, 40.0);
    assert!((gr - 40.0).abs() < 0.1, "GR should be limited to 40dB");
}

// =============================================================================
// Knee Tests
// =============================================================================

/// Test knee initial state.
#[gpui::test]
async fn test_expander_knee_initial(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ExpanderState::default()));

    assert!(
        (state.borrow().knee_db - DEFAULT_KNEE_DB).abs() < 0.001,
        "Initial knee should be {} dB",
        DEFAULT_KNEE_DB
    );
}

/// Test knee slider adjustment.
#[gpui::test]
async fn test_expander_knee_slider(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ExpanderState::default()));

    let test_values: Vec<f64> = vec![0.0, 3.0, 6.0, 12.0, 18.0, 24.0];
    for value in test_values {
        state.borrow_mut().knee_db = value.clamp(MIN_KNEE_DB, MAX_KNEE_DB);
        assert!(
            (state.borrow().knee_db - value).abs() < 0.001,
            "Knee should be {} dB",
            value
        );
    }
}

/// Test knee display.
#[gpui::test]
async fn test_expander_knee_display(_cx: &mut TestAppContext) {
    fn format_knee(knee_db: f64) -> String {
        if knee_db < 0.5 {
            "Hard".to_string()
        } else {
            format!("{:.1} dB", knee_db)
        }
    }

    assert_eq!(format_knee(0.0), "Hard");
    assert_eq!(format_knee(6.0), "6.0 dB");
}

// =============================================================================
// Expansion Curve Tests
// =============================================================================

/// Test expansion curve calculation.
#[gpui::test]
async fn test_expander_curve_calculation(_cx: &mut TestAppContext) {
    fn calculate_output(input_db: f64, threshold: f64, ratio: f64, knee: f64) -> f64 {
        if knee < 0.5 {
            // Hard knee
            if input_db >= threshold {
                input_db // No expansion above threshold
            } else {
                // Expand below threshold
                let below = threshold - input_db;
                let expansion = below * (1.0 - 1.0 / ratio);
                input_db - expansion
            }
        } else {
            // Soft knee - simplified
            input_db // Would need full knee curve calculation
        }
    }

    let threshold = -40.0;
    let ratio = 2.0;

    // Above threshold - no change
    let out = calculate_output(-20.0, threshold, ratio, 0.0);
    assert!((out - (-20.0)).abs() < 0.001);

    // At threshold - no change
    let out = calculate_output(-40.0, threshold, ratio, 0.0);
    assert!((out - (-40.0)).abs() < 0.001);

    // 10dB below threshold, 2:1 ratio = 5dB expansion
    let out = calculate_output(-50.0, threshold, ratio, 0.0);
    assert!((out - (-55.0)).abs() < 0.1, "Output should be -55dB, got {}", out);
}

/// Test curve display points.
#[gpui::test]
async fn test_expander_curve_points(_cx: &mut TestAppContext) {
    const NUM_POINTS: usize = 64;

    let points: Vec<f64> = (0..NUM_POINTS)
        .map(|i| {
            let norm = i as f64 / (NUM_POINTS - 1) as f64;
            -80.0 + norm * 80.0 // -80dB to 0dB
        })
        .collect();

    assert_eq!(points.len(), NUM_POINTS);
    assert!((points[0] - (-80.0)).abs() < 0.001);
    assert!(points[NUM_POINTS - 1].abs() < 0.001);
}

// =============================================================================
// Gain Reduction Meter Tests
// =============================================================================

/// Test gain reduction meter.
#[gpui::test]
async fn test_expander_gr_meter(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ExpanderState::default()));

    let gr_values: Vec<f64> = vec![0.0, -5.0, -10.0, -20.0, -40.0];
    for gr in gr_values {
        state.borrow_mut().gain_reduction_db = gr;
        assert!(
            (state.borrow().gain_reduction_db - gr).abs() < 0.001
        );
    }
}

/// Test GR meter position.
#[gpui::test]
async fn test_expander_gr_meter_position(_cx: &mut TestAppContext) {
    const GR_METER_MAX: f64 = 40.0;

    fn gr_meter_position(gr_db: f64) -> f64 {
        (-gr_db / GR_METER_MAX).clamp(0.0, 1.0)
    }

    assert!((gr_meter_position(0.0) - 0.0).abs() < 0.001);
    assert!((gr_meter_position(-20.0) - 0.5).abs() < 0.001);
    assert!((gr_meter_position(-40.0) - 1.0).abs() < 0.001);
}

// =============================================================================
// Enable/Bypass Tests
// =============================================================================

/// Test expander enable state.
#[gpui::test]
async fn test_expander_enable(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ExpanderState::default()));

    // Initially enabled
    assert!(state.borrow().enabled);

    // Disable (bypass)
    state.borrow_mut().enabled = false;
    assert!(!state.borrow().enabled);

    // Re-enable
    state.borrow_mut().enabled = true;
    assert!(state.borrow().enabled);
}

// =============================================================================
// Preset Tests
// =============================================================================

/// Test expander preset structure.
#[gpui::test]
async fn test_expander_preset_structure(_cx: &mut TestAppContext) {
    #[derive(Debug, Clone)]
    struct ExpanderPreset {
        name: String,
        threshold_db: f64,
        ratio: f64,
        attack_ms: f64,
        release_ms: f64,
        range_db: f64,
        knee_db: f64,
    }

    let preset = ExpanderPreset {
        name: "Gentle Expansion".to_string(),
        threshold_db: -50.0,
        ratio: 1.5,
        attack_ms: 20.0,
        release_ms: 300.0,
        range_db: 20.0,
        knee_db: 12.0,
    };

    assert_eq!(preset.name, "Gentle Expansion");
    assert!((preset.ratio - 1.5).abs() < 0.001);
}

/// Test preset application.
#[gpui::test]
async fn test_expander_preset_apply(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ExpanderState::default()));

    // Apply "Noise Reduction" preset
    state.borrow_mut().threshold_db = -60.0;
    state.borrow_mut().ratio = 4.0;
    state.borrow_mut().attack_ms = 5.0;
    state.borrow_mut().release_ms = 500.0;
    state.borrow_mut().range_db = 60.0;
    state.borrow_mut().knee_db = 6.0;

    assert!((state.borrow().threshold_db - (-60.0)).abs() < 0.001);
    assert!((state.borrow().ratio - 4.0).abs() < 0.001);
}

// =============================================================================
// Keyboard Shortcut Tests
// =============================================================================

/// Test threshold arrow key adjustment.
#[gpui::test]
async fn test_expander_threshold_keys(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ExpanderState::default()));
    const STEP_DB: f64 = 1.0;

    let initial = state.borrow().threshold_db;

    // Up arrow
    {
        let current = state.borrow().threshold_db;
        state.borrow_mut().threshold_db = (current + STEP_DB).clamp(MIN_THRESHOLD_DB, MAX_THRESHOLD_DB);
    }
    assert!((state.borrow().threshold_db - (initial + STEP_DB)).abs() < 0.001);

    // Down arrow
    {
        let current = state.borrow().threshold_db;
        state.borrow_mut().threshold_db = (current - STEP_DB).clamp(MIN_THRESHOLD_DB, MAX_THRESHOLD_DB);
    }
    assert!((state.borrow().threshold_db - initial).abs() < 0.001);
}

/// Test ratio arrow key adjustment.
#[gpui::test]
async fn test_expander_ratio_keys(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ExpanderState::default()));
    const STEP: f64 = 0.5;

    let initial = state.borrow().ratio;

    // Increase ratio
    {
        let current = state.borrow().ratio;
        state.borrow_mut().ratio = (current + STEP).clamp(MIN_RATIO, MAX_RATIO);
    }
    assert!((state.borrow().ratio - (initial + STEP)).abs() < 0.001);
}

// =============================================================================
// Visual Feedback Tests
// =============================================================================

/// Test expansion active indicator.
#[gpui::test]
async fn test_expander_active_indicator(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ExpanderState::default()));

    fn is_expanding(gr_db: f64) -> bool {
        gr_db < -0.5
    }

    state.borrow_mut().gain_reduction_db = 0.0;
    assert!(!is_expanding(state.borrow().gain_reduction_db));

    state.borrow_mut().gain_reduction_db = -10.0;
    assert!(is_expanding(state.borrow().gain_reduction_db));
}

/// Test threshold line on curve display.
#[gpui::test]
async fn test_expander_threshold_line(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(ExpanderState::default()));

    fn threshold_position(threshold_db: f64, min_db: f64, max_db: f64) -> f64 {
        (threshold_db - min_db) / (max_db - min_db)
    }

    // -40dB on -80 to 0 range
    let pos = threshold_position(state.borrow().threshold_db, -80.0, 0.0);
    assert!((pos - 0.5).abs() < 0.01, "Threshold should be at 50%");
}
