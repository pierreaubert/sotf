//! Play statistics tracking.
//!
//! Records a "play" in the database after a track has been playing for 30+ seconds.
//! Shared between all app frontends (GPUI, TUI, etc.)

use std::path::PathBuf;
use std::time::Instant;

use crate::database::MusicDatabase;

const PLAY_THRESHOLD_SECS: u64 = 30;

/// Tracks how long the current track has been playing and records a play
/// in the database after 30 seconds.
#[derive(Debug, Default)]
pub struct PlayTracker {
    pub current_track_path: Option<PathBuf>,
    pub start_time: Option<Instant>,
    pub already_recorded: bool,
}

impl PlayTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start tracking a new track.
    pub fn start(&mut self, track_path: PathBuf) {
        self.current_track_path = Some(track_path);
        self.start_time = Some(Instant::now());
        self.already_recorded = false;
    }

    /// Stop tracking (called when playback stops or track changes).
    pub fn stop(&mut self) {
        self.current_track_path = None;
        self.start_time = None;
        self.already_recorded = false;
    }

    /// Check if the current track has been playing for 30+ seconds and record it.
    /// `duration` is the current playback position in seconds (for the database record).
    /// Returns the path of the recorded track if a play was just recorded.
    pub fn check_and_record(&mut self, db: &MusicDatabase, duration: u64) -> Option<PathBuf> {
        if self.already_recorded {
            return None;
        }

        if let (Some(path), Some(start_time)) = (&self.current_track_path, self.start_time) {
            let elapsed = start_time.elapsed().as_secs();
            if elapsed >= PLAY_THRESHOLD_SECS {
                if let Err(e) = db.record_play(path, duration) {
                    log::error!("Failed to record play: {}", e);
                    return None;
                }
                log::info!("Recorded play for {:?} ({}s)", path, duration);
                self.already_recorded = true;
                return Some(path.clone());
            }
        }

        None
    }
}
