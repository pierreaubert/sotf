use super::types::InvalidTransition;
use super::types::PlaybackEvent;
use super::types::PlaybackState;

/// Playback state machine
#[derive(Debug)]
pub struct PlaybackStateMachine {
    pub state: PlaybackState,
    pub volume: f32,
    pub position: f64,
    pub track_loaded: bool,
}

impl Default for PlaybackStateMachine {
    fn default() -> Self {
        Self {
            state: PlaybackState::Idle,
            volume: 1.0,
            position: 0.0,
            track_loaded: false,
        }
    }
}

impl PlaybackStateMachine {
    /// Attempt a state transition. Returns error if transition is invalid.
    pub fn transition(&mut self, event: PlaybackEvent) -> Result<PlaybackState, InvalidTransition> {
        let new_state = match (self.state, event) {
            // From Idle
            (PlaybackState::Idle, PlaybackEvent::LoadTrack) => {
                self.track_loaded = true;
                PlaybackState::Loaded
            }
            (PlaybackState::Idle, _) => {
                return Err(InvalidTransition {
                    from: self.state,
                    event,
                    reason: "Cannot perform action without loading track",
                });
            }

            // From Loaded
            (PlaybackState::Loaded, PlaybackEvent::Play) => PlaybackState::Playing,
            (PlaybackState::Loaded, PlaybackEvent::Stop) => {
                self.track_loaded = false;
                self.position = 0.0;
                PlaybackState::Idle
            }

            // From Playing
            (PlaybackState::Playing, PlaybackEvent::Pause) => PlaybackState::Paused,
            (PlaybackState::Playing, PlaybackEvent::Seek) => PlaybackState::Seeking,
            (PlaybackState::Playing, PlaybackEvent::Stop) => {
                self.track_loaded = false;
                self.position = 0.0;
                PlaybackState::Idle
            }
            (PlaybackState::Playing, PlaybackEvent::EndOfTrack) => PlaybackState::Loaded,
            (PlaybackState::Playing, PlaybackEvent::EndOfQueue) => {
                self.track_loaded = false;
                PlaybackState::Idle
            }

            // From Paused
            (PlaybackState::Paused, PlaybackEvent::Resume) => PlaybackState::Playing,
            (PlaybackState::Paused, PlaybackEvent::Play) => PlaybackState::Playing,
            (PlaybackState::Paused, PlaybackEvent::Seek) => PlaybackState::Seeking,
            (PlaybackState::Paused, PlaybackEvent::Stop) => {
                self.track_loaded = false;
                self.position = 0.0;
                PlaybackState::Idle
            }

            // From Seeking
            (PlaybackState::Seeking, PlaybackEvent::SeekComplete) => PlaybackState::Playing,

            // Invalid transitions
            (from, event) => {
                return Err(InvalidTransition {
                    from,
                    event,
                    reason: "Invalid state transition",
                });
            }
        };

        self.state = new_state;
        Ok(new_state)
    }

    /// Check if volume is preserved (should always be true)
    pub fn verify_volume_preserved(&self, expected: f32) -> bool {
        (self.volume - expected).abs() < f32::EPSILON
    }
}
