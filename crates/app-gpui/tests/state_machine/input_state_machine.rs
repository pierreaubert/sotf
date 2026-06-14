use super::types::InputEvent;
use super::types::InputState;

/// Input state machine
#[derive(Debug)]
pub struct InputStateMachine {
    pub state: InputState,
    pub buffer: String,
}

impl Default for InputStateMachine {
    fn default() -> Self {
        Self {
            state: InputState::Normal,
            buffer: String::new(),
        }
    }
}

impl InputStateMachine {
    pub fn transition(&mut self, event: InputEvent) -> Result<InputState, &'static str> {
        let new_state = match (self.state, event) {
            // From Normal
            (InputState::Normal, InputEvent::PressSlash) => {
                self.buffer.clear();
                InputState::Search
            }
            (InputState::Normal, _) => InputState::Normal, // Normal mode accepts all

            // From Search
            (InputState::Search, InputEvent::PressEscape) => {
                self.buffer.clear();
                InputState::Normal
            }
            (InputState::Search, InputEvent::TypeCharacter) => InputState::Search,
            (InputState::Search, InputEvent::Confirm) => InputState::Normal,

            // From AddDirectory
            (InputState::AddDirectory, InputEvent::PressEscape) => {
                self.buffer.clear();
                InputState::Normal
            }
            (InputState::AddDirectory, InputEvent::Confirm) => InputState::Normal,

            // Similar patterns for other modes...
            (_state, InputEvent::PressEscape) => {
                self.buffer.clear();
                InputState::Normal
            }

            (state, _) => state, // Default: stay in current state
        };

        self.state = new_state;
        Ok(new_state)
    }

    pub fn is_text_input_mode(&self) -> bool {
        !matches!(self.state, InputState::Normal)
    }
}
