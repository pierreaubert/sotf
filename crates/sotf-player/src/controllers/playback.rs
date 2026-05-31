//! Playback controller — owns playback state, volume, replay gain, and play tracking.

use std::path::PathBuf;

use crate::Track;
use crate::database::MusicDatabase;
use crate::play_tracker::PlayTracker;
use crate::replay_gain_scanner::ReplayGainMode;

/// Default startup volume (0.0 to 1.0).
const DEFAULT_STARTUP_VOLUME: f32 = 0.8;

/// Volume step for increase/decrease operations.
const VOLUME_STEP: f32 = 0.05;

#[derive(Debug)]
pub struct PlaybackController {
    pub is_playing: bool,
    pub volume: f32,
    pub muted: bool,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub sample_rate: Option<u32>,

    // Replay gain
    pub replay_gain_enabled: bool,
    pub replay_gain_mode: ReplayGainMode,
    pub replay_gain_preamp: f32,

    // Play tracking
    play_tracker: PlayTracker,
}

impl Default for PlaybackController {
    fn default() -> Self {
        Self::new()
    }
}

impl PlaybackController {
    pub fn new() -> Self {
        Self {
            is_playing: false,
            volume: DEFAULT_STARTUP_VOLUME,
            muted: false,
            position_secs: 0.0,
            duration_secs: 0.0,
            sample_rate: None,
            replay_gain_enabled: true,
            replay_gain_mode: ReplayGainMode::Track,
            replay_gain_preamp: 0.0,
            play_tracker: PlayTracker::new(),
        }
    }

    // =========================================================================
    // Volume
    // =========================================================================

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    pub fn increase_volume(&mut self) {
        self.set_volume(self.volume + VOLUME_STEP);
    }

    pub fn decrease_volume(&mut self) {
        self.set_volume(self.volume - VOLUME_STEP);
    }

    pub fn toggle_mute(&mut self) {
        self.muted = !self.muted;
    }

    /// Get effective volume (0.0 if muted).
    pub fn effective_volume(&self) -> f32 {
        if self.muted { 0.0 } else { self.volume }
    }

    // =========================================================================
    // Replay Gain
    // =========================================================================

    /// Get the replay gain adjustment in dB for a track.
    /// Returns `None` if replay gain is disabled or the selected mode has no
    /// matching gain data. Album mode intentionally does not fall back to track
    /// gain: the user chose album-level consistency, so unity is less surprising
    /// than per-track loudness jumps when album gain has not been scanned yet.
    pub fn get_replay_gain_adjustment(&self, track: &Track) -> Option<f64> {
        if !self.replay_gain_enabled {
            return None;
        }

        let gain = match self.replay_gain_mode {
            ReplayGainMode::Track => track.replay_gain?,
            ReplayGainMode::Album => track.album_gain?,
        };

        Some(gain + self.replay_gain_preamp as f64)
    }

    // =========================================================================
    // Play Tracking
    // =========================================================================

    /// Start tracking a new track for play statistics.
    pub fn start_tracking(&mut self, path: PathBuf) {
        self.play_tracker.start(path);
    }

    /// Stop tracking the current track.
    pub fn stop_tracking(&mut self) {
        self.play_tracker.stop();
    }

    /// Check if the current track should be recorded as played (30s threshold).
    /// Returns the path of the recorded track if a play was just recorded.
    pub fn check_and_record(&mut self, db: &MusicDatabase, duration: u64) -> Option<PathBuf> {
        self.play_tracker.check_and_record(db, duration)
    }

    /// Get the path being tracked.
    pub fn tracked_path(&self) -> Option<&PathBuf> {
        self.play_tracker.current_track_path.as_ref()
    }

    /// Whether the current track has already been recorded.
    pub fn already_recorded(&self) -> bool {
        self.play_tracker.already_recorded
    }

    // =========================================================================
    // State sync
    // =========================================================================

    /// Update playback position and duration from the audio engine.
    pub fn update_position(&mut self, position_secs: f64, duration_secs: f64) {
        self.position_secs = position_secs;
        self.duration_secs = duration_secs;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn album_replay_gain_mode_requires_album_gain() {
        let mut controller = PlaybackController::new();
        controller.replay_gain_mode = ReplayGainMode::Album;
        let track = Track {
            replay_gain: Some(-6.0),
            album_gain: None,
            ..Default::default()
        };

        assert_eq!(controller.get_replay_gain_adjustment(&track), None);
    }

    #[test]
    fn album_replay_gain_mode_uses_album_gain_with_preamp() {
        let mut controller = PlaybackController::new();
        controller.replay_gain_mode = ReplayGainMode::Album;
        controller.replay_gain_preamp = 1.5;
        let track = Track {
            replay_gain: Some(-9.0),
            album_gain: Some(-4.0),
            ..Default::default()
        };

        assert_eq!(controller.get_replay_gain_adjustment(&track), Some(-2.5));
    }
}
