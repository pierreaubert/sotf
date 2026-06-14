use super::interactive_chart_state_wrapper::InteractiveChartStateWrapper;
use super::room_eq_dropdowns::RoomEqDropdowns;
use super::room_eq_review_graph_settings_set::RoomEqReviewGraphSettingsSet;
use sotf_audio_player::room_eq_types::CustomTargetCurve;
use sotf_audio_player::ui_models::room_eq::RoomEqScreenModel;
use std::ops::{Deref, DerefMut};

/// Complete Room EQ screen state.
///
/// Domain state lives in the shared [`RoomEqScreenModel`] so both GPUI and TUI
/// shells operate on the same data. This struct keeps only UI-specific view
/// state (chart zoom, dropdowns, edit buffers, etc.).
#[derive(Debug, Clone)]
pub struct RoomEqState {
    /// Shared, UI-agnostic Room EQ domain state.
    pub model: RoomEqScreenModel,

    // === UI State ===
    pub dropdowns: RoomEqDropdowns,
    /// Review graph smoothing level in octaves (0 = none, 1 = 1 octave, etc.)
    pub review_smoothing_octaves: f64,
    /// Selected channel index for review (0-based)
    pub review_selected_channel: usize,
    /// Interactive chart state for review graph (zoom/pan) - initialized lazily
    pub review_chart_state: Option<InteractiveChartStateWrapper>,
    /// Whether to auto-scale Y axis for review graph.
    pub review_y_axis_auto: bool,
    /// Per-graph controls used by the Python-style RoomEQ report charts.
    pub review_graph_settings: RoomEqReviewGraphSettingsSet,
    /// Interactive chart state for progress chart (zoom/pan) - initialized lazily
    pub progress_chart_state: Option<InteractiveChartStateWrapper>,
    /// Custom target curve for manual entry mode
    pub custom_target_curve: CustomTargetCurve,

    /// When false (default), the Configure step shows only basic settings.
    pub show_advanced_config: bool,
    /// Detail level for the configuration form (Simple / Intermediate / Expert)
    pub detail_level: sotf_audio_player::autoeq::DetailLevel,
    /// Currently selected preset id
    pub selected_preset: String,
}

impl Default for RoomEqState {
    fn default() -> Self {
        Self {
            model: RoomEqScreenModel::default(),
            dropdowns: RoomEqDropdowns::default(),
            review_smoothing_octaves: 1.0 / 6.0, // Match display-roomeq.py default
            review_selected_channel: 0,
            review_chart_state: None,
            review_y_axis_auto: false,
            review_graph_settings: RoomEqReviewGraphSettingsSet::default(),
            progress_chart_state: None,
            custom_target_curve: CustomTargetCurve::new_flat(),
            show_advanced_config: false,
            detail_level: sotf_audio_player::autoeq::DetailLevel::Simple,
            selected_preset: "full-range".to_string(),
        }
    }
}

impl Deref for RoomEqState {
    type Target = RoomEqScreenModel;

    fn deref(&self) -> &Self::Target {
        &self.model
    }
}

impl DerefMut for RoomEqState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.model
    }
}

impl RoomEqState {
    /// Load Room EQ domain state from the app-specific RecordingState.
    ///
    /// This is a thin adapter over [`RoomEqScreenModel::load_from_recording`] that
    /// converts the GPUI recording wrapper into the shared player type.
    pub fn load_from_recording(&mut self, recording_state: &crate::app::types::RecordingState) {
        let player_state = sotf_audio_player::recording_types::RecordingState {
            playback_config: recording_state.playback_config.clone(),
            recording_config: recording_state.recording_config.clone(),
            channel_recordings: recording_state.channel_recordings.clone(),
            transfer_matrix_loopbacks: recording_state.transfer_matrix_loopbacks.clone(),
            ctc_reference_sweep_path: recording_state.ctc_reference_sweep_path.clone(),
            probe_capture: recording_state.probe_capture.clone(),
            signal_duration_secs: recording_state.signal_duration_secs,
            recording_directory: recording_state.recording_directory.clone(),
        };
        self.model.load_from_recording(&player_state);
    }

    /// Normalize an SPL curve so its average over 1–2 kHz is 0 dB.
    pub fn calculate_normalization_offset(frequencies: &[f64], spl: &[f64]) -> f64 {
        RoomEqScreenModel::calculate_normalization_offset(frequencies, spl)
    }

    /// Normalize a set of points by subtracting an offset.
    pub fn normalize_points(points: &[(f64, f64)], offset: f64) -> Vec<(f64, f64)> {
        RoomEqScreenModel::normalize_points(points, offset)
    }
}
