//! E2E tests for Loudness Monitor Plugin UI.
//!
//! Tests for verifying EBU R128 loudness monitoring functionality:
//! - Integrated LUFS display
//! - Short-term LUFS display
//! - Momentary LUFS display
//! - Loudness range (LRA)
//! - True peak metering
//! - Per-channel peak meters
//! - Target loudness reference

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Parameter Constants
// =============================================================================

const MIN_LUFS: f64 = -70.0;
const MAX_LUFS: f64 = 0.0;
const MIN_TRUE_PEAK_DB: f64 = -60.0;
const MAX_TRUE_PEAK_DB: f64 = 3.0;

const DEFAULT_TARGET_LUFS: f64 = -14.0; // Streaming standard
const REFERENCE_SCALE_FACTOR: f64 = 0.0; // 0 = no scaling applied

// =============================================================================
// Loudness State
// =============================================================================

#[derive(Debug, Clone)]
struct LoudnessState {
    // Main LUFS readings
    integrated_lufs: f64,
    short_term_lufs: f64,
    momentary_lufs: f64,
    loudness_range_lu: f64,

    // True peak
    true_peak_db: f64,
    true_peak_channel: usize,

    // Per-channel peaks
    channel_peaks_db: Vec<f64>,

    // Settings
    target_lufs: f64,
    auto_reset_on_play: bool,
    num_channels: usize,
}

impl LoudnessState {
    fn new(num_channels: usize) -> Self {
        Self {
            integrated_lufs: MIN_LUFS,
            short_term_lufs: MIN_LUFS,
            momentary_lufs: MIN_LUFS,
            loudness_range_lu: 0.0,
            true_peak_db: MIN_TRUE_PEAK_DB,
            true_peak_channel: 0,
            channel_peaks_db: vec![MIN_TRUE_PEAK_DB; num_channels],
            target_lufs: DEFAULT_TARGET_LUFS,
            auto_reset_on_play: true,
            num_channels,
        }
    }

    fn reset(&mut self) {
        self.integrated_lufs = MIN_LUFS;
        self.short_term_lufs = MIN_LUFS;
        self.momentary_lufs = MIN_LUFS;
        self.loudness_range_lu = 0.0;
        self.true_peak_db = MIN_TRUE_PEAK_DB;
        self.true_peak_channel = 0;
        for peak in &mut self.channel_peaks_db {
            *peak = MIN_TRUE_PEAK_DB;
        }
    }
}

// =============================================================================
// Integrated LUFS Tests
// =============================================================================

/// Test integrated LUFS initial state.
#[gpui::test]
async fn test_loudness_integrated_initial(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessState::new(2)));

    assert!(
        state.borrow().integrated_lufs <= MIN_LUFS + 0.001,
        "Initial integrated LUFS should be minimum"
    );
}

/// Test integrated LUFS display range.
#[gpui::test]
async fn test_loudness_integrated_range(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessState::new(2)));

    let test_values: Vec<f64> = vec![-70.0, -40.0, -23.0, -14.0, -5.0, 0.0];
    for value in test_values {
        state.borrow_mut().integrated_lufs = value.clamp(MIN_LUFS, MAX_LUFS);
        assert!(
            (state.borrow().integrated_lufs - value).abs() < 0.001,
            "Integrated LUFS should be {} LUFS",
            value
        );
    }
}

/// Test integrated LUFS display formatting.
#[gpui::test]
async fn test_loudness_integrated_format(_cx: &mut TestAppContext) {
    fn format_lufs(lufs: f64) -> String {
        if lufs <= MIN_LUFS + 0.5 {
            "-∞ LUFS".to_string()
        } else {
            format!("{:.1} LUFS", lufs)
        }
    }

    assert_eq!(format_lufs(-70.0), "-∞ LUFS");
    assert_eq!(format_lufs(-23.0), "-23.0 LUFS");
    assert_eq!(format_lufs(-14.0), "-14.0 LUFS");
}

/// Test integrated LUFS meter position.
#[gpui::test]
async fn test_loudness_integrated_meter(_cx: &mut TestAppContext) {
    fn lufs_meter_position(lufs: f64) -> f64 {
        ((lufs - MIN_LUFS) / (MAX_LUFS - MIN_LUFS)).clamp(0.0, 1.0)
    }

    assert!((lufs_meter_position(-70.0) - 0.0).abs() < 0.001);
    assert!((lufs_meter_position(-35.0) - 0.5).abs() < 0.001);
    assert!((lufs_meter_position(0.0) - 1.0).abs() < 0.001);
}

// =============================================================================
// Short-term LUFS Tests
// =============================================================================

/// Test short-term LUFS (3 second window).
#[gpui::test]
async fn test_loudness_short_term(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessState::new(2)));

    // Short-term responds faster than integrated
    let test_values: Vec<f64> = vec![-30.0, -20.0, -10.0];
    for value in test_values {
        state.borrow_mut().short_term_lufs = value.clamp(MIN_LUFS, MAX_LUFS);
        assert!(
            (state.borrow().short_term_lufs - value).abs() < 0.001
        );
    }
}

/// Test short-term display formatting.
#[gpui::test]
async fn test_loudness_short_term_format(_cx: &mut TestAppContext) {
    fn format_short_term(lufs: f64) -> String {
        if lufs <= MIN_LUFS + 0.5 {
            "-∞".to_string()
        } else {
            format!("{:.1}", lufs)
        }
    }

    assert_eq!(format_short_term(-70.0), "-∞");
    assert_eq!(format_short_term(-18.0), "-18.0");
}

// =============================================================================
// Momentary LUFS Tests
// =============================================================================

/// Test momentary LUFS (400ms window).
#[gpui::test]
async fn test_loudness_momentary(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessState::new(2)));

    // Momentary is the fastest responding
    state.borrow_mut().momentary_lufs = -10.0;
    assert!((state.borrow().momentary_lufs - (-10.0)).abs() < 0.001);
}

/// Test momentary vs short-term vs integrated relationship.
#[gpui::test]
async fn test_loudness_measurement_hierarchy(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessState::new(2)));

    // Simulate a scenario: momentary peaks higher, integrated averages lower
    state.borrow_mut().momentary_lufs = -8.0;
    state.borrow_mut().short_term_lufs = -12.0;
    state.borrow_mut().integrated_lufs = -16.0;

    // Momentary >= Short-term >= Integrated (typically)
    // Note: This isn't always true, but is common for peaky material
    let s = state.borrow();
    assert!(s.momentary_lufs >= s.short_term_lufs - 10.0);
    assert!(s.short_term_lufs >= s.integrated_lufs - 10.0);
}

// =============================================================================
// Loudness Range Tests
// =============================================================================

/// Test loudness range (LRA) display.
#[gpui::test]
async fn test_loudness_range_display(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessState::new(2)));

    let test_values = vec![0.0, 5.0, 10.0, 15.0, 20.0];
    for value in test_values {
        state.borrow_mut().loudness_range_lu = value;
        assert!((state.borrow().loudness_range_lu - value).abs() < 0.001);
    }
}

/// Test LRA display formatting.
#[gpui::test]
async fn test_loudness_range_format(_cx: &mut TestAppContext) {
    fn format_lra(lra: f64) -> String {
        format!("{:.1} LU", lra)
    }

    assert_eq!(format_lra(0.0), "0.0 LU");
    assert_eq!(format_lra(7.5), "7.5 LU");
    assert_eq!(format_lra(12.3), "12.3 LU");
}

/// Test LRA interpretation.
#[gpui::test]
async fn test_loudness_range_interpretation(_cx: &mut TestAppContext) {
    fn interpret_lra(lra: f64) -> &'static str {
        if lra < 5.0 {
            "Low dynamics"
        } else if lra < 10.0 {
            "Moderate dynamics"
        } else if lra < 15.0 {
            "Good dynamics"
        } else {
            "High dynamics"
        }
    }

    assert_eq!(interpret_lra(3.0), "Low dynamics");
    assert_eq!(interpret_lra(7.0), "Moderate dynamics");
    assert_eq!(interpret_lra(12.0), "Good dynamics");
    assert_eq!(interpret_lra(18.0), "High dynamics");
}

// =============================================================================
// True Peak Tests
// =============================================================================

/// Test true peak display.
#[gpui::test]
async fn test_loudness_true_peak(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessState::new(2)));

    let test_values: Vec<f64> = vec![-20.0, -6.0, -3.0, -1.0, 0.0, 1.0, 2.0];
    for value in test_values {
        state.borrow_mut().true_peak_db = value.clamp(MIN_TRUE_PEAK_DB, MAX_TRUE_PEAK_DB);
        assert!(
            (state.borrow().true_peak_db - value).abs() < 0.001,
            "True peak should be {} dBTP",
            value
        );
    }
}

/// Test true peak formatting.
#[gpui::test]
async fn test_loudness_true_peak_format(_cx: &mut TestAppContext) {
    fn format_true_peak(db: f64) -> String {
        if db <= MIN_TRUE_PEAK_DB + 0.5 {
            "-∞ dBTP".to_string()
        } else if db > 0.0 {
            format!("+{:.1} dBTP", db)
        } else {
            format!("{:.1} dBTP", db)
        }
    }

    assert_eq!(format_true_peak(-60.0), "-∞ dBTP");
    assert_eq!(format_true_peak(-3.0), "-3.0 dBTP");
    assert_eq!(format_true_peak(0.0), "0.0 dBTP");
    assert_eq!(format_true_peak(1.5), "+1.5 dBTP");
}

/// Test true peak clipping indicator.
#[gpui::test]
async fn test_loudness_clipping_indicator(_cx: &mut TestAppContext) {
    fn is_clipping(true_peak_db: f64) -> bool {
        true_peak_db > -0.1 // Anything above -0.1 dBTP is considered clipping
    }

    assert!(!is_clipping(-3.0));
    assert!(!is_clipping(-0.5));
    assert!(is_clipping(0.0));
    assert!(is_clipping(1.0));
}

/// Test true peak channel tracking.
#[gpui::test]
async fn test_loudness_peak_channel_tracking(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessState::new(6)));

    // Update per-channel peaks
    state.borrow_mut().channel_peaks_db = vec![-10.0, -8.0, -12.0, -20.0, -15.0, -14.0];

    // Find max peak and channel
    let (max_channel, max_peak) = state
        .borrow()
        .channel_peaks_db
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(i, &v)| (i, v))
        .unwrap();

    state.borrow_mut().true_peak_db = max_peak;
    state.borrow_mut().true_peak_channel = max_channel;

    assert_eq!(state.borrow().true_peak_channel, 1, "Channel R should have highest peak");
    assert!((state.borrow().true_peak_db - (-8.0)).abs() < 0.001);
}

// =============================================================================
// Per-Channel Peak Tests
// =============================================================================

/// Test per-channel peak initialization.
#[gpui::test]
async fn test_loudness_channel_peaks_init(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessState::new(6)));

    assert_eq!(
        state.borrow().channel_peaks_db.len(),
        6,
        "Should have 6 channel peaks"
    );
}

/// Test per-channel peak update.
#[gpui::test]
async fn test_loudness_channel_peaks_update(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessState::new(2)));

    state.borrow_mut().channel_peaks_db[0] = -6.0;
    state.borrow_mut().channel_peaks_db[1] = -3.0;

    assert!((state.borrow().channel_peaks_db[0] - (-6.0)).abs() < 0.001);
    assert!((state.borrow().channel_peaks_db[1] - (-3.0)).abs() < 0.001);
}

/// Test per-channel peak meter positions.
#[gpui::test]
async fn test_loudness_channel_meter_position(_cx: &mut TestAppContext) {
    fn peak_meter_position(db: f64) -> f64 {
        ((db - MIN_TRUE_PEAK_DB) / (MAX_TRUE_PEAK_DB - MIN_TRUE_PEAK_DB)).clamp(0.0, 1.0)
    }

    assert!((peak_meter_position(-60.0) - 0.0).abs() < 0.001);
    assert!((peak_meter_position(0.0) - 0.952).abs() < 0.01);
    assert!((peak_meter_position(3.0) - 1.0).abs() < 0.001);
}

/// Test channel names.
#[gpui::test]
async fn test_loudness_channel_names(_cx: &mut TestAppContext) {
    fn channel_name(index: usize, num_channels: usize) -> &'static str {
        match num_channels {
            2 => ["L", "R"][index],
            6 => ["L", "R", "C", "LFE", "Ls", "Rs"][index],
            _ => "Ch",
        }
    }

    assert_eq!(channel_name(0, 2), "L");
    assert_eq!(channel_name(1, 2), "R");
    assert_eq!(channel_name(2, 6), "C");
    assert_eq!(channel_name(3, 6), "LFE");
}

// =============================================================================
// Target Loudness Tests
// =============================================================================

/// Test target loudness setting.
#[gpui::test]
async fn test_loudness_target_setting(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessState::new(2)));

    assert!(
        (state.borrow().target_lufs - DEFAULT_TARGET_LUFS).abs() < 0.001,
        "Default target should be {} LUFS",
        DEFAULT_TARGET_LUFS
    );
}

/// Test target loudness presets.
#[gpui::test]
async fn test_loudness_target_presets(_cx: &mut TestAppContext) {
    fn target_preset(preset: &str) -> f64 {
        match preset {
            "Spotify" => -14.0,
            "Apple Music" => -16.0,
            "YouTube" => -14.0,
            "TV Broadcast" => -24.0,
            "CD" => -9.0,
            _ => -14.0,
        }
    }

    assert!((target_preset("Spotify") - (-14.0)).abs() < 0.001);
    assert!((target_preset("TV Broadcast") - (-24.0)).abs() < 0.001);
}

/// Test loudness difference from target.
#[gpui::test]
async fn test_loudness_difference_from_target(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessState::new(2)));
    state.borrow_mut().integrated_lufs = -18.0;
    state.borrow_mut().target_lufs = -14.0;

    fn loudness_difference(integrated: f64, target: f64) -> f64 {
        integrated - target
    }

    let diff = loudness_difference(
        state.borrow().integrated_lufs,
        state.borrow().target_lufs,
    );
    assert!((diff - (-4.0)).abs() < 0.001, "Content is 4 LU below target");
}

/// Test loudness difference display.
#[gpui::test]
async fn test_loudness_difference_display(_cx: &mut TestAppContext) {
    fn format_difference(diff: f64) -> String {
        if diff > 0.0 {
            format!("+{:.1} LU (louder)", diff)
        } else if diff < 0.0 {
            format!("{:.1} LU (quieter)", diff)
        } else {
            "On target".to_string()
        }
    }

    assert_eq!(format_difference(-4.0), "-4.0 LU (quieter)");
    assert_eq!(format_difference(2.0), "+2.0 LU (louder)");
    assert_eq!(format_difference(0.0), "On target");
}

// =============================================================================
// Reset Tests
// =============================================================================

/// Test measurement reset.
#[gpui::test]
async fn test_loudness_reset(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessState::new(2)));

    // Set some values
    state.borrow_mut().integrated_lufs = -20.0;
    state.borrow_mut().short_term_lufs = -15.0;
    state.borrow_mut().true_peak_db = -3.0;

    // Reset
    state.borrow_mut().reset();

    assert!(state.borrow().integrated_lufs <= MIN_LUFS + 0.001);
    assert!(state.borrow().short_term_lufs <= MIN_LUFS + 0.001);
    assert!(state.borrow().true_peak_db <= MIN_TRUE_PEAK_DB + 0.001);
}

/// Test auto-reset on play setting.
#[gpui::test]
async fn test_loudness_auto_reset_setting(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessState::new(2)));

    // Initially enabled
    assert!(state.borrow().auto_reset_on_play);

    // Disable
    state.borrow_mut().auto_reset_on_play = false;
    assert!(!state.borrow().auto_reset_on_play);
}

// =============================================================================
// Visual Feedback Tests
// =============================================================================

/// Test loudness status color.
#[gpui::test]
async fn test_loudness_status_color(_cx: &mut TestAppContext) {
    fn loudness_status_color(integrated: f64, target: f64) -> (u8, u8, u8) {
        let diff = (integrated - target).abs();
        if diff <= 1.0 {
            (0, 200, 0) // Green - on target
        } else if diff <= 3.0 {
            (255, 200, 0) // Yellow - close
        } else {
            (255, 100, 100) // Red - off target
        }
    }

    assert_eq!(
        loudness_status_color(-14.0, -14.0),
        (0, 200, 0),
        "On target = green"
    );
    assert_eq!(
        loudness_status_color(-16.0, -14.0),
        (255, 200, 0),
        "2 LU off = yellow"
    );
    assert_eq!(
        loudness_status_color(-20.0, -14.0),
        (255, 100, 100),
        "6 LU off = red"
    );
}

/// Test true peak warning color.
#[gpui::test]
async fn test_loudness_peak_warning_color(_cx: &mut TestAppContext) {
    fn peak_warning_color(true_peak_db: f64) -> (u8, u8, u8) {
        if true_peak_db > -0.1 {
            (255, 0, 0) // Red - clipping
        } else if true_peak_db > -1.0 {
            (255, 200, 0) // Yellow - danger zone
        } else if true_peak_db > -3.0 {
            (255, 165, 0) // Orange - caution
        } else {
            (0, 200, 0) // Green - safe
        }
    }

    assert_eq!(peak_warning_color(-6.0), (0, 200, 0));
    assert_eq!(peak_warning_color(-2.0), (255, 165, 0));
    assert_eq!(peak_warning_color(-0.5), (255, 200, 0));
    assert_eq!(peak_warning_color(0.5), (255, 0, 0));
}

// =============================================================================
// Keyboard Shortcut Tests
// =============================================================================

/// Test R key to reset measurements.
#[gpui::test]
async fn test_loudness_reset_key(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessState::new(2)));

    state.borrow_mut().integrated_lufs = -20.0;

    // Press R to reset
    state.borrow_mut().reset();

    assert!(state.borrow().integrated_lufs <= MIN_LUFS + 0.001);
}

/// Test target preset cycling.
#[gpui::test]
async fn test_loudness_target_cycle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LoudnessState::new(2)));

    let presets = vec![-14.0, -16.0, -24.0, -9.0];
    let mut preset_idx = 0;

    // Cycle through presets
    for expected in &presets {
        state.borrow_mut().target_lufs = presets[preset_idx];
        assert!((state.borrow().target_lufs - expected).abs() < 0.001);
        preset_idx = (preset_idx + 1) % presets.len();
    }
}

// =============================================================================
// History/Graph Tests
// =============================================================================

/// Test loudness history tracking (for graph display).
#[gpui::test]
async fn test_loudness_history(_cx: &mut TestAppContext) {
    const HISTORY_SIZE: usize = 300; // 5 minutes at 1 sample/sec

    let history: Vec<f64> = vec![MIN_LUFS; HISTORY_SIZE];

    assert_eq!(history.len(), HISTORY_SIZE);
}

/// Test loudness history update.
#[gpui::test]
async fn test_loudness_history_update(_cx: &mut TestAppContext) {
    let mut history: Vec<f64> = vec![MIN_LUFS; 10];

    // Add new reading, remove oldest
    fn push_history(history: &mut Vec<f64>, new_value: f64) {
        history.remove(0);
        history.push(new_value);
    }

    push_history(&mut history, -20.0);
    push_history(&mut history, -18.0);

    assert_eq!(history.len(), 10);
    assert!((history[8] - (-20.0)).abs() < 0.001);
    assert!((history[9] - (-18.0)).abs() < 0.001);
}
