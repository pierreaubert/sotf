//! Playback state management.
//!
//! Thin wrapper around `PlaybackController` from sotf-player, adding GPUI-specific
//! display fields (loudness, spectrum, compressor).

use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use crate::app::constants;
use sotf_audio_player::{LoudnessData, PlaybackController, SignalPath, SpectrumData};
use sotf_plugins::CompressorData;

#[derive(Debug, Clone)]
pub struct HeldSpectrumFrame {
    pub data: Arc<SpectrumData>,
    pub sample_rate: Option<u32>,
}

pub struct PlaybackState {
    pub track_information_open: bool,
    /// Whether the Now Playing signal-path details are expanded. Closed by
    /// default so negotiated formats stay one explicit disclosure away.
    pub signal_path_open: bool,
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
    /// Deterministic meter data used only by black-box rendered QA.
    #[cfg(feature = "dev-api")]
    pub qa_loudness_fixture: Option<Arc<LoudnessData>>,
    /// Deterministic spectrum data used only by isolated rendered QA.
    #[cfg(feature = "dev-api")]
    pub qa_spectrum_fixture: Option<HeldSpectrumFrame>,
    /// Duration override for rendered transport QA (for example live audio).
    #[cfg(feature = "dev-api")]
    pub qa_duration_fixture: Option<f64>,
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
    /// Output-rate context for spectrum displays. Do not label the decoded
    /// source rate as the current output rate when the engine resamples.
    pub fn spectrum_output_sample_rate(&self) -> Option<u32> {
        #[cfg(feature = "dev-api")]
        if let Some(frame) = &self.qa_spectrum_fixture {
            return frame.sample_rate;
        }
        self.signal_path
            .as_ref()
            .and_then(|path| u32::try_from(path.output.sample_rate_hz).ok())
            .filter(|rate| *rate > 0)
    }

    pub fn new() -> Self {
        let mut ctrl = PlaybackController::new();
        // Override the default volume with GPUI's startup volume
        ctrl.volume = constants::ui::DEFAULT_STARTUP_VOLUME;
        Self {
            track_information_open: false,
            signal_path_open: false,
            ctrl,
            current_queue_index: None,
            input_loudness_info: None,
            loudness_info: None,
            spectrum_info: None,
            compressor_info: None,
            rack_plugin_data: None,
            signal_path: None,
            #[cfg(feature = "dev-api")]
            qa_loudness_fixture: None,
            #[cfg(feature = "dev-api")]
            qa_spectrum_fixture: None,
            #[cfg(feature = "dev-api")]
            qa_duration_fixture: None,
        }
    }

    pub fn display_duration_secs(&self) -> f64 {
        #[cfg(feature = "dev-api")]
        if let Some(duration) = self.qa_duration_fixture {
            return duration;
        }
        self.duration_secs
    }
}
