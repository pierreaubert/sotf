//! Playback state management.
//!
//! Thin wrapper around `PlaybackController` from sotf-player, adding GPUI-specific
//! display fields (loudness, spectrum, compressor).

use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use crate::app::constants;
use sotf_audio_player::{LoudnessData, PlaybackController, SignalPath, SpectrumData};
use sotf_plugins::CompressorData;

pub struct PlaybackState {
    ctrl: PlaybackController,

    // GPUI-specific: synced from queue
    pub current_queue_index: Option<usize>,

    // GPUI-specific: display data from audio engine
    pub input_loudness_info: Option<Arc<LoudnessData>>,
    /// Output-side loudness data. Includes per-channel true-peaks (level
    /// meters, SPL spider) AND the inter-channel correlation matrix
    /// (Correlation spider) — both are produced by the same LoudnessMonitor.
    pub loudness_info: Option<Arc<LoudnessData>>,
    pub spectrum_info: Option<Arc<SpectrumData>>,
    pub compressor_info: Option<Arc<CompressorData>>,
    /// Type-erased real-time data for the plugin currently visible in the rack.
    pub rack_plugin_data: Option<Arc<dyn std::any::Any + Send + Sync>>,
    /// Latest read-only signal-path snapshot for UI status badges.
    pub signal_path: Option<SignalPath>,
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
            rack_plugin_data: None,
            signal_path: None,
        }
    }
}
