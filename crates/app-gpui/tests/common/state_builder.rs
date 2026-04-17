//! Behavioral specification models for input mode testing.
//!
//! These are NOT mirrors of production types — they model expected keyboard
//! dispatch behavior as a test specification. The real key processing goes
//! through GPUI's event system, which requires `#[gpui::test]`.
//!
//! For production type testing, use:
//! - `sotf_audio_player::controllers::playback::PlaybackController`
//! - `sotf_audio_player::controllers::queue::QueueController`
//! - `sotf_audio_player::LibraryController`
//! - `sotf_audio_player_gpui::InputMode`

/// Behavioral model of input mode state for testing key dispatch logic.
#[derive(Debug, Clone)]
pub struct TestInputState {
    pub input_mode: TestInputMode,
    pub search_query: String,
    pub triggered_actions: Vec<TestAction>,
}

impl Default for TestInputState {
    fn default() -> Self {
        Self {
            input_mode: TestInputMode::Normal,
            search_query: String::new(),
            triggered_actions: Vec::new(),
        }
    }
}

impl TestInputState {
    pub fn enter_input_mode(&mut self, mode: TestInputMode) {
        self.input_mode = mode;
    }

    pub fn exit_input_mode(&mut self) {
        self.input_mode = TestInputMode::Normal;
    }

    /// Process a key press based on current input mode.
    /// Returns true if the key was consumed by the current mode.
    pub fn process_key(&mut self, key: char) -> bool {
        match self.input_mode {
            TestInputMode::Search => {
                if key == '\x1b' {
                    self.search_query.clear();
                    self.input_mode = TestInputMode::Normal;
                } else if key == '\x08' {
                    self.search_query.pop();
                } else if !key.is_control() {
                    self.search_query.push(key);
                }
                true
            }
            TestInputMode::Normal => match key {
                '0' => {
                    self.triggered_actions.push(TestAction::SetFilterAll);
                    true
                }
                '1'..='5' => {
                    self.triggered_actions
                        .push(TestAction::SetFilterRating(key.to_digit(10).unwrap() as u8));
                    true
                }
                ' ' => {
                    self.triggered_actions.push(TestAction::PlayPause);
                    true
                }
                '+' | '=' => {
                    self.triggered_actions.push(TestAction::VolumeUp);
                    true
                }
                '-' | '_' => {
                    self.triggered_actions.push(TestAction::VolumeDown);
                    true
                }
                '/' => {
                    self.input_mode = TestInputMode::Search;
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestInputMode {
    Normal,
    Search,
    AddDirectory,
    SavePlugins,
    LoadPlugins,
    EditingParam,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TestAction {
    PlayPause,
    VolumeUp,
    VolumeDown,
    SetFilterAll,
    SetFilterRating(u8),
}
