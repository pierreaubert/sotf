#![allow(clippy::field_reassign_with_default)]
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

mod input_state_machine;
mod playback_state_machine;
#[cfg(test)]
mod tests;
mod types;

pub use input_state_machine::*;
pub use playback_state_machine::*;
pub use types::*;
