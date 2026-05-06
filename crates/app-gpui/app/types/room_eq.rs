// ============================================================================
// Room EQ Screen Types
// ============================================================================

use super::recording::{RecordingResult, RecordingState};
use autoeq::roomeq::{
    CrossoverConfig as BackendCrossoverConfig, MeasurementSource, RoomConfig, SpeakerConfig,
    SpeakerGroup,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// Wrapper for InteractiveChartState that implements Debug
#[derive(Clone)]
pub struct InteractiveChartStateWrapper(pub gpui_px::interaction::InteractiveChartState);

impl std::fmt::Debug for InteractiveChartStateWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InteractiveChartStateWrapper")
            .field("is_zoomed", &self.0.is_zoomed())
            .finish()
    }
}

impl InteractiveChartStateWrapper {
    pub fn new(x_min: f64, x_max: f64, y_min: f64, y_max: f64) -> Self {
        Self(gpui_px::interaction::InteractiveChartState::new(
            x_min, x_max, y_min, y_max,
        ))
    }

    pub fn with_log_x(mut self, is_log: bool) -> Self {
        self.0 = self.0.with_log_x(is_log);
        self
    }

    pub fn with_size(mut self, width: f32, height: f32) -> Self {
        self.0 = self.0.with_size(width, height);
        self
    }

    pub fn inner(&self) -> &gpui_px::interaction::InteractiveChartState {
        &self.0
    }
}

// Domain types shared via sotf-player crate — single source of truth for all apps.
pub use sotf_audio_player::room_eq_types::RoomEqWizardMode;
pub use sotf_audio_player::room_eq_types::{
    ChannelDspChain, ChannelMatchingUiConfig, ChannelMeasurement, ChannelOptResult,
    CustomTargetCurve, DelayDetectionState, DelayDetectionStatus, DriverDspChain, DspChainMetadata,
    DspChainOutput, DspPluginConfig, EqFilterConfig, ExcursionProtectionConfig, MixedModeUiConfig,
    MixedPhaseUiConfig, MultiMeasurementUiConfig, MultiSeatConfig, MultiSpeakerMode,
    PhaseAlignmentConfig, PreRingingConfig, RecordingConfiguration, RoomEqDataSource,
    RoomEqFirConfig, RoomEqMeasurementsFile, RoomEqOptimizationMode, RoomEqSpeakerConfig,
    RoomEqStep, SchroederSplitConfig, SimplePresetConfig, SpeakerConfigType, SubOptimizerUiConfig,
    TargetCurveControlPoint, TargetResponseUiConfig, VoGConfig,
};
pub type CrossoverType = sotf_audio_player::room_eq_types::RoomEqCrossoverType;
pub use sotf_audio_player::room_eq_types::AutoEqField;
pub use sotf_audio_player::room_eq_types::OptimizationStatus;
pub use sotf_audio_player::room_eq_types::RoomEqAlgorithm;
pub use sotf_audio_player::room_eq_types::RoomEqOptimizerConfig;

/// UI state for Room EQ dropdowns
#[derive(Debug, Clone)]
pub struct RoomEqDropdowns {
    pub data_source_open: bool,
    pub opt_mode_open: bool,
    pub fir_phase_open: bool,
    pub algorithm_open: bool,
    pub peq_model_open: bool,
    pub crossover_type_open: bool,
    pub export_format_open: bool,
    /// DE strategy dropdown
    pub strategy_open: bool,
    /// Local algorithm dropdown
    pub local_algo_open: bool,
    /// Bayesian optimization acquisition dropdown
    pub bo_acquisition_open: bool,
    /// Loss type dropdown
    pub loss_type_open: bool,
    /// Target curve dropdown
    pub target_curve_open: bool,
    /// System type dropdown
    pub system_type_open: bool,

    // Advanced dropdowns
    pub tilt_type_open: bool,
    pub excursion_filter_type_open: bool,
    pub multi_seat_strategy_open: bool,

    // v2 dropdowns
    pub mixed_crossover_type_open: bool,
    pub mixed_fir_band_open: bool,
    pub vog_reference_channel_open: bool,
    pub multi_measurement_strategy_open: bool,

    /// Review step smoothing dropdown
    pub review_smoothing_open: bool,
    /// AutoEQ form editing state
    pub autoeq_editing_field: Option<AutoEqField>,
    /// AutoEQ form edit text
    pub autoeq_edit_text: String,
    /// Custom target curve editor modal open
    pub custom_target_modal_open: bool,
    /// Custom target presets dropdown open
    pub custom_target_presets_open: bool,
    /// Currently dragging control point index (None if not dragging)
    pub dragging_control_point: Option<usize>,
}

impl Default for RoomEqDropdowns {
    fn default() -> Self {
        Self {
            data_source_open: false,
            opt_mode_open: false,
            fir_phase_open: false,
            algorithm_open: false,
            peq_model_open: false,
            crossover_type_open: false,
            export_format_open: false,
            strategy_open: false,
            local_algo_open: false,
            bo_acquisition_open: false,
            loss_type_open: false,
            target_curve_open: false,
            system_type_open: false,
            tilt_type_open: true,
            excursion_filter_type_open: false,
            multi_seat_strategy_open: false,
            mixed_crossover_type_open: false,
            mixed_fir_band_open: false,
            vog_reference_channel_open: false,
            multi_measurement_strategy_open: false,
            review_smoothing_open: false,
            autoeq_editing_field: None,
            autoeq_edit_text: String::new(),
            custom_target_modal_open: false,
            custom_target_presets_open: false,
            dragging_control_point: None,
        }
    }
}

/// Complete Room EQ screen state
#[derive(Debug, Clone)]
pub struct RoomEqState {
    /// Current step in the workflow
    pub step: RoomEqStep,

    // === Step 1: Load Data ===
    /// Source of measurement data
    pub data_source: RoomEqDataSource,
    /// Loaded channel measurements
    pub channel_measurements: Vec<ChannelMeasurement>,
    /// Optional measured speaker × ear transfer matrix captured by the
    /// Recording wizard and forwarded to roomeq CTC optimization.
    pub ctc_measurements: Option<autoeq::roomeq::CtcMeasurementConfig>,
    pub ctc_config: Option<autoeq::roomeq::CtcConfig>,

    // === Step 2: Delay Detection (tone-burst probe) ===
    /// Shared state for the tone-burst delay-detection step. Business
    /// logic (probe/silence durations, status, results, overrides) lives
    /// in `sotf_audio_player::room_eq_types::DelayDetectionState`. The
    /// UI in `components/room_eq/step_2_delay_detection.rs` reads and
    /// mutates this through the normal `state.update(cx, ...)` path.
    pub delay_detection: DelayDetectionState,

    // === Step 3: Configuration ===
    /// Per-channel speaker configurations
    pub speaker_configs: Vec<RoomEqSpeakerConfig>,
    /// Global optimizer configuration
    pub optimizer_config: RoomEqOptimizerConfig,

    // === Step 3: Optimization ===
    /// Current optimization status
    pub optimization_status: OptimizationStatus,
    /// Cancel-request flag polled by the optimisation callback. UI sets
    /// this to true when the user clicks Cancel; the autoeq callback
    /// returns `CallbackAction::Stop` on the next iteration. Lives behind
    /// `Arc<AtomicBool>` so the spawn closure can clone it cheaply.
    pub cancel_requested: Arc<AtomicBool>,
    /// Currently optimizing channel name
    pub current_channel: Option<String>,
    /// Per-channel optimization results
    pub channel_results: Vec<ChannelOptResult>,
    /// Overall progress (0.0 - 1.0)
    pub overall_progress: f32,
    /// Progress history for visualization: (iteration, loss, channel_name)
    pub progress_history: Vec<(usize, f64, String, Option<f64>)>,
    /// Current iteration number
    pub current_iteration: usize,
    /// Current loss value
    pub current_loss: f64,

    // === Step 5: Export ===
    /// Generated DSP chain output
    pub dsp_output: Option<DspChainOutput>,

    // === Export State ===
    /// Selected export format index (0 = SotF JSON, 1..=6 = external formats).
    pub export_format_index: usize,

    // === UI State ===
    pub dropdowns: RoomEqDropdowns,
    pub status_message: String,
    pub error_message: Option<String>,
    /// Review graph smoothing level in octaves (0 = none, 1 = 1 octave, etc.)
    pub review_smoothing_octaves: f64,
    /// Selected channel index for review (0-based)
    pub review_selected_channel: usize,
    /// Interactive chart state for review graph (zoom/pan) - initialized lazily
    pub review_chart_state: Option<InteractiveChartStateWrapper>,
    /// Whether to auto-scale Y axis for review graph (if false, uses fixed range)
    pub review_y_axis_auto: bool,
    /// When true, normalize graphs relative to target curve (target becomes 0dB line)
    pub review_normalize_to_target: bool,
    /// Interactive chart state for progress chart (zoom/pan) - initialized lazily
    pub progress_chart_state: Option<InteractiveChartStateWrapper>,
    /// Custom target curve for manual entry mode
    pub custom_target_curve: CustomTargetCurve,

    /// When false (default), the Configure step shows only basic settings
    /// (mode, algorithm, num_filters, target_curve). Toggle to show all parameters.
    pub show_advanced_config: bool,
    /// Detail level for the configuration form (Simple / Intermediate / Expert)
    pub detail_level: sotf_audio_player::autoeq::DetailLevel,
    /// Currently selected preset id
    pub selected_preset: String,
    /// Wizard mode selected in the Process step.
    pub wizard_mode: RoomEqWizardMode,
    /// Simple-wizard collected choices (only used when wizard_mode == Simple).
    pub simple_preset: SimplePresetConfig,

    // === Multi-position data detection ===
    /// Whether loaded data has multi-position measurements (MeasurementSource::Multiple)
    pub has_multi_position_data: bool,
    /// Per-speaker measurement counts: (channel_name, count)
    pub multi_position_counts: Vec<(String, usize)>,
}

impl Default for RoomEqState {
    fn default() -> Self {
        Self {
            step: RoomEqStep::LoadData,
            data_source: RoomEqDataSource::FromRecording,
            channel_measurements: Vec::new(),
            ctc_measurements: None,
            ctc_config: None,
            delay_detection: DelayDetectionState::default(),
            speaker_configs: Vec::new(),
            optimizer_config: RoomEqOptimizerConfig::default(),
            optimization_status: OptimizationStatus::Idle,
            cancel_requested: Arc::new(AtomicBool::new(false)),
            current_channel: None,
            channel_results: Vec::new(),
            overall_progress: 0.0,
            progress_history: Vec::new(),
            current_iteration: 0,
            current_loss: 0.0,
            dsp_output: None,
            export_format_index: 0,
            dropdowns: RoomEqDropdowns::default(),
            status_message: String::new(),
            error_message: None,
            review_smoothing_octaves: 1.0, // Default to 1 octave smoothing
            review_selected_channel: 0,
            review_chart_state: None,
            review_y_axis_auto: true,
            review_normalize_to_target: false,
            progress_chart_state: None,
            custom_target_curve: CustomTargetCurve::new_flat(),
            show_advanced_config: false,
            wizard_mode: RoomEqWizardMode::default(),
            simple_preset: SimplePresetConfig::default(),
            detail_level: sotf_audio_player::autoeq::DetailLevel::Simple,
            selected_preset: "full-range".to_string(),
            has_multi_position_data: false,
            multi_position_counts: Vec::new(),
        }
    }
}

impl RoomEqState {
    /// Check if we have measurement data loaded
    pub fn has_measurements(&self) -> bool {
        !self.channel_measurements.is_empty()
    }

    /// Get the number of channels
    pub fn channel_count(&self) -> usize {
        self.channel_measurements.len()
    }

    /// Check if any channel is a subwoofer (LFE, Sub, SW)
    pub fn has_subwoofer(&self) -> bool {
        self.channel_names().iter().any(|name| {
            let upper = name.to_uppercase();
            upper == "LFE" || upper == "SUB" || upper == "SW" || upper.starts_with("SUB")
        })
    }

    /// Check if the setup is surround (3+ channels, excluding subs)
    pub fn is_surround(&self) -> bool {
        let non_sub_count = self
            .channel_names()
            .iter()
            .filter(|name| {
                let upper = name.to_uppercase();
                upper != "LFE" && upper != "SUB" && upper != "SW" && !upper.starts_with("SUB")
            })
            .count();
        non_sub_count >= 3
    }

    /// Check if any channel has phase data
    pub fn has_phase_data(&self) -> bool {
        self.channel_measurements
            .iter()
            .any(|m| !m.measurement.phase_deg.is_empty())
    }

    /// Check if any channel is a multi-driver group
    pub fn has_multi_driver(&self) -> bool {
        self.channel_measurements.iter().any(|m| m.is_group)
    }

    /// Check if multi-position measurement data is available
    pub fn has_multiple_measurements(&self) -> bool {
        self.has_multi_position_data
    }

    /// Height channel names used for Voice of God detection
    const HEIGHT_CHANNELS: &[&str] = &[
        "TFL", "TFR", "TSL", "TSR", "TBL", "TBR", "VOG", "TFC", "TBC", "TSC",
    ];

    /// Check if measurement has height channels (for VoG)
    pub fn has_height_channels(&self) -> bool {
        self.channel_names().iter().any(|name| {
            let upper = name.to_uppercase();
            Self::HEIGHT_CHANNELS.iter().any(|&h| upper == h)
        })
    }

    /// Check if the setup is home cinema (5+ non-sub channels)
    pub fn is_home_cinema(&self) -> bool {
        let non_sub_count = self
            .channel_names()
            .iter()
            .filter(|name| {
                let upper = name.to_uppercase();
                upper != "LFE" && upper != "SUB" && upper != "SW" && !upper.starts_with("SUB")
            })
            .count();
        non_sub_count >= 5
    }

    /// Apply smart defaults based on loaded measurement data.
    /// Called after loading measurements to set sensible initial values.
    pub fn apply_smart_defaults(&mut self, playback_sample_rate: Option<u32>) {
        use sotf_audio_player::room_eq_types::ChannelMetadata;
        let meta = ChannelMetadata {
            channel_names: self.channel_names(),
            playback_sample_rate,
        };
        self.optimizer_config.apply_smart_defaults(&meta);
    }

    /// Get channel names from speaker configs
    pub fn channel_names(&self) -> Vec<String> {
        self.speaker_configs
            .iter()
            .map(|c| c.channel_name.clone())
            .collect()
    }

    /// Check if optimization is complete
    pub fn is_optimization_complete(&self) -> bool {
        self.optimization_status == OptimizationStatus::Completed
    }

    /// Check if optimization is running
    pub fn is_optimizing(&self) -> bool {
        self.optimization_status == OptimizationStatus::Running
    }

    /// Initialize speaker configs from measurements
    pub fn init_speaker_configs(&mut self) {
        self.speaker_configs = self
            .channel_measurements
            .iter()
            .map(|m| RoomEqSpeakerConfig {
                channel_name: m.channel_name.clone(),
                config_type: if m.is_group {
                    SpeakerConfigType::MultiDriver
                } else {
                    SpeakerConfigType::Single
                },
                crossover_type: CrossoverType::LR24,
                driver_names: if m.is_group {
                    m.group_drivers
                        .iter()
                        .enumerate()
                        .map(|(i, _)| format!("driver_{}", i + 1))
                        .collect()
                } else {
                    Vec::new()
                },
                crossover_freq_hints: Vec::new(),
            })
            .collect();
    }

    /// Load measurements from recording state.
    ///
    /// Groups multi-mic recordings by speaker index so that each physical channel
    /// produces one `ChannelMeasurement` with additional mic data stored in
    /// `multi_mic_measurements` for multi-position optimization.
    pub fn load_from_recording(&mut self, recording_state: &RecordingState) {
        use std::collections::BTreeMap;

        // Group completed recordings by speaker index (channel_index)
        let mut grouped: BTreeMap<
            usize,
            Vec<&sotf_audio_player::recording_types::ChannelRecording>,
        > = BTreeMap::new();
        for r in &recording_state.channel_recordings {
            if r.result.is_some() {
                grouped.entry(r.channel_index).or_default().push(r);
            }
        }

        self.channel_measurements = grouped
            .into_values()
            .map(|recordings| {
                let first = recordings[0];
                // Strip the first parenthetical suffix — handles
                // " (Mic N)", " (Pos N)", and " (Pos N / Mic M)" naming.
                let base_name = first
                    .channel_name
                    .find(" (")
                    .map_or(first.channel_name.as_str(), |pos| {
                        &first.channel_name[..pos]
                    })
                    .to_string();

                let primary_result = first.result.clone().unwrap();
                let multi_mic = if recordings.len() > 1 {
                    recordings
                        .iter()
                        .skip(1)
                        .filter_map(|r| r.result.clone())
                        .collect()
                } else {
                    Vec::new()
                };

                ChannelMeasurement {
                    channel_name: base_name,
                    measurement: primary_result,
                    is_group: false,
                    group_drivers: Vec::new(),
                    multi_mic_measurements: multi_mic,
                }
            })
            .collect();

        let mut ctc_exported_raw = false;
        let mut ctc_raw_sweep_range = None;
        self.ctc_measurements = recording_state
            .recording_directory
            .as_ref()
            .and_then(|dir| {
                let speaker_names: Vec<String> = recording_state
                    .playback_config
                    .channel_mappings
                    .iter()
                    .map(|m| m.group_name.clone())
                    .collect();
                let mic_names = vec!["left_ear".to_string(), "right_ear".to_string()];
                let output_dir = std::path::Path::new(dir);
                if recording_state.recording_config.ctc_matrix_strategy
                    == sotf_audio_player::recording_types::CtcMatrixExportStrategy::RawSweep
                {
                    match RoomEqMeasurementsFile::build_ctc_measurements_from_recordings_with_strategy(
                        &recording_state.channel_recordings,
                        &speaker_names,
                        &mic_names,
                        recording_state.recording_config.sample_rate,
                        output_dir,
                        recording_state.recording_config.ctc_matrix_strategy,
                        recording_state.recording_config.ctc_loopback_input_channel,
                        &recording_state.transfer_matrix_loopbacks,
                    ) {
                        Ok(Some(measurements)) => match sotf_audio_player::room_eq_types::ctc_uniform_sweep_range_for_measurements(
                            &recording_state.channel_recordings,
                            &speaker_names,
                            &measurements,
                        ) {
                            Some(range) => {
                                ctc_exported_raw = true;
                                ctc_raw_sweep_range = Some(range);
                                Some(measurements)
                            }
                            None => {
                                log::warn!(
                                    "Raw-sweep CTC matrix mixes sweep ranges or references missing recordings; falling back to measured impulse-response export"
                                );
                                None
                            }
                        },
                        Ok(None) => {
                            log::warn!(
                                "Raw-sweep CTC matrix is incomplete; falling back to measured impulse-response export"
                            );
                            None
                        }
                        Err(e) => {
                            log::warn!("Could not export raw-sweep CTC transfer matrix: {}", e);
                            None
                        }
                    }
                } else {
                    None
                }
                .or_else(|| {
                    match RoomEqMeasurementsFile::build_ctc_measurements_from_recordings(
                        &recording_state.channel_recordings,
                        &speaker_names,
                        &mic_names,
                        recording_state.recording_config.sample_rate,
                        output_dir,
                    ) {
                        Ok(measurements) => measurements,
                        Err(e) => {
                            log::warn!("Could not export CTC transfer matrix: {}", e);
                            None
                        }
                    }
                })
            });
        self.ctc_config = self.ctc_measurements.clone().map(|measurements| {
            let raw = ctc_exported_raw && recording_state.ctc_reference_sweep_path.is_some();
            autoeq::roomeq::CtcConfig {
                enabled: true,
                matrix_source: if raw { "raw_sweep" } else { "measured" }.to_string(),
                measurements: Some(measurements),
                reference_sweep: if raw {
                    recording_state
                        .ctc_reference_sweep_path
                        .as_ref()
                        .map(std::path::PathBuf::from)
                } else {
                    None
                },
                sweep_duration_s: if raw {
                    Some(recording_state.signal_duration_secs as f64)
                } else {
                    None
                },
                sweep_start_hz: if raw {
                    ctc_raw_sweep_range.map(|(start, _)| start as f64)
                } else {
                    None
                },
                sweep_end_hz: if raw {
                    ctc_raw_sweep_range.map(|(_, end)| end as f64)
                } else {
                    None
                },
                ..Default::default()
            }
        });

        self.data_source = RoomEqDataSource::FromRecording;
        self.init_speaker_configs();
    }

    /// Reset optimization state
    pub fn reset_optimization(&mut self) {
        self.optimization_status = OptimizationStatus::Idle;
        self.cancel_requested
            .store(false, std::sync::atomic::Ordering::Relaxed);
        self.current_channel = None;
        self.channel_results.clear();
        self.overall_progress = 0.0;
        self.progress_history.clear();
        self.current_iteration = 0;
        self.current_loss = 0.0;
        self.error_message = None;
    }

    /// Get average pre-score
    pub fn average_pre_score(&self) -> f64 {
        if self.channel_results.is_empty() {
            0.0
        } else {
            self.channel_results
                .iter()
                .map(|r| r.pre_score)
                .sum::<f64>()
                / self.channel_results.len() as f64
        }
    }

    /// Get average post-score
    pub fn average_post_score(&self) -> f64 {
        if self.channel_results.is_empty() {
            0.0
        } else {
            self.channel_results
                .iter()
                .map(|r| r.post_score)
                .sum::<f64>()
                / self.channel_results.len() as f64
        }
    }

    /// Convert UI state to backend RoomConfig
    pub fn to_room_config(&self) -> RoomConfig {
        let mut speakers: HashMap<String, SpeakerConfig> = HashMap::new();
        let mut crossovers: HashMap<String, BackendCrossoverConfig> = HashMap::new();

        // Helper to convert measurement to curve (preserving phase if available)
        let to_curve = |meas: &ChannelMeasurement| -> autoeq::Curve {
            let frequencies: Vec<f64> = meas
                .measurement
                .frequencies
                .iter()
                .map(|&f| f as f64)
                .collect();
            let magnitude_db: Vec<f64> = meas
                .measurement
                .magnitude_db
                .iter()
                .map(|&db| db as f64)
                .collect();
            let phase = if !meas.measurement.phase_deg.is_empty()
                && meas.measurement.phase_deg.len() == frequencies.len()
            {
                Some(ndarray::Array1::from_vec(
                    meas.measurement
                        .phase_deg
                        .iter()
                        .map(|&p| p as f64)
                        .collect(),
                ))
            } else {
                None
            };

            autoeq::Curve {
                freq: ndarray::Array1::from_vec(frequencies),
                spl: ndarray::Array1::from_vec(magnitude_db),
                phase,
                ..Default::default()
            }
        };

        // Helper to convert recording result to curve (preserving phase if available)
        let result_to_curve = |res: &RecordingResult| -> autoeq::Curve {
            let frequencies: Vec<f64> = res.frequencies.iter().map(|&f| f as f64).collect();
            let magnitude_db: Vec<f64> = res.magnitude_db.iter().map(|&db| db as f64).collect();
            let phase = if !res.phase_deg.is_empty() && res.phase_deg.len() == frequencies.len() {
                Some(ndarray::Array1::from_vec(
                    res.phase_deg.iter().map(|&p| p as f64).collect(),
                ))
            } else {
                None
            };

            autoeq::Curve {
                freq: ndarray::Array1::from_vec(frequencies),
                spl: ndarray::Array1::from_vec(magnitude_db),
                phase,
                ..Default::default()
            }
        };

        // Iterate over configured speakers
        for speaker_config in &self.speaker_configs {
            let channel_name = &speaker_config.channel_name;

            // Find corresponding measurement
            if let Some(meas) = self
                .channel_measurements
                .iter()
                .find(|m| &m.channel_name == channel_name)
            {
                match speaker_config.config_type {
                    SpeakerConfigType::Single => {
                        if meas.multi_mic_measurements.is_empty() {
                            let curve = to_curve(meas);
                            speakers.insert(
                                channel_name.clone(),
                                SpeakerConfig::Single(MeasurementSource::InMemory(curve)),
                            );
                        } else {
                            // Multiple mic measurements → InMemoryMultiple for multi-position optimization
                            let mut curves = vec![to_curve(meas)];
                            for extra in &meas.multi_mic_measurements {
                                curves.push(result_to_curve(extra));
                            }
                            speakers.insert(
                                channel_name.clone(),
                                SpeakerConfig::Single(MeasurementSource::InMemoryMultiple(curves)),
                            );
                        }
                    }
                    SpeakerConfigType::MultiDriver => {
                        let mut driver_measurements = Vec::new();
                        if meas.is_group && !meas.group_drivers.is_empty() {
                            for driver_res in &meas.group_drivers {
                                driver_measurements
                                    .push(MeasurementSource::InMemory(result_to_curve(driver_res)));
                            }
                        } else {
                            driver_measurements.push(MeasurementSource::InMemory(to_curve(meas)));
                        }

                        let xover_id = format!("xover_{}", channel_name);
                        let xover_type = match speaker_config.crossover_type {
                            CrossoverType::LR12 => "LR12",
                            CrossoverType::LR24 => "LR24",
                            CrossoverType::LR48 => "LR48",
                            CrossoverType::Butterworth12 => "Butterworth12",
                            CrossoverType::Butterworth24 => "Butterworth24",
                        };

                        crossovers.insert(
                            xover_id.clone(),
                            BackendCrossoverConfig {
                                crossover_type: xover_type.to_string(),
                                frequency: None,
                                frequencies: None,
                                frequency_range: None,
                            },
                        );

                        speakers.insert(
                            channel_name.clone(),
                            SpeakerConfig::Group(SpeakerGroup {
                                name: channel_name.clone(),
                                speaker_name: None,
                                measurements: driver_measurements,
                                crossover: Some(xover_id),
                            }),
                        );
                    }
                }
            }
        }

        let optimizer = self.optimizer_config.to_optimizer_config();

        log::info!(
            "RoomConfig: filters={}, max_q={}, max_freq={}, schroeder={}, target_response={}, excursion={}, broadband={}, imported={}",
            optimizer.num_filters,
            optimizer.max_q,
            optimizer.max_freq,
            optimizer.schroeder_split.is_some(),
            optimizer.target_response.is_some(),
            optimizer.excursion_protection.is_some(),
            optimizer
                .target_response
                .as_ref()
                .is_some_and(|tr| tr.broadband_precorrection),
            self.optimizer_config.imported_from_file,
        );

        RoomConfig {
            version: autoeq::roomeq::default_config_version(),
            system: None,
            speakers,
            crossovers: Some(crossovers),
            target_curve: None,
            optimizer,
            recording_config: None,
            ctc: self.ctc_config.clone().or_else(|| {
                self.ctc_measurements
                    .clone()
                    .map(|measurements| autoeq::roomeq::CtcConfig {
                        enabled: true,
                        matrix_source: "measured".to_string(),
                        measurements: Some(measurements),
                        ..Default::default()
                    })
            }),
            cea2034_cache: None,
        }
    }

    /// Validate the current configuration
    pub fn validate(&self) -> autoeq::roomeq::ValidationResult {
        let config = self.to_room_config();
        autoeq::roomeq::validate_room_config(&config)
    }

    /// Calculate the level offset needed to normalize a curve to 0dB.
    /// Uses mean SPL in the 1kHz to 2kHz range by default.
    pub fn calculate_normalization_offset(frequencies: &[f64], spl: &[f64]) -> f64 {
        let min_freq = 1000.0;
        let max_freq = 2000.0;

        let mut sum = 0.0;
        let mut count = 0;

        for (i, &f) in frequencies.iter().enumerate() {
            if f >= min_freq
                && f <= max_freq
                && let Some(&db) = spl.get(i)
            {
                sum += db;
                count += 1;
            }
        }

        if count > 0 {
            sum / count as f64
        } else {
            // Fallback: overall mean
            if spl.is_empty() {
                0.0
            } else {
                spl.iter().sum::<f64>() / spl.len() as f64
            }
        }
    }

    /// Normalize a set of points by subtracting an offset.
    pub fn normalize_points(points: &[(f64, f64)], offset: f64) -> Vec<(f64, f64)> {
        points.iter().map(|&(f, db)| (f, db - offset)).collect()
    }

    /// Compute the average slope for L and R channels in dB/octave.
    pub fn compute_lr_slope(&self) -> Option<(f64, f64, f64)> {
        sotf_audio_player::room_eq_types::compute_lr_slope(&self.channel_measurements)
    }
}
