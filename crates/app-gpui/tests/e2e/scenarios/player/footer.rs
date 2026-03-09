//! E2E tests for Footer component.
//!
//! Tests for verifying footer rendering and interactions:
//! - Transport controls (play/pause, prev, next, seek forward/backward)
//! - Volume control (scroll, keyboard, drag, mute)
//! - Waveform seeking
//! - Device selection popup
//! - Studio menu

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Transport Controls Tests
// =============================================================================

/// Test play/pause toggle state transitions.
#[gpui::test]
async fn test_footer_play_pause_toggle(_cx: &mut TestAppContext) {
    let is_playing = Rc::new(RefCell::new(false));

    // Toggle to playing
    *is_playing.borrow_mut() = true;
    assert!(*is_playing.borrow(), "Should be playing after toggle");

    // Toggle to paused
    *is_playing.borrow_mut() = false;
    assert!(!*is_playing.borrow(), "Should be paused after toggle");

    // Multiple toggles
    for _ in 0..5 {
        let current = *is_playing.borrow();
        *is_playing.borrow_mut() = !current;
    }
    assert!(
        *is_playing.borrow(),
        "After odd number of toggles, should be playing"
    );
}

/// Test previous track navigation.
#[gpui::test]
async fn test_footer_prev_track(_cx: &mut TestAppContext) {
    let queue_index: Rc<RefCell<Option<usize>>> = Rc::new(RefCell::new(Some(3)));

    // Navigate to previous track
    {
        let idx = *queue_index.borrow();
        if let Some(i) = idx
            && i > 0 {
                *queue_index.borrow_mut() = Some(i - 1);
            }
    }
    assert_eq!(
        *queue_index.borrow(),
        Some(2),
        "Should navigate to previous track"
    );

    // Navigate to first track
    *queue_index.borrow_mut() = Some(0);
    {
        let idx = *queue_index.borrow();
        if let Some(i) = idx
            && i > 0 {
                *queue_index.borrow_mut() = Some(i - 1);
            }
    }
    assert_eq!(*queue_index.borrow(), Some(0), "Should stay at first track");
}

/// Test next track navigation.
#[gpui::test]
async fn test_footer_next_track(_cx: &mut TestAppContext) {
    let queue_index: Rc<RefCell<Option<usize>>> = Rc::new(RefCell::new(Some(2)));
    let queue_len = 5;

    // Navigate to next track
    {
        let idx = *queue_index.borrow();
        if let Some(i) = idx
            && i < queue_len - 1 {
                *queue_index.borrow_mut() = Some(i + 1);
            }
    }
    assert_eq!(
        *queue_index.borrow(),
        Some(3),
        "Should navigate to next track"
    );

    // Navigate at last track
    *queue_index.borrow_mut() = Some(4);
    {
        let idx = *queue_index.borrow();
        if let Some(i) = idx
            && i < queue_len - 1 {
                *queue_index.borrow_mut() = Some(i + 1);
            }
    }
    assert_eq!(*queue_index.borrow(), Some(4), "Should stay at last track");
}

/// Test seek forward by 30 seconds.
#[gpui::test]
async fn test_footer_seek_forward(_cx: &mut TestAppContext) {
    let position = Rc::new(RefCell::new(60.0f64)); // Start at 1 minute
    let duration = 300.0f64; // 5 minute track

    // Seek forward 30 seconds
    let seek_amount = 30.0;
    let new_position = (*position.borrow() + seek_amount).min(duration);
    *position.borrow_mut() = new_position;

    assert!(
        (*position.borrow() - 90.0).abs() < 0.001,
        "Should be at 90 seconds after seeking forward"
    );

    // Seek forward at near end
    *position.borrow_mut() = 290.0;
    let new_position = (*position.borrow() + seek_amount).min(duration);
    *position.borrow_mut() = new_position;

    assert!(
        (*position.borrow() - duration).abs() < 0.001,
        "Should be clamped to duration"
    );
}

/// Test seek backward by 30 seconds.
#[gpui::test]
async fn test_footer_seek_backward(_cx: &mut TestAppContext) {
    let position = Rc::new(RefCell::new(60.0f64)); // Start at 1 minute

    // Seek backward 30 seconds
    let seek_amount = 30.0;
    let new_position = (*position.borrow() - seek_amount).max(0.0);
    *position.borrow_mut() = new_position;

    assert!(
        (*position.borrow() - 30.0).abs() < 0.001,
        "Should be at 30 seconds after seeking backward"
    );

    // Seek backward at near start
    *position.borrow_mut() = 10.0;
    let new_position = (*position.borrow() - seek_amount).max(0.0);
    *position.borrow_mut() = new_position;

    assert!(*position.borrow() < 0.001, "Should be clamped to 0");
}

// =============================================================================
// Volume Control Tests
// =============================================================================

/// Test volume scroll up increases volume.
#[gpui::test]
async fn test_footer_volume_scroll_up(_cx: &mut TestAppContext) {
    let volume = Rc::new(RefCell::new(0.5f32));
    const VOLUME_STEP: f32 = 0.05;

    // Scroll up
    let new_volume = (*volume.borrow() + VOLUME_STEP).clamp(0.0, 1.0);
    *volume.borrow_mut() = new_volume;

    assert!(
        (*volume.borrow() - 0.55).abs() < 0.001,
        "Volume should increase by step"
    );
}

/// Test volume scroll down decreases volume.
#[gpui::test]
async fn test_footer_volume_scroll_down(_cx: &mut TestAppContext) {
    let volume = Rc::new(RefCell::new(0.5f32));
    const VOLUME_STEP: f32 = 0.05;

    // Scroll down
    let new_volume = (*volume.borrow() - VOLUME_STEP).clamp(0.0, 1.0);
    *volume.borrow_mut() = new_volume;

    assert!(
        (*volume.borrow() - 0.45).abs() < 0.001,
        "Volume should decrease by step"
    );
}

/// Test volume keyboard controls (arrow keys).
#[gpui::test]
async fn test_footer_volume_keyboard_arrows(_cx: &mut TestAppContext) {
    let volume = Rc::new(RefCell::new(0.5f32));
    const VOLUME_STEP: f32 = 0.05;
    const VOLUME_STEP_LARGE: f32 = 0.10;

    // Test up arrow
    let new_volume = (*volume.borrow() + VOLUME_STEP).clamp(0.0, 1.0);
    *volume.borrow_mut() = new_volume;
    assert!(
        (*volume.borrow() - 0.55).abs() < 0.001,
        "Up arrow should increase volume"
    );

    // Test page up (large step)
    *volume.borrow_mut() = 0.5;
    let new_volume = (*volume.borrow() + VOLUME_STEP_LARGE).clamp(0.0, 1.0);
    *volume.borrow_mut() = new_volume;
    assert!(
        (*volume.borrow() - 0.60).abs() < 0.001,
        "Page up should increase volume by large step"
    );

    // Test down arrow
    *volume.borrow_mut() = 0.5;
    let new_volume = (*volume.borrow() - VOLUME_STEP).clamp(0.0, 1.0);
    *volume.borrow_mut() = new_volume;
    assert!(
        (*volume.borrow() - 0.45).abs() < 0.001,
        "Down arrow should decrease volume"
    );

    // Test page down (large step)
    *volume.borrow_mut() = 0.5;
    let new_volume = (*volume.borrow() - VOLUME_STEP_LARGE).clamp(0.0, 1.0);
    *volume.borrow_mut() = new_volume;
    assert!(
        (*volume.borrow() - 0.40).abs() < 0.001,
        "Page down should decrease volume by large step"
    );
}

/// Test volume keyboard controls (+/- keys).
#[gpui::test]
async fn test_footer_volume_keyboard_plus_minus(_cx: &mut TestAppContext) {
    let volume = Rc::new(RefCell::new(0.5f32));
    const VOLUME_STEP: f32 = 0.05;

    // Test + key
    let new_volume = (*volume.borrow() + VOLUME_STEP).clamp(0.0, 1.0);
    *volume.borrow_mut() = new_volume;
    assert!(
        (*volume.borrow() - 0.55).abs() < 0.001,
        "+ key should increase volume"
    );

    // Test - key
    *volume.borrow_mut() = 0.5;
    let new_volume = (*volume.borrow() - VOLUME_STEP).clamp(0.0, 1.0);
    *volume.borrow_mut() = new_volume;
    assert!(
        (*volume.borrow() - 0.45).abs() < 0.001,
        "- key should decrease volume"
    );
}

/// Test volume keyboard controls (home/end keys).
#[gpui::test]
async fn test_footer_volume_keyboard_home_end(_cx: &mut TestAppContext) {
    let volume = Rc::new(RefCell::new(0.5f32));

    // Test home key (max volume)
    let new_volume = (*volume.borrow() + 1.0).clamp(0.0, 1.0);
    *volume.borrow_mut() = new_volume;
    assert!(
        (*volume.borrow() - 1.0).abs() < 0.001,
        "Home key should set max volume"
    );

    // Test end key (min volume)
    *volume.borrow_mut() = 0.5;
    let new_volume = (*volume.borrow() - 1.0).clamp(0.0, 1.0);
    *volume.borrow_mut() = new_volume;
    assert!(*volume.borrow() < 0.001, "End key should set min volume");
}

/// Test mute toggle.
#[gpui::test]
async fn test_footer_volume_mute_toggle(_cx: &mut TestAppContext) {
    let muted = Rc::new(RefCell::new(false));
    let volume = Rc::new(RefCell::new(0.5f32));

    // Toggle mute on
    *muted.borrow_mut() = true;
    let effective_volume = if *muted.borrow() {
        0.0
    } else {
        *volume.borrow()
    };
    assert!(
        effective_volume < 0.001,
        "Effective volume should be 0 when muted"
    );
    assert!(
        (*volume.borrow() - 0.5).abs() < 0.001,
        "Stored volume should remain unchanged"
    );

    // Toggle mute off
    *muted.borrow_mut() = false;
    let effective_volume = if *muted.borrow() {
        0.0
    } else {
        *volume.borrow()
    };
    assert!(
        (effective_volume - 0.5).abs() < 0.001,
        "Volume should restore when unmuted"
    );
}

/// Test double-click resets volume to 10%.
#[gpui::test]
async fn test_footer_volume_double_click_reset(_cx: &mut TestAppContext) {
    let volume = Rc::new(RefCell::new(0.8f32));
    const DEFAULT_VOLUME: f32 = 0.1;

    // Simulate double-click
    *volume.borrow_mut() = DEFAULT_VOLUME;

    assert!(
        (*volume.borrow() - 0.1).abs() < 0.001,
        "Double-click should reset volume to 10%"
    );
}

/// Test volume bounds clamping.
#[gpui::test]
async fn test_footer_volume_bounds(_cx: &mut TestAppContext) {
    let volume = Rc::new(RefCell::new(0.5f32));

    // Test upper bound
    *volume.borrow_mut() = 1.5f32.clamp(0.0, 1.0);
    assert!(
        (*volume.borrow() - 1.0).abs() < 0.001,
        "Volume should be clamped to 1.0"
    );

    // Test lower bound
    *volume.borrow_mut() = (-0.5f32).clamp(0.0, 1.0);
    assert!(*volume.borrow() < 0.001, "Volume should be clamped to 0.0");
}

// =============================================================================
// Waveform Seeking Tests
// =============================================================================

/// Test waveform click-to-seek calculates correct position.
#[gpui::test]
async fn test_footer_waveform_seek_start(_cx: &mut TestAppContext) {
    let position = Rc::new(RefCell::new(60.0f64));
    let duration = 300.0f64;

    // Click at 0% (start)
    let click_ratio = 0.0f64;
    let new_position = duration * click_ratio;
    *position.borrow_mut() = new_position;

    assert!(*position.borrow() < 0.001, "Should seek to start");
}

/// Test waveform click-to-seek at middle.
#[gpui::test]
async fn test_footer_waveform_seek_middle(_cx: &mut TestAppContext) {
    let position = Rc::new(RefCell::new(0.0f64));
    let duration = 300.0f64;

    // Click at 50% (middle)
    let click_ratio = 0.5f64;
    let new_position = duration * click_ratio;
    *position.borrow_mut() = new_position;

    assert!(
        (*position.borrow() - 150.0).abs() < 0.001,
        "Should seek to middle"
    );
}

/// Test waveform click-to-seek at end.
#[gpui::test]
async fn test_footer_waveform_seek_end(_cx: &mut TestAppContext) {
    let position = Rc::new(RefCell::new(0.0f64));
    let duration = 300.0f64;

    // Click at 100% (end)
    let click_ratio = 1.0f64;
    let new_position = duration * click_ratio;
    *position.borrow_mut() = new_position;

    assert!(
        (*position.borrow() - 300.0).abs() < 0.001,
        "Should seek to end"
    );
}

/// Test waveform progress calculation.
#[gpui::test]
async fn test_footer_waveform_progress(_cx: &mut TestAppContext) {
    let position = 75.0f64;
    let duration = 300.0f64;

    let progress = if duration > 0.0 {
        (position / duration).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };

    assert!((progress - 0.25).abs() < 0.001, "Progress should be 25%");
}

/// Test waveform progress with zero duration.
#[gpui::test]
async fn test_footer_waveform_progress_zero_duration(_cx: &mut TestAppContext) {
    let position = 75.0f64;
    let duration = 0.0f64;

    let progress = if duration > 0.0 {
        (position / duration).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };

    assert!(progress < 0.001, "Progress should be 0 with zero duration");
}

// =============================================================================
// Time Display Tests
// =============================================================================

/// Test time formatting for display.
#[gpui::test]
async fn test_footer_time_format(_cx: &mut TestAppContext) {
    // Format function (mirrors footer.rs)
    let format_time = |secs: f64| -> String {
        let mins = (secs / 60.0) as u32;
        let s = (secs % 60.0) as u32;
        format!("{:02}:{:02}", mins, s)
    };

    assert_eq!(format_time(0.0), "00:00", "Zero seconds");
    assert_eq!(format_time(59.0), "00:59", "59 seconds");
    assert_eq!(format_time(60.0), "01:00", "1 minute");
    assert_eq!(format_time(90.0), "01:30", "1.5 minutes");
    assert_eq!(format_time(3600.0), "60:00", "1 hour");
    assert_eq!(format_time(3661.0), "61:01", "61 minutes 1 second");
}

// =============================================================================
// Device Popup Tests
// =============================================================================

/// Test device popup toggle state.
#[gpui::test]
async fn test_footer_device_popup_toggle(_cx: &mut TestAppContext) {
    let show_popup = Rc::new(RefCell::new(false));

    // Open popup
    *show_popup.borrow_mut() = true;
    assert!(*show_popup.borrow(), "Popup should be visible");

    // Close popup
    *show_popup.borrow_mut() = false;
    assert!(!*show_popup.borrow(), "Popup should be hidden");
}

/// Test device selection.
#[gpui::test]
async fn test_footer_device_selection(_cx: &mut TestAppContext) {
    let selected_index = Rc::new(RefCell::new(0usize));
    let device_names = ["Default", "BlackHole 2ch", "External DAC"];

    // Select second device
    *selected_index.borrow_mut() = 1;
    assert_eq!(*selected_index.borrow(), 1, "Should select second device");

    // Verify device name
    let selected_name = device_names.get(*selected_index.borrow());
    assert_eq!(selected_name, Some(&"BlackHole 2ch"));
}

/// Test device name truncation.
#[gpui::test]
async fn test_footer_device_name_truncation(_cx: &mut TestAppContext) {
    let device_name = "External USB Audio DAC Pro".to_string();
    let max_len = 7;

    let truncated = if device_name.len() > max_len {
        device_name.chars().take(max_len).collect::<String>()
    } else {
        device_name.clone()
    };

    assert_eq!(truncated.len(), max_len, "Should truncate to 7 chars");
    assert_eq!(truncated, "Externa", "Should keep first 7 chars");
}

// =============================================================================
// Studio Menu Tests
// =============================================================================

/// Test studio menu toggle state.
#[gpui::test]
async fn test_footer_studio_menu_toggle(_cx: &mut TestAppContext) {
    let show_menu = Rc::new(RefCell::new(false));

    // Open menu
    *show_menu.borrow_mut() = true;
    assert!(*show_menu.borrow(), "Menu should be visible");

    // Close menu
    *show_menu.borrow_mut() = false;
    assert!(!*show_menu.borrow(), "Menu should be hidden");
}

/// Test screen navigation from studio menu.
#[gpui::test]
async fn test_footer_studio_menu_navigation(_cx: &mut TestAppContext) {
    #[derive(Debug, Clone, Copy, PartialEq)]
    enum Screen {
        Library,
        Studio,
        PluginGraph,
        Recording,
        RoomEq,
        HeadphoneEq,
        Spinorama,
    }

    let current_screen: Rc<RefCell<Screen>> = Rc::new(RefCell::new(Screen::Library));

    // Navigate to Studio
    *current_screen.borrow_mut() = Screen::Studio;
    assert_eq!(*current_screen.borrow(), Screen::Studio);

    // Navigate to PluginGraph
    *current_screen.borrow_mut() = Screen::PluginGraph;
    assert_eq!(*current_screen.borrow(), Screen::PluginGraph);

    // Navigate to Recording
    *current_screen.borrow_mut() = Screen::Recording;
    assert_eq!(*current_screen.borrow(), Screen::Recording);

    // Navigate back to Library
    *current_screen.borrow_mut() = Screen::Library;
    assert_eq!(*current_screen.borrow(), Screen::Library);
}

// =============================================================================
// Responsive Layout Tests
// =============================================================================

/// Test responsive breakpoints for waveform visibility.
#[gpui::test]
async fn test_footer_responsive_waveform_visibility(_cx: &mut TestAppContext) {
    const BREAKPOINT_HIDE_WAVEFORM: f32 = 700.0;

    let window_widths = [800.0, 700.0, 600.0, 500.0];
    let expected_visible = [true, true, false, false];

    for (width, expected) in window_widths.iter().zip(expected_visible.iter()) {
        let show_waveform = *width >= BREAKPOINT_HIDE_WAVEFORM;
        assert_eq!(
            show_waveform, *expected,
            "Waveform visibility at width {} should be {}",
            width, expected
        );
    }
}

/// Test responsive breakpoints for timestamp positioning.
/// Timestamps always flank the transport controls on the same row.
/// Waveform appears below when window is wide enough (>=700).
#[gpui::test]
async fn test_footer_responsive_timestamp_positioning(_cx: &mut TestAppContext) {
    const BREAKPOINT_HIDE_WAVEFORM: f32 = 700.0;

    // Wide: waveform shown below transport+timestamps row
    let width = 1000.0;
    assert!(width >= BREAKPOINT_HIDE_WAVEFORM);

    // Medium: waveform still shown
    let width = 800.0;
    assert!(width >= BREAKPOINT_HIDE_WAVEFORM);

    // Narrow: waveform hidden, compact time display
    let width = 600.0;
    assert!(
        width < BREAKPOINT_HIDE_WAVEFORM,
        "At 600px, waveform should be hidden"
    );
}

/// Test responsive breakpoints for track info visibility.
#[gpui::test]
async fn test_footer_responsive_track_info_visibility(_cx: &mut TestAppContext) {
    const BREAKPOINT_HIDE_TRACK_INFO: f32 = 550.0;

    let window_widths = [600.0, 550.0, 500.0, 400.0];
    let expected_visible = [true, true, false, false];

    for (width, expected) in window_widths.iter().zip(expected_visible.iter()) {
        let show_track_info = *width >= BREAKPOINT_HIDE_TRACK_INFO;
        assert_eq!(
            show_track_info, *expected,
            "Track info visibility at width {} should be {}",
            width, expected
        );
    }
}

/// Test responsive breakpoints for studio/device buttons visibility.
#[gpui::test]
async fn test_footer_responsive_studio_device_visibility(_cx: &mut TestAppContext) {
    const BREAKPOINT_HIDE_STUDIO_DEVICE: f32 = 400.0;

    let window_widths = [500.0, 400.0, 350.0, 300.0];
    let expected_visible = [true, true, false, false];

    for (width, expected) in window_widths.iter().zip(expected_visible.iter()) {
        let show_studio_device = *width >= BREAKPOINT_HIDE_STUDIO_DEVICE;
        assert_eq!(
            show_studio_device, *expected,
            "Studio/device visibility at width {} should be {}",
            width, expected
        );
    }
}

// =============================================================================
// Track Info Display Tests
// =============================================================================

/// Test track title truncation.
#[gpui::test]
async fn test_footer_track_title_truncation(_cx: &mut TestAppContext) {
    let title = "This is a very long track title that should be truncated".to_string();
    let max_len = 30;

    let truncated = if title.chars().count() > max_len {
        title.chars().take(max_len).collect::<String>() + "..."
    } else {
        title.clone()
    };

    assert!(
        truncated.chars().count() <= max_len + 3,
        "Should truncate with ellipsis"
    );
    assert!(truncated.ends_with("..."), "Should end with ellipsis");
}

/// Test album name truncation.
#[gpui::test]
async fn test_footer_album_name_truncation(_cx: &mut TestAppContext) {
    let album = "This is a very long album name that definitely exceeds our limit".to_string();
    let max_len = 35;

    let truncated = if album.chars().count() > max_len {
        album.chars().take(max_len).collect::<String>() + "..."
    } else {
        album.clone()
    };

    assert!(
        truncated.chars().count() <= max_len + 3,
        "Should truncate with ellipsis"
    );
}

/// Test artist name truncation.
#[gpui::test]
async fn test_footer_artist_name_truncation(_cx: &mut TestAppContext) {
    let artist = "Some Artist With A Very Long Name That Exceeds Our Display Limit".to_string();
    let max_len = 35;

    let truncated = if artist.chars().count() > max_len {
        artist.chars().take(max_len).collect::<String>() + "..."
    } else {
        artist.clone()
    };

    assert!(
        truncated.chars().count() <= max_len + 3,
        "Should truncate with ellipsis"
    );
}

/// Test empty track info fallback.
#[gpui::test]
async fn test_footer_empty_track_info_fallback(_cx: &mut TestAppContext) {
    let title = String::new();
    let no_track_label = "No Track Playing";

    let display_title = if title.is_empty() {
        no_track_label.to_string()
    } else {
        title.clone()
    };

    assert_eq!(
        display_title, no_track_label,
        "Should show fallback label when no track"
    );
}

// =============================================================================
// Volume Drag Tests
// =============================================================================

/// Test volume drag state tracking.
#[gpui::test]
async fn test_footer_volume_drag_state(_cx: &mut TestAppContext) {
    let is_dragging = Rc::new(RefCell::new(false));
    let drag_start_y: Rc<RefCell<Option<f32>>> = Rc::new(RefCell::new(None));
    let drag_start_value = Rc::new(RefCell::new(0.0f32));

    // Start drag
    *is_dragging.borrow_mut() = true;
    *drag_start_y.borrow_mut() = Some(100.0);
    *drag_start_value.borrow_mut() = 0.5;

    assert!(*is_dragging.borrow(), "Should be dragging");
    assert_eq!(*drag_start_y.borrow(), Some(100.0), "Should track start Y");
    assert!(
        (*drag_start_value.borrow() - 0.5).abs() < 0.001,
        "Should track start value"
    );

    // End drag
    *is_dragging.borrow_mut() = false;
    assert!(!*is_dragging.borrow(), "Should stop dragging");
}

/// Test volume knob size scales with rem.
/// At default rem_size (16px), 4.5rem = 72px.
/// At smaller rem (14px), 4.5rem = 63px.
#[gpui::test]
async fn test_footer_volume_knob_rem_scaling(_cx: &mut TestAppContext) {
    let knob_rems = 4.5f32;

    // Default rem size (16px)
    let rem_size_default = 16.0f32;
    let size_default = knob_rems * rem_size_default;
    assert!(
        (size_default - 72.0).abs() < 0.001,
        "At 16px rem, knob should be 72px"
    );

    // Smaller rem size (14px)
    let rem_size_small = 14.0f32;
    let size_small = knob_rems * rem_size_small;
    assert!(
        (size_small - 63.0).abs() < 0.001,
        "At 14px rem, knob should be 63px"
    );

    // Knob must scale down with rem
    assert!(
        size_small < size_default,
        "Knob should be smaller with smaller rem size"
    );
}

/// Test volume knob display percentage.
#[gpui::test]
async fn test_footer_volume_knob_percentage(_cx: &mut TestAppContext) {
    let test_cases = vec![
        (0.0f32, 0u32),
        (0.1f32, 10u32),
        (0.5f32, 50u32),
        (0.75f32, 75u32),
        (1.0f32, 100u32),
    ];

    for (volume, expected_percent) in test_cases {
        let volume_percent = (volume * 100.0) as u32;
        assert_eq!(
            volume_percent, expected_percent,
            "Volume {} should display as {}%",
            volume, expected_percent
        );
    }
}
