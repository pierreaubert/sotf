//! Event types and simulation infrastructure.
//!
//! These types model the GPUI event system for testing purposes.
//! They can be used both for state-level testing and (when the GPUI
//! macro issue is resolved) for full event integration testing.

use std::collections::VecDeque;

// =============================================================================
// Simulated Event Types
// =============================================================================

/// Keyboard modifiers
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub cmd: bool,
}

impl Modifiers {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn shift() -> Self {
        Self {
            shift: true,
            ..Default::default()
        }
    }

    pub fn ctrl() -> Self {
        Self {
            ctrl: true,
            ..Default::default()
        }
    }

    pub fn cmd() -> Self {
        Self {
            cmd: true,
            ..Default::default()
        }
    }
}

/// Simulated key event
#[derive(Debug, Clone)]
pub struct KeyEvent {
    pub key: String,
    pub modifiers: Modifiers,
    pub is_repeat: bool,
}

impl KeyEvent {
    pub fn new(key: &str) -> Self {
        Self {
            key: key.to_string(),
            modifiers: Modifiers::default(),
            is_repeat: false,
        }
    }

    pub fn with_modifiers(mut self, modifiers: Modifiers) -> Self {
        self.modifiers = modifiers;
        self
    }

    pub fn with_shift(mut self) -> Self {
        self.modifiers.shift = true;
        self
    }

    pub fn with_ctrl(mut self) -> Self {
        self.modifiers.ctrl = true;
        self
    }

    pub fn with_cmd(mut self) -> Self {
        self.modifiers.cmd = true;
        self
    }
}

/// Mouse button
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Simulated mouse event
#[derive(Debug, Clone)]
pub struct MouseEvent {
    pub x: f32,
    pub y: f32,
    pub button: MouseButton,
    pub event_type: MouseEventType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseEventType {
    Down,
    Up,
    Click,
    Move,
}

// =============================================================================
// Event Handler Recording
// =============================================================================

/// Record of a handler being called
#[derive(Debug, Clone)]
pub struct HandlerRecord {
    pub name: String,
    pub consumed: bool,
    pub timestamp_ms: u64,
}

/// Record of a state change
#[derive(Debug, Clone)]
pub struct StateChangeRecord {
    pub field: String,
    pub old_value: String,
    pub new_value: String,
    pub timestamp_ms: u64,
}

/// Event trace for debugging and verification
#[derive(Debug, Default)]
pub struct EventTrace {
    pub handlers: Vec<HandlerRecord>,
    pub state_changes: Vec<StateChangeRecord>,
    pub events: Vec<String>,
    timestamp_counter: u64,
}

impl EventTrace {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_handler(&mut self, name: &str, consumed: bool) {
        self.timestamp_counter += 1;
        let timestamp_ms = self.timestamp_counter;
        self.handlers.push(HandlerRecord {
            name: name.to_string(),
            consumed,
            timestamp_ms,
        });
    }

    pub fn record_state_change(&mut self, field: &str, old: &str, new: &str) {
        self.timestamp_counter += 1;
        let timestamp_ms = self.timestamp_counter;
        self.state_changes.push(StateChangeRecord {
            field: field.to_string(),
            old_value: old.to_string(),
            new_value: new.to_string(),
            timestamp_ms,
        });
    }

    pub fn record_event(&mut self, event: &str) {
        self.events.push(event.to_string());
    }

    #[allow(dead_code)]
    fn next_timestamp(&mut self) -> u64 {
        self.timestamp_counter += 1;
        self.timestamp_counter
    }

    pub fn handler_called(&self, name: &str) -> bool {
        self.handlers.iter().any(|h| h.name == name)
    }

    pub fn handler_count(&self, name: &str) -> usize {
        self.handlers.iter().filter(|h| h.name == name).count()
    }

    pub fn state_changed(&self, field: &str) -> bool {
        self.state_changes.iter().any(|s| s.field == field)
    }

    pub fn last_state_value(&self, field: &str) -> Option<&str> {
        self.state_changes
            .iter()
            .rev()
            .find(|s| s.field == field)
            .map(|s| s.new_value.as_str())
    }

    pub fn clear(&mut self) {
        self.handlers.clear();
        self.state_changes.clear();
        self.events.clear();
    }
}

// =============================================================================
// Event Queue for Simulation
// =============================================================================

/// Queue of events to be processed
#[derive(Debug, Default)]
pub struct EventQueue {
    keys: VecDeque<KeyEvent>,
    mouse: VecDeque<MouseEvent>,
}

impl EventQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_key(&mut self, event: KeyEvent) {
        self.keys.push_back(event);
    }

    pub fn push_keystroke(&mut self, key: &str) {
        self.keys.push_back(KeyEvent::new(key));
    }

    pub fn push_text(&mut self, text: &str) {
        for c in text.chars() {
            self.keys.push_back(KeyEvent::new(&c.to_string()));
        }
    }

    pub fn pop_key(&mut self) -> Option<KeyEvent> {
        self.keys.pop_front()
    }

    pub fn push_mouse(&mut self, event: MouseEvent) {
        self.mouse.push_back(event);
    }

    pub fn push_click(&mut self, x: f32, y: f32) {
        self.mouse.push_back(MouseEvent {
            x,
            y,
            button: MouseButton::Left,
            event_type: MouseEventType::Click,
        });
    }

    pub fn pop_mouse(&mut self) -> Option<MouseEvent> {
        self.mouse.pop_front()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty() && self.mouse.is_empty()
    }

    pub fn clear(&mut self) {
        self.keys.clear();
        self.mouse.clear();
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_event_builder() {
        let event = KeyEvent::new("a").with_shift().with_ctrl();

        assert_eq!(event.key, "a");
        assert!(event.modifiers.shift);
        assert!(event.modifiers.ctrl);
        assert!(!event.modifiers.alt);
        assert!(!event.modifiers.cmd);
    }

    #[test]
    fn test_event_trace_recording() {
        let mut trace = EventTrace::new();

        trace.record_handler("handle_key_down", true);
        trace.record_handler("toggle_search", true);
        trace.record_state_change("input_mode", "Normal", "Search");

        assert!(trace.handler_called("handle_key_down"));
        assert!(trace.handler_called("toggle_search"));
        assert!(!trace.handler_called("nonexistent"));

        assert!(trace.state_changed("input_mode"));
        assert_eq!(trace.last_state_value("input_mode"), Some("Search"));
    }

    #[test]
    fn test_event_trace_counting() {
        let mut trace = EventTrace::new();

        for _ in 0..5 {
            trace.record_handler("volume_up", true);
        }
        trace.record_handler("play_pause", true);

        assert_eq!(trace.handler_count("volume_up"), 5);
        assert_eq!(trace.handler_count("play_pause"), 1);
        assert_eq!(trace.handler_count("nonexistent"), 0);
    }

    #[test]
    fn test_event_queue() {
        let mut queue = EventQueue::new();

        queue.push_keystroke("a");
        queue.push_keystroke("b");
        queue.push_text("cd");

        assert!(!queue.is_empty());

        let events: Vec<_> = std::iter::from_fn(|| queue.pop_key())
            .map(|e| e.key)
            .collect();

        assert_eq!(events, vec!["a", "b", "c", "d"]);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_event_queue_mouse() {
        let mut queue = EventQueue::new();

        queue.push_click(100.0, 200.0);

        let event = queue.pop_mouse().unwrap();
        assert_eq!(event.x, 100.0);
        assert_eq!(event.y, 200.0);
        assert_eq!(event.button, MouseButton::Left);
        assert_eq!(event.event_type, MouseEventType::Click);
    }
}
