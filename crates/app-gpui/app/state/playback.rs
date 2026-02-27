//! Playback state management.
//!
//! Thin wrapper around `PlaybackController` from sotf-player, adding GPUI-specific
//! display fields (loudness, spectrum, compressor).

use std::ops::{Deref, DerefMut};

use crate::app::constants;
use sotf_audio_player::{LoudnessData, PlaybackController, SpectrumData};
use sotf_plugins::CompressorData;

#[derive(Debug)]
pub struct PlaybackState {
    ctrl: PlaybackController,

    // GPUI-specific: synced from queue
    pub current_queue_index: Option<usize>,

    // GPUI-specific: display data from audio engine
    pub input_loudness_info: Option<LoudnessData>,
    pub loudness_info: Option<LoudnessData>,
    pub spectrum_info: Option<SpectrumData>,
    pub compressor_info: Option<CompressorData>,
}

impl Deref for PlaybackState {
    type Target = PlaybackController;
    fn deref(&self) -> &Self::Target {
        &self.ctrl
    }
}

impl DerefMut for PlaybackState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ctrl
    }
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self::new()
    }
}

impl PlaybackState {
    pub fn new() -> Self {
        let mut ctrl = PlaybackController::new();
        // Override the default volume with GPUI's startup volume
        ctrl.volume = constants::ui::DEFAULT_STARTUP_VOLUME;
        Self {
            ctrl,
            current_queue_index: None,
            input_loudness_info: None,
            loudness_info: None,
            spectrum_info: None,
            compressor_info: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::constants;

    #[test]
    fn playback_state_defaults() {
        let state = PlaybackState::default();

        assert!(!state.is_playing);
        assert_eq!(state.current_queue_index, None);
        assert_eq!(state.volume, constants::ui::DEFAULT_STARTUP_VOLUME);
        assert!(!state.muted);
        assert_eq!(state.position_secs, 0.0);
        assert_eq!(state.duration_secs, 0.0);
        assert!(state.input_loudness_info.is_none());
        assert!(state.loudness_info.is_none());
        assert!(state.spectrum_info.is_none());
        assert!(state.compressor_info.is_none());
    }
}
