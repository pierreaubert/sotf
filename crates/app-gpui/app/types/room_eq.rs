// ============================================================================
// Room EQ Screen Types
// ============================================================================

use super::recording::{RecordingResult, RecordingState};
use autoeq::roomeq::{
    CrossoverConfig as BackendCrossoverConfig,
    ExcursionProtectionConfig as BackendExcursionProtectionConfig, FirConfig as BackendFirConfig,
    HighFreqFilterConfig, HighpassType, LowFreqFilterConfig, MeasurementSource,
    MixedPhaseSerdeConfig as BackendMixedPhaseConfig, MultiMeasurementConfig,
    MultiMeasurementStrategy, MultiSeatConfig as BackendMultiSeatConfig, MultiSeatStrategy,
    OptimizerConfig as BackendOptimizerConfig, PhaseAlignmentConfig as BackendPhaseAlignmentConfig,
    PreRingingSerdeConfig as BackendPreRingingConfig, ProcessingMode as BackendProcessingMode,
    RoomConfig, SchroederSplitConfig as BackendSchroederSplitConfig, SpeakerConfig, SpeakerGroup,
    TargetTiltConfig as BackendTargetTiltConfig, TiltType,
};
use std::collections::HashMap;

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
pub use sotf_audio_player::room_eq_types::{
    BroadbandTargetMatchingConfig, ChannelDspChain, ChannelMatchingUiConfig, ChannelMeasurement,
    ChannelOptResult, CustomTargetCurve, DelayDetectionState, DelayDetectionStatus, DriverDspChain,
    DspChainMetadata, DspChainOutput, DspPluginConfig, EqFilterConfig, ExcursionProtectionConfig,
    GroupDelayOptConfig, MixedModeUiConfig, MixedPhaseUiConfig, MultiMeasurementUiConfig,
    MultiSeatConfig, MultiSpeakerMode, PhaseAlignmentConfig, PreRingingConfig,
    RecordingConfiguration, RoomEqDataSource, RoomEqFirConfig, RoomEqMeasurementsFile,
    RoomEqOptimizationMode, RoomEqSpeakerConfig, RoomEqStep, SchroederSplitConfig,
    SpeakerConfigType, SubOptimizerUiConfig, TargetCurveControlPoint, TargetTiltConfig, VoGConfig,
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
    /// Currently optimizing channel name
    pub current_channel: Option<String>,
    /// Per-channel optimization results
    pub channel_results: Vec<ChannelOptResult>,
    /// Overall progress (0.0 - 1.0)
    pub overall_progress: f32,
    /// Progress history for visualization: (iteration, loss, channel_name)
    pub progress_history: Vec<(usize, f64, String)>,
    /// Current iteration number
    pub current_iteration: usize,
    /// Current loss value
    pub current_loss: f64,

    // === Step 5: Export ===
    /// Generated DSP chain output
    pub dsp_output: Option<DspChainOutput>,

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
            delay_detection: DelayDetectionState::default(),
            speaker_configs: Vec::new(),
            optimizer_config: RoomEqOptimizerConfig::default(),
            optimization_status: OptimizationStatus::Idle,
            current_channel: None,
            channel_results: Vec::new(),
            overall_progress: 0.0,
            progress_history: Vec::new(),
            current_iteration: 0,
            current_loss: 0.0,
            dsp_output: None,
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
    pub fn apply_smart_defaults(&mut self) {
        // Compute detection values before mutably borrowing optimizer_config
        let is_surround = self.is_surround();
        let has_subwoofer = self.has_subwoofer();
        let has_height = self.has_height_channels();
        let is_cinema = self.is_home_cinema();

        let config = &mut self.optimizer_config;

        // Loss type is always flat for room EQ
        config.loss_type = "flat".to_string();

        // Only override algorithm/seed defaults when not imported from file
        if !config.imported_from_file {
            config.local_algo = "cobyla".to_string();
            config.refine = true;
            config.seed = None;
        }

        // System type: auto-detect from channel count
        config.system_type = if is_surround {
            "multichannel".to_string()
        } else {
            "stereo".to_string()
        };

        // Feature flags: only auto-enable when NOT imported from file.
        // When imported, the file's feature state is authoritative
        // (None = disabled, Some = enabled with those params).
        if !config.imported_from_file {
            config.target_tilt.enabled = true;
            config.target_tilt.tilt_type = "harman".to_string();
            config.excursion_protection.enabled = true;
            config.schroeder_split.enabled = true;
            config.allow_delay = true;
            config.broadband_target_matching.enabled = true;
            config.gd_opt.enabled = has_subwoofer;
            config.vog.enabled = has_height;
            config.vog.reference_channel = if is_cinema {
                "C".to_string()
            } else {
                "L".to_string()
            };
        }
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
                // Strip " (Mic N)" suffix for the channel name
                let base_name = first
                    .channel_name
                    .find(" (Mic ")
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

        self.data_source = RoomEqDataSource::FromRecording;
        self.init_speaker_configs();
    }

    /// Reset optimization state
    pub fn reset_optimization(&mut self) {
        self.optimization_status = OptimizationStatus::Idle;
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

        let algorithm = self.optimizer_config.algorithm.clone();

        let processing_mode = match self.optimizer_config.mode {
            RoomEqOptimizationMode::Iir => BackendProcessingMode::LowLatency,
            RoomEqOptimizationMode::Fir => BackendProcessingMode::PhaseLinear,
            RoomEqOptimizationMode::Mixed => BackendProcessingMode::Hybrid,
            RoomEqOptimizationMode::MixedPhase => BackendProcessingMode::MixedPhase,
        };

        let optimizer = BackendOptimizerConfig {
            loss_type: self.optimizer_config.loss_type.clone(),
            algorithm,
            strategy: self.optimizer_config.strategy.clone(),
            num_filters: self.optimizer_config.num_filters,
            min_q: self.optimizer_config.min_q,
            max_q: self.optimizer_config.max_q,
            min_db: self.optimizer_config.min_db,
            max_db: self.optimizer_config.max_db,
            min_freq: self.optimizer_config.min_freq,
            max_freq: self.optimizer_config.max_freq,
            max_iter: self.optimizer_config.max_iter,
            population: self.optimizer_config.population,
            peq_model: self.optimizer_config.peq_model.clone(),
            mode: self.optimizer_config.mode.to_code().to_string(),
            processing_mode,
            fir: Some(BackendFirConfig {
                taps: self.optimizer_config.fir.taps,
                phase: self.optimizer_config.fir.phase.clone(),
                correct_excess_phase: self.optimizer_config.fir.correct_excess_phase,
                phase_smoothing: self.optimizer_config.fir.phase_smoothing,
                pre_ringing: self.optimizer_config.fir.pre_ringing.as_ref().map(|pr| {
                    BackendPreRingingConfig {
                        threshold_db: pr.threshold_db,
                        max_time_s: pr.max_time_s,
                    }
                }),
            }),
            mixed_phase: if self.optimizer_config.mode == RoomEqOptimizationMode::MixedPhase {
                Some(BackendMixedPhaseConfig {
                    max_fir_length_ms: self.optimizer_config.mixed_phase.max_fir_length_ms,
                    pre_ringing_threshold_db: self
                        .optimizer_config
                        .mixed_phase
                        .pre_ringing_threshold_db,
                    min_spatial_depth: self.optimizer_config.mixed_phase.min_spatial_depth,
                    phase_smoothing_octaves: self
                        .optimizer_config
                        .mixed_phase
                        .phase_smoothing_octaves,
                })
            } else {
                None
            },
            seed: self.optimizer_config.seed,
            mixed_config: if self.optimizer_config.mode == RoomEqOptimizationMode::Mixed {
                Some(autoeq::roomeq::MixedModeConfig {
                    crossover_freq: self.optimizer_config.mixed_config.crossover_freq,
                    crossover_type: self.optimizer_config.mixed_config.crossover_type.clone(),
                    fir_band: self.optimizer_config.mixed_config.fir_band.clone(),
                })
            } else {
                None
            },
            refine: self.optimizer_config.refine,
            local_algo: self.optimizer_config.local_algo.clone(),
            psychoacoustic: self.optimizer_config.psychoacoustic,
            asymmetric_loss: self.optimizer_config.asymmetric_loss,
            tolerance: self.optimizer_config.tolerance,
            atolerance: self.optimizer_config.atolerance,
            allow_delay: Some(self.optimizer_config.allow_delay),
            target_tilt: if self.optimizer_config.target_tilt.enabled {
                let tilt_type = match self.optimizer_config.target_tilt.tilt_type.as_str() {
                    "harman" => TiltType::Harman,
                    "custom" => TiltType::Custom,
                    _ => TiltType::Custom,
                };
                Some(BackendTargetTiltConfig {
                    tilt_type,
                    slope_db_per_octave: self.optimizer_config.target_tilt.slope,
                    reference_freq: self.optimizer_config.target_tilt.reference_freq,
                    bass_shelf_db: self.optimizer_config.target_tilt.bass_shelf_db,
                    bass_shelf_freq: self.optimizer_config.target_tilt.bass_shelf_freq,
                })
            } else {
                None
            },
            excursion_protection: if self.optimizer_config.excursion_protection.enabled {
                let filter_type = if self.optimizer_config.excursion_protection.filter_type == "bw"
                {
                    HighpassType::Butterworth
                } else {
                    HighpassType::LinkwitzRiley
                };
                Some(BackendExcursionProtectionConfig {
                    enabled: true,
                    auto_detect_f3: self.optimizer_config.excursion_protection.auto_detect_f3,
                    manual_f3_hz: Some(self.optimizer_config.excursion_protection.manual_f3_hz),
                    filter_order: self.optimizer_config.excursion_protection.filter_order,
                    filter_type,
                    margin_octaves: self.optimizer_config.excursion_protection.margin_octaves,
                })
            } else {
                None
            },
            schroeder_split: if self.optimizer_config.schroeder_split.enabled {
                Some(BackendSchroederSplitConfig {
                    enabled: true,
                    schroeder_freq: self.optimizer_config.schroeder_split.schroeder_freq,
                    room_dimensions: None,
                    low_freq_config: LowFreqFilterConfig {
                        max_q: self.optimizer_config.schroeder_split.low_freq_max_q,
                        min_q: 0.5,
                        allow_boost: self.optimizer_config.schroeder_split.low_freq_allow_boost,
                        max_db: self.optimizer_config.schroeder_split.low_freq_max_db,
                    },
                    high_freq_config: HighFreqFilterConfig {
                        max_q: self.optimizer_config.schroeder_split.high_freq_max_q,
                        shelving_only: self
                            .optimizer_config
                            .schroeder_split
                            .high_freq_shelving_only,
                    },
                })
            } else {
                None
            },
            phase_alignment: if self.optimizer_config.phase_alignment.enabled {
                Some(BackendPhaseAlignmentConfig {
                    enabled: true,
                    min_freq: self.optimizer_config.phase_alignment.min_freq,
                    max_freq: self.optimizer_config.phase_alignment.max_freq,
                    optimize_polarity: self.optimizer_config.phase_alignment.optimize_polarity,
                    max_delay_ms: self.optimizer_config.phase_alignment.max_delay_ms,
                })
            } else {
                None
            },
            multi_seat: if self.optimizer_config.multi_seat.enabled {
                let strategy = match self.optimizer_config.multi_seat.strategy.as_str() {
                    "primary" => MultiSeatStrategy::PrimaryWithConstraints,
                    "average" => MultiSeatStrategy::Average,
                    _ => MultiSeatStrategy::MinimizeVariance,
                };
                Some(BackendMultiSeatConfig {
                    enabled: true,
                    strategy,
                    primary_seat: self.optimizer_config.multi_seat.primary_seat,
                    max_deviation_db: self.optimizer_config.multi_seat.max_deviation_db,
                })
            } else {
                None
            },
            gd_opt: if self.optimizer_config.gd_opt.enabled {
                Some(autoeq::roomeq::GroupDelayOptimizationConfig {
                    enabled: true,
                    target_ms: self.optimizer_config.gd_opt.target_ms,
                })
            } else {
                None
            },
            vog: if self.optimizer_config.vog.enabled {
                Some(autoeq::roomeq::VoiceOfGodConfig {
                    enabled: true,
                    reference_channel: self.optimizer_config.vog.reference_channel.clone(),
                })
            } else {
                None
            },
            broadband_target_matching: if self.optimizer_config.broadband_target_matching.enabled {
                Some(autoeq::roomeq::BroadbandTargetMatchingConfig { enabled: true })
            } else {
                None
            },
            multi_measurement: if self.optimizer_config.multi_measurement.enabled {
                let strategy = match self.optimizer_config.multi_measurement.strategy.as_str() {
                    "average" => MultiMeasurementStrategy::Average,
                    "weighted_sum" => MultiMeasurementStrategy::WeightedSum,
                    "minimax" => MultiMeasurementStrategy::Minimax,
                    "variance_penalized" => MultiMeasurementStrategy::VariancePenalized,
                    "spatial_robustness" => MultiMeasurementStrategy::SpatialRobustness,
                    s => panic!("Unknown multi_measurement strategy: {s}"),
                };
                let weights = if self.optimizer_config.multi_measurement.weights.is_empty() {
                    None
                } else {
                    Some(self.optimizer_config.multi_measurement.weights.clone())
                };
                Some(MultiMeasurementConfig {
                    strategy,
                    weights,
                    variance_lambda: self.optimizer_config.multi_measurement.variance_lambda,
                    spatial_robustness: None,
                })
            } else {
                None
            },
            smooth_n: self.optimizer_config.smooth_n,
            decomposed_correction: None,
            target_response: None,
            cea2034_correction: None,
            sub_config: if self.optimizer_config.sub_config.enabled {
                Some(autoeq::roomeq::SubOptimizerConfig {
                    num_filters: self.optimizer_config.sub_config.num_filters,
                    max_db: self.optimizer_config.sub_config.max_db,
                    min_db: self.optimizer_config.sub_config.min_db,
                    min_q: self.optimizer_config.sub_config.min_q,
                    max_q: self.optimizer_config.sub_config.max_q,
                })
            } else {
                None
            },
            channel_matching: if self.optimizer_config.channel_matching.enabled {
                Some(autoeq::roomeq::ChannelMatchingConfig {
                    enabled: true,
                    threshold_db: self.optimizer_config.channel_matching.threshold_db,
                    max_filters: self.optimizer_config.channel_matching.max_filters,
                })
            } else {
                None
            },
            min_filter_improvement: 0.0,
            elimination_threshold: 0.0,
            ssir_wav_path: None,
            max_boost_envelope: None,
            min_cut_envelope: None,
            epa_config: None,
            phase_correction: None,
        };

        log::info!(
            "RoomConfig: filters={}, max_q={}, max_freq={}, schroeder={}, target_tilt={}, excursion={}, broadband={}, imported={}",
            optimizer.num_filters,
            optimizer.max_q,
            optimizer.max_freq,
            optimizer.schroeder_split.is_some(),
            optimizer.target_tilt.is_some(),
            optimizer.excursion_protection.is_some(),
            optimizer.broadband_target_matching.is_some(),
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
