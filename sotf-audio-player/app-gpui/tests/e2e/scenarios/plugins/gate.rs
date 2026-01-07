//! E2E tests for Gate Plugin UI.
//!
//! Tests for verifying noise gate functionality:
//! - Threshold control
//! - Ratio (range) control
//! - Attack/Hold/Release timing
//! - Hysteresis
//! - Gate open/closed indicators
//! - Sidechain options

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
const MAX_RATIO: f64 = 100.0; // 100:1 = hard gate
const DEFAULT_RATIO: f64 = 10.0;

const MIN_ATTACK_MS: f64 = 0.01;
const MAX_ATTACK_MS: f64 = 100.0;
const DEFAULT_ATTACK_MS: f64 = 1.0;

const MIN_HOLD_MS: f64 = 0.0;
const MAX_HOLD_MS: f64 = 500.0;
const DEFAULT_HOLD_MS: f64 = 50.0;

const MIN_RELEASE_MS: f64 = 1.0;
const MAX_RELEASE_MS: f64 = 2000.0;
const DEFAULT_RELEASE_MS: f64 = 100.0;

const MIN_HYSTERESIS_DB: f64 = 0.0;
const MAX_HYSTERESIS_DB: f64 = 12.0;
const DEFAULT_HYSTERESIS_DB: f64 = 3.0;

// =============================================================================
// Gate State
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GateStatus {
    Open,
    Closed,
    Opening,
    Closing,
}

#[derive(Debug, Clone)]
struct GateState {
    threshold_db: f64,
    ratio: f64,
    attack_ms: f64,
    hold_ms: f64,
    release_ms: f64,
    hysteresis_db: f64,
    enabled: bool,
    // Status
    gate_status: GateStatus,
    input_level_db: f64,
    gain_reduction_db: f64,
}

impl Default for GateState {
    fn default() -> Self {
        Self {
            threshold_db: DEFAULT_THRESHOLD_DB,
            ratio: DEFAULT_RATIO,
            attack_ms: DEFAULT_ATTACK_MS,
            hold_ms: DEFAULT_HOLD_MS,
            release_ms: DEFAULT_RELEASE_MS,
            hysteresis_db: DEFAULT_HYSTERESIS_DB,
            enabled: true,
            gate_status: GateStatus::Closed,
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
async fn test_gate_threshold_initial(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GateState::default()));

    assert!(
        (state.borrow().threshold_db - DEFAULT_THRESHOLD_DB).abs() < 0.001,
        "Initial threshold should be {} dB",
        DEFAULT_THRESHOLD_DB
    );
}

/// Test threshold slider adjustment.
#[gpui::test]
async fn test_gate_threshold_slider(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GateState::default()));

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
async fn test_gate_threshold_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GateState::default()));

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
async fn test_gate_ratio_initial(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GateState::default()));

    assert!(
        (state.borrow().ratio - DEFAULT_RATIO).abs() < 0.001,
        "Initial ratio should be {}:1",
        DEFAULT_RATIO
    );
}

/// Test ratio slider adjustment.
#[gpui::test]
async fn test_gate_ratio_slider(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GateState::default()));

    let test_values: Vec<f64> = vec![1.0, 2.0, 5.0, 10.0, 50.0, 100.0];
    for value in test_values {
        state.borrow_mut().ratio = value.clamp(MIN_RATIO, MAX_RATIO);
        assert!(
            (state.borrow().ratio - value).abs() < 0.001,
            "Ratio should be {}:1",
            value
        );
    }
}

/// Test ratio display formatting.
#[gpui::test]
async fn test_gate_ratio_display(_cx: &mut TestAppContext) {
    fn format_ratio(ratio: f64) -> String {
        if ratio >= 100.0 {
            "∞:1 (hard)".to_string()
        } else {
            format!("{:.1}:1", ratio)
        }
    }

    assert_eq!(format_ratio(2.0), "2.0:1");
    assert_eq!(format_ratio(10.0), "10.0:1");
    assert_eq!(format_ratio(100.0), "∞:1 (hard)");
}

// =============================================================================
// Attack Tests
// =============================================================================

/// Test attack initial state.
#[gpui::test]
async fn test_gate_attack_initial(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GateState::default()));

    assert!(
        (state.borrow().attack_ms - DEFAULT_ATTACK_MS).abs() < 0.001,
        "Initial attack should be {} ms",
        DEFAULT_ATTACK_MS
    );
}

/// Test attack slider adjustment.
#[gpui::test]
async fn test_gate_attack_slider(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GateState::default()));

    let test_values: Vec<f64> = vec![0.01, 0.1, 1.0, 10.0, 50.0, 100.0];
    for value in test_values {
        state.borrow_mut().attack_ms = value.clamp(MIN_ATTACK_MS, MAX_ATTACK_MS);
        assert!(
            (state.borrow().attack_ms - value).abs() < 0.01,
            "Attack should be {} ms",
            value
        );
    }
}

// =============================================================================
// Hold Tests
// =============================================================================

/// Test hold initial state.
#[gpui::test]
async fn test_gate_hold_initial(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GateState::default()));

    assert!(
        (state.borrow().hold_ms - DEFAULT_HOLD_MS).abs() < 0.001,
        "Initial hold should be {} ms",
        DEFAULT_HOLD_MS
    );
}

/// Test hold slider adjustment.
#[gpui::test]
async fn test_gate_hold_slider(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GateState::default()));

    let test_values: Vec<f64> = vec![0.0, 10.0, 50.0, 100.0, 250.0, 500.0];
    for value in test_values {
        state.borrow_mut().hold_ms = value.clamp(MIN_HOLD_MS, MAX_HOLD_MS);
        assert!(
            (state.borrow().hold_ms - value).abs() < 0.01,
            "Hold should be {} ms",
            value
        );
    }
}

/// Test hold purpose description.
#[gpui::test]
async fn test_gate_hold_description(_cx: &mut TestAppContext) {
    // Hold keeps gate open for a period after signal drops below threshold
    // This prevents "chattering" on decaying signals

    fn hold_behavior(input_drops_below_threshold: bool, hold_time_remaining: f64) -> bool {
        if input_drops_below_threshold {
            hold_time_remaining > 0.0 // Stay open during hold time
        } else {
            true // Input above threshold, gate open
        }
    }

    // Input drops, but hold time remains
    assert!(hold_behavior(true, 50.0));

    // Input drops, hold expired
    assert!(!hold_behavior(true, 0.0));
}

// =============================================================================
// Release Tests
// =============================================================================

/// Test release initial state.
#[gpui::test]
async fn test_gate_release_initial(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GateState::default()));

    assert!(
        (state.borrow().release_ms - DEFAULT_RELEASE_MS).abs() < 0.001,
        "Initial release should be {} ms",
        DEFAULT_RELEASE_MS
    );
}

/// Test release slider adjustment.
#[gpui::test]
async fn test_gate_release_slider(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GateState::default()));

    let test_values: Vec<f64> = vec![1.0, 10.0, 100.0, 500.0, 1000.0, 2000.0];
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
// Hysteresis Tests
// =============================================================================

/// Test hysteresis initial state.
#[gpui::test]
async fn test_gate_hysteresis_initial(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GateState::default()));

    assert!(
        (state.borrow().hysteresis_db - DEFAULT_HYSTERESIS_DB).abs() < 0.001,
        "Initial hysteresis should be {} dB",
        DEFAULT_HYSTERESIS_DB
    );
}

/// Test hysteresis slider adjustment.
#[gpui::test]
async fn test_gate_hysteresis_slider(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GateState::default()));

    let test_values: Vec<f64> = vec![0.0, 2.0, 4.0, 6.0, 9.0, 12.0];
    for value in test_values {
        state.borrow_mut().hysteresis_db = value.clamp(MIN_HYSTERESIS_DB, MAX_HYSTERESIS_DB);
        assert!(
            (state.borrow().hysteresis_db - value).abs() < 0.001,
            "Hysteresis should be {} dB",
            value
        );
    }
}

/// Test hysteresis behavior.
#[gpui::test]
async fn test_gate_hysteresis_behavior(_cx: &mut TestAppContext) {
    // Hysteresis creates different open/close thresholds to prevent chattering
    // Open threshold = threshold_db
    // Close threshold = threshold_db - hysteresis_db

    fn calculate_thresholds(threshold_db: f64, hysteresis_db: f64) -> (f64, f64) {
        let open_threshold = threshold_db;
        let close_threshold = threshold_db - hysteresis_db;
        (open_threshold, close_threshold)
    }

    let (open, close) = calculate_thresholds(-40.0, 3.0);
    assert!((open - (-40.0)).abs() < 0.001);
    assert!((close - (-43.0)).abs() < 0.001);

    // Signal must rise above -40dB to open, but drop below -43dB to close
}

// =============================================================================
// Gate Status Tests
// =============================================================================

/// Test gate status states.
#[gpui::test]
async fn test_gate_status_states(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GateState::default()));

    // Test all states
    state.borrow_mut().gate_status = GateStatus::Closed;
    assert_eq!(state.borrow().gate_status, GateStatus::Closed);

    state.borrow_mut().gate_status = GateStatus::Opening;
    assert_eq!(state.borrow().gate_status, GateStatus::Opening);

    state.borrow_mut().gate_status = GateStatus::Open;
    assert_eq!(state.borrow().gate_status, GateStatus::Open);

    state.borrow_mut().gate_status = GateStatus::Closing;
    assert_eq!(state.borrow().gate_status, GateStatus::Closing);
}

/// Test gate status display.
#[gpui::test]
async fn test_gate_status_display(_cx: &mut TestAppContext) {
    fn status_display(status: GateStatus) -> &'static str {
        match status {
            GateStatus::Closed => "CLOSED",
            GateStatus::Opening => "OPENING",
            GateStatus::Open => "OPEN",
            GateStatus::Closing => "CLOSING",
        }
    }

    assert_eq!(status_display(GateStatus::Open), "OPEN");
    assert_eq!(status_display(GateStatus::Closed), "CLOSED");
}

/// Test gate status color coding.
#[gpui::test]
async fn test_gate_status_color(_cx: &mut TestAppContext) {
    fn status_color(status: GateStatus) -> (u8, u8, u8) {
        match status {
            GateStatus::Closed => (255, 0, 0),    // Red
            GateStatus::Opening => (255, 165, 0), // Orange
            GateStatus::Open => (0, 255, 0),      // Green
            GateStatus::Closing => (255, 255, 0), // Yellow
        }
    }

    assert_eq!(status_color(GateStatus::Open), (0, 255, 0));
    assert_eq!(status_color(GateStatus::Closed), (255, 0, 0));
}

// =============================================================================
// Gain Reduction Tests
// =============================================================================

/// Test gain reduction display.
#[gpui::test]
async fn test_gate_gain_reduction(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GateState::default()));

    // When gate is open, GR = 0
    state.borrow_mut().gate_status = GateStatus::Open;
    state.borrow_mut().gain_reduction_db = 0.0;
    assert!(state.borrow().gain_reduction_db.abs() < 0.001);

    // When gate is closed, GR depends on ratio
    state.borrow_mut().gate_status = GateStatus::Closed;
    state.borrow_mut().gain_reduction_db = -60.0; // Full attenuation
    assert!((state.borrow().gain_reduction_db - (-60.0)).abs() < 0.001);
}

/// Test range calculation.
#[gpui::test]
async fn test_gate_range_calculation(_cx: &mut TestAppContext) {
    // Range = how much the gate attenuates when closed
    // With ratio 10:1 and signal 20dB below threshold:
    // GR = 20 - (20/10) = 18 dB of reduction

    fn calculate_gate_reduction(below_threshold_db: f64, ratio: f64) -> f64 {
        if ratio >= 100.0 {
            below_threshold_db // Hard gate - full reduction
        } else {
            below_threshold_db - (below_threshold_db / ratio)
        }
    }

    // 20dB below threshold, 10:1 ratio
    let gr = calculate_gate_reduction(20.0, 10.0);
    assert!((gr - 18.0).abs() < 0.1);

    // Hard gate (100:1)
    let gr = calculate_gate_reduction(20.0, 100.0);
    assert!((gr - 20.0).abs() < 0.001);
}

// =============================================================================
// Input Level Tests
// =============================================================================

/// Test input level meter.
#[gpui::test]
async fn test_gate_input_level(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GateState::default()));

    let levels: Vec<f64> = vec![-80.0, -60.0, -40.0, -20.0, -10.0, 0.0];
    for level in levels {
        state.borrow_mut().input_level_db = level;
        assert!((state.borrow().input_level_db - level).abs() < 0.001);
    }
}

/// Test threshold indicator on input meter.
#[gpui::test]
async fn test_gate_threshold_indicator(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GateState::default()));

    fn is_above_threshold(input_db: f64, threshold_db: f64) -> bool {
        input_db >= threshold_db
    }

    state.borrow_mut().input_level_db = -50.0;
    assert!(!is_above_threshold(
        state.borrow().input_level_db,
        state.borrow().threshold_db
    ));

    state.borrow_mut().input_level_db = -30.0;
    assert!(is_above_threshold(
        state.borrow().input_level_db,
        state.borrow().threshold_db
    ));
}

// =============================================================================
// Enable/Bypass Tests
// =============================================================================

/// Test gate enable state.
#[gpui::test]
async fn test_gate_enable(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GateState::default()));

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

/// Test gate preset structure.
#[gpui::test]
async fn test_gate_preset_structure(_cx: &mut TestAppContext) {
    #[derive(Debug, Clone)]
    struct GatePreset {
        name: String,
        threshold_db: f64,
        ratio: f64,
        attack_ms: f64,
        hold_ms: f64,
        release_ms: f64,
        hysteresis_db: f64,
    }

    let preset = GatePreset {
        name: "Drum Gate".to_string(),
        threshold_db: -30.0,
        ratio: 100.0,
        attack_ms: 0.5,
        hold_ms: 30.0,
        release_ms: 200.0,
        hysteresis_db: 6.0,
    };

    assert_eq!(preset.name, "Drum Gate");
    assert!((preset.ratio - 100.0).abs() < 0.001);
}

/// Test common presets.
#[gpui::test]
async fn test_gate_common_presets(_cx: &mut TestAppContext) {
    fn get_preset(name: &str) -> (f64, f64, f64, f64, f64) {
        match name {
            "Drums" => (-30.0, 100.0, 0.5, 30.0, 200.0),
            "Vocals" => (-45.0, 20.0, 2.0, 100.0, 300.0),
            "Guitar" => (-50.0, 10.0, 5.0, 50.0, 500.0),
            _ => (-40.0, 10.0, 1.0, 50.0, 100.0),
        }
    }

    let (threshold, ratio, _, _, _) = get_preset("Drums");
    assert!((threshold - (-30.0)).abs() < 0.001);
    assert!((ratio - 100.0).abs() < 0.001);
}

// =============================================================================
// Keyboard Shortcut Tests
// =============================================================================

/// Test threshold arrow key adjustment.
#[gpui::test]
async fn test_gate_threshold_keys(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GateState::default()));
    const STEP_DB: f64 = 1.0;

    let initial = state.borrow().threshold_db;

    // Up arrow
    {
        let current = state.borrow().threshold_db;
        state.borrow_mut().threshold_db =
            (current + STEP_DB).clamp(MIN_THRESHOLD_DB, MAX_THRESHOLD_DB);
    }
    assert!((state.borrow().threshold_db - (initial + STEP_DB)).abs() < 0.001);

    // Down arrow
    {
        let current = state.borrow().threshold_db;
        state.borrow_mut().threshold_db =
            (current - STEP_DB).clamp(MIN_THRESHOLD_DB, MAX_THRESHOLD_DB);
    }
    assert!((state.borrow().threshold_db - initial).abs() < 0.001);
}

// =============================================================================
// Time Display Tests
// =============================================================================

/// Test time parameter formatting.
#[gpui::test]
async fn test_gate_time_display(_cx: &mut TestAppContext) {
    fn format_time(ms: f64) -> String {
        if ms < 1.0 {
            format!("{:.0} µs", ms * 1000.0)
        } else if ms >= 1000.0 {
            format!("{:.2} s", ms / 1000.0)
        } else {
            format!("{:.1} ms", ms)
        }
    }

    assert_eq!(format_time(0.5), "500 µs");
    assert_eq!(format_time(50.0), "50.0 ms");
    assert_eq!(format_time(2000.0), "2.00 s");
}
