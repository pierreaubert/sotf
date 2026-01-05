//! Playback state management.
//!
//! Contains all state related to audio playback including position, volume,
//! loudness monitoring, and spectrum analysis.

use crate::app::constants;
use sotf_audio_player::{LoudnessData, SpectrumData};
use sotf_plugins::CompressorData;

#[derive(Debug, Clone)]
pub struct PlaybackState {
    pub is_playing: bool,
    pub current_queue_index: Option<usize>,
    pub volume: f32,
    pub muted: bool,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub input_loudness_info: Option<LoudnessData>,
    pub loudness_info: Option<LoudnessData>,
    pub spectrum_info: Option<SpectrumData>,
    pub compressor_info: Option<CompressorData>,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            is_playing: false,
            current_queue_index: None,
            volume: constants::ui::DEFAULT_STARTUP_VOLUME,
            muted: false,
            position_secs: 0.0,
            duration_secs: 0.0,
            input_loudness_info: None,
            loudness_info: None,
            spectrum_info: None,
            compressor_info: None,
        }
    }
}

impl PlaybackState {
    pub fn new() -> Self {
        Self::default()
    }
}
