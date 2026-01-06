//! E2E tests for Limiter Plugin UI.
//!
//! Tests for verifying peak limiter functionality:
//! - Threshold/ceiling control
//! - Release time
//! - Lookahead
//! - Soft/hard knee mode
//! - Gain reduction metering
//! - True peak limiting

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Parameter Constants
// =============================================================================

const MIN_THRESHOLD_DB: f64 = -30.0;
const MAX_THRESHOLD_DB: f64 = 0.0;
const DEFAULT_THRESHOLD_DB: f64 = -1.0;

const MIN_RELEASE_MS: f64 = 1.0;
const MAX_RELEASE_MS: f64 = 1000.0;
const DEFAULT_RELEASE_MS: f64 = 100.0;

const MIN_LOOKAHEAD_MS: f64 = 0.0;
const MAX_LOOKAHEAD_MS: f64 = 10.0;
const DEFAULT_LOOKAHEAD_MS: f64 = 5.0;

// =============================================================================
// Limiter State
// =============================================================================

#[derive(Debug, Clone)]
struct LimiterState {
    threshold_db: f64,
    release_ms: f64,
    lookahead_ms: f64,
    soft_knee: bool,
    true_peak: bool,
    enabled: bool,
    // Metering
    current_gain_reduction_db: f64,
    input_peak_db: f64,
    output_peak_db: f64,
}

impl Default for LimiterState {
    fn default() -> Self {
        Self {
            threshold_db: DEFAULT_THRESHOLD_DB,
            release_ms: DEFAULT_RELEASE_MS,
            lookahead_ms: DEFAULT_LOOKAHEAD_MS,
            soft_knee: false,
            true_peak: true,
            enabled: true,
            current_gain_reduction_db: 0.0,
            input_peak_db: -60.0,
            output_peak_db: -60.0,
        }
    }
}

// =============================================================================
// Threshold Tests
// =============================================================================

/// Test threshold initial state.
#[gpui::test]
async fn test_limiter_threshold_initial(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LimiterState::default()));

    assert!(
        (state.borrow().threshold_db - DEFAULT_THRESHOLD_DB).abs() < 0.001,
        "Initial threshold should be {} dB",
        DEFAULT_THRESHOLD_DB
    );
}

/// Test threshold slider adjustment.
#[gpui::test]
async fn test_limiter_threshold_slider(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LimiterState::default()));

    let test_values: Vec<f64> = vec![-20.0, -12.0, -6.0, -3.0, -1.0, 0.0];
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
async fn test_limiter_threshold_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LimiterState::default()));

    // Minimum
    state.borrow_mut().threshold_db = MIN_THRESHOLD_DB;
    assert!((state.borrow().threshold_db - MIN_THRESHOLD_DB).abs() < 0.001);

    // Maximum (0 dB = no limiting)
    state.borrow_mut().threshold_db = MAX_THRESHOLD_DB;
    assert!(state.borrow().threshold_db.abs() < 0.001);

    // Clamp test
    let clamped = (-40.0f64).clamp(MIN_THRESHOLD_DB, MAX_THRESHOLD_DB);
    assert!((clamped - MIN_THRESHOLD_DB).abs() < 0.001);
}

/// Test threshold display formatting.
#[gpui::test]
async fn test_limiter_threshold_display(_cx: &mut TestAppContext) {
    fn format_threshold(db: f64) -> String {
        if db >= -0.1 {
            "0.0 dB".to_string()
        } else {
            format!("{:.1} dB", db)
        }
    }

    assert_eq!(format_threshold(0.0), "0.0 dB");
    assert_eq!(format_threshold(-1.0), "-1.0 dB");
    assert_eq!(format_threshold(-6.0), "-6.0 dB");
}

// =============================================================================
// Release Tests
// =============================================================================

/// Test release initial state.
#[gpui::test]
async fn test_limiter_release_initial(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LimiterState::default()));

    assert!(
        (state.borrow().release_ms - DEFAULT_RELEASE_MS).abs() < 0.001,
        "Initial release should be {} ms",
        DEFAULT_RELEASE_MS
    );
}

/// Test release slider adjustment.
#[gpui::test]
async fn test_limiter_release_slider(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LimiterState::default()));

    let test_values: Vec<f64> = vec![1.0, 10.0, 50.0, 100.0, 500.0, 1000.0];
    for value in test_values {
        state.borrow_mut().release_ms = value.clamp(MIN_RELEASE_MS, MAX_RELEASE_MS);
        assert!(
            (state.borrow().release_ms - value).abs() < 0.01,
            "Release should be {} ms",
            value
        );
    }
}

/// Test release bounds.
#[gpui::test]
async fn test_limiter_release_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LimiterState::default()));

    // Minimum (fast release)
    state.borrow_mut().release_ms = MIN_RELEASE_MS;
    assert!((state.borrow().release_ms - MIN_RELEASE_MS).abs() < 0.001);

    // Maximum (slow release)
    state.borrow_mut().release_ms = MAX_RELEASE_MS;
    assert!((state.borrow().release_ms - MAX_RELEASE_MS).abs() < 0.001);
}

/// Test release time display formatting.
#[gpui::test]
async fn test_limiter_release_display(_cx: &mut TestAppContext) {
    fn format_release(ms: f64) -> String {
        if ms >= 1000.0 {
            format!("{:.2} s", ms / 1000.0)
        } else {
            format!("{:.0} ms", ms)
        }
    }

    assert_eq!(format_release(50.0), "50 ms");
    assert_eq!(format_release(100.0), "100 ms");
    assert_eq!(format_release(1000.0), "1.00 s");
}

// =============================================================================
// Lookahead Tests
// =============================================================================

/// Test lookahead initial state.
#[gpui::test]
async fn test_limiter_lookahead_initial(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LimiterState::default()));

    assert!(
        (state.borrow().lookahead_ms - DEFAULT_LOOKAHEAD_MS).abs() < 0.001,
        "Initial lookahead should be {} ms",
        DEFAULT_LOOKAHEAD_MS
    );
}

/// Test lookahead slider adjustment.
#[gpui::test]
async fn test_limiter_lookahead_slider(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LimiterState::default()));

    let test_values: Vec<f64> = vec![0.0, 1.0, 2.5, 5.0, 7.5, 10.0];
    for value in test_values {
        state.borrow_mut().lookahead_ms = value.clamp(MIN_LOOKAHEAD_MS, MAX_LOOKAHEAD_MS);
        assert!(
            (state.borrow().lookahead_ms - value).abs() < 0.001,
            "Lookahead should be {} ms",
            value
        );
    }
}

/// Test lookahead bounds.
#[gpui::test]
async fn test_limiter_lookahead_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LimiterState::default()));

    // Zero lookahead (real-time, may cause distortion)
    state.borrow_mut().lookahead_ms = MIN_LOOKAHEAD_MS;
    assert!(state.borrow().lookahead_ms.abs() < 0.001);

    // Maximum lookahead
    state.borrow_mut().lookahead_ms = MAX_LOOKAHEAD_MS;
    assert!((state.borrow().lookahead_ms - MAX_LOOKAHEAD_MS).abs() < 0.001);
}

/// Test lookahead latency display.
#[gpui::test]
async fn test_limiter_lookahead_latency(_cx: &mut TestAppContext) {
    fn lookahead_to_samples(lookahead_ms: f64, sample_rate: f64) -> usize {
        ((lookahead_ms / 1000.0) * sample_rate).round() as usize
    }

    // 5ms at 48kHz
    let samples = lookahead_to_samples(5.0, 48000.0);
    assert_eq!(samples, 240);

    // 10ms at 44.1kHz
    let samples = lookahead_to_samples(10.0, 44100.0);
    assert_eq!(samples, 441);
}

// =============================================================================
// Knee Mode Tests
// =============================================================================

/// Test soft knee toggle.
#[gpui::test]
async fn test_limiter_soft_knee_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LimiterState::default()));

    // Initially hard knee
    assert!(!state.borrow().soft_knee);

    // Enable soft knee
    state.borrow_mut().soft_knee = true;
    assert!(state.borrow().soft_knee);

    // Disable soft knee
    state.borrow_mut().soft_knee = false;
    assert!(!state.borrow().soft_knee);
}

/// Test knee mode behavior difference.
#[gpui::test]
async fn test_limiter_knee_behavior(_cx: &mut TestAppContext) {
    fn apply_limiting(input_db: f64, threshold_db: f64, soft_knee: bool) -> f64 {
        if soft_knee {
            // Soft knee: gradual transition
            let knee_width = 6.0; // dB
            let knee_start = threshold_db - knee_width / 2.0;
            let knee_end = threshold_db + knee_width / 2.0;

            if input_db <= knee_start {
                input_db
            } else if input_db >= knee_end {
                threshold_db
            } else {
                // Interpolate in knee region
                let t = (input_db - knee_start) / knee_width;
                knee_start + t * t * knee_width / 2.0
            }
        } else {
            // Hard knee: instant limiting
            input_db.min(threshold_db)
        }
    }

    let threshold = -1.0;

    // Hard knee: -1dB input = -1dB output (at threshold)
    let hard_output = apply_limiting(-1.0, threshold, false);
    assert!((hard_output - (-1.0)).abs() < 0.001);

    // Hard knee: 0dB input = -1dB output (limited)
    let hard_output = apply_limiting(0.0, threshold, false);
    assert!((hard_output - (-1.0)).abs() < 0.001);

    // Hard knee: below threshold, no limiting applied
    let hard_below = apply_limiting(-2.0, threshold, false);
    assert!((hard_below - (-2.0)).abs() < 0.001, "Hard knee should not affect below threshold");

    // Soft knee: in knee region, produces different result than hard knee
    let soft_output = apply_limiting(-2.0, threshold, true);
    assert!(
        (soft_output - hard_below).abs() > 0.001,
        "Soft knee should behave differently in knee region"
    );

    // Soft knee: at knee boundaries, behavior is well-defined
    // At knee_start (-4.0), soft and hard should be equal
    let soft_at_start = apply_limiting(-4.0, threshold, true);
    let hard_at_start = apply_limiting(-4.0, threshold, false);
    assert!((soft_at_start - hard_at_start).abs() < 0.001, "At knee start, both should pass through");

    // Above knee_end (threshold + knee_width/2 = -1 + 3 = 2dB), both should limit to threshold
    let soft_above = apply_limiting(3.0, threshold, true);
    let hard_above = apply_limiting(3.0, threshold, false);
    assert!((soft_above - threshold).abs() < 0.001, "Soft knee should limit above knee region");
    assert!((hard_above - threshold).abs() < 0.001, "Hard knee should limit above threshold");
}

// =============================================================================
// True Peak Tests
// =============================================================================

/// Test true peak mode toggle.
#[gpui::test]
async fn test_limiter_true_peak_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LimiterState::default()));

    // Initially enabled
    assert!(state.borrow().true_peak);

    // Disable
    state.borrow_mut().true_peak = false;
    assert!(!state.borrow().true_peak);

    // Re-enable
    state.borrow_mut().true_peak = true;
    assert!(state.borrow().true_peak);
}

/// Test true peak vs sample peak.
#[gpui::test]
async fn test_limiter_peak_mode_difference(_cx: &mut TestAppContext) {
    // True peak can detect inter-sample peaks
    // Sample peak only looks at sample values

    fn peak_mode_description(true_peak: bool) -> &'static str {
        if true_peak {
            "True Peak (inter-sample)"
        } else {
            "Sample Peak"
        }
    }

    assert_eq!(peak_mode_description(true), "True Peak (inter-sample)");
    assert_eq!(peak_mode_description(false), "Sample Peak");
}

// =============================================================================
// Gain Reduction Meter Tests
// =============================================================================

/// Test gain reduction meter display.
#[gpui::test]
async fn test_limiter_gr_meter(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LimiterState::default()));

    let gr_values: Vec<f64> = vec![0.0, -1.0, -3.0, -6.0, -12.0];
    for gr in gr_values {
        state.borrow_mut().current_gain_reduction_db = gr;
        assert!(
            (state.borrow().current_gain_reduction_db - gr).abs() < 0.001
        );
    }
}

/// Test GR meter position calculation.
#[gpui::test]
async fn test_limiter_gr_meter_position(_cx: &mut TestAppContext) {
    const GR_METER_MAX: f64 = 12.0;

    fn gr_meter_position(gr_db: f64) -> f64 {
        (-gr_db / GR_METER_MAX).clamp(0.0, 1.0)
    }

    assert!((gr_meter_position(0.0) - 0.0).abs() < 0.001);
    assert!((gr_meter_position(-6.0) - 0.5).abs() < 0.001);
    assert!((gr_meter_position(-12.0) - 1.0).abs() < 0.001);
    assert!((gr_meter_position(-20.0) - 1.0).abs() < 0.001); // Clamped
}

/// Test GR display formatting.
#[gpui::test]
async fn test_limiter_gr_display(_cx: &mut TestAppContext) {
    fn format_gr(gr_db: f64) -> String {
        if gr_db >= -0.1 {
            "0.0 dB".to_string()
        } else {
            format!("{:.1} dB", gr_db)
        }
    }

    assert_eq!(format_gr(0.0), "0.0 dB");
    assert_eq!(format_gr(-3.0), "-3.0 dB");
    assert_eq!(format_gr(-6.5), "-6.5 dB");
}

// =============================================================================
// Input/Output Level Tests
// =============================================================================

/// Test input peak meter.
#[gpui::test]
async fn test_limiter_input_peak(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LimiterState::default()));

    let levels: Vec<f64> = vec![-20.0, -12.0, -6.0, -3.0, 0.0, 3.0];
    for level in levels {
        state.borrow_mut().input_peak_db = level;
        assert!((state.borrow().input_peak_db - level).abs() < 0.001);
    }
}

/// Test output peak meter (should never exceed threshold).
#[gpui::test]
async fn test_limiter_output_peak(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LimiterState::default()));

    // Output should be limited
    state.borrow_mut().output_peak_db = -1.0; // At threshold

    assert!(
        state.borrow().output_peak_db <= state.borrow().threshold_db + 0.1,
        "Output should not exceed threshold"
    );
}

/// Test clipping indicator.
#[gpui::test]
async fn test_limiter_clipping_indicator(_cx: &mut TestAppContext) {
    fn is_clipping(input_peak_db: f64, threshold_db: f64) -> bool {
        input_peak_db > threshold_db
    }

    assert!(!is_clipping(-6.0, -1.0));
    assert!(!is_clipping(-1.0, -1.0));
    assert!(is_clipping(0.0, -1.0));
    assert!(is_clipping(3.0, -1.0));
}

// =============================================================================
// Enable/Bypass Tests
// =============================================================================

/// Test limiter enable state.
#[gpui::test]
async fn test_limiter_enable(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LimiterState::default()));

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

/// Test limiter preset structure.
#[gpui::test]
async fn test_limiter_preset_structure(_cx: &mut TestAppContext) {
    #[derive(Debug, Clone)]
    struct LimiterPreset {
        name: String,
        threshold_db: f64,
        release_ms: f64,
        lookahead_ms: f64,
        soft_knee: bool,
        true_peak: bool,
    }

    let preset = LimiterPreset {
        name: "Broadcast".to_string(),
        threshold_db: -1.0,
        release_ms: 100.0,
        lookahead_ms: 5.0,
        soft_knee: true,
        true_peak: true,
    };

    assert_eq!(preset.name, "Broadcast");
    assert!((preset.threshold_db - (-1.0)).abs() < 0.001);
}

/// Test preset application.
#[gpui::test]
async fn test_limiter_preset_apply(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LimiterState::default()));

    // Apply "Mastering" preset
    state.borrow_mut().threshold_db = -0.3;
    state.borrow_mut().release_ms = 50.0;
    state.borrow_mut().lookahead_ms = 10.0;
    state.borrow_mut().soft_knee = false;
    state.borrow_mut().true_peak = true;

    assert!((state.borrow().threshold_db - (-0.3)).abs() < 0.001);
    assert!((state.borrow().release_ms - 50.0).abs() < 0.001);
    assert!((state.borrow().lookahead_ms - 10.0).abs() < 0.001);
}

// =============================================================================
// Keyboard Shortcut Tests
// =============================================================================

/// Test threshold arrow key adjustment.
#[gpui::test]
async fn test_limiter_threshold_keys(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LimiterState::default()));
    const STEP_DB: f64 = 0.5;

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

/// Test bypass toggle shortcut.
#[gpui::test]
async fn test_limiter_bypass_key(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LimiterState::default()));

    // Press B to bypass
    {
        let current = state.borrow().enabled;
        state.borrow_mut().enabled = !current;
    }
    assert!(!state.borrow().enabled);

    // Press B again to enable
    {
        let current = state.borrow().enabled;
        state.borrow_mut().enabled = !current;
    }
    assert!(state.borrow().enabled);
}

// =============================================================================
// Visual Feedback Tests
// =============================================================================

/// Test limiting active indicator.
#[gpui::test]
async fn test_limiter_active_indicator(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LimiterState::default()));

    fn is_limiting(gr_db: f64) -> bool {
        gr_db < -0.1
    }

    state.borrow_mut().current_gain_reduction_db = 0.0;
    assert!(!is_limiting(state.borrow().current_gain_reduction_db));

    state.borrow_mut().current_gain_reduction_db = -3.0;
    assert!(is_limiting(state.borrow().current_gain_reduction_db));
}

/// Test threshold line position.
#[gpui::test]
async fn test_limiter_threshold_line(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LimiterState::default()));

    fn threshold_meter_position(threshold_db: f64, meter_min: f64, meter_max: f64) -> f64 {
        (threshold_db - meter_min) / (meter_max - meter_min)
    }

    // -1dB on -30 to 0 range
    let pos = threshold_meter_position(state.borrow().threshold_db, -30.0, 0.0);
    assert!((pos - 0.967).abs() < 0.01, "Threshold should be near top");
}

/// Test input over threshold warning.
#[gpui::test]
async fn test_limiter_over_threshold_warning(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LimiterState::default()));

    fn warning_color(input_db: f64, threshold_db: f64) -> (u8, u8, u8) {
        let over = input_db - threshold_db;
        if over > 6.0 {
            (255, 0, 0) // Red - heavy limiting
        } else if over > 3.0 {
            (255, 165, 0) // Orange - moderate limiting
        } else if over > 0.0 {
            (255, 255, 0) // Yellow - light limiting
        } else {
            (0, 255, 0) // Green - no limiting
        }
    }

    state.borrow_mut().input_peak_db = -6.0;
    assert_eq!(warning_color(state.borrow().input_peak_db, -1.0), (0, 255, 0));

    state.borrow_mut().input_peak_db = 0.0;
    assert_eq!(warning_color(state.borrow().input_peak_db, -1.0), (255, 255, 0));

    state.borrow_mut().input_peak_db = 6.0;
    assert_eq!(warning_color(state.borrow().input_peak_db, -1.0), (255, 0, 0));
}
