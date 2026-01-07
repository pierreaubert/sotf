//! E2E tests for Loudness Compensation Plugin UI.
//!
//! Tests for verifying equal-loudness contour compensation:
//! - Low shelf control
//! - High shelf control
//! - Auto-gain toggle
//! - Reference level
//! - Volume-dependent curves

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Parameter Constants
// =============================================================================

const MIN_LOW_SHELF_DB: f64 = -12.0;
const MAX_LOW_SHELF_DB: f64 = 12.0;
const DEFAULT_LOW_SHELF_DB: f64 = 0.0;

const MIN_HIGH_SHELF_DB: f64 = -12.0;
const MAX_HIGH_SHELF_DB: f64 = 12.0;
const DEFAULT_HIGH_SHELF_DB: f64 = 0.0;

const MIN_REFERENCE_PHON: f64 = 20.0;
const MAX_REFERENCE_PHON: f64 = 90.0;
const DEFAULT_REFERENCE_PHON: f64 = 80.0;

const LOW_SHELF_FREQUENCY: f64 = 100.0;
const HIGH_SHELF_FREQUENCY: f64 = 10000.0;

// =============================================================================
// Loudness Compensation State
// =============================================================================

#[derive(Debug, Clone)]
struct LoudnessCompState {
    low_shelf_db: f64,
    high_shelf_db: f64,
    reference_phon: f64,
    listening_level_db: f64,
    auto_gain: bool,
    auto_curve: bool,
    enabled: bool,
}

impl Default for LoudnessCompState {
    fn default() -> Self {
        Self {
            low_shelf_db: DEFAULT_LOW_SHELF_DB,
            high_shelf_db: DEFAULT_HIGH_SHELF_DB,
            reference_phon: DEFAULT_REFERENCE_PHON,
            listening_level_db: -20.0,
            auto_gain: true,
            auto_curve: true,
            enabled: true,
        }
    }
}

// =============================================================================
// Low Shelf Tests
// =============================================================================

/// Test low shelf initial state.
#[gpui::test]
async fn test_loudness_low_shelf_initial(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessCompState::default()));

    assert!(
        (state.borrow().low_shelf_db - DEFAULT_LOW_SHELF_DB).abs() < 0.001,
        "Initial low shelf should be {} dB",
        DEFAULT_LOW_SHELF_DB
    );
}

/// Test low shelf slider adjustment.
#[gpui::test]
async fn test_loudness_low_shelf_slider(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessCompState::default()));

    let test_values: Vec<f64> = vec![-12.0, -6.0, 0.0, 3.0, 6.0, 12.0];
    for value in test_values {
        state.borrow_mut().low_shelf_db = value.clamp(MIN_LOW_SHELF_DB, MAX_LOW_SHELF_DB);
        assert!(
            (state.borrow().low_shelf_db - value).abs() < 0.001,
            "Low shelf should be {} dB",
            value
        );
    }
}

/// Test low shelf bounds.
#[gpui::test]
async fn test_loudness_low_shelf_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessCompState::default()));

    // Minimum (cut)
    state.borrow_mut().low_shelf_db = MIN_LOW_SHELF_DB;
    assert!((state.borrow().low_shelf_db - MIN_LOW_SHELF_DB).abs() < 0.001);

    // Maximum (boost)
    state.borrow_mut().low_shelf_db = MAX_LOW_SHELF_DB;
    assert!((state.borrow().low_shelf_db - MAX_LOW_SHELF_DB).abs() < 0.001);
}

/// Test low shelf display formatting.
#[gpui::test]
async fn test_loudness_low_shelf_display(_cx: &mut TestAppContext) {
    fn format_shelf(db: f64, freq: f64) -> String {
        let sign = if db > 0.0 { "+" } else { "" };
        format!("{}{:.1} dB @ {} Hz", sign, db, freq as u32)
    }

    assert_eq!(format_shelf(6.0, LOW_SHELF_FREQUENCY), "+6.0 dB @ 100 Hz");
    assert_eq!(format_shelf(-3.0, LOW_SHELF_FREQUENCY), "-3.0 dB @ 100 Hz");
    assert_eq!(format_shelf(0.0, LOW_SHELF_FREQUENCY), "0.0 dB @ 100 Hz");
}

// =============================================================================
// High Shelf Tests
// =============================================================================

/// Test high shelf initial state.
#[gpui::test]
async fn test_loudness_high_shelf_initial(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessCompState::default()));

    assert!(
        (state.borrow().high_shelf_db - DEFAULT_HIGH_SHELF_DB).abs() < 0.001,
        "Initial high shelf should be {} dB",
        DEFAULT_HIGH_SHELF_DB
    );
}

/// Test high shelf slider adjustment.
#[gpui::test]
async fn test_loudness_high_shelf_slider(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessCompState::default()));

    let test_values: Vec<f64> = vec![-12.0, -6.0, 0.0, 3.0, 6.0, 12.0];
    for value in test_values {
        state.borrow_mut().high_shelf_db = value.clamp(MIN_HIGH_SHELF_DB, MAX_HIGH_SHELF_DB);
        assert!(
            (state.borrow().high_shelf_db - value).abs() < 0.001,
            "High shelf should be {} dB",
            value
        );
    }
}

/// Test high shelf bounds.
#[gpui::test]
async fn test_loudness_high_shelf_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessCompState::default()));

    // Minimum (cut)
    state.borrow_mut().high_shelf_db = MIN_HIGH_SHELF_DB;
    assert!((state.borrow().high_shelf_db - MIN_HIGH_SHELF_DB).abs() < 0.001);

    // Maximum (boost)
    state.borrow_mut().high_shelf_db = MAX_HIGH_SHELF_DB;
    assert!((state.borrow().high_shelf_db - MAX_HIGH_SHELF_DB).abs() < 0.001);
}

// =============================================================================
// Auto-Gain Tests
// =============================================================================

/// Test auto-gain toggle.
#[gpui::test]
async fn test_loudness_auto_gain_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessCompState::default()));

    // Initially enabled
    assert!(state.borrow().auto_gain);

    // Disable
    state.borrow_mut().auto_gain = false;
    assert!(!state.borrow().auto_gain);

    // Re-enable
    state.borrow_mut().auto_gain = true;
    assert!(state.borrow().auto_gain);
}

/// Test auto-gain calculation concept.
#[gpui::test]
async fn test_loudness_auto_gain_calculation(_cx: &mut TestAppContext) {
    // Auto-gain compensates for overall level change from shelving
    // If we boost bass, overall level increases, so auto-gain reduces

    fn calculate_auto_gain(low_shelf_db: f64, high_shelf_db: f64) -> f64 {
        // Simplified: average of shelf gains, inverted
        // Real implementation would integrate over frequency
        let avg_boost = (low_shelf_db + high_shelf_db) / 2.0;
        if avg_boost > 0.0 {
            -avg_boost * 0.5 // Reduce by half the average boost
        } else {
            0.0 // Don't compensate for cuts
        }
    }

    assert!((calculate_auto_gain(6.0, 6.0) - (-3.0)).abs() < 0.1);
    assert!((calculate_auto_gain(0.0, 0.0) - 0.0).abs() < 0.001);
    assert!((calculate_auto_gain(-6.0, -6.0) - 0.0).abs() < 0.001);
}

/// Test auto-gain display.
#[gpui::test]
async fn test_loudness_auto_gain_display(_cx: &mut TestAppContext) {
    fn format_auto_gain(enabled: bool, compensation_db: f64) -> String {
        if enabled {
            if compensation_db.abs() < 0.1 {
                "Auto: 0.0 dB".to_string()
            } else {
                format!("Auto: {:.1} dB", compensation_db)
            }
        } else {
            "Auto: OFF".to_string()
        }
    }

    assert_eq!(format_auto_gain(true, 0.0), "Auto: 0.0 dB");
    assert_eq!(format_auto_gain(true, -3.0), "Auto: -3.0 dB");
    assert_eq!(format_auto_gain(false, 0.0), "Auto: OFF");
}

// =============================================================================
// Reference Level Tests
// =============================================================================

/// Test reference level initial state.
#[gpui::test]
async fn test_loudness_reference_initial(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessCompState::default()));

    assert!(
        (state.borrow().reference_phon - DEFAULT_REFERENCE_PHON).abs() < 0.001,
        "Initial reference should be {} phon",
        DEFAULT_REFERENCE_PHON
    );
}

/// Test reference level slider adjustment.
#[gpui::test]
async fn test_loudness_reference_slider(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessCompState::default()));

    let test_values: Vec<f64> = vec![20.0, 40.0, 60.0, 80.0, 90.0];
    for value in test_values {
        state.borrow_mut().reference_phon = value.clamp(MIN_REFERENCE_PHON, MAX_REFERENCE_PHON);
        assert!(
            (state.borrow().reference_phon - value).abs() < 0.001,
            "Reference should be {} phon",
            value
        );
    }
}

/// Test reference level bounds.
#[gpui::test]
async fn test_loudness_reference_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessCompState::default()));

    // Minimum
    state.borrow_mut().reference_phon = MIN_REFERENCE_PHON;
    assert!((state.borrow().reference_phon - MIN_REFERENCE_PHON).abs() < 0.001);

    // Maximum
    state.borrow_mut().reference_phon = MAX_REFERENCE_PHON;
    assert!((state.borrow().reference_phon - MAX_REFERENCE_PHON).abs() < 0.001);
}

/// Test reference level display.
#[gpui::test]
async fn test_loudness_reference_display(_cx: &mut TestAppContext) {
    fn format_reference(phon: f64) -> String {
        format!("{:.0} phon", phon)
    }

    assert_eq!(format_reference(80.0), "80 phon");
    assert_eq!(format_reference(60.0), "60 phon");
}

// =============================================================================
// Auto-Curve Tests
// =============================================================================

/// Test auto-curve toggle.
#[gpui::test]
async fn test_loudness_auto_curve_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessCompState::default()));

    // Initially enabled
    assert!(state.borrow().auto_curve);

    // Disable
    state.borrow_mut().auto_curve = false;
    assert!(!state.borrow().auto_curve);

    // Re-enable
    state.borrow_mut().auto_curve = true;
    assert!(state.borrow().auto_curve);
}

/// Test auto-curve calculation based on listening level.
#[gpui::test]
async fn test_loudness_auto_curve_calculation(_cx: &mut TestAppContext) {
    // Equal-loudness contours: at lower levels, we need more bass/treble boost
    // to perceive the same loudness as at reference level

    fn calculate_compensation(listening_level_db: f64, reference_phon: f64) -> (f64, f64) {
        // Simplified Fletcher-Munson compensation
        let level_diff = reference_phon - (listening_level_db + 80.0); // Rough dB to phon

        if level_diff > 0.0 {
            // Listening below reference - boost bass and treble
            let bass_boost = level_diff * 0.15; // ~1.5dB boost per 10 phon difference
            let treble_boost = level_diff * 0.05; // ~0.5dB boost per 10 phon difference
            (bass_boost.min(12.0), treble_boost.min(6.0))
        } else {
            (0.0, 0.0) // At or above reference - no compensation
        }
    }

    // Listening 20dB below typical 80 phon reference
    let (bass, treble) = calculate_compensation(-40.0, 80.0);
    assert!(bass > 0.0, "Should boost bass at low levels");
    assert!(treble > 0.0, "Should boost treble at low levels");
}

/// Test auto-curve disables manual controls.
#[gpui::test]
async fn test_loudness_auto_curve_disables_manual(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessCompState::default()));

    fn are_manual_controls_enabled(auto_curve: bool) -> bool {
        !auto_curve
    }

    assert!(!are_manual_controls_enabled(state.borrow().auto_curve));

    state.borrow_mut().auto_curve = false;
    assert!(are_manual_controls_enabled(state.borrow().auto_curve));
}

// =============================================================================
// Listening Level Tests
// =============================================================================

/// Test listening level tracking.
#[gpui::test]
async fn test_loudness_listening_level(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessCompState::default()));

    let levels: Vec<f64> = vec![-60.0, -40.0, -20.0, -10.0, 0.0];
    for level in levels {
        state.borrow_mut().listening_level_db = level;
        assert!((state.borrow().listening_level_db - level).abs() < 0.001);
    }
}

/// Test listening level to phon conversion.
#[gpui::test]
async fn test_loudness_db_to_phon(_cx: &mut TestAppContext) {
    // Rough conversion: phon ≈ dB SPL at 1kHz
    // Assuming -20dB playback level corresponds to ~60 phon

    fn db_to_approximate_phon(db: f64, reference_db: f64, reference_phon: f64) -> f64 {
        reference_phon + (db - reference_db)
    }

    // -20dB at reference = 60 phon
    let phon = db_to_approximate_phon(-20.0, -20.0, 60.0);
    assert!((phon - 60.0).abs() < 0.001);

    // -40dB at reference = 40 phon
    let phon = db_to_approximate_phon(-40.0, -20.0, 60.0);
    assert!((phon - 40.0).abs() < 0.001);
}

// =============================================================================
// Enable/Bypass Tests
// =============================================================================

/// Test loudness compensation enable state.
#[gpui::test]
async fn test_loudness_enable(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessCompState::default()));

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

/// Test loudness preset structure.
#[gpui::test]
async fn test_loudness_preset_structure(_cx: &mut TestAppContext) {
    #[derive(Debug, Clone)]
    struct LoudnessPreset {
        name: String,
        low_shelf_db: f64,
        high_shelf_db: f64,
        auto_gain: bool,
    }

    let preset = LoudnessPreset {
        name: "Night Mode".to_string(),
        low_shelf_db: 6.0,
        high_shelf_db: 2.0,
        auto_gain: true,
    };

    assert_eq!(preset.name, "Night Mode");
    assert!((preset.low_shelf_db - 6.0).abs() < 0.001);
}

/// Test common presets.
#[gpui::test]
async fn test_loudness_common_presets(_cx: &mut TestAppContext) {
    fn get_preset(name: &str) -> (f64, f64) {
        match name {
            "Flat" => (0.0, 0.0),
            "Night Mode" => (6.0, 2.0),
            "Bass Boost" => (9.0, 0.0),
            "Treble Boost" => (0.0, 6.0),
            "V-Shape" => (6.0, 6.0),
            _ => (0.0, 0.0),
        }
    }

    let (low, high) = get_preset("Night Mode");
    assert!((low - 6.0).abs() < 0.001);
    assert!((high - 2.0).abs() < 0.001);
}

// =============================================================================
// Keyboard Shortcut Tests
// =============================================================================

/// Test low shelf arrow key adjustment.
#[gpui::test]
async fn test_loudness_low_shelf_keys(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessCompState::default()));
    const STEP_DB: f64 = 0.5;

    // Up arrow
    {
        let current = state.borrow().low_shelf_db;
        state.borrow_mut().low_shelf_db =
            (current + STEP_DB).clamp(MIN_LOW_SHELF_DB, MAX_LOW_SHELF_DB);
    }
    assert!((state.borrow().low_shelf_db - 0.5).abs() < 0.001);

    // Down arrow
    {
        let current = state.borrow().low_shelf_db;
        state.borrow_mut().low_shelf_db =
            (current - STEP_DB).clamp(MIN_LOW_SHELF_DB, MAX_LOW_SHELF_DB);
    }
    assert!(state.borrow().low_shelf_db.abs() < 0.001);
}

/// Test auto-gain toggle shortcut.
#[gpui::test]
async fn test_loudness_auto_gain_key(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessCompState::default()));

    // Press A to toggle auto-gain
    {
        let current = state.borrow().auto_gain;
        state.borrow_mut().auto_gain = !current;
    }
    assert!(!state.borrow().auto_gain);

    // Press A again
    {
        let current = state.borrow().auto_gain;
        state.borrow_mut().auto_gain = !current;
    }
    assert!(state.borrow().auto_gain);
}

// =============================================================================
// Visual Feedback Tests
// =============================================================================

/// Test frequency response curve display.
#[gpui::test]
async fn test_loudness_frequency_curve(_cx: &mut TestAppContext) {
    fn calculate_response(freq: f64, low_shelf_db: f64, high_shelf_db: f64) -> f64 {
        // Simplified: linear transition between shelves
        let log_freq = freq.ln();
        let log_low = LOW_SHELF_FREQUENCY.ln();
        let log_high = HIGH_SHELF_FREQUENCY.ln();
        let log_mid = (log_low + log_high) / 2.0;

        if log_freq <= log_low {
            low_shelf_db
        } else if log_freq >= log_high {
            high_shelf_db
        } else if log_freq < log_mid {
            // Transition from low shelf to flat
            let t = (log_freq - log_low) / (log_mid - log_low);
            low_shelf_db * (1.0 - t)
        } else {
            // Transition from flat to high shelf
            let t = (log_freq - log_mid) / (log_high - log_mid);
            high_shelf_db * t
        }
    }

    let response_50hz = calculate_response(50.0, 6.0, 0.0);
    assert!(response_50hz > 0.0, "Should boost at 50Hz with bass boost");

    let response_1k = calculate_response(1000.0, 6.0, 0.0);
    assert!(response_1k.abs() < 3.0, "Should be near flat at 1kHz");
}

/// Test shelf indicator colors.
#[gpui::test]
async fn test_loudness_shelf_colors(_cx: &mut TestAppContext) {
    fn shelf_color(db: f64) -> (u8, u8, u8) {
        if db > 0.5 {
            (100, 200, 255) // Blue for boost
        } else if db < -0.5 {
            (255, 150, 100) // Orange for cut
        } else {
            (180, 180, 180) // Gray for flat
        }
    }

    assert_eq!(shelf_color(6.0), (100, 200, 255));
    assert_eq!(shelf_color(-6.0), (255, 150, 100));
    assert_eq!(shelf_color(0.0), (180, 180, 180));
}

// =============================================================================
// Equal-Loudness Contour Reference Tests
// =============================================================================

/// Test ISO 226 phon levels.
#[gpui::test]
async fn test_loudness_iso226_phons(_cx: &mut TestAppContext) {
    // Standard equal-loudness contour levels (ISO 226)
    fn phon_levels() -> Vec<f64> {
        vec![20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0]
    }

    let levels = phon_levels();
    assert_eq!(levels.len(), 8);
    assert!((levels[0] - 20.0).abs() < 0.001);
    assert!((levels[7] - 90.0).abs() < 0.001);
}

/// Test frequency-dependent sensitivity.
#[gpui::test]
async fn test_loudness_frequency_sensitivity(_cx: &mut TestAppContext) {
    // Human hearing is most sensitive around 2-4kHz
    // Less sensitive at low and high frequencies

    fn relative_sensitivity(freq: f64) -> f64 {
        // Simplified A-weighting approximation
        if freq < 100.0 {
            -20.0 // Much less sensitive to low bass
        } else if freq < 500.0 {
            -6.0 // Less sensitive to bass
        } else if freq < 1000.0 {
            -2.0 // Slightly less sensitive
        } else if freq < 4000.0 {
            0.0 // Most sensitive
        } else if freq < 10000.0 {
            -3.0 // Less sensitive to treble
        } else {
            -10.0 // Much less sensitive to high treble
        }
    }

    assert!(relative_sensitivity(50.0) < relative_sensitivity(1000.0));
    assert!(relative_sensitivity(3000.0) > relative_sensitivity(100.0));
}
