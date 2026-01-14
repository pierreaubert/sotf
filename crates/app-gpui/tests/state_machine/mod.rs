//! Phase 5: State Machine Modeling
//!
//! This module models the application state as explicit state machines,
//! enabling exhaustive testing of state transitions and verification
//! that invalid transitions are rejected.
//!
//! # Architecture
//!
//! The application has several interconnected state machines:
//!
//! ## PlaybackStateMachine
//! ```text
//! ┌──────────┐
//! │  Idle    │◄────────────────────────────────┐
//! └────┬─────┘                                 │
//!      │ load_track()                          │
//!      ▼                                       │
//! ┌──────────┐                                 │
//! │  Loaded  │◄─────────────┐                  │
//! └────┬─────┘              │                  │
//!      │ play()             │ stop()           │ end_of_queue()
//!      ▼                    │                  │
//! ┌──────────┐         ┌────┴─────┐            │
//! │ Playing  │◄───────►│  Paused  │            │
//! └────┬─────┘ pause() └──────────┘            │
//!      │ resume()                              │
//!      │ end_of_track()                        │
//!      ▼                                       │
//! ┌──────────┐                                 │
//! │ Seeking  │ (temporary state)               │
//! └────┬─────┘                                 │
//!      │ seek_complete()                       │
//!      └───────────────────────────────────────┘
//! ```
//!
//! ## InputStateMachine
//! ```text
//! ┌──────────┐
//! │  Normal  │◄─────────────────────────────────┐
//! └────┬─────┘                                  │
//!      │ '/'                                    │ ESC
//!      ▼                                        │
//! ┌──────────┐                                  │
//! │  Search  │──────────────────────────────────┘
//! └──────────┘
//!
//!      │ 'a' (add dir)
//!      ▼
//! ┌──────────────┐
//! │ AddDirectory │──► ESC ──► Normal
//! └──────────────┘
//!
//! (similar for SavePlugins, LoadPlugins, EditingParam)
//! ```
//!
//! ## LibraryViewStateMachine
//! ```text
//! ┌─────────────┐
//! │   Browse    │◄────────────────────────────┐
//! └─────┬───────┘                             │
//!       │ apply_filter()                      │ clear_all()
//!       ▼                                     │
//! ┌─────────────┐                             │
//! │  Filtered   │─────────────────────────────┘
//! └─────┬───────┘
//!       │ search()
//!       ▼
//! ┌─────────────┐
//! │  Searching  │──► clear_search() ──► Filtered or Browse
//! └─────────────┘
//! ```
//!
//! # Implementation Approach
//!
//! 1. Define state machine types with explicit states and transitions
//! 2. Implement transition functions that return Result<NewState, InvalidTransition>
//! 3. Generate all possible transition sequences
//! 4. Verify each sequence maintains valid state
//! 5. Test that invalid transitions are rejected

#[path = "../common/mod.rs"]
mod common;

// =============================================================================
// Playback State Machine
// =============================================================================

/// Playback states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Idle,
    Loaded,
    Playing,
    Paused,
    Seeking,
}

/// Playback events/transitions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackEvent {
    LoadTrack,
    Play,
    Pause,
    Resume,
    Stop,
    Seek,
    SeekComplete,
    EndOfTrack,
    EndOfQueue,
}

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

/// Error for invalid state transitions
#[derive(Debug)]
pub struct InvalidTransition {
    pub from: PlaybackState,
    pub event: PlaybackEvent,
    pub reason: &'static str,
}

// =============================================================================
// Input State Machine
// =============================================================================

/// Input mode states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputState {
    Normal,
    Search,
    AddDirectory,
    SavePlugins,
    LoadPlugins,
    EditingParam,
}

/// Input events
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    PressSlash,
    PressEscape,
    PressA, // For add directory mode
    TypeCharacter,
    Confirm,
}

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
            (state, InputEvent::PressEscape) => {
                self.buffer.clear();
                if state == InputState::Normal {
                    InputState::Normal
                } else {
                    InputState::Normal
                }
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

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Playback State Machine Tests
    // =========================================================================

    #[test]
    fn test_playback_valid_sequence() {
        let mut sm = PlaybackStateMachine::default();
        sm.volume = 0.5;

        // Valid sequence: Idle → Loaded → Playing → Paused → Playing → Stop
        assert!(sm.transition(PlaybackEvent::LoadTrack).is_ok());
        assert_eq!(sm.state, PlaybackState::Loaded);
        assert!(sm.verify_volume_preserved(0.5));

        assert!(sm.transition(PlaybackEvent::Play).is_ok());
        assert_eq!(sm.state, PlaybackState::Playing);
        assert!(sm.verify_volume_preserved(0.5));

        assert!(sm.transition(PlaybackEvent::Pause).is_ok());
        assert_eq!(sm.state, PlaybackState::Paused);
        assert!(sm.verify_volume_preserved(0.5));

        assert!(sm.transition(PlaybackEvent::Resume).is_ok());
        assert_eq!(sm.state, PlaybackState::Playing);
        assert!(sm.verify_volume_preserved(0.5));

        assert!(sm.transition(PlaybackEvent::Stop).is_ok());
        assert_eq!(sm.state, PlaybackState::Idle);
        assert!(sm.verify_volume_preserved(0.5));
    }

    #[test]
    fn test_playback_invalid_transitions() {
        let mut sm = PlaybackStateMachine::default();

        // Cannot play from Idle (no track loaded)
        let result = sm.transition(PlaybackEvent::Play);
        assert!(result.is_err());

        // Cannot pause from Idle
        let result = sm.transition(PlaybackEvent::Pause);
        assert!(result.is_err());

        // Cannot seek from Idle
        let result = sm.transition(PlaybackEvent::Seek);
        assert!(result.is_err());
    }

    #[test]
    fn test_playback_seek_sequence() {
        let mut sm = PlaybackStateMachine::default();
        sm.transition(PlaybackEvent::LoadTrack).unwrap();
        sm.transition(PlaybackEvent::Play).unwrap();

        // Enter seeking state
        assert!(sm.transition(PlaybackEvent::Seek).is_ok());
        assert_eq!(sm.state, PlaybackState::Seeking);

        // Complete seek
        assert!(sm.transition(PlaybackEvent::SeekComplete).is_ok());
        assert_eq!(sm.state, PlaybackState::Playing);
    }

    #[test]
    fn test_playback_end_of_track() {
        let mut sm = PlaybackStateMachine::default();
        sm.transition(PlaybackEvent::LoadTrack).unwrap();
        sm.transition(PlaybackEvent::Play).unwrap();

        // End of track returns to Loaded (ready for next track)
        assert!(sm.transition(PlaybackEvent::EndOfTrack).is_ok());
        assert_eq!(sm.state, PlaybackState::Loaded);
    }

    #[test]
    fn test_playback_end_of_queue() {
        let mut sm = PlaybackStateMachine::default();
        sm.transition(PlaybackEvent::LoadTrack).unwrap();
        sm.transition(PlaybackEvent::Play).unwrap();

        // End of queue returns to Idle
        assert!(sm.transition(PlaybackEvent::EndOfQueue).is_ok());
        assert_eq!(sm.state, PlaybackState::Idle);
    }

    #[test]
    fn test_playback_volume_preserved_through_all_transitions() {
        let mut sm = PlaybackStateMachine::default();
        sm.volume = 0.42;

        let events = [
            PlaybackEvent::LoadTrack,
            PlaybackEvent::Play,
            PlaybackEvent::Pause,
            PlaybackEvent::Resume,
            PlaybackEvent::Seek,
            PlaybackEvent::SeekComplete,
            PlaybackEvent::Stop,
        ];

        for event in events {
            let _ = sm.transition(event);
            assert!(
                sm.verify_volume_preserved(0.42),
                "Volume changed after {:?}",
                event
            );
        }
    }

    // =========================================================================
    // Input State Machine Tests
    // =========================================================================

    #[test]
    fn test_input_search_cycle() {
        let mut sm = InputStateMachine::default();

        // Enter search
        sm.transition(InputEvent::PressSlash).unwrap();
        assert_eq!(sm.state, InputState::Search);

        // Type characters
        sm.transition(InputEvent::TypeCharacter).unwrap();
        assert_eq!(sm.state, InputState::Search);

        // Exit with escape
        sm.transition(InputEvent::PressEscape).unwrap();
        assert_eq!(sm.state, InputState::Normal);
    }

    #[test]
    fn test_input_text_mode_detection() {
        let mut sm = InputStateMachine::default();

        assert!(!sm.is_text_input_mode());

        sm.transition(InputEvent::PressSlash).unwrap();
        assert!(sm.is_text_input_mode());

        sm.transition(InputEvent::PressEscape).unwrap();
        assert!(!sm.is_text_input_mode());
    }

    #[test]
    fn test_input_escape_from_all_modes() {
        let modes = [
            InputState::Search,
            InputState::AddDirectory,
            InputState::SavePlugins,
            InputState::LoadPlugins,
            InputState::EditingParam,
        ];

        for mode in modes {
            let mut sm = InputStateMachine {
                state: mode,
                buffer: "test".to_string(),
            };

            sm.transition(InputEvent::PressEscape).unwrap();
            assert_eq!(
                sm.state,
                InputState::Normal,
                "Escape didn't exit {:?}",
                mode
            );
            assert!(sm.buffer.is_empty(), "Buffer not cleared for {:?}", mode);
        }
    }

    // =========================================================================
    // Exhaustive Transition Tests
    // =========================================================================

    #[test]
    fn test_all_playback_transitions_documented() {
        // This test verifies that all state/event combinations are handled
        let states = [
            PlaybackState::Idle,
            PlaybackState::Loaded,
            PlaybackState::Playing,
            PlaybackState::Paused,
            PlaybackState::Seeking,
        ];

        let events = [
            PlaybackEvent::LoadTrack,
            PlaybackEvent::Play,
            PlaybackEvent::Pause,
            PlaybackEvent::Resume,
            PlaybackEvent::Stop,
            PlaybackEvent::Seek,
            PlaybackEvent::SeekComplete,
            PlaybackEvent::EndOfTrack,
            PlaybackEvent::EndOfQueue,
        ];

        // Every combination should either succeed or return a proper error
        // (not panic)
        for state in states {
            for event in events {
                let mut sm = PlaybackStateMachine::default();
                sm.state = state;
                sm.track_loaded = state != PlaybackState::Idle;

                // This should not panic
                let _result = sm.transition(event);
            }
        }
    }
}

// =============================================================================
// Future: Property-Based State Machine Testing
// =============================================================================

/*
When implementing full state machine testing:

1. Use proptest to generate random event sequences:
   ```rust
   proptest! {
       #[test]
       fn random_event_sequence_never_panics(
           events in prop::collection::vec(playback_event_strategy(), 0..100)
       ) {
           let mut sm = PlaybackStateMachine::default();
           for event in events {
               let _ = sm.transition(event); // Should never panic
           }
       }
   }
   ```

2. Verify invariants hold after any transition:
   - Volume is preserved
   - State is always valid
   - track_loaded matches state expectations

3. Generate and test all reachable states:
   - BFS/DFS from initial state
   - Verify no unreachable states
   - Verify no dead-end states (except Idle)

4. Test state machine composition:
   - Multiple state machines interact correctly
   - No conflicting state combinations
*/
