//! Event sourcing for playback state.
//!
//! Stores all playback state changes as immutable events, enabling:
//! - Complete audit trail of state changes
//! - Replay/debug of state history
//! - Time-travel debugging
//!
//! # Architecture
//!
//! Events are immutable records of state changes. The current state can be
//! reconstructed by replaying events from any point in time.
//!
//! ```text
//! PlaybackEvent::Started { queue_index: 0 }
//!     -> PlaybackEvent::VolumeChanged { from: 0.7, to: 0.5 }
//!     -> PlaybackEvent::PositionChanged { position: 30.5 }
//!     -> PlaybackEvent::Paused
//!     -> PlaybackEvent::Resumed
//!     -> PlaybackEvent::TrackChanged { queue_index: 1 }
//! ```

use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Maximum number of events to retain in memory
const MAX_EVENTS: usize = 1000;

/// A playback state change event with full context for replay
#[derive(Debug, Clone)]
pub enum PlaybackEvent {
    /// Playback started for a track
    Started {
        queue_index: usize,
        track_path: Option<PathBuf>,
    },
    /// Playback paused
    Paused,
    /// Playback resumed from pause
    Resumed,
    /// Playback stopped completely
    Stopped,
    /// Track changed (next/previous/jump)
    TrackChanged {
        from_index: Option<usize>,
        to_index: usize,
        trigger: TrackChangeTrigger,
    },
    /// Volume changed
    VolumeChanged { from: f32, to: f32 },
    /// Mute state changed
    MuteChanged { muted: bool },
    /// Seek position changed
    Seeked {
        from_position: f64,
        to_position: f64,
    },
    /// Position updated (periodic, not stored for every frame)
    PositionUpdated { position: f64 },
    /// Track duration set (when new track loads)
    DurationSet { duration: f64 },
    /// Queue index changed directly (without track change)
    QueueIndexSet { index: Option<usize> },
    /// Playback ended naturally (track finished)
    TrackEnded { queue_index: usize },
    /// Error occurred during playback
    Error { message: String },
}

/// What triggered a track change
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackChangeTrigger {
    /// User pressed next
    NextTrack,
    /// User pressed previous
    PrevTrack,
    /// Auto-advance at end of track
    AutoAdvance,
    /// User jumped to specific track
    Jump,
    /// Queue was started
    QueueStart,
}

/// A timestamped event record
#[derive(Debug, Clone)]
pub struct EventRecord {
    /// When the event occurred
    pub timestamp: Instant,
    /// Time since playback session started
    pub session_offset: Duration,
    /// The event
    pub event: PlaybackEvent,
    /// Optional context/trigger description
    pub trigger: Option<String>,
}

/// Event store for playback state changes
#[derive(Debug)]
pub struct PlaybackEventStore {
    /// Event history (ring buffer)
    events: VecDeque<EventRecord>,
    /// When the current playback session started
    session_start: Instant,
    /// Whether event recording is enabled
    enabled: bool,
    /// Position update interval (to avoid flooding with position events)
    last_position_update: Option<Instant>,
    /// Minimum interval between position updates
    position_update_interval: Duration,
}

impl Default for PlaybackEventStore {
    fn default() -> Self {
        Self::new()
    }
}

impl PlaybackEventStore {
    pub fn new() -> Self {
        Self {
            events: VecDeque::with_capacity(MAX_EVENTS),
            session_start: Instant::now(),
            enabled: true,
            last_position_update: None,
            position_update_interval: Duration::from_secs(5), // Only log position every 5 seconds
        }
    }

    /// Record an event with optional trigger context
    pub fn record(&mut self, event: PlaybackEvent, trigger: Option<&str>) {
        if !self.enabled {
            return;
        }

        // Throttle position updates to avoid flooding
        if matches!(event, PlaybackEvent::PositionUpdated { .. }) {
            if let Some(last) = self.last_position_update
                && last.elapsed() < self.position_update_interval {
                    return;
                }
            self.last_position_update = Some(Instant::now());
        }

        let now = Instant::now();
        let record = EventRecord {
            timestamp: now,
            session_offset: now.duration_since(self.session_start),
            event: event.clone(),
            trigger: trigger.map(String::from),
        };

        // Log significant events
        match &event {
            PlaybackEvent::Started { queue_index, .. } => {
                log::info!("[PlaybackEvent] Started: queue_index={}", queue_index);
            }
            PlaybackEvent::Paused => {
                log::info!("[PlaybackEvent] Paused");
            }
            PlaybackEvent::Resumed => {
                log::info!("[PlaybackEvent] Resumed");
            }
            PlaybackEvent::Stopped => {
                log::info!("[PlaybackEvent] Stopped");
            }
            PlaybackEvent::TrackChanged {
                from_index,
                to_index,
                trigger,
            } => {
                log::info!(
                    "[PlaybackEvent] TrackChanged: {:?} -> {} ({:?})",
                    from_index,
                    to_index,
                    trigger
                );
            }
            PlaybackEvent::VolumeChanged { from, to } => {
                log::debug!("[PlaybackEvent] Volume: {:.2} -> {:.2}", from, to);
            }
            PlaybackEvent::MuteChanged { muted } => {
                log::info!("[PlaybackEvent] Mute: {}", muted);
            }
            PlaybackEvent::Seeked {
                from_position,
                to_position,
            } => {
                log::info!(
                    "[PlaybackEvent] Seek: {:.1}s -> {:.1}s",
                    from_position,
                    to_position
                );
            }
            PlaybackEvent::TrackEnded { queue_index } => {
                log::info!("[PlaybackEvent] TrackEnded: queue_index={}", queue_index);
            }
            PlaybackEvent::Error { message } => {
                log::error!("[PlaybackEvent] Error: {}", message);
            }
            _ => {}
        }

        // Maintain max size
        if self.events.len() >= MAX_EVENTS {
            self.events.pop_front();
        }

        self.events.push_back(record);
    }

    /// Record an event without trigger context
    pub fn record_event(&mut self, event: PlaybackEvent) {
        self.record(event, None);
    }

    /// Start a new playback session (resets session timer)
    pub fn start_session(&mut self) {
        self.session_start = Instant::now();
        self.last_position_update = None;
        log::info!("[PlaybackEventStore] New session started");
    }

    /// Get all events
    pub fn events(&self) -> impl Iterator<Item = &EventRecord> {
        self.events.iter()
    }

    /// Get events since a given time
    pub fn events_since(&self, since: Instant) -> impl Iterator<Item = &EventRecord> {
        self.events.iter().filter(move |e| e.timestamp >= since)
    }

    /// Get the last N events
    pub fn last_n(&self, n: usize) -> impl Iterator<Item = &EventRecord> {
        self.events.iter().rev().take(n)
    }

    /// Get events within the last duration
    pub fn events_within(&self, duration: Duration) -> impl Iterator<Item = &EventRecord> {
        let cutoff = Instant::now() - duration;
        self.events.iter().filter(move |e| e.timestamp >= cutoff)
    }

    /// Count events of a specific type
    pub fn count_events<F>(&self, predicate: F) -> usize
    where
        F: Fn(&PlaybackEvent) -> bool,
    {
        self.events.iter().filter(|e| predicate(&e.event)).count()
    }

    /// Find the last event matching a predicate
    pub fn find_last<F>(&self, predicate: F) -> Option<&EventRecord>
    where
        F: Fn(&PlaybackEvent) -> bool,
    {
        self.events.iter().rev().find(|e| predicate(&e.event))
    }

    /// Get a summary of recent activity
    pub fn summary(&self) -> EventStoreSummary {
        let total_events = self.events.len();
        let session_duration = self.session_start.elapsed();

        let play_count = self.count_events(|e| matches!(e, PlaybackEvent::Started { .. }));
        let pause_count = self.count_events(|e| matches!(e, PlaybackEvent::Paused));
        let seek_count = self.count_events(|e| matches!(e, PlaybackEvent::Seeked { .. }));
        let track_changes = self.count_events(|e| matches!(e, PlaybackEvent::TrackChanged { .. }));
        let errors = self.count_events(|e| matches!(e, PlaybackEvent::Error { .. }));

        EventStoreSummary {
            total_events,
            session_duration,
            play_count,
            pause_count,
            seek_count,
            track_changes,
            errors,
        }
    }

    /// Clear all events
    pub fn clear(&mut self) {
        self.events.clear();
        log::info!("[PlaybackEventStore] Events cleared");
    }

    /// Enable or disable event recording
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        log::info!(
            "[PlaybackEventStore] Recording {}",
            if enabled { "enabled" } else { "disabled" }
        );
    }

    /// Check if recording is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get total event count
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Check if store is empty
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Replay events to reconstruct state at a given point
    /// Returns a snapshot of playback state after replaying events up to the given index
    pub fn replay_to(&self, event_index: usize) -> PlaybackSnapshot {
        let mut snapshot = PlaybackSnapshot::default();

        for (i, record) in self.events.iter().enumerate() {
            if i > event_index {
                break;
            }

            match &record.event {
                PlaybackEvent::Started {
                    queue_index,
                    track_path,
                } => {
                    snapshot.is_playing = true;
                    snapshot.queue_index = Some(*queue_index);
                    snapshot.current_track = track_path.clone();
                    snapshot.position = 0.0;
                }
                PlaybackEvent::Paused => {
                    snapshot.is_playing = false;
                }
                PlaybackEvent::Resumed => {
                    snapshot.is_playing = true;
                }
                PlaybackEvent::Stopped => {
                    snapshot.is_playing = false;
                    snapshot.queue_index = None;
                    snapshot.position = 0.0;
                }
                PlaybackEvent::TrackChanged { to_index, .. } => {
                    snapshot.queue_index = Some(*to_index);
                    snapshot.position = 0.0;
                }
                PlaybackEvent::VolumeChanged { to, .. } => {
                    snapshot.volume = *to;
                }
                PlaybackEvent::MuteChanged { muted } => {
                    snapshot.muted = *muted;
                }
                PlaybackEvent::Seeked { to_position, .. } => {
                    snapshot.position = *to_position;
                }
                PlaybackEvent::PositionUpdated { position } => {
                    snapshot.position = *position;
                }
                PlaybackEvent::DurationSet { duration } => {
                    snapshot.duration = *duration;
                }
                PlaybackEvent::QueueIndexSet { index } => {
                    snapshot.queue_index = *index;
                }
                PlaybackEvent::TrackEnded { .. } => {
                    snapshot.is_playing = false;
                }
                PlaybackEvent::Error { .. } => {
                    // Errors don't change state
                }
            }
        }

        snapshot
    }

    /// Get current state by replaying all events
    pub fn current_snapshot(&self) -> PlaybackSnapshot {
        if self.events.is_empty() {
            return PlaybackSnapshot::default();
        }
        self.replay_to(self.events.len() - 1)
    }
}

/// Summary of event store contents
#[derive(Debug, Clone)]
pub struct EventStoreSummary {
    pub total_events: usize,
    pub session_duration: Duration,
    pub play_count: usize,
    pub pause_count: usize,
    pub seek_count: usize,
    pub track_changes: usize,
    pub errors: usize,
}

/// Reconstructed playback state from events
#[derive(Debug, Clone, Default)]
pub struct PlaybackSnapshot {
    pub is_playing: bool,
    pub queue_index: Option<usize>,
    pub current_track: Option<PathBuf>,
    pub volume: f32,
    pub muted: bool,
    pub position: f64,
    pub duration: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_recording() {
        let mut store = PlaybackEventStore::new();

        store.record_event(PlaybackEvent::Started {
            queue_index: 0,
            track_path: Some(PathBuf::from("/music/track.flac")),
        });
        store.record_event(PlaybackEvent::VolumeChanged { from: 0.7, to: 0.5 });
        store.record_event(PlaybackEvent::Paused);

        assert_eq!(store.len(), 3);
    }

    #[test]
    fn test_replay() {
        let mut store = PlaybackEventStore::new();

        store.record_event(PlaybackEvent::Started {
            queue_index: 0,
            track_path: None,
        });
        store.record_event(PlaybackEvent::VolumeChanged { from: 0.7, to: 0.5 });
        store.record_event(PlaybackEvent::Paused);
        store.record_event(PlaybackEvent::Resumed);

        let snapshot = store.current_snapshot();
        assert!(snapshot.is_playing);
        assert_eq!(snapshot.queue_index, Some(0));
        assert_eq!(snapshot.volume, 0.5);

        // Replay to before resume
        let snapshot_at_2 = store.replay_to(2);
        assert!(!snapshot_at_2.is_playing); // Was paused
    }

    #[test]
    fn test_max_events() {
        let mut store = PlaybackEventStore::new();

        for i in 0..1500 {
            store.record_event(PlaybackEvent::PositionUpdated { position: i as f64 });
            // Force update by resetting the throttle
            store.last_position_update = None;
        }

        assert!(store.len() <= MAX_EVENTS);
    }

    #[test]
    fn test_summary() {
        let mut store = PlaybackEventStore::new();

        store.record_event(PlaybackEvent::Started {
            queue_index: 0,
            track_path: None,
        });
        store.record_event(PlaybackEvent::Paused);
        store.record_event(PlaybackEvent::Resumed);
        store.record_event(PlaybackEvent::Seeked {
            from_position: 0.0,
            to_position: 30.0,
        });
        store.record_event(PlaybackEvent::TrackChanged {
            from_index: Some(0),
            to_index: 1,
            trigger: TrackChangeTrigger::NextTrack,
        });

        let summary = store.summary();
        assert_eq!(summary.play_count, 1);
        assert_eq!(summary.pause_count, 1);
        assert_eq!(summary.seek_count, 1);
        assert_eq!(summary.track_changes, 1);
    }
}
