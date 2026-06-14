use super::super::event_types::*;
use super::types::InputMode;

/// Simulated app state for event handling tests
#[derive(Debug)]
pub struct EventHandlerState {
    pub input_mode: InputMode,
    pub search_query: String,
    pub volume: f32,
    pub is_playing: bool,
    pub filter_rating: Option<u8>,
    pub trace: EventTrace,
}

impl Default for EventHandlerState {
    fn default() -> Self {
        Self {
            input_mode: InputMode::Normal,
            search_query: String::new(),
            volume: 1.0,
            is_playing: false,
            filter_rating: None,
            trace: EventTrace::new(),
        }
    }
}

impl EventHandlerState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if we're in a text input mode
    pub fn is_text_input_mode(&self) -> bool {
        matches!(self.input_mode, InputMode::Search | InputMode::TextEntry)
    }

    /// Process a key event through the simulated handler chain
    pub fn process_key(&mut self, event: &KeyEvent) -> bool {
        self.trace.record_event(&format!("key:{}", event.key));

        // In text input mode, handle keys directly
        if self.is_text_input_mode() {
            return self.handle_text_input(event);
        }

        // In normal mode, check keybindings
        self.handle_keybinding(event)
    }

    /// Handle text input in search/text entry modes
    pub(super) fn handle_text_input(&mut self, event: &KeyEvent) -> bool {
        self.trace.record_handler("handle_text_input", true);

        match event.key.as_str() {
            "escape" => {
                let old_mode = format!("{:?}", self.input_mode);
                self.input_mode = InputMode::Normal;
                self.search_query.clear();
                self.trace
                    .record_state_change("input_mode", &old_mode, "Normal");
                true
            }
            "backspace" => {
                self.search_query.pop();
                true
            }
            "enter" => {
                // Execute search / confirm input
                self.input_mode = InputMode::Normal;
                true
            }
            key if key.len() == 1 && !key.chars().next().unwrap().is_control() => {
                self.search_query.push_str(key);
                true
            }
            _ => false,
        }
    }

    /// Handle keybindings in normal mode
    pub(super) fn handle_keybinding(&mut self, event: &KeyEvent) -> bool {
        let key = event.key.as_str();

        match key {
            "space" => {
                self.trace.record_handler("play_pause", true);
                let old = self.is_playing.to_string();
                self.is_playing = !self.is_playing;
                self.trace
                    .record_state_change("is_playing", &old, &self.is_playing.to_string());
                true
            }
            "+" | "=" => {
                self.trace.record_handler("volume_up", true);
                let old = self.volume.to_string();
                self.volume = (self.volume + 0.1).min(1.0);
                self.trace
                    .record_state_change("volume", &old, &self.volume.to_string());
                true
            }
            "-" | "_" => {
                self.trace.record_handler("volume_down", true);
                let old = self.volume.to_string();
                self.volume = (self.volume - 0.1).max(0.0);
                self.trace
                    .record_state_change("volume", &old, &self.volume.to_string());
                true
            }
            "/" => {
                self.trace.record_handler("toggle_search", true);
                let old_mode = format!("{:?}", self.input_mode);
                self.input_mode = InputMode::Search;
                self.trace
                    .record_state_change("input_mode", &old_mode, "Search");
                true
            }
            "0" => {
                self.trace.record_handler("set_filter_all", true);
                self.filter_rating = None;
                true
            }
            "1" | "2" | "3" | "4" | "5" => {
                let rating = key.parse::<u8>().unwrap();
                self.trace
                    .record_handler(&format!("set_filter_{}", rating), true);
                self.filter_rating = Some(rating);
                true
            }
            "n" => {
                self.trace.record_handler("next_track", true);
                true
            }
            "p" => {
                self.trace.record_handler("prev_track", true);
                true
            }
            _ => false,
        }
    }

    /// Convenience method to process multiple keystrokes
    pub fn process_keystrokes(&mut self, keys: &[&str]) {
        for key in keys {
            self.process_key(&KeyEvent::new(key));
        }
    }

    /// Convenience method to type text
    pub fn type_text(&mut self, text: &str) {
        for c in text.chars() {
            self.process_key(&KeyEvent::new(&c.to_string()));
        }
    }
}
