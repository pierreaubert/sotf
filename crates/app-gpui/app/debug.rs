//! Debug utilities for state transition tracking and action logging.
//!
//! Provides:
//! - StateHistory: Captures state snapshots before major transitions
//! - Logging helpers for InputMode, Screen, and AudioDevice changes
//! - Action dispatch logging with blocked action tracking

use std::collections::VecDeque;
use std::time::Instant;

use crate::app::types::{InputMode, Screen};

/// Maximum number of state snapshots to retain
pub const MAX_HISTORY_SIZE: usize = 100;

/// A snapshot of important state at a point in time
#[derive(Debug, Clone)]
pub struct StateSnapshot {
    pub timestamp: Instant,
    pub screen: Screen,
    pub input_mode: InputMode,
    pub output_device: Option<String>,
    pub trigger: String,
}

/// Tracks state history for debugging
#[derive(Debug)]
pub struct StateHistory {
    history: VecDeque<StateSnapshot>,
}

impl Default for StateHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl StateHistory {
    pub fn new() -> Self {
        Self {
            history: VecDeque::with_capacity(MAX_HISTORY_SIZE),
        }
    }

    /// Capture a state snapshot with the given trigger description
    pub fn capture(
        &mut self,
        screen: Screen,
        input_mode: InputMode,
        output_device: Option<String>,
        trigger: impl Into<String>,
    ) {
        if self.history.len() >= MAX_HISTORY_SIZE {
            self.history.pop_front();
        }

        let snapshot = StateSnapshot {
            timestamp: Instant::now(),
            screen,
            input_mode,
            output_device,
            trigger: trigger.into(),
        };

        log::debug!(
            "[StateHistory] Captured: screen={:?}, input_mode={:?}, device={:?}, trigger={}",
            snapshot.screen,
            snapshot.input_mode,
            snapshot.output_device,
            snapshot.trigger
        );

        self.history.push_back(snapshot);
    }

    /// Get the last N snapshots
    pub fn last_n(&self, n: usize) -> Vec<&StateSnapshot> {
        self.history.iter().rev().take(n).collect()
    }

    /// Get all snapshots
    pub fn all(&self) -> impl Iterator<Item = &StateSnapshot> {
        self.history.iter()
    }

    /// Clear all history
    pub fn clear(&mut self) {
        self.history.clear();
    }

    /// Get the number of snapshots
    pub fn len(&self) -> usize {
        self.history.len()
    }

    /// Check if history is empty
    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }
}

/// Log an InputMode transition
pub fn log_input_mode_transition(from: InputMode, to: InputMode, trigger: &str) {
    if from != to {
        log::info!("[InputMode] {:?} -> {:?} (trigger: {})", from, to, trigger);
    }
}

/// Log a Screen transition
pub fn log_screen_transition(from: Screen, to: Screen, trigger: &str) {
    if from != to {
        log::info!("[Screen] {:?} -> {:?} (trigger: {})", from, to, trigger);
    }
}

/// Log an audio device selection change
pub fn log_device_change(from: Option<&str>, to: Option<&str>, trigger: &str) {
    if from != to {
        log::info!(
            "[AudioDevice] {:?} -> {:?} (trigger: {})",
            from.unwrap_or("<default>"),
            to.unwrap_or("<default>"),
            trigger
        );
    }
}

/// Log action dispatch
pub fn log_action_dispatch(action_name: &str, handler: &str) {
    log::debug!(
        "[Action] Dispatched '{}' to handler '{}'",
        action_name,
        handler
    );
}

/// Log blocked action (due to input mode)
pub fn log_action_blocked(action_name: &str, input_mode: InputMode, reason: &str) {
    log::debug!(
        "[Action] Blocked '{}' in {:?} mode: {}",
        action_name,
        input_mode,
        reason
    );
}
