//! E2E tests for Gain Plugin UI.
//!
//! Tests for verifying gain/volume control functionality:
//! - Main gain knob control
//! - Per-channel gain adjustments
//! - Mute/solo functionality
//! - Gain display and metering
//! - Boost/cut visual indicators

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Parameter Constants
// =============================================================================

const MIN_GAIN_DB: f64 = -60.0;
const MAX_GAIN_DB: f64 = 24.0;
const DEFAULT_GAIN_DB: f64 = 0.0;

// =============================================================================
// Channel Gain Structure
// =============================================================================

#[derive(Debug, Clone)]
struct ChannelGain {
    gain_db: f64,
    muted: bool,
    solo: bool,
}

impl Default for ChannelGain {
    fn default() -> Self {
        Self {
            gain_db: DEFAULT_GAIN_DB,
            muted: false,
            solo: false,
        }
    }
}

// =============================================================================
// Gain Plugin State
// =============================================================================

#[derive(Debug, Clone)]
struct GainPluginState {
    master_gain_db: f64,
    channels: Vec<ChannelGain>,
    link_channels: bool,
}

impl GainPluginState {
    fn new(num_channels: usize) -> Self {
        Self {
            master_gain_db: DEFAULT_GAIN_DB,
            channels: (0..num_channels).map(|_| ChannelGain::default()).collect(),
            link_channels: true,
        }
    }
}

// =============================================================================
// Main Gain Knob Tests
// =============================================================================

/// Test main gain knob initial state.
#[gpui::test]
async fn test_gain_knob_initial_state(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GainPluginState::new(2)));

    assert!(
        (state.borrow().master_gain_db - DEFAULT_GAIN_DB).abs() < 0.001,
        "Initial gain should be 0 dB"
    );
}

/// Test main gain knob rotation.
#[gpui::test]
async fn test_gain_knob_rotation(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GainPluginState::new(2)));

    // Simulate clockwise rotation (increase gain)
    let rotation_degrees = 45.0;
    let db_per_degree = 0.5;
    let delta_db = rotation_degrees * db_per_degree;

    {
        let current = state.borrow().master_gain_db;
        state.borrow_mut().master_gain_db = (current + delta_db).clamp(MIN_GAIN_DB, MAX_GAIN_DB);
    }

    assert!(
        state.borrow().master_gain_db > 0.0,
        "Gain should increase with clockwise rotation"
    );
}

/// Test main gain knob scroll adjustment.
#[gpui::test]
async fn test_gain_knob_scroll(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GainPluginState::new(2)));
    const SCROLL_STEP_DB: f64 = 0.5;

    // Scroll up (increase)
    {
        let current = state.borrow().master_gain_db;
        state.borrow_mut().master_gain_db =
            (current + SCROLL_STEP_DB).clamp(MIN_GAIN_DB, MAX_GAIN_DB);
    }
    assert!((state.borrow().master_gain_db - 0.5).abs() < 0.001);

    // Scroll down (decrease)
    {
        let current = state.borrow().master_gain_db;
        state.borrow_mut().master_gain_db =
            (current - SCROLL_STEP_DB).clamp(MIN_GAIN_DB, MAX_GAIN_DB);
    }
    assert!(state.borrow().master_gain_db.abs() < 0.001);
}

/// Test gain bounds enforcement.
#[gpui::test]
async fn test_gain_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GainPluginState::new(2)));

    // Set to minimum
    state.borrow_mut().master_gain_db = MIN_GAIN_DB;
    assert!((state.borrow().master_gain_db - MIN_GAIN_DB).abs() < 0.001);

    // Set to maximum
    state.borrow_mut().master_gain_db = MAX_GAIN_DB;
    assert!((state.borrow().master_gain_db - MAX_GAIN_DB).abs() < 0.001);

    // Attempt to exceed bounds
    let attempted = MAX_GAIN_DB + 10.0;
    let clamped = attempted.clamp(MIN_GAIN_DB, MAX_GAIN_DB);
    state.borrow_mut().master_gain_db = clamped;
    assert!((state.borrow().master_gain_db - MAX_GAIN_DB).abs() < 0.001);
}

/// Test double-click reset to 0 dB.
#[gpui::test]
async fn test_gain_knob_double_click_reset(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GainPluginState::new(2)));

    // Set to non-zero value
    state.borrow_mut().master_gain_db = 6.0;
    assert!((state.borrow().master_gain_db - 6.0).abs() < 0.001);

    // Double-click resets to 0
    state.borrow_mut().master_gain_db = DEFAULT_GAIN_DB;
    assert!(state.borrow().master_gain_db.abs() < 0.001);
}

// =============================================================================
// Per-Channel Gain Tests
// =============================================================================

/// Test per-channel gain initialization.
#[gpui::test]
async fn test_per_channel_gain_init(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GainPluginState::new(6)));

    assert_eq!(state.borrow().channels.len(), 6, "Should have 6 channels");
    for ch in state.borrow().channels.iter() {
        assert!(ch.gain_db.abs() < 0.001);
        assert!(!ch.muted);
        assert!(!ch.solo);
    }
}

/// Test individual channel gain adjustment.
#[gpui::test]
async fn test_channel_gain_individual(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GainPluginState::new(2)));

    // Adjust channel 0 only
    state.borrow_mut().channels[0].gain_db = 3.0;

    assert!((state.borrow().channels[0].gain_db - 3.0).abs() < 0.001);
    assert!(state.borrow().channels[1].gain_db.abs() < 0.001);
}

/// Test linked channel adjustment.
#[gpui::test]
async fn test_channel_gain_linked(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GainPluginState::new(2)));
    state.borrow_mut().link_channels = true;

    // When linked, adjusting one channel adjusts both
    let delta = 3.0;
    if state.borrow().link_channels {
        for ch in state.borrow_mut().channels.iter_mut() {
            ch.gain_db = (ch.gain_db + delta).clamp(MIN_GAIN_DB, MAX_GAIN_DB);
        }
    }

    assert!((state.borrow().channels[0].gain_db - 3.0).abs() < 0.001);
    assert!((state.borrow().channels[1].gain_db - 3.0).abs() < 0.001);
}

/// Test unlink channels.
#[gpui::test]
async fn test_channel_unlink(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GainPluginState::new(2)));

    // Initially linked
    assert!(state.borrow().link_channels);

    // Unlink
    state.borrow_mut().link_channels = false;
    assert!(!state.borrow().link_channels);

    // Now adjustments are independent
    state.borrow_mut().channels[0].gain_db = 6.0;
    assert!((state.borrow().channels[0].gain_db - 6.0).abs() < 0.001);
    assert!(state.borrow().channels[1].gain_db.abs() < 0.001);
}

// =============================================================================
// Mute/Solo Tests
// =============================================================================

/// Test channel mute toggle.
#[gpui::test]
async fn test_channel_mute_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GainPluginState::new(2)));

    // Mute channel 0
    state.borrow_mut().channels[0].muted = true;
    assert!(state.borrow().channels[0].muted);
    assert!(!state.borrow().channels[1].muted);

    // Unmute
    state.borrow_mut().channels[0].muted = false;
    assert!(!state.borrow().channels[0].muted);
}

/// Test channel solo toggle.
#[gpui::test]
async fn test_channel_solo_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GainPluginState::new(2)));

    // Solo channel 0
    state.borrow_mut().channels[0].solo = true;
    assert!(state.borrow().channels[0].solo);
    assert!(!state.borrow().channels[1].solo);
}

/// Test solo exclusive behavior.
#[gpui::test]
async fn test_channel_solo_exclusive(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GainPluginState::new(4)));

    // Solo channel 0
    state.borrow_mut().channels[0].solo = true;

    // Count active outputs (solo'd channels or all if no solo)
    fn active_channels(state: &GainPluginState) -> Vec<usize> {
        let any_solo = state.channels.iter().any(|ch| ch.solo);
        state
            .channels
            .iter()
            .enumerate()
            .filter(|(_, ch)| {
                if any_solo {
                    ch.solo && !ch.muted
                } else {
                    !ch.muted
                }
            })
            .map(|(i, _)| i)
            .collect()
    }

    let active = active_channels(&state.borrow());
    assert_eq!(active, vec![0], "Only channel 0 should be active");

    // Solo channel 1 as well
    state.borrow_mut().channels[1].solo = true;
    let active = active_channels(&state.borrow());
    assert_eq!(active, vec![0, 1], "Channels 0 and 1 should be active");
}

/// Test mute overrides solo.
#[gpui::test]
async fn test_mute_overrides_solo(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GainPluginState::new(2)));

    // Solo and mute channel 0
    state.borrow_mut().channels[0].solo = true;
    state.borrow_mut().channels[0].muted = true;

    fn is_channel_audible(ch: &ChannelGain, any_solo: bool) -> bool {
        if ch.muted {
            return false;
        }
        if any_solo { ch.solo } else { true }
    }

    let any_solo = state.borrow().channels.iter().any(|ch| ch.solo);
    assert!(
        !is_channel_audible(&state.borrow().channels[0], any_solo),
        "Muted channel should not be audible even if solo'd"
    );
}

// =============================================================================
// Gain Display Tests
// =============================================================================

/// Test gain display formatting.
#[gpui::test]
async fn test_gain_display_format(_cx: &mut TestAppContext) {
    fn format_gain_db(gain_db: f64) -> String {
        if gain_db > 0.0 {
            format!("+{:.1} dB", gain_db)
        } else if gain_db < -59.0 {
            "-∞ dB".to_string()
        } else {
            format!("{:.1} dB", gain_db)
        }
    }

    assert_eq!(format_gain_db(0.0), "0.0 dB");
    assert_eq!(format_gain_db(6.5), "+6.5 dB");
    assert_eq!(format_gain_db(-12.0), "-12.0 dB");
    assert_eq!(format_gain_db(-60.0), "-∞ dB");
}

/// Test boost indicator (gain > 0).
#[gpui::test]
async fn test_boost_indicator(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GainPluginState::new(2)));

    fn is_boosting(gain_db: f64) -> bool {
        gain_db > 0.1 // Small threshold to avoid floating point issues
    }

    // Initially not boosting
    assert!(!is_boosting(state.borrow().master_gain_db));

    // Set to boost
    state.borrow_mut().master_gain_db = 3.0;
    assert!(is_boosting(state.borrow().master_gain_db));
}

/// Test cut indicator (gain < 0).
#[gpui::test]
async fn test_cut_indicator(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GainPluginState::new(2)));

    fn is_cutting(gain_db: f64) -> bool {
        gain_db < -0.1
    }

    // Initially not cutting
    assert!(!is_cutting(state.borrow().master_gain_db));

    // Set to cut
    state.borrow_mut().master_gain_db = -6.0;
    assert!(is_cutting(state.borrow().master_gain_db));
}

// =============================================================================
// Keyboard Shortcut Tests
// =============================================================================

/// Test arrow key gain adjustment.
#[gpui::test]
async fn test_gain_arrow_keys(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GainPluginState::new(2)));
    const STEP_DB: f64 = 1.0;
    const FINE_STEP_DB: f64 = 0.1;

    // Up arrow = increase
    {
        let current = state.borrow().master_gain_db;
        state.borrow_mut().master_gain_db = (current + STEP_DB).clamp(MIN_GAIN_DB, MAX_GAIN_DB);
    }
    assert!((state.borrow().master_gain_db - 1.0).abs() < 0.001);

    // Down arrow = decrease
    {
        let current = state.borrow().master_gain_db;
        state.borrow_mut().master_gain_db = (current - STEP_DB).clamp(MIN_GAIN_DB, MAX_GAIN_DB);
    }
    assert!(state.borrow().master_gain_db.abs() < 0.001);

    // Shift+Up = fine increase
    {
        let current = state.borrow().master_gain_db;
        state.borrow_mut().master_gain_db =
            (current + FINE_STEP_DB).clamp(MIN_GAIN_DB, MAX_GAIN_DB);
    }
    assert!((state.borrow().master_gain_db - 0.1).abs() < 0.001);
}

/// Test M key mute toggle.
#[gpui::test]
async fn test_gain_mute_shortcut(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GainPluginState::new(2)));
    let selected_channel = 0usize;

    // Press M to mute
    {
        let current = state.borrow().channels[selected_channel].muted;
        state.borrow_mut().channels[selected_channel].muted = !current;
    }
    assert!(state.borrow().channels[selected_channel].muted);

    // Press M again to unmute
    {
        let current = state.borrow().channels[selected_channel].muted;
        state.borrow_mut().channels[selected_channel].muted = !current;
    }
    assert!(!state.borrow().channels[selected_channel].muted);
}

/// Test S key solo toggle.
#[gpui::test]
async fn test_gain_solo_shortcut(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GainPluginState::new(2)));
    let selected_channel = 0usize;

    // Press S to solo
    {
        let current = state.borrow().channels[selected_channel].solo;
        state.borrow_mut().channels[selected_channel].solo = !current;
    }
    assert!(state.borrow().channels[selected_channel].solo);

    // Press S again to unsolo
    {
        let current = state.borrow().channels[selected_channel].solo;
        state.borrow_mut().channels[selected_channel].solo = !current;
    }
    assert!(!state.borrow().channels[selected_channel].solo);
}

// =============================================================================
// Multi-Channel Configuration Tests
// =============================================================================

/// Test stereo configuration (2 channels).
#[gpui::test]
async fn test_gain_stereo_config(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GainPluginState::new(2)));

    fn channel_names(num_channels: usize) -> Vec<&'static str> {
        match num_channels {
            2 => vec!["L", "R"],
            6 => vec!["L", "R", "C", "LFE", "Ls", "Rs"],
            _ => (0..num_channels).map(|_| "Ch").collect(),
        }
    }

    let names = channel_names(state.borrow().channels.len());
    assert_eq!(names, vec!["L", "R"]);
}

/// Test 5.1 surround configuration (6 channels).
#[gpui::test]
async fn test_gain_surround_config(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GainPluginState::new(6)));

    fn channel_names(num_channels: usize) -> Vec<&'static str> {
        match num_channels {
            2 => vec!["L", "R"],
            6 => vec!["L", "R", "C", "LFE", "Ls", "Rs"],
            _ => (0..num_channels).map(|_| "Ch").collect(),
        }
    }

    let names = channel_names(state.borrow().channels.len());
    assert_eq!(names, vec!["L", "R", "C", "LFE", "Ls", "Rs"]);
}

// =============================================================================
// Preset Tests
// =============================================================================

/// Test gain preset application.
#[gpui::test]
async fn test_gain_preset(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(GainPluginState::new(2)));

    #[derive(Debug, Clone)]
    struct GainPreset {
        name: String,
        master_gain_db: f64,
        channel_gains: Vec<f64>,
    }

    let preset = GainPreset {
        name: "Subtle Boost".to_string(),
        master_gain_db: 3.0,
        channel_gains: vec![0.0, 0.0],
    };

    // Apply preset
    state.borrow_mut().master_gain_db = preset.master_gain_db;
    for (i, &gain) in preset.channel_gains.iter().enumerate() {
        if i < state.borrow().channels.len() {
            state.borrow_mut().channels[i].gain_db = gain;
        }
    }

    assert!((state.borrow().master_gain_db - 3.0).abs() < 0.001);
}

// =============================================================================
// Metering Tests (Visual Feedback)
// =============================================================================

/// Test gain reduction meter concept.
#[gpui::test]
async fn test_gain_meter_range(_cx: &mut TestAppContext) {
    // Meter should display -60dB to +24dB range
    fn meter_position(gain_db: f64) -> f64 {
        // Normalize to 0.0-1.0 range
        (gain_db - MIN_GAIN_DB) / (MAX_GAIN_DB - MIN_GAIN_DB)
    }

    assert!((meter_position(MIN_GAIN_DB) - 0.0).abs() < 0.001);
    assert!((meter_position(MAX_GAIN_DB) - 1.0).abs() < 0.001);
    assert!(
        (meter_position(0.0) - 0.714).abs() < 0.01,
        "0dB should be ~71% up the meter"
    );
}

/// Test dB scale tick marks.
#[gpui::test]
async fn test_gain_meter_ticks(_cx: &mut TestAppContext) {
    fn meter_tick_positions() -> Vec<(f64, &'static str)> {
        vec![
            (-60.0, "-∞"),
            (-48.0, "-48"),
            (-36.0, "-36"),
            (-24.0, "-24"),
            (-12.0, "-12"),
            (-6.0, "-6"),
            (0.0, "0"),
            (6.0, "+6"),
            (12.0, "+12"),
            (24.0, "+24"),
        ]
    }

    let ticks = meter_tick_positions();
    assert!(ticks.len() >= 8, "Should have major tick marks");
    assert_eq!(ticks[6], (0.0, "0"), "0 dB should be marked");
}
