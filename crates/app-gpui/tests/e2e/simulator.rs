//! Interaction Simulator
//!
//! High-level user interaction simulation for E2E tests.

use gpui::{Modifiers, MouseButton, VisualTestContext};

/// Simulator for user interactions with the application.
pub struct InteractionSimulator<'a> {
    cx: &'a mut VisualTestContext,
}

impl<'a> InteractionSimulator<'a> {
    /// Create a new interaction simulator.
    pub fn new(cx: &'a mut VisualTestContext) -> Self {
        Self { cx }
    }

    /// Simulate a click at a specific position.
    pub fn simulate_click_at(&mut self, position: gpui::Point<gpui::Pixels>) {
        self.cx
            .simulate_mouse_down(position, MouseButton::Left, Modifiers::default());
        self.cx
            .simulate_mouse_up(position, MouseButton::Left, Modifiers::default());
        self.cx.run_until_parked();
    }

    /// Simulate pressing a key.
    pub fn press_key(&mut self, key: &str) {
        self.cx.simulate_keystrokes(key);
        self.cx.run_until_parked();
    }
}
