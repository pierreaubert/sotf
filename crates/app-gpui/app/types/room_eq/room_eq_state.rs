use super::interactive_chart_state_wrapper::InteractiveChartStateWrapper;
use super::room_eq_dropdowns::RoomEqDropdowns;
use super::room_eq_review_graph_settings_set::RoomEqReviewGraphSettingsSet;
use sotf_audio_player::room_eq_types::{
    CustomTargetCurve, RoomEqEasyLayout, validate_room_eq_easy_layout,
};
use sotf_audio_player::ui_models::room_eq::RoomEqScreenModel;
use std::ops::{Deref, DerefMut};

/// Identifies one completed optimization and the inputs that produced it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoomEqResultIdentity {
    generation: u64,
    inputs: u64,
}

/// Complete Room EQ screen state.
///
/// Domain state lives in the shared [`RoomEqScreenModel`] so both GPUI and TUI
/// shells operate on the same data. This struct keeps only UI-specific view
/// state (chart zoom, dropdowns, edit buffers, etc.).
#[derive(Debug, Clone)]
pub struct RoomEqState {
    /// Shared, UI-agnostic Room EQ domain state.
    pub model: RoomEqScreenModel,
    pub result_inputs: Option<u64>,
    /// Frozen source identity of the inputs the current result was
    /// produced from (channel/driver names and capture provenance),
    /// captured when the run starts so the Review step can describe
    /// the result's sources without serializing measurement payloads.
    pub result_sources: Option<Vec<RoomEqResultSource>>,
    pub delivery: sotf_audio_player::ui_models::correction_delivery::CorrectionDelivery,

    // === UI State ===
    pub dropdowns: RoomEqDropdowns,
    /// Review graph smoothing level in octaves (0 = none, 1 = 1 octave, etc.)
    pub review_smoothing_octaves: f64,
    /// Selected channel index for review (0-based)
    pub review_selected_channel: usize,
    pub review_sources_open: bool,
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
    pub autoeq_stage: crate::components::autoeq::AutoEqStage,
    pub detail_level: sotf_audio_player::autoeq::DetailLevel,
    /// Currently selected preset id
    pub selected_preset: String,
    /// Explicit speaker layout used by the beginner RoomEQ workflow.
    pub easy_layout: RoomEqEasyLayout,

    /// Monotonic generation of the current optimization run. Progress
    /// receivers and completions from older generations are dropped,
    /// so a cancelled/superseded run cannot write into a newer one.
    pub optimization_generation: u64,
}

impl Default for RoomEqState {
    fn default() -> Self {
        Self {
            model: RoomEqScreenModel::default(),
            result_inputs: None,
            result_sources: None,
            delivery: Default::default(),
            dropdowns: RoomEqDropdowns::default(),
            review_smoothing_octaves: 1.0 / 6.0, // Match display-roomeq.py default
            review_selected_channel: 0,
            review_sources_open: false,
            review_chart_state: None,
            review_y_axis_auto: false,
            review_graph_settings: RoomEqReviewGraphSettingsSet::default(),
            progress_chart_state: None,
            custom_target_curve: CustomTargetCurve::new_flat(),
            show_advanced_config: false,
            autoeq_stage: Default::default(),
            detail_level: sotf_audio_player::autoeq::DetailLevel::Simple,
            selected_preset: "full-range".to_string(),
            easy_layout: RoomEqEasyLayout::Stereo20,
            optimization_generation: 0,
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
    /// Capture the typed source identity of the current measurements.
    pub fn capture_result_sources(&self) -> Vec<RoomEqResultSource> {
        self.model
            .channel_measurements
            .iter()
            .map(|channel| RoomEqResultSource {
                channel_name: channel.channel_name.clone(),
                provenance: channel.provenance.clone(),
                drivers: channel
                    .driver_measurement_sets
                    .iter()
                    .map(|driver| RoomEqResultDriverSource {
                        name: driver.name.clone(),
                        provenance: driver.provenance.clone(),
                    })
                    .collect(),
            })
            .collect()
    }

    /// Cheap digest of every input that determines the optimization
    /// result: optimizer configuration, speaker configs, wizard
    /// preset/mode/layout, probe arrivals, and measurement identity
    /// with sampled response points. Replaces the previous full serde
    /// snapshot, which serialized every response curve on each
    /// freshness check (and therefore on each review/save render).
    pub fn input_digest(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();

        if let Ok(config) = serde_json::to_string(&self.model.optimizer_config) {
            config.hash(&mut hasher);
        }
        if let Ok(speakers) = serde_json::to_string(&self.model.speaker_configs) {
            speakers.hash(&mut hasher);
        }
        format!("{:?}", self.model.simple_preset).hash(&mut hasher);
        format!("{:?}", self.model.wizard_mode).hash(&mut hasher);
        format!("{:?}", self.easy_layout).hash(&mut hasher);

        if let Some(arrivals) = self.model.delay_detection.probe_arrival_map() {
            arrivals.len().hash(&mut hasher);
            let mut arrival_entries: Vec<(&String, &f64)> = arrivals.iter().collect();
            arrival_entries.sort_by(|a, b| a.0.cmp(b.0));
            for (name, seconds) in arrival_entries {
                name.hash(&mut hasher);
                (*seconds).to_bits().hash(&mut hasher);
            }
        }

        fn hash_result(
            hasher: &mut std::collections::hash_map::DefaultHasher,
            result: &sotf_audio_player::recording_types::RecordingResult,
        ) {
            use std::hash::Hash;
            result.frequencies.len().hash(hasher);
            result.sample_rate_hz.hash(hasher);
            if let Some(ir) = &result.impulse_response {
                ir.len().hash(hasher);
            }
            // Hash every grid value: still orders of magnitude cheaper
            // than serializing the responses to JSON, and an interior
            // edit cannot slip past sampled points.
            result.frequencies.len().hash(hasher);
            for value in &result.frequencies {
                value.to_bits().hash(hasher);
            }
            for values in [&result.magnitude_db, &result.phase_deg] {
                values.len().hash(hasher);
                for value in values {
                    value.to_bits().hash(hasher);
                }
            }
        }

        for channel in &self.model.channel_measurements {
            channel.channel_name.hash(&mut hasher);
            channel.is_group.hash(&mut hasher);
            hash_result(&mut hasher, &channel.measurement);
            for driver in &channel.group_drivers {
                hash_result(&mut hasher, driver);
            }
            for extra in &channel.multi_mic_measurements {
                hash_result(&mut hasher, extra);
            }
        }

        hasher.finish()
    }

    pub fn result_is_current(&self) -> bool {
        self.result_inputs == Some(self.input_digest())
    }

    /// Returns the current DSP result's identity for asynchronous consumers.
    pub fn current_result_identity(&self) -> Option<RoomEqResultIdentity> {
        let inputs = self.result_inputs?;
        if self.dsp_output.is_none() || !self.result_is_current() {
            return None;
        }
        Some(RoomEqResultIdentity {
            generation: self.optimization_generation,
            inputs,
        })
    }

    /// Load Room EQ domain state from the app-specific RecordingState.
    ///
    /// This is a thin adapter over [`RoomEqScreenModel::load_from_recording`] that
    /// converts the GPUI recording wrapper into the shared player type.
    pub fn load_from_recording(
        &mut self,
        recording_state: &crate::app::types::RecordingState,
    ) -> Result<(), String> {
        let player_state = sotf_audio_player::recording_types::RecordingState {
            playback_config: recording_state.playback_config.clone(),
            recording_config: recording_state.recording_config.clone(),
            channel_recordings: recording_state.channel_recordings.clone(),
            transfer_matrix_loopbacks: recording_state.transfer_matrix_loopbacks.clone(),
            ctc_reference_sweep_path: recording_state.ctc_reference_sweep_path.clone(),
            ctc_reference_sweep_duration_s: recording_state.ctc_reference_sweep_duration_s,
            probe_capture: recording_state.probe_capture.clone(),
            signal_duration_secs: recording_state.signal_duration_secs,
            recording_directory: recording_state.recording_directory.clone(),
        };
        if let Some(imported) = &recording_state.imported_session {
            self.model
                .load_from_imported_recording(&player_state, imported)?;
        } else {
            self.model.load_from_recording(&player_state);
        }

        let channel_names = self
            .model
            .channel_measurements
            .iter()
            .map(|measurement| measurement.channel_name.as_str())
            .collect::<Vec<_>>();
        if let Some(layout) = RoomEqEasyLayout::ALL
            .into_iter()
            .find(|layout| validate_room_eq_easy_layout(*layout, &channel_names).is_ok())
        {
            self.easy_layout = layout;
            layout.configure_preset_defaults(&mut self.model.simple_preset);
        }
        Ok(())
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

/// One channel's frozen source identity for the Review step.
#[derive(Debug, Clone, Default)]
pub struct RoomEqResultSource {
    pub channel_name: String,
    pub provenance: Vec<sotf_audio_player::room_eq_types::MeasurementProvenance>,
    pub drivers: Vec<RoomEqResultDriverSource>,
}

/// One driver's frozen source identity for the Review step.
#[derive(Debug, Clone, Default)]
pub struct RoomEqResultDriverSource {
    pub name: Option<String>,
    pub provenance: Vec<sotf_audio_player::room_eq_types::MeasurementProvenance>,
}
