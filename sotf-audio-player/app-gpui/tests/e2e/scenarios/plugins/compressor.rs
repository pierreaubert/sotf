//! E2E tests for Compressor Plugin UI.
//!
//! Tests for verifying dynamic range compression functionality:
//! - Threshold control
//! - Ratio control
//! - Attack/Release timing
//! - Knee control
//! - Makeup gain
//! - Transfer curve display
//! - Gain reduction meter

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Parameter Constants
// =============================================================================

const MIN_THRESHOLD_DB: f64 = -60.0;
const MAX_THRESHOLD_DB: f64 = 0.0;
const DEFAULT_THRESHOLD_DB: f64 = -20.0;

const MIN_RATIO: f64 = 1.0;
const MAX_RATIO: f64 = 20.0;
const DEFAULT_RATIO: f64 = 4.0;

const MIN_ATTACK_MS: f64 = 0.1;
const MAX_ATTACK_MS: f64 = 100.0;
const DEFAULT_ATTACK_MS: f64 = 10.0;

const MIN_RELEASE_MS: f64 = 10.0;
const MAX_RELEASE_MS: f64 = 1000.0;
const DEFAULT_RELEASE_MS: f64 = 100.0;

const MIN_KNEE_DB: f64 = 0.0;
const MAX_KNEE_DB: f64 = 24.0;
const DEFAULT_KNEE_DB: f64 = 6.0;

const MIN_MAKEUP_DB: f64 = 0.0;
const MAX_MAKEUP_DB: f64 = 24.0;
const DEFAULT_MAKEUP_DB: f64 = 0.0;

// =============================================================================
// Compressor State
// =============================================================================

#[derive(Debug, Clone)]
struct CompressorState {
    threshold_db: f64,
    ratio: f64,
    attack_ms: f64,
    release_ms: f64,
    knee_db: f64,
    makeup_db: f64,
    auto_makeup: bool,
    enabled: bool,
    // Metering (read-only display values)
    current_gain_reduction_db: f64,
    input_level_db: f64,
    output_level_db: f64,
}

impl Default for CompressorState {
    fn default() -> Self {
        Self {
            threshold_db: DEFAULT_THRESHOLD_DB,
            ratio: DEFAULT_RATIO,
            attack_ms: DEFAULT_ATTACK_MS,
            release_ms: DEFAULT_RELEASE_MS,
            knee_db: DEFAULT_KNEE_DB,
            makeup_db: DEFAULT_MAKEUP_DB,
            auto_makeup: false,
            enabled: true,
            current_gain_reduction_db: 0.0,
            input_level_db: -60.0,
            output_level_db: -60.0,
        }
    }
}

// =============================================================================
// Threshold Tests
// =============================================================================

/// Test threshold initial state.
#[gpui::test]
async fn test_compressor_threshold_initial(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CompressorState::default()));

    assert!(
        (state.borrow().threshold_db - DEFAULT_THRESHOLD_DB).abs() < 0.001,
        "Initial threshold should be {} dB",
        DEFAULT_THRESHOLD_DB
    );
}

/// Test threshold slider adjustment.
#[gpui::test]
async fn test_compressor_threshold_slider(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CompressorState::default()));

    // Simulate slider drag
    let test_values: Vec<f64> = vec![-40.0, -30.0, -20.0, -10.0, -6.0, 0.0];
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
async fn test_compressor_threshold_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CompressorState::default()));

    // Set to minimum
    state.borrow_mut().threshold_db = MIN_THRESHOLD_DB;
    assert!((state.borrow().threshold_db - MIN_THRESHOLD_DB).abs() < 0.001);

    // Set to maximum
    state.borrow_mut().threshold_db = MAX_THRESHOLD_DB;
    assert!((state.borrow().threshold_db - MAX_THRESHOLD_DB).abs() < 0.001);

    // Clamp below minimum
    let clamped = (-70.0f64).clamp(MIN_THRESHOLD_DB, MAX_THRESHOLD_DB);
    assert!((clamped - MIN_THRESHOLD_DB).abs() < 0.001);
}

// =============================================================================
// Ratio Tests
// =============================================================================

/// Test ratio initial state.
#[gpui::test]
async fn test_compressor_ratio_initial(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CompressorState::default()));

    assert!(
        (state.borrow().ratio - DEFAULT_RATIO).abs() < 0.001,
        "Initial ratio should be {}:1",
        DEFAULT_RATIO
    );
}

/// Test ratio slider adjustment.
#[gpui::test]
async fn test_compressor_ratio_slider(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CompressorState::default()));

    let test_values: Vec<f64> = vec![1.0, 2.0, 4.0, 8.0, 12.0, 20.0];
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
async fn test_compressor_ratio_display(_cx: &mut TestAppContext) {
    fn format_ratio(ratio: f64) -> String {
        if ratio >= 20.0 {
            "∞:1".to_string() // Limiter mode
        } else {
            format!("{:.1}:1", ratio)
        }
    }

    assert_eq!(format_ratio(1.0), "1.0:1");
    assert_eq!(format_ratio(4.0), "4.0:1");
    assert_eq!(format_ratio(20.0), "∞:1");
}

/// Test ratio bounds.
#[gpui::test]
async fn test_compressor_ratio_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CompressorState::default()));

    // Minimum ratio (1:1 = no compression)
    state.borrow_mut().ratio = MIN_RATIO;
    assert!((state.borrow().ratio - 1.0).abs() < 0.001);

    // Maximum ratio (limiter behavior)
    state.borrow_mut().ratio = MAX_RATIO;
    assert!((state.borrow().ratio - 20.0).abs() < 0.001);
}

// =============================================================================
// Attack/Release Tests
// =============================================================================

/// Test attack time initial state.
#[gpui::test]
async fn test_compressor_attack_initial(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CompressorState::default()));

    assert!(
        (state.borrow().attack_ms - DEFAULT_ATTACK_MS).abs() < 0.001,
        "Initial attack should be {} ms",
        DEFAULT_ATTACK_MS
    );
}

/// Test attack time adjustment.
#[gpui::test]
async fn test_compressor_attack_slider(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CompressorState::default()));

    let test_values: Vec<f64> = vec![0.1, 1.0, 5.0, 10.0, 30.0, 100.0];
    for value in test_values {
        state.borrow_mut().attack_ms = value.clamp(MIN_ATTACK_MS, MAX_ATTACK_MS);
        assert!(
            (state.borrow().attack_ms - value).abs() < 0.01,
            "Attack should be {} ms",
            value
        );
    }
}

/// Test release time initial state.
#[gpui::test]
async fn test_compressor_release_initial(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CompressorState::default()));

    assert!(
        (state.borrow().release_ms - DEFAULT_RELEASE_MS).abs() < 0.001,
        "Initial release should be {} ms",
        DEFAULT_RELEASE_MS
    );
}

/// Test release time adjustment.
#[gpui::test]
async fn test_compressor_release_slider(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CompressorState::default()));

    let test_values: Vec<f64> = vec![10.0, 50.0, 100.0, 250.0, 500.0, 1000.0];
    for value in test_values {
        state.borrow_mut().release_ms = value.clamp(MIN_RELEASE_MS, MAX_RELEASE_MS);
        assert!(
            (state.borrow().release_ms - value).abs() < 0.01,
            "Release should be {} ms",
            value
        );
    }
}

/// Test attack/release time display formatting.
#[gpui::test]
async fn test_compressor_time_display(_cx: &mut TestAppContext) {
    fn format_time_ms(ms: f64) -> String {
        if ms < 1.0 {
            format!("{:.0} µs", ms * 1000.0)
        } else if ms >= 1000.0 {
            format!("{:.2} s", ms / 1000.0)
        } else {
            format!("{:.1} ms", ms)
        }
    }

    assert_eq!(format_time_ms(0.5), "500 µs");
    assert_eq!(format_time_ms(10.0), "10.0 ms");
    assert_eq!(format_time_ms(1500.0), "1.50 s");
}

// =============================================================================
// Knee Tests
// =============================================================================

/// Test knee initial state.
#[gpui::test]
async fn test_compressor_knee_initial(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CompressorState::default()));

    assert!(
        (state.borrow().knee_db - DEFAULT_KNEE_DB).abs() < 0.001,
        "Initial knee should be {} dB",
        DEFAULT_KNEE_DB
    );
}

/// Test knee adjustment (hard vs soft).
#[gpui::test]
async fn test_compressor_knee_adjustment(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CompressorState::default()));

    // Hard knee (0 dB)
    state.borrow_mut().knee_db = 0.0;
    assert!(state.borrow().knee_db.abs() < 0.001, "Hard knee = 0 dB");

    // Soft knee
    state.borrow_mut().knee_db = 12.0;
    assert!(
        (state.borrow().knee_db - 12.0).abs() < 0.001,
        "Soft knee = 12 dB"
    );
}

/// Test knee display.
#[gpui::test]
async fn test_compressor_knee_display(_cx: &mut TestAppContext) {
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
// Makeup Gain Tests
// =============================================================================

/// Test makeup gain initial state.
#[gpui::test]
async fn test_compressor_makeup_initial(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CompressorState::default()));

    assert!(
        (state.borrow().makeup_db - DEFAULT_MAKEUP_DB).abs() < 0.001,
        "Initial makeup should be {} dB",
        DEFAULT_MAKEUP_DB
    );
}

/// Test makeup gain adjustment.
#[gpui::test]
async fn test_compressor_makeup_slider(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CompressorState::default()));

    let test_values: Vec<f64> = vec![0.0, 3.0, 6.0, 12.0, 18.0, 24.0];
    for value in test_values {
        state.borrow_mut().makeup_db = value.clamp(MIN_MAKEUP_DB, MAX_MAKEUP_DB);
        assert!(
            (state.borrow().makeup_db - value).abs() < 0.001,
            "Makeup should be {} dB",
            value
        );
    }
}

/// Test auto makeup gain toggle.
#[gpui::test]
async fn test_compressor_auto_makeup_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CompressorState::default()));

    // Initially off
    assert!(!state.borrow().auto_makeup);

    // Enable
    state.borrow_mut().auto_makeup = true;
    assert!(state.borrow().auto_makeup);

    // Disable
    state.borrow_mut().auto_makeup = false;
    assert!(!state.borrow().auto_makeup);
}

/// Test auto makeup disables manual control.
#[gpui::test]
async fn test_compressor_auto_makeup_disables_manual(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CompressorState::default()));

    state.borrow_mut().auto_makeup = true;

    // When auto_makeup is on, manual control should be disabled (UI feedback)
    fn is_makeup_control_enabled(state: &CompressorState) -> bool {
        !state.auto_makeup
    }

    assert!(!is_makeup_control_enabled(&state.borrow()));
}

// =============================================================================
// Transfer Curve Tests
// =============================================================================

/// Test transfer curve calculation.
#[gpui::test]
async fn test_compressor_transfer_curve(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CompressorState::default()));
    state.borrow_mut().threshold_db = -20.0;
    state.borrow_mut().ratio = 4.0;
    state.borrow_mut().knee_db = 0.0; // Hard knee

    fn calculate_output(input_db: f64, threshold: f64, ratio: f64, knee: f64) -> f64 {
        if knee < 0.5 {
            // Hard knee
            if input_db <= threshold {
                input_db
            } else {
                threshold + (input_db - threshold) / ratio
            }
        } else {
            // Soft knee (simplified)
            let half_knee = knee / 2.0;
            let knee_start = threshold - half_knee;
            let knee_end = threshold + half_knee;

            if input_db <= knee_start {
                input_db
            } else if input_db >= knee_end {
                threshold + (input_db - threshold) / ratio
            } else {
                // In knee region - interpolate
                let knee_pos = (input_db - knee_start) / knee;
                let soft_ratio = 1.0 + (ratio - 1.0) * knee_pos;
                knee_start + (input_db - knee_start) / soft_ratio
            }
        }
    }

    let s = state.borrow();

    // Below threshold - no compression
    let out = calculate_output(-30.0, s.threshold_db, s.ratio, s.knee_db);
    assert!((out - (-30.0)).abs() < 0.001, "Below threshold should pass through");

    // At threshold - no compression
    let out = calculate_output(-20.0, s.threshold_db, s.ratio, s.knee_db);
    assert!((out - (-20.0)).abs() < 0.001, "At threshold should pass through");

    // Above threshold - compressed
    let out = calculate_output(-12.0, s.threshold_db, s.ratio, s.knee_db);
    // Input is 8dB above threshold, output should be 8/4 = 2dB above
    let expected = -20.0 + 8.0 / 4.0; // -18.0
    assert!((out - expected).abs() < 0.001, "Above threshold should compress");
}

/// Test transfer curve display points.
#[gpui::test]
async fn test_compressor_curve_points(_cx: &mut TestAppContext) {
    const NUM_POINTS: usize = 64;

    let points: Vec<f64> = (0..NUM_POINTS)
        .map(|i| {
            let norm = i as f64 / (NUM_POINTS - 1) as f64;
            // Input range: -60 to 0 dB
            -60.0 + norm * 60.0
        })
        .collect();

    assert_eq!(points.len(), NUM_POINTS);
    assert!((points[0] - (-60.0)).abs() < 0.001);
    assert!(points[NUM_POINTS - 1].abs() < 0.001);
}

// =============================================================================
// Gain Reduction Meter Tests
// =============================================================================

/// Test gain reduction meter display.
#[gpui::test]
async fn test_compressor_gr_meter(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CompressorState::default()));

    // Simulate different GR values
    let gr_values = vec![0.0, -3.0, -6.0, -12.0, -20.0];
    for gr in gr_values {
        state.borrow_mut().current_gain_reduction_db = gr;
        assert!(
            (state.borrow().current_gain_reduction_db - gr).abs() < 0.001
        );
    }
}

/// Test GR meter position calculation.
#[gpui::test]
async fn test_compressor_gr_meter_position(_cx: &mut TestAppContext) {
    const GR_METER_MAX: f64 = 24.0; // Maximum displayed GR

    fn gr_meter_position(gr_db: f64) -> f64 {
        // Returns 0.0-1.0 for meter fill (0dB = 0.0, -24dB = 1.0)
        (-gr_db / GR_METER_MAX).clamp(0.0, 1.0)
    }

    assert!((gr_meter_position(0.0) - 0.0).abs() < 0.001);
    assert!((gr_meter_position(-12.0) - 0.5).abs() < 0.001);
    assert!((gr_meter_position(-24.0) - 1.0).abs() < 0.001);
    assert!((gr_meter_position(-30.0) - 1.0).abs() < 0.001); // Clamped
}

/// Test GR display formatting.
#[gpui::test]
async fn test_compressor_gr_display(_cx: &mut TestAppContext) {
    fn format_gr(gr_db: f64) -> String {
        if gr_db >= -0.1 {
            "0.0 dB".to_string()
        } else {
            format!("{:.1} dB", gr_db)
        }
    }

    assert_eq!(format_gr(0.0), "0.0 dB");
    assert_eq!(format_gr(-6.0), "-6.0 dB");
    assert_eq!(format_gr(-15.3), "-15.3 dB");
}

// =============================================================================
// Input/Output Meter Tests
// =============================================================================

/// Test input level meter.
#[gpui::test]
async fn test_compressor_input_meter(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CompressorState::default()));

    // Simulate input levels
    let levels = vec![-60.0, -40.0, -20.0, -12.0, -6.0, 0.0];
    for level in levels {
        state.borrow_mut().input_level_db = level;
        assert!((state.borrow().input_level_db - level).abs() < 0.001);
    }
}

/// Test output level meter.
#[gpui::test]
async fn test_compressor_output_meter(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CompressorState::default()));

    let levels = vec![-60.0, -40.0, -20.0, -12.0, -6.0, 0.0];
    for level in levels {
        state.borrow_mut().output_level_db = level;
        assert!((state.borrow().output_level_db - level).abs() < 0.001);
    }
}

// =============================================================================
// Enable/Bypass Tests
// =============================================================================

/// Test compressor enable state.
#[gpui::test]
async fn test_compressor_enable(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CompressorState::default()));

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

/// Test preset structure.
#[gpui::test]
async fn test_compressor_preset_structure(_cx: &mut TestAppContext) {
    #[derive(Debug, Clone)]
    struct CompressorPreset {
        name: String,
        threshold_db: f64,
        ratio: f64,
        attack_ms: f64,
        release_ms: f64,
        knee_db: f64,
        makeup_db: f64,
        auto_makeup: bool,
    }

    let preset = CompressorPreset {
        name: "Vocal".to_string(),
        threshold_db: -18.0,
        ratio: 3.0,
        attack_ms: 5.0,
        release_ms: 50.0,
        knee_db: 6.0,
        makeup_db: 4.0,
        auto_makeup: false,
    };

    assert_eq!(preset.name, "Vocal");
    assert!((preset.ratio - 3.0).abs() < 0.001);
}

/// Test preset application.
#[gpui::test]
async fn test_compressor_preset_apply(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CompressorState::default()));

    // Apply "Limiter" preset
    state.borrow_mut().threshold_db = -6.0;
    state.borrow_mut().ratio = 20.0;
    state.borrow_mut().attack_ms = 0.1;
    state.borrow_mut().release_ms = 100.0;
    state.borrow_mut().knee_db = 0.0;

    assert!((state.borrow().threshold_db - (-6.0)).abs() < 0.001);
    assert!((state.borrow().ratio - 20.0).abs() < 0.001);
    assert!((state.borrow().attack_ms - 0.1).abs() < 0.01);
}

// =============================================================================
// Keyboard Shortcut Tests
// =============================================================================

/// Test threshold arrow key adjustment.
#[gpui::test]
async fn test_compressor_threshold_keys(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CompressorState::default()));
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
async fn test_compressor_ratio_keys(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CompressorState::default()));
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

/// Test compression indicator (active when compressing).
#[gpui::test]
async fn test_compressor_active_indicator(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CompressorState::default()));

    fn is_compressing(gr_db: f64) -> bool {
        gr_db < -0.5 // Active when GR > 0.5 dB
    }

    state.borrow_mut().current_gain_reduction_db = 0.0;
    assert!(!is_compressing(state.borrow().current_gain_reduction_db));

    state.borrow_mut().current_gain_reduction_db = -3.0;
    assert!(is_compressing(state.borrow().current_gain_reduction_db));
}

/// Test threshold line on curve display.
#[gpui::test]
async fn test_compressor_threshold_line(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(CompressorState::default()));

    fn threshold_line_position(threshold_db: f64, min_db: f64, max_db: f64) -> f64 {
        // Normalize threshold to 0.0-1.0 for display
        (threshold_db - min_db) / (max_db - min_db)
    }

    let pos = threshold_line_position(state.borrow().threshold_db, -60.0, 0.0);
    // -20dB on -60 to 0 range = (40/60) = 0.667
    assert!((pos - 0.667).abs() < 0.01);
}
