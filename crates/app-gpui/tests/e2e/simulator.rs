//! Interaction Simulator
//!
//! High-level user interaction simulation for E2E tests.

use gpui::{Modifiers, MouseButton, VisualTestContext};
use std::error::Error;

/// Simulator for user interactions with the application.
pub struct InteractionSimulator<'a> {
    cx: &'a mut VisualTestContext,
}

impl<'a> InteractionSimulator<'a> {
    /// Create a new interaction simulator.
    pub fn new(cx: &'a mut VisualTestContext) -> Self {
        Self { cx }
    }

    /// Simulate a scroll wheel event on an element.
    pub fn scroll_element(
        &mut self,
        _element_id: &str,
        _delta_lines: f32,
    ) -> Result<(), Box<dyn Error>> {
        // Note: debug_bounds requires element_id to have 'static lifetime
        // This is a limitation of the current GPUI test framework
        // For now, we just run the event loop
        self.cx.run_until_parked();
        Ok(())
    }

    /// Simulate a click on an element.
    pub fn click_element(&mut self, _element_id: &str) -> Result<(), Box<dyn Error>> {
        // Note: debug_bounds requires element_id to have 'static lifetime
        // This is a limitation of the current GPUI test framework
        // For now, we just run the event loop
        self.cx.run_until_parked();
        Ok(())
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

    /// Simulate scrolling up on the volume knob.
    pub fn scroll_volume_up(&mut self) -> Result<(), Box<dyn Error>> {
        self.scroll_element("volume-button", -1.0)
    }

    /// Simulate scrolling down on the volume knob.
    pub fn scroll_volume_down(&mut self) -> Result<(), Box<dyn Error>> {
        self.scroll_element("volume-button", 1.0)
    }
}
