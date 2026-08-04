//! Deterministic control-plane helpers for manager and device tests.

use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A manually advanced clock for timeout and scheduling tests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct ManualClock {
    elapsed: Duration,
}

impl ManualClock {
    /// Return the current test time.
    pub fn now(self) -> Duration {
        self.elapsed
    }

    /// Advance the clock without sleeping the test thread.
    pub fn advance(&mut self, duration: Duration) {
        self.elapsed = self.elapsed.saturating_add(duration);
    }
}

/// Thread-safe ordered event trace for actor and orchestration tests.
#[derive(Debug, Clone, Default)]
pub struct EventTrace {
    events: Arc<Mutex<Vec<String>>>,
}

impl EventTrace {
    /// Record one event in the trace.
    pub fn record(&self, event: impl Into<String>) {
        self.events
            .lock()
            .expect("event trace lock poisoned")
            .push(event.into());
    }

    /// Return a stable copy of all events recorded so far.
    pub fn snapshot(&self) -> Vec<String> {
        self.events
            .lock()
            .expect("event trace lock poisoned")
            .clone()
    }

    /// Remove all events and return them in recording order.
    pub fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.events.lock().expect("event trace lock poisoned"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_clock_advances_without_sleeping() {
        let mut clock = ManualClock::default();
        clock.advance(Duration::from_millis(25));
        clock.advance(Duration::from_millis(5));
        assert_eq!(clock.now(), Duration::from_millis(30));
    }

    #[test]
    fn event_trace_preserves_order_and_can_be_taken() {
        let trace = EventTrace::default();
        trace.record("start");
        trace.record("stop");
        assert_eq!(trace.snapshot(), ["start", "stop"]);
        assert_eq!(trace.take(), ["start", "stop"]);
        assert!(trace.snapshot().is_empty());
    }
}
