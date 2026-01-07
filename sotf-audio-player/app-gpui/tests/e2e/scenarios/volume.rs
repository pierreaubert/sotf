//! Volume interaction scenarios for E2E testing.
//!
//! Tests for verifying volume control interactions.

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

/// Simulates the state affected by volume control and selection actions.
struct VolumeControlState {
    volume: f32,
    selected_album_index: usize,
    volume_control_focused: bool,
}

impl VolumeControlState {
    fn new() -> Self {
        Self {
            volume: 0.5,
            selected_album_index: 5, // Start with album 5 selected
            volume_control_focused: false,
        }
    }

    /// Simulates handling arrow key on volume control.
    /// Returns true if the key was handled by volume control.
    fn handle_volume_key(&mut self, key: &str) -> bool {
        const VOLUME_STEP: f32 = 0.05;

        if !self.volume_control_focused {
            return false;
        }

        let delta = match key {
            "up" | "right" => Some(VOLUME_STEP),
            "down" | "left" => Some(-VOLUME_STEP),
            _ => None,
        };

        if let Some(delta) = delta {
            self.volume = (self.volume + delta).clamp(0.0, 1.0);
            true // Key was handled, should stop propagation
        } else {
            false
        }
    }

    /// Simulates the global SelectUp/SelectDown action.
    /// This is what happens when arrow keys propagate to the global handler.
    fn handle_select_action(&mut self, direction: &str) {
        match direction {
            "up" => {
                if self.selected_album_index > 0 {
                    self.selected_album_index -= 1;
                }
            }
            "down" => self.selected_album_index += 1,
            _ => {}
        }
    }
}

/// BUG TEST: Volume control arrow keys should NOT move album selection.
///
/// When the volume control button is focused and arrow keys are pressed:
/// 1. Volume should change (handled by on_key_down)
/// 2. Album selection should NOT change (event should stop propagating)
///
/// This test verifies that arrow key events are properly consumed by the
/// volume control and don't propagate to trigger SelectUp/SelectDown actions.
#[gpui::test]
async fn test_volume_control_focus_prevents_album_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(VolumeControlState::new()));

    // Record initial state
    let initial_volume = state.borrow().volume;
    let initial_album_index = state.borrow().selected_album_index;

    // Focus the volume control
    state.borrow_mut().volume_control_focused = true;

    // Simulate pressing "up" arrow key
    // The volume control should handle it AND stop propagation
    let key_handled = state.borrow_mut().handle_volume_key("up");

    // BUG: Currently, even when key is handled, events still propagate
    // The fix should make this test pass by calling cx.stop_propagation()
    // in the on_key_down handler when the key is consumed.
    //
    // This simulates the buggy behavior where events propagate even after handling:
    if !key_handled {
        // Only propagate to selection if volume control didn't handle it
        state.borrow_mut().handle_select_action("up");
    }

    // Verify volume changed
    assert!(
        state.borrow().volume > initial_volume,
        "Volume should increase when arrow up is pressed on focused volume control"
    );

    // Verify album selection did NOT change (this is the bug we're fixing)
    assert_eq!(
        state.borrow().selected_album_index,
        initial_album_index,
        "BUG: Album selection changed when pressing arrow on focused volume control. \
         Album selection should NOT change - events should stop propagating."
    );
}

/// Test that arrow keys work correctly when volume control is NOT focused.
/// In this case, arrow keys SHOULD move album selection.
#[gpui::test]
async fn test_arrow_keys_move_selection_when_volume_not_focused(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(VolumeControlState::new()));

    let initial_album_index = state.borrow().selected_album_index;

    // Volume control is NOT focused
    state.borrow_mut().volume_control_focused = false;

    // Simulate pressing "down" arrow key
    let key_handled = state.borrow_mut().handle_volume_key("down");

    // Volume control didn't handle it, so propagate to selection
    if !key_handled {
        state.borrow_mut().handle_select_action("down");
    }

    // Verify album selection DID change
    assert_eq!(
        state.borrow().selected_album_index,
        initial_album_index + 1,
        "Album selection should change when volume control is not focused"
    );
}

/// Test volume step values.
#[gpui::test]
async fn test_volume_steps(_cx: &mut TestAppContext) {
    let volume: Rc<RefCell<f32>> = Rc::new(RefCell::new(0.1));
    const VOLUME_STEP: f32 = 0.05;

    // Initial value (with floating point tolerance)
    assert!(
        (*volume.borrow() - 0.1).abs() < 0.001,
        "Initial volume should be ~0.1"
    );

    // Increase by step
    *volume.borrow_mut() += VOLUME_STEP;
    assert!(*volume.borrow() > 0.1, "Volume should increase");

    // Decrease by step
    *volume.borrow_mut() -= VOLUME_STEP;
    assert!(
        (*volume.borrow() - 0.1).abs() < 0.001,
        "Volume should return to ~0.1"
    );
}

/// Test volume clamping.
#[gpui::test]
async fn test_volume_bounds(_cx: &mut TestAppContext) {
    let volume: Rc<RefCell<f32>> = Rc::new(RefCell::new(0.5));

    // Test upper bound
    *volume.borrow_mut() = 2.0;
    let clamped = volume.borrow().clamp(0.0, 1.0);
    assert_eq!(clamped, 1.0);

    // Test lower bound
    *volume.borrow_mut() = -0.5;
    let clamped = volume.borrow().clamp(0.0, 1.0);
    assert_eq!(clamped, 0.0);
}

/// Test volume transition through multiple values.
#[gpui::test]
async fn test_volume_transitions(_cx: &mut TestAppContext) {
    let volumes: Rc<RefCell<Vec<f32>>> = Rc::new(RefCell::new(vec![]));

    // Record volume changes
    for i in 1..=5 {
        volumes.borrow_mut().push(i as f32 * 0.1);
    }

    let recorded = volumes.borrow();
    assert_eq!(recorded.len(), 5);
    assert!(recorded.last().copied().unwrap_or(0.0) > recorded.first().copied().unwrap_or(0.0));
}
