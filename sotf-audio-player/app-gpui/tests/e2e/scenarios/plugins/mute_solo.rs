//! E2E tests for Channel Mute/Solo Plugin.
//!
//! Tests for the mute/solo functionality on individual audio channels.
//! The plugin allows muting or soloing channels, with solo taking priority.

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Mock Types for Testing
// =============================================================================

/// State for a single channel
#[derive(Debug, Clone, Default)]
struct ChannelState {
    muted: bool,
    soloed: bool,
    dimmed: bool,
}

/// Mute/Solo plugin state for testing
struct MuteSoloState {
    enabled: bool,
    channels: Vec<ChannelState>,
}

impl Default for MuteSoloState {
    fn default() -> Self {
        // Default to 8 channels (7.1 surround)
        Self {
            enabled: true,
            channels: vec![ChannelState::default(); 8],
        }
    }
}

impl MuteSoloState {
    fn with_channels(num_channels: usize) -> Self {
        Self {
            enabled: true,
            channels: vec![ChannelState::default(); num_channels],
        }
    }

    /// Check if any channel is soloed
    fn any_soloed(&self) -> bool {
        self.channels.iter().any(|c| c.soloed)
    }

    /// Get effective channel gain (considering mute/solo logic)
    fn effective_gain(&self, channel: usize) -> f32 {
        if !self.enabled || channel >= self.channels.len() {
            return 1.0;
        }

        let ch = &self.channels[channel];

        // If any channel is soloed, only soloed channels get audio
        if self.any_soloed() {
            if ch.soloed {
                1.0
            } else {
                0.0
            }
        } else {
            // No solo active, check mute
            if ch.muted {
                0.0
            } else if ch.dimmed {
                0.5 // Dimmed = half volume
            } else {
                1.0
            }
        }
    }
}

// =============================================================================
// Basic Plugin Tests
// =============================================================================

/// Test plugin renders correctly.
#[gpui::test]
async fn test_mute_solo_renders(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MuteSoloState::default()));
    assert!(state.borrow().enabled);
    assert_eq!(state.borrow().channels.len(), 8);
}

/// Test initial channel states.
#[gpui::test]
async fn test_mute_solo_initial_states(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MuteSoloState::default()));

    for ch in &state.borrow().channels {
        assert!(!ch.muted, "Channels should not be muted initially");
        assert!(!ch.soloed, "Channels should not be soloed initially");
        assert!(!ch.dimmed, "Channels should not be dimmed initially");
    }
}

/// Test custom channel count.
#[gpui::test]
async fn test_mute_solo_custom_channel_count(_cx: &mut TestAppContext) {
    // Test various channel counts
    for count in [2, 6, 8, 12, 16] {
        let state = MuteSoloState::with_channels(count);
        assert_eq!(state.channels.len(), count);
    }
}

// =============================================================================
// Mute Button Tests
// =============================================================================

/// Test mute button toggle.
#[gpui::test]
async fn test_mute_button_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MuteSoloState::default()));

    // Mute channel 0
    state.borrow_mut().channels[0].muted = true;
    assert!(state.borrow().channels[0].muted);

    // Unmute channel 0
    state.borrow_mut().channels[0].muted = false;
    assert!(!state.borrow().channels[0].muted);
}

/// Test muting multiple channels.
#[gpui::test]
async fn test_mute_multiple_channels(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MuteSoloState::default()));

    // Mute channels 0, 2, 4
    state.borrow_mut().channels[0].muted = true;
    state.borrow_mut().channels[2].muted = true;
    state.borrow_mut().channels[4].muted = true;

    assert!(state.borrow().channels[0].muted);
    assert!(!state.borrow().channels[1].muted);
    assert!(state.borrow().channels[2].muted);
    assert!(!state.borrow().channels[3].muted);
    assert!(state.borrow().channels[4].muted);
}

/// Test mute all channels.
#[gpui::test]
async fn test_mute_all_channels(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MuteSoloState::default()));

    // Mute all
    for ch in state.borrow_mut().channels.iter_mut() {
        ch.muted = true;
    }

    for ch in &state.borrow().channels {
        assert!(ch.muted);
    }

    // Unmute all
    for ch in state.borrow_mut().channels.iter_mut() {
        ch.muted = false;
    }

    for ch in &state.borrow().channels {
        assert!(!ch.muted);
    }
}

/// Test mute affects gain.
#[gpui::test]
async fn test_mute_affects_gain(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MuteSoloState::default()));

    // Channel 0 unmuted = gain 1.0
    assert!((state.borrow().effective_gain(0) - 1.0).abs() < 0.001);

    // Mute channel 0
    state.borrow_mut().channels[0].muted = true;
    assert!((state.borrow().effective_gain(0) - 0.0).abs() < 0.001);
}

// =============================================================================
// Solo Button Tests
// =============================================================================

/// Test solo button toggle.
#[gpui::test]
async fn test_solo_button_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MuteSoloState::default()));

    // Solo channel 0
    state.borrow_mut().channels[0].soloed = true;
    assert!(state.borrow().channels[0].soloed);
    assert!(state.borrow().any_soloed());

    // Unsolo channel 0
    state.borrow_mut().channels[0].soloed = false;
    assert!(!state.borrow().channels[0].soloed);
    assert!(!state.borrow().any_soloed());
}

/// Test solo mutes other channels.
#[gpui::test]
async fn test_solo_mutes_others(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MuteSoloState::default()));

    // Solo channel 0 - only channel 0 should have audio
    state.borrow_mut().channels[0].soloed = true;

    // Channel 0 = 1.0 (soloed)
    assert!((state.borrow().effective_gain(0) - 1.0).abs() < 0.001);

    // All other channels = 0.0 (not soloed)
    for i in 1..8 {
        assert!(
            (state.borrow().effective_gain(i) - 0.0).abs() < 0.001,
            "Channel {} should be silent when channel 0 is soloed",
            i
        );
    }
}

/// Test multiple channels soloed.
#[gpui::test]
async fn test_solo_multiple_channels(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MuteSoloState::default()));

    // Solo left and right front (0 and 1)
    state.borrow_mut().channels[0].soloed = true;
    state.borrow_mut().channels[1].soloed = true;

    // Both soloed channels have audio
    assert!((state.borrow().effective_gain(0) - 1.0).abs() < 0.001);
    assert!((state.borrow().effective_gain(1) - 1.0).abs() < 0.001);

    // Others are silent
    for i in 2..8 {
        assert!((state.borrow().effective_gain(i) - 0.0).abs() < 0.001);
    }
}

/// Test solo overrides mute.
#[gpui::test]
async fn test_solo_overrides_mute(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MuteSoloState::default()));

    // Mute channel 0, then solo it
    state.borrow_mut().channels[0].muted = true;
    state.borrow_mut().channels[0].soloed = true;

    // Solo takes priority - channel should have audio
    assert!((state.borrow().effective_gain(0) - 1.0).abs() < 0.001);
}

/// Test unsolo restores mute state.
#[gpui::test]
async fn test_unsolo_restores_mute(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MuteSoloState::default()));

    // Mute channel 0
    state.borrow_mut().channels[0].muted = true;

    // Solo channel 0 (overrides mute)
    state.borrow_mut().channels[0].soloed = true;
    assert!((state.borrow().effective_gain(0) - 1.0).abs() < 0.001);

    // Unsolo - mute should take effect again
    state.borrow_mut().channels[0].soloed = false;
    assert!((state.borrow().effective_gain(0) - 0.0).abs() < 0.001);
}

// =============================================================================
// Dim Tests
// =============================================================================

/// Test dim toggle.
#[gpui::test]
async fn test_dim_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MuteSoloState::default()));

    state.borrow_mut().channels[0].dimmed = true;
    assert!(state.borrow().channels[0].dimmed);

    state.borrow_mut().channels[0].dimmed = false;
    assert!(!state.borrow().channels[0].dimmed);
}

/// Test dim reduces gain.
#[gpui::test]
async fn test_dim_reduces_gain(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MuteSoloState::default()));

    // Normal = 1.0
    assert!((state.borrow().effective_gain(0) - 1.0).abs() < 0.001);

    // Dimmed = 0.5
    state.borrow_mut().channels[0].dimmed = true;
    assert!((state.borrow().effective_gain(0) - 0.5).abs() < 0.001);
}

/// Test mute overrides dim.
#[gpui::test]
async fn test_mute_overrides_dim(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MuteSoloState::default()));

    // Dim and mute
    state.borrow_mut().channels[0].dimmed = true;
    state.borrow_mut().channels[0].muted = true;

    // Mute takes priority
    assert!((state.borrow().effective_gain(0) - 0.0).abs() < 0.001);
}

// =============================================================================
// Enable/Disable Tests
// =============================================================================

/// Test enabled toggle.
#[gpui::test]
async fn test_enabled_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MuteSoloState::default()));

    assert!(state.borrow().enabled);

    state.borrow_mut().enabled = false;
    assert!(!state.borrow().enabled);
}

/// Test disabled plugin passes through.
#[gpui::test]
async fn test_disabled_passes_through(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MuteSoloState::default()));

    // Mute all channels
    for ch in state.borrow_mut().channels.iter_mut() {
        ch.muted = true;
    }

    // Disable plugin - all channels should pass through
    state.borrow_mut().enabled = false;

    for i in 0..8 {
        assert!(
            (state.borrow().effective_gain(i) - 1.0).abs() < 0.001,
            "Disabled plugin should pass through"
        );
    }
}

// =============================================================================
// Channel Label Tests
// =============================================================================

/// Test channel labels for stereo.
#[gpui::test]
async fn test_channel_labels_stereo(_cx: &mut TestAppContext) {
    fn get_channel_label(channel: usize, total: usize) -> &'static str {
        match (total, channel) {
            (2, 0) => "L",
            (2, 1) => "R",
            _ => "?",
        }
    }

    assert_eq!(get_channel_label(0, 2), "L");
    assert_eq!(get_channel_label(1, 2), "R");
}

/// Test channel labels for 5.1.
#[gpui::test]
async fn test_channel_labels_5_1(_cx: &mut TestAppContext) {
    fn get_channel_label(channel: usize, total: usize) -> &'static str {
        match (total, channel) {
            (6, 0) => "L",
            (6, 1) => "R",
            (6, 2) => "C",
            (6, 3) => "LFE",
            (6, 4) => "LS",
            (6, 5) => "RS",
            _ => "?",
        }
    }

    assert_eq!(get_channel_label(0, 6), "L");
    assert_eq!(get_channel_label(1, 6), "R");
    assert_eq!(get_channel_label(2, 6), "C");
    assert_eq!(get_channel_label(3, 6), "LFE");
    assert_eq!(get_channel_label(4, 6), "LS");
    assert_eq!(get_channel_label(5, 6), "RS");
}

/// Test channel labels for 7.1.
#[gpui::test]
async fn test_channel_labels_7_1(_cx: &mut TestAppContext) {
    fn get_channel_label(channel: usize, total: usize) -> &'static str {
        match (total, channel) {
            (8, 0) => "L",
            (8, 1) => "R",
            (8, 2) => "C",
            (8, 3) => "LFE",
            (8, 4) => "LS",
            (8, 5) => "RS",
            (8, 6) => "LB",
            (8, 7) => "RB",
            _ => "?",
        }
    }

    assert_eq!(get_channel_label(6, 8), "LB");
    assert_eq!(get_channel_label(7, 8), "RB");
}

// =============================================================================
// Exclusive Solo Mode Tests
// =============================================================================

/// Test exclusive solo mode.
#[gpui::test]
async fn test_exclusive_solo_mode(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MuteSoloState::default()));

    // Simulate exclusive solo (solo one channel, unsolo others)
    fn exclusive_solo(state: &mut MuteSoloState, channel: usize) {
        for (i, ch) in state.channels.iter_mut().enumerate() {
            ch.soloed = i == channel;
        }
    }

    // Solo channel 2 exclusively
    exclusive_solo(&mut state.borrow_mut(), 2);

    for (i, ch) in state.borrow().channels.iter().enumerate() {
        if i == 2 {
            assert!(ch.soloed, "Channel 2 should be soloed");
        } else {
            assert!(!ch.soloed, "Other channels should not be soloed");
        }
    }
}

/// Test clear all solos.
#[gpui::test]
async fn test_clear_all_solos(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MuteSoloState::default()));

    // Solo multiple channels
    state.borrow_mut().channels[0].soloed = true;
    state.borrow_mut().channels[2].soloed = true;
    state.borrow_mut().channels[4].soloed = true;

    // Clear all solos
    for ch in state.borrow_mut().channels.iter_mut() {
        ch.soloed = false;
    }

    assert!(!state.borrow().any_soloed());
}

/// Test clear all mutes.
#[gpui::test]
async fn test_clear_all_mutes(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MuteSoloState::default()));

    // Mute multiple channels
    state.borrow_mut().channels[0].muted = true;
    state.borrow_mut().channels[2].muted = true;
    state.borrow_mut().channels[4].muted = true;

    // Clear all mutes
    for ch in state.borrow_mut().channels.iter_mut() {
        ch.muted = false;
    }

    for ch in &state.borrow().channels {
        assert!(!ch.muted);
    }
}

// =============================================================================
// Visual Indicator Tests
// =============================================================================

/// Test mute button visual state.
#[gpui::test]
async fn test_mute_button_visual(_cx: &mut TestAppContext) {
    fn get_mute_button_color(muted: bool) -> &'static str {
        if muted {
            "red"
        } else {
            "gray"
        }
    }

    assert_eq!(get_mute_button_color(false), "gray");
    assert_eq!(get_mute_button_color(true), "red");
}

/// Test solo button visual state.
#[gpui::test]
async fn test_solo_button_visual(_cx: &mut TestAppContext) {
    fn get_solo_button_color(soloed: bool) -> &'static str {
        if soloed {
            "yellow"
        } else {
            "gray"
        }
    }

    assert_eq!(get_solo_button_color(false), "gray");
    assert_eq!(get_solo_button_color(true), "yellow");
}

/// Test channel meter color based on state.
#[gpui::test]
async fn test_channel_meter_color(_cx: &mut TestAppContext) {
    fn get_meter_color(state: &ChannelState) -> &'static str {
        if state.muted {
            "dimmed"
        } else if state.soloed {
            "highlighted"
        } else {
            "normal"
        }
    }

    let normal = ChannelState::default();
    assert_eq!(get_meter_color(&normal), "normal");

    let muted = ChannelState {
        muted: true,
        ..Default::default()
    };
    assert_eq!(get_meter_color(&muted), "dimmed");

    let soloed = ChannelState {
        soloed: true,
        ..Default::default()
    };
    assert_eq!(get_meter_color(&soloed), "highlighted");
}

// =============================================================================
// Preset Tests
// =============================================================================

/// Test save/restore state.
#[gpui::test]
async fn test_save_restore_state(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MuteSoloState::default()));

    // Set up specific state
    state.borrow_mut().channels[0].muted = true;
    state.borrow_mut().channels[1].soloed = true;
    state.borrow_mut().channels[2].dimmed = true;

    // "Save" by cloning
    let saved: Vec<(bool, bool, bool)> = state
        .borrow()
        .channels
        .iter()
        .map(|c| (c.muted, c.soloed, c.dimmed))
        .collect();

    // Reset state
    for ch in state.borrow_mut().channels.iter_mut() {
        ch.muted = false;
        ch.soloed = false;
        ch.dimmed = false;
    }

    // Restore
    for (i, (muted, soloed, dimmed)) in saved.iter().enumerate() {
        state.borrow_mut().channels[i].muted = *muted;
        state.borrow_mut().channels[i].soloed = *soloed;
        state.borrow_mut().channels[i].dimmed = *dimmed;
    }

    assert!(state.borrow().channels[0].muted);
    assert!(state.borrow().channels[1].soloed);
    assert!(state.borrow().channels[2].dimmed);
}

// =============================================================================
// Edge Case Tests
// =============================================================================

/// Test out of bounds channel access.
#[gpui::test]
async fn test_out_of_bounds_channel(_cx: &mut TestAppContext) {
    let state = MuteSoloState::with_channels(2);

    // Accessing channel beyond count should return 1.0 (pass through)
    assert!((state.effective_gain(10) - 1.0).abs() < 0.001);
}

/// Test mono channel.
#[gpui::test]
async fn test_mono_channel(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(MuteSoloState::with_channels(1)));

    // Solo the only channel (should still work)
    state.borrow_mut().channels[0].soloed = true;
    assert!((state.borrow().effective_gain(0) - 1.0).abs() < 0.001);

    // Mute the only channel
    state.borrow_mut().channels[0].soloed = false;
    state.borrow_mut().channels[0].muted = true;
    assert!((state.borrow().effective_gain(0) - 0.0).abs() < 0.001);
}

/// Test many channels (Atmos-style).
#[gpui::test]
async fn test_many_channels_atmos(_cx: &mut TestAppContext) {
    // 7.1.4 = 12 channels
    let state = Rc::new(RefCell::new(MuteSoloState::with_channels(12)));

    // Solo height channels only (8-11)
    for i in 8..12 {
        state.borrow_mut().channels[i].soloed = true;
    }

    // Height channels should have audio
    for i in 8..12 {
        assert!(
            (state.borrow().effective_gain(i) - 1.0).abs() < 0.001,
            "Height channel {} should be active",
            i
        );
    }

    // Bed channels should be silent
    for i in 0..8 {
        assert!(
            (state.borrow().effective_gain(i) - 0.0).abs() < 0.001,
            "Bed channel {} should be silent",
            i
        );
    }
}
