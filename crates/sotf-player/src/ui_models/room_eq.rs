//! Shared, UI-agnostic view model for the Room EQ wizard.
//!
//! This module intentionally separates *domain* state (measurements, optimizer
//! config, optimization results) from *view* state (selected rows, edit buffers,
//! focus flags). The latter stays in `app-gpui` / `app-tui`.

use crate::autoeq::{PipelineStepId, PipelineStepStatus};
use crate::recording_types::{CtcMatrixExportStrategy, RecordingState};
use crate::room_eq_types::{
    ChannelMeasurement, ChannelMetadata, ChannelOptResult, CrossoverType, DelayDetectionState,
    DelayDetectionStatus, DspChainOutput, OptimizationStatus, RoomEqDataSource,
    RoomEqMeasurementsFile, RoomEqOptimizerConfig, RoomEqSpeakerConfig, RoomEqStep,
    RoomEqWizardMode, SimpleCrossoverChoice, SimplePresetConfig, apply_simple_preset,
    ctc_system_config_for_speaker_names, room_eq_channel_is_bass_output,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Domain state for the Room EQ wizard, independent of any UI toolkit.
#[derive(Debug, Clone)]
pub struct RoomEqScreenModel {
    /// Current step in the workflow.
    pub step: RoomEqStep,

    /// Source of the measurement data.
    pub data_source: RoomEqDataSource,

    /// Loaded channel measurements.
    pub channel_measurements: Vec<ChannelMeasurement>,

    /// Optional CTC transfer-matrix measurements captured by the Recording wizard.
    pub ctc_measurements: Option<autoeq::roomeq::CtcMeasurementConfig>,

    /// Parsed CTC configuration forwarded to the optimizer.
    pub ctc_config: Option<autoeq::roomeq::CtcConfig>,

    /// Original `system` block imported from a RoomConfig file.
    ///
    /// This carries home-cinema role mapping and bass-management policy.
    /// Rebuilding it from UI-only state can change the optimizer result.
    pub imported_system: Option<autoeq::roomeq::SystemConfig>,

    /// Original crossover map imported from a RoomConfig file.
    pub imported_crossovers: Option<HashMap<String, autoeq::roomeq::CrossoverConfig>>,

    /// Shared delay-detection state (probe form, results, overrides).
    pub delay_detection: DelayDetectionState,

    /// Per-channel speaker configurations.
    pub speaker_configs: Vec<RoomEqSpeakerConfig>,

    /// Global optimizer configuration.
    pub optimizer_config: RoomEqOptimizerConfig,

    /// Wizard mode selected in the Process step.
    pub wizard_mode: RoomEqWizardMode,

    /// Simple-wizard collected choices (only meaningful when `wizard_mode == Simple`).
    pub simple_preset: SimplePresetConfig,

    /// Current optimization status.
    pub optimization_status: OptimizationStatus,

    /// Cancel flag polled by the optimization callback.
    pub cancel_requested: Arc<AtomicBool>,

    /// Name of the channel currently being optimized, if any.
    pub current_channel: Option<String>,

    /// Per-channel optimization results.
    pub channel_results: Vec<ChannelOptResult>,

    /// Overall optimization progress (0.0 - 1.0).
    pub overall_progress: f32,

    /// Progress history: (iteration, loss, channel_name, optional_extra).
    pub progress_history: Vec<(usize, f64, String, Option<f64>)>,

    /// Current iteration number reported by the optimizer.
    pub current_iteration: usize,

    /// Current loss value reported by the optimizer.
    pub current_loss: f64,

    /// Pipeline step the optimizer is currently working on.
    pub current_pipeline_step: Option<PipelineStepId>,

    /// Latest reported status for every pipeline step touched in this run.
    pub step_history: HashMap<PipelineStepId, PipelineStepStatus>,

    /// Generated DSP chain output.
    pub dsp_output: Option<DspChainOutput>,

    /// Directory containing generated Room EQ convolution WAV sidecars.
    pub artifact_dir: Option<PathBuf>,

    /// Selected export format index.
    pub export_format_index: usize,

    /// Short status message surfaced by the view model.
    pub status_message: String,

    /// Active error message, if any.
    pub error_message: Option<String>,

    /// Whether loaded data has multi-position measurements.
    pub has_multi_position_data: bool,

    /// Per-speaker measurement counts: (channel_name, count).
    pub multi_position_counts: Vec<(String, usize)>,
}

impl Default for RoomEqScreenModel {
    fn default() -> Self {
        Self {
            step: RoomEqStep::LoadData,
            data_source: RoomEqDataSource::FromRecording,
            channel_measurements: Vec::new(),
            ctc_measurements: None,
            ctc_config: None,
            imported_system: None,
            imported_crossovers: None,
            delay_detection: DelayDetectionState::default(),
            speaker_configs: Vec::new(),
            optimizer_config: RoomEqOptimizerConfig::default(),
            wizard_mode: RoomEqWizardMode::Simple,
            simple_preset: SimplePresetConfig::default(),
            optimization_status: OptimizationStatus::Idle,
            cancel_requested: Arc::new(AtomicBool::new(false)),
            current_channel: None,
            channel_results: Vec::new(),
            overall_progress: 0.0,
            progress_history: Vec::new(),
            current_iteration: 0,
            current_loss: 0.0,
            current_pipeline_step: None,
            step_history: HashMap::new(),
            dsp_output: None,
            artifact_dir: None,
            export_format_index: 0,
            status_message: String::new(),
            error_message: None,
            has_multi_position_data: false,
            multi_position_counts: Vec::new(),
        }
    }
}

/// User intent events for the Room EQ wizard.
#[derive(Debug, Clone)]
pub enum RoomEqViewEvent {
    /// Jump directly to a workflow step.
    NavigateToStep(RoomEqStep),
    /// Move to the next workflow step.
    NextStep,
    /// Move to the previous workflow step.
    PreviousStep,

    /// Change the source of measurement data.
    SetDataSource(RoomEqDataSource),
    /// Replace the loaded measurements and re-derive speaker configs.
    LoadMeasurements(Vec<ChannelMeasurement>),
    /// Load measurements from a completed recording session.
    LoadFromRecording(Box<RecordingState>),
    /// Remove all loaded measurements.
    ClearMeasurements,

    /// Choose the simple/full wizard mode.
    SetWizardMode(RoomEqWizardMode),
    /// Apply a simple-wizard preset to the optimizer config.
    SetSimplePreset(SimplePresetConfig),
    /// Replace the entire optimizer configuration.
    SetOptimizerConfig(Box<RoomEqOptimizerConfig>),

    /// Start the optimization if measurements are loaded.
    StartOptimization,
    /// Request cancellation of a running optimization.
    CancelOptimization,
    /// Update progress reported by the optimization callback.
    SetOptimizationProgress {
        progress: f32,
        iteration: usize,
        loss: f64,
        current_channel: Option<String>,
        current_step: Option<PipelineStepId>,
        step_status: Option<(PipelineStepId, PipelineStepStatus)>,
    },
    /// Store final optimization results.
    SetOptimizationResults {
        channel_results: Vec<ChannelOptResult>,
        dsp_output: Box<DspChainOutput>,
        artifact_dir: PathBuf,
    },
    /// Mark the optimization as failed.
    SetOptimizationError(String),
    /// Clear optimization state without starting a new run.
    ResetOptimization,

    /// Choose an export format by index.
    SelectExportFormat(usize),
    /// Set the export file path.
    SetExportPath(PathBuf),
    /// Apply the generated DSP output to the current rack.
    RequestApplyToRack,

    /// Set a short status message.
    SetStatusMessage(String),
    /// Clear the active error message.
    ClearError,
}

/// Effects emitted by [`RoomEqScreenModel::apply`] for the UI shell to execute.
#[derive(Debug, Clone)]
pub enum RoomEqEffect {
    /// The shell should navigate to the given step.
    NavigateToStep(RoomEqStep),
    /// The shell should open a file picker for the measurement JSON.
    RequestMeasurementFilePicker,
    /// The shell should open a file picker for the export path.
    RequestExportPathPicker,
    /// The shell should start the optimizer, polling the cancel flag.
    StartOptimization(Arc<AtomicBool>),
    /// The shell should apply the given DSP output to the current rack.
    ApplyToRack(Box<DspChainOutput>),
    /// The shell should show a status message.
    ShowStatus(String),
    /// The shell should show an error message.
    ShowError(String),
}

impl RoomEqScreenModel {
    /// Apply a user-intent event to the model, returning side-effects for the UI shell.
    pub fn apply(&mut self, event: RoomEqViewEvent) -> Vec<RoomEqEffect> {
        let mut effects = Vec::new();

        match event {
            RoomEqViewEvent::NavigateToStep(step) => {
                self.step = step;
                effects.push(RoomEqEffect::NavigateToStep(step));
            }
            RoomEqViewEvent::NextStep => {
                if let Some(next) = self.step.next() {
                    self.step = next;
                    effects.push(RoomEqEffect::NavigateToStep(next));
                }
            }
            RoomEqViewEvent::PreviousStep => {
                if let Some(prev) = self.step.previous() {
                    self.step = prev;
                    effects.push(RoomEqEffect::NavigateToStep(prev));
                }
            }
            RoomEqViewEvent::SetDataSource(src) => {
                self.data_source = src;
            }
            RoomEqViewEvent::LoadMeasurements(measurements) => {
                self.channel_measurements = measurements;
                self.init_speaker_configs();
                self.apply_smart_defaults(None);
                self.refresh_position_metadata();
            }
            RoomEqViewEvent::LoadFromRecording(recording_state) => {
                self.load_from_recording(&recording_state);
                self.init_speaker_configs();
                self.apply_smart_defaults(None);
            }
            RoomEqViewEvent::ClearMeasurements => {
                self.channel_measurements.clear();
                self.speaker_configs.clear();
                self.has_multi_position_data = false;
                self.multi_position_counts.clear();
            }
            RoomEqViewEvent::SetWizardMode(mode) => {
                self.wizard_mode = mode;
            }
            RoomEqViewEvent::SetSimplePreset(preset) => {
                self.simple_preset = preset.clone();
                apply_simple_preset(&preset, &mut self.optimizer_config);
            }
            RoomEqViewEvent::SetOptimizerConfig(config) => {
                self.optimizer_config = *config;
            }
            RoomEqViewEvent::StartOptimization => {
                if self.channel_measurements.is_empty() {
                    self.error_message = Some("No measurements loaded".to_string());
                    effects.push(RoomEqEffect::ShowError(
                        "No measurements loaded".to_string(),
                    ));
                } else {
                    self.reset_optimization();
                    self.optimization_status = OptimizationStatus::Running;
                    self.cancel_requested = Arc::new(AtomicBool::new(false));
                    effects.push(RoomEqEffect::StartOptimization(
                        self.cancel_requested.clone(),
                    ));
                }
            }
            RoomEqViewEvent::CancelOptimization => {
                self.cancel_requested.store(true, Ordering::Relaxed);
            }
            RoomEqViewEvent::SetOptimizationProgress {
                progress,
                iteration,
                loss,
                current_channel,
                current_step,
                step_status,
            } => {
                self.overall_progress = progress.clamp(0.0, 1.0);
                self.current_iteration = iteration;
                self.current_loss = loss;
                self.current_channel = current_channel.clone();
                if let Some(step) = current_step {
                    self.current_pipeline_step = Some(step);
                }
                if let Some((step, status)) = step_status {
                    self.step_history.insert(step, status);
                }
                self.progress_history.push((
                    iteration,
                    loss,
                    current_channel.unwrap_or_default(),
                    None,
                ));
            }
            RoomEqViewEvent::SetOptimizationResults {
                channel_results,
                dsp_output,
                artifact_dir,
            } => {
                self.channel_results = channel_results;
                self.dsp_output = Some(*dsp_output);
                self.artifact_dir = Some(artifact_dir);
                self.optimization_status = OptimizationStatus::Completed;
                self.overall_progress = 1.0;
                self.status_message = "Optimization completed".to_string();
            }
            RoomEqViewEvent::SetOptimizationError(err) => {
                self.optimization_status = OptimizationStatus::Failed;
                self.error_message = Some(err.clone());
                effects.push(RoomEqEffect::ShowError(err));
            }
            RoomEqViewEvent::ResetOptimization => {
                self.reset_optimization();
            }
            RoomEqViewEvent::SelectExportFormat(idx) => {
                self.export_format_index = idx;
            }
            RoomEqViewEvent::SetExportPath(path) => {
                self.status_message = format!("Export path set to {}", path.display());
            }
            RoomEqViewEvent::RequestApplyToRack => {
                if let Some(out) = self.dsp_output.clone() {
                    effects.push(RoomEqEffect::ApplyToRack(Box::new(out)));
                } else {
                    let msg = "No DSP output to apply".to_string();
                    self.error_message = Some(msg.clone());
                    effects.push(RoomEqEffect::ShowError(msg));
                }
            }
            RoomEqViewEvent::SetStatusMessage(msg) => {
                self.status_message = msg;
            }
            RoomEqViewEvent::ClearError => {
                self.error_message = None;
            }
        }

        effects
    }

    /// Return true if measurement data is loaded.
    pub fn has_measurements(&self) -> bool {
        !self.channel_measurements.is_empty()
    }

    /// Number of loaded channels.
    pub fn channel_count(&self) -> usize {
        self.channel_measurements.len()
    }

    /// Names of configured channels.
    pub fn channel_names(&self) -> Vec<String> {
        self.speaker_configs
            .iter()
            .map(|c| c.channel_name.clone())
            .collect()
    }

    /// True when an optimization is running.
    pub fn is_optimizing(&self) -> bool {
        self.optimization_status == OptimizationStatus::Running
    }

    /// True when the last optimization completed successfully.
    pub fn is_optimization_complete(&self) -> bool {
        self.optimization_status == OptimizationStatus::Completed
    }

    /// Reset optimization state without modifying measurements or config.
    pub fn reset_optimization(&mut self) {
        self.optimization_status = OptimizationStatus::Idle;
        self.cancel_requested.store(false, Ordering::Relaxed);
        self.current_channel = None;
        self.channel_results.clear();
        self.overall_progress = 0.0;
        self.progress_history.clear();
        self.current_iteration = 0;
        self.current_loss = 0.0;
        self.current_pipeline_step = None;
        self.step_history.clear();
        self.dsp_output = None;
        self.artifact_dir = None;
        self.error_message = None;
    }

    /// Initialize speaker configs from loaded measurements.
    pub fn init_speaker_configs(&mut self) {
        use crate::room_eq_types::SpeakerConfigType;
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
                driver_names: if !m.driver_measurement_sets.is_empty() {
                    m.driver_measurement_sets
                        .iter()
                        .enumerate()
                        .map(|(index, driver)| {
                            driver
                                .name
                                .clone()
                                .unwrap_or_else(|| format!("driver_{}", index + 1))
                        })
                        .collect()
                } else if m.is_group {
                    m.group_drivers
                        .iter()
                        .enumerate()
                        .map(|(i, _)| format!("driver_{}", i + 1))
                        .collect()
                } else {
                    Vec::new()
                },
                crossover_freq_hints: Vec::new(),
                ..Default::default()
            })
            .collect();
    }

    /// Valid crossover-frequency hint bounds in Hz.
    pub const CROSSOVER_FREQ_MIN_HZ: f64 = 20.0;
    /// Valid crossover-frequency hint bounds in Hz.
    pub const CROSSOVER_FREQ_MAX_HZ: f64 = 20000.0;
    /// Default hint used when materializing a missing entry for editing.
    pub const CROSSOVER_FREQ_DEFAULT_HZ: f64 = 2000.0;

    /// Number of crossover frequencies expected for a channel.
    ///
    /// A multi-driver channel with `D` drivers needs `D - 1` crossovers.
    /// Single-driver channels and unknown indices expect zero.
    pub fn expected_crossover_count(&self, channel_idx: usize) -> usize {
        let Some(config) = self.speaker_configs.get(channel_idx) else {
            return 0;
        };
        if config.config_type != crate::room_eq_types::SpeakerConfigType::MultiDriver {
            return 0;
        }
        config.driver_names.len().saturating_sub(1)
    }

    /// Validated write for one crossover-frequency hint.
    ///
    /// Extends the hint vector with the default frequency when the entry is
    /// missing. Returns false and mutates nothing for unknown channels,
    /// single-driver channels, out-of-range crossover indices, or
    /// non-finite/out-of-bounds frequencies.
    pub fn set_crossover_freq_hint(
        &mut self,
        channel_idx: usize,
        crossover_idx: usize,
        freq_hz: f64,
    ) -> bool {
        if !freq_hz.is_finite()
            || freq_hz < Self::CROSSOVER_FREQ_MIN_HZ
            || freq_hz > Self::CROSSOVER_FREQ_MAX_HZ
        {
            return false;
        }
        if crossover_idx >= self.expected_crossover_count(channel_idx) {
            return false;
        }
        let Some(config) = self.speaker_configs.get_mut(channel_idx) else {
            return false;
        };
        if config.crossover_freq_hints.len() <= crossover_idx {
            config
                .crossover_freq_hints
                .resize(crossover_idx + 1, Self::CROSSOVER_FREQ_DEFAULT_HZ);
        }
        config.crossover_freq_hints[crossover_idx] = freq_hz;
        true
    }

    /// Apply smart defaults based on loaded measurement metadata.
    pub fn apply_smart_defaults(&mut self, playback_sample_rate: Option<u32>) {
        let meta = ChannelMetadata {
            channel_names: self.channel_names(),
            playback_sample_rate,
        };
        self.optimizer_config.apply_smart_defaults(&meta);
    }

    /// Check if any channel is a subwoofer (LFE, Sub, SW).
    pub fn has_subwoofer(&self) -> bool {
        self.channel_names().iter().any(|name| {
            let upper = name.to_uppercase();
            upper == "LFE" || upper == "SUB" || upper == "SW" || upper.starts_with("SUB")
        })
    }

    /// Check if the setup is surround (3+ channels, excluding subs).
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

    /// Check if any channel has phase data.
    pub fn has_phase_data(&self) -> bool {
        self.channel_measurements
            .iter()
            .any(|m| !m.measurement.phase_deg.is_empty())
    }

    /// Check if any channel is a multi-driver group.
    pub fn has_multi_driver(&self) -> bool {
        self.channel_measurements.iter().any(|m| m.is_group)
    }

    /// Check if multi-position measurement data is available.
    pub fn has_multiple_measurements(&self) -> bool {
        self.has_multi_position_data
    }

    fn refresh_position_metadata(&mut self) {
        self.multi_position_counts = self
            .channel_measurements
            .iter()
            .filter_map(|channel| {
                // Drivers share positions; adding their counts would count each seat twice.
                let count = channel
                    .driver_measurement_sets
                    .iter()
                    .map(|driver| 1 + driver.multi_mic_measurements.len())
                    .chain(std::iter::once(1 + channel.multi_mic_measurements.len()))
                    .max()
                    .unwrap_or(1);
                (count > 1).then(|| (channel.channel_name.clone(), count))
            })
            .collect();
        self.has_multi_position_data = !self.multi_position_counts.is_empty();
    }

    /// Height channel names used for Voice of God detection.
    const HEIGHT_CHANNELS: &[&str] = &[
        "TFL", "TFR", "TSL", "TSR", "TBL", "TBR", "VOG", "TFC", "TBC", "TSC",
    ];

    /// Check if measurement has height channels (for VoG).
    pub fn has_height_channels(&self) -> bool {
        self.channel_names().iter().any(|name| {
            let upper = name.to_uppercase();
            Self::HEIGHT_CHANNELS.iter().any(|&h| upper == h)
        })
    }

    /// Check if the setup is home cinema (5+ non-sub channels).
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

    /// Get average pre-score.
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

    /// Get average post-score.
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

    /// Load measurements from recording state.
    ///
    /// Groups multi-mic recordings by speaker index so that each physical channel
    /// produces one `ChannelMeasurement` with additional mic data stored in
    /// `multi_mic_measurements` for multi-position optimization.
    pub fn load_from_recording(&mut self, recording_state: &RecordingState) {
        self.load_recording_with_channels(recording_state, None, None);
    }

    pub fn load_from_imported_recording(
        &mut self,
        recording_state: &RecordingState,
        imported: &crate::recording_types::RecordingImport,
    ) -> Result<(), String> {
        let channels = imported.channels_with_recordings(&recording_state.channel_recordings)?;
        self.load_recording_with_channels(recording_state, Some(channels), Some(imported));
        Ok(())
    }

    fn load_recording_with_channels(
        &mut self,
        recording_state: &RecordingState,
        imported_channels: Option<Vec<ChannelMeasurement>>,
        imported: Option<&crate::recording_types::RecordingImport>,
    ) {
        use std::collections::BTreeMap;

        // Metadata belongs to this source, including an explicit absence. Clear
        // settings retained from a previous file when loading captured takes.
        self.imported_system = imported.and_then(|source| source.configuration.system.clone());
        self.imported_crossovers =
            imported.and_then(|source| source.configuration.crossovers.clone());

        // Group completed recordings by speaker index (channel_index)
        let mut grouped: BTreeMap<usize, Vec<&crate::recording_types::ChannelRecording>> =
            BTreeMap::new();
        for r in &recording_state.channel_recordings {
            if r.result.is_some() {
                grouped.entry(r.channel_index).or_default().push(r);
            }
        }

        self.channel_measurements = imported_channels.unwrap_or_else(|| {
            grouped
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
                        acquisition: None,
                        driver_measurement_sets: Vec::new(),
                        provenance: recordings
                            .iter()
                            .map(|recording| crate::room_eq_types::MeasurementProvenance {
                                name: Some(recording.channel_name.clone()),
                                mic_index: Some(recording.mic_index),
                                mic_position_index: Some(recording.mic_position_index),
                            })
                            .collect(),
                        channel_name: base_name,
                        measurement: primary_result,
                        is_group: false,
                        group_drivers: Vec::new(),
                        multi_mic_measurements: multi_mic,
                    }
                })
                .collect()
        });
        normalize_channel_measurement_grids(&mut self.channel_measurements);
        self.refresh_position_metadata();

        if let Some(imported) = imported {
            // Imported CTC references already have source-relative paths resolved.
            // Loading a session must not regenerate files from current devices.
            self.ctc_config = imported.configuration.ctc.clone();
            self.ctc_measurements = self
                .ctc_config
                .as_ref()
                .and_then(|config| config.measurements.clone());
        } else {
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
                    == CtcMatrixExportStrategy::RawSweep
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
                        Ok(Some(measurements)) => match crate::room_eq_types::ctc_uniform_sweep_range_for_measurements(
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
                    enabled: false,
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
                        // Truthful duration (task 10): the octave-scaled sweep is
                        // self-timed, so persist the actual generated-signal
                        // duration measured at capture time — `None` when unknown,
                        // never the nominal `signal_duration_secs` knob value.
                        recording_state
                            .ctc_reference_sweep_duration_s
                            .map(f64::from)
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
        }

        self.delay_detection.sample_rate = recording_state.probe_capture.sample_rate;
        self.delay_detection.input_channel = recording_state.probe_capture.input_channel;
        self.delay_detection.output_device_name =
            (!recording_state.playback_config.device_name.is_empty())
                .then(|| recording_state.playback_config.device_name.clone());
        self.delay_detection.input_device_name =
            (!recording_state.recording_config.device_name.is_empty())
                .then(|| recording_state.recording_config.device_name.clone());
        if let Some(results) = recording_state.probe_capture.results.clone() {
            self.delay_detection.apply_results(results);
        } else {
            self.delay_detection.results = None;
            self.delay_detection.edited_arrival_ms.clear();
            self.delay_detection.status = DelayDetectionStatus::Idle;
        }

        self.data_source = RoomEqDataSource::FromRecording;
    }

    /// Convert UI state to backend RoomConfig.
    pub fn to_room_config(&self) -> autoeq::roomeq::RoomConfig {
        use ndarray::Array1;

        let mut speakers: HashMap<String, autoeq::roomeq::SpeakerConfig> = HashMap::new();
        let mut crossovers: HashMap<String, autoeq::roomeq::CrossoverConfig> =
            self.imported_crossovers.clone().unwrap_or_default();

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
                Some(Array1::from_vec(
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
                freq: Array1::from_vec(frequencies),
                spl: Array1::from_vec(magnitude_db),
                phase,
                ..Default::default()
            }
        };

        let result_to_curve = |res: &crate::recording_types::RecordingResult| -> autoeq::Curve {
            let frequencies: Vec<f64> = res.frequencies.iter().map(|&f| f as f64).collect();
            let magnitude_db: Vec<f64> = res.magnitude_db.iter().map(|&db| db as f64).collect();
            let phase = if !res.phase_deg.is_empty() && res.phase_deg.len() == frequencies.len() {
                Some(Array1::from_vec(
                    res.phase_deg.iter().map(|&p| p as f64).collect(),
                ))
            } else {
                None
            };

            autoeq::Curve {
                freq: Array1::from_vec(frequencies),
                spl: Array1::from_vec(magnitude_db),
                phase,
                ..Default::default()
            }
        };

        for speaker_config in &self.speaker_configs {
            let channel_name = &speaker_config.channel_name;

            if let Some(meas) = self
                .channel_measurements
                .iter()
                .find(|m| &m.channel_name == channel_name)
            {
                match speaker_config.config_type {
                    crate::room_eq_types::SpeakerConfigType::Single => {
                        let acquisition =
                            meas.acquisition
                                .as_ref()
                                .map_or_else(Default::default, |evidence| {
                                    evidence.for_responses(
                                        std::iter::once(&meas.measurement)
                                            .chain(&meas.multi_mic_measurements),
                                    )
                                });
                        let to_named_measurement = |curve: autoeq::Curve, index: usize| {
                            autoeq::read::MeasurementRef::Inline(autoeq::read::InlineMeasurement {
                                frequencies: curve.freq.to_vec(),
                                magnitude_db: curve.spl.to_vec(),
                                phase_deg: curve.phase.map(|phase| phase.to_vec()),
                                name: meas
                                    .provenance
                                    .get(index)
                                    .and_then(|source| source.name.clone()),
                                wav_path: None,
                                csv_path: None,
                            })
                        };
                        if meas.multi_mic_measurements.is_empty() {
                            let curve = to_curve(meas);
                            speakers.insert(
                                channel_name.clone(),
                                autoeq::roomeq::SpeakerConfig::Single(
                                    autoeq::roomeq::MeasurementSource::Single(
                                        autoeq::read::MeasurementSingle {
                                            measurement: to_named_measurement(curve, 0),
                                            speaker_name: None,
                                            provenance: acquisition,
                                        },
                                    ),
                                ),
                            );
                        } else {
                            let mut measurements = vec![to_named_measurement(to_curve(meas), 0)];
                            for (index, extra) in meas.multi_mic_measurements.iter().enumerate() {
                                measurements
                                    .push(to_named_measurement(result_to_curve(extra), index + 1));
                            }
                            speakers.insert(
                                channel_name.clone(),
                                autoeq::roomeq::SpeakerConfig::Single(
                                    autoeq::roomeq::MeasurementSource::Multiple(
                                        autoeq::read::MeasurementMultiple {
                                            measurements,
                                            speaker_name: None,
                                            provenance: acquisition,
                                        },
                                    ),
                                ),
                            );
                        }
                    }
                    crate::room_eq_types::SpeakerConfigType::MultiDriver => {
                        let mut driver_measurements = Vec::new();
                        if !meas.driver_measurement_sets.is_empty() {
                            for driver in &meas.driver_measurement_sets {
                                let acquisition = driver.acquisition.as_ref().map_or_else(
                                    Default::default,
                                    |evidence| {
                                        evidence.for_responses(
                                            std::iter::once(&driver.measurement)
                                                .chain(&driver.multi_mic_measurements),
                                        )
                                    },
                                );
                                let to_ref =
                                    |result: &crate::recording_types::RecordingResult,
                                     index: usize| {
                                        let curve = result_to_curve(result);
                                        autoeq::read::MeasurementRef::Inline(
                                            autoeq::read::InlineMeasurement {
                                                frequencies: curve.freq.to_vec(),
                                                magnitude_db: curve.spl.to_vec(),
                                                phase_deg: curve.phase.map(|phase| phase.to_vec()),
                                                name: driver
                                                    .provenance
                                                    .get(index)
                                                    .and_then(|source| source.name.clone()),
                                                wav_path: None,
                                                csv_path: None,
                                            },
                                        )
                                    };
                                let primary = to_ref(&driver.measurement, 0);
                                let source = if driver.multi_mic_measurements.is_empty() {
                                    autoeq::MeasurementSource::Single(
                                        autoeq::read::MeasurementSingle {
                                            measurement: primary,
                                            speaker_name: driver.name.clone(),
                                            provenance: acquisition,
                                        },
                                    )
                                } else {
                                    let measurements = std::iter::once(primary)
                                        .chain(
                                            driver
                                                .multi_mic_measurements
                                                .iter()
                                                .enumerate()
                                                .map(|(index, result)| to_ref(result, index + 1)),
                                        )
                                        .collect();
                                    autoeq::MeasurementSource::Multiple(
                                        autoeq::read::MeasurementMultiple {
                                            measurements,
                                            speaker_name: driver.name.clone(),
                                            provenance: acquisition,
                                        },
                                    )
                                };
                                driver_measurements.push(source);
                            }
                        } else if meas.is_group && !meas.group_drivers.is_empty() {
                            for driver_res in &meas.group_drivers {
                                driver_measurements.push(
                                    autoeq::roomeq::MeasurementSource::InMemory(result_to_curve(
                                        driver_res,
                                    )),
                                );
                            }
                        } else {
                            driver_measurements
                                .push(autoeq::roomeq::MeasurementSource::InMemory(to_curve(meas)));
                        }

                        let xover_id = format!("xover_{}", channel_name);
                        let xover_type = match speaker_config.crossover_type {
                            CrossoverType::LR12 => "LR12",
                            CrossoverType::LR24 => "LR24",
                            CrossoverType::LR48 => "LR48",
                            CrossoverType::LinearPhase => "LinearPhase",
                            CrossoverType::Butterworth12 => "Butterworth12",
                            CrossoverType::Butterworth24 => "Butterworth24",
                        };

                        crossovers.insert(
                            xover_id.clone(),
                            autoeq::roomeq::CrossoverConfig {
                                crossover_type: xover_type.to_string(),
                                frequency: match speaker_config.crossover_freq_hints.as_slice() {
                                    [frequency] => Some(*frequency),
                                    _ => None,
                                },
                                frequencies: (speaker_config.crossover_freq_hints.len() > 1)
                                    .then(|| speaker_config.crossover_freq_hints.clone()),
                                frequency_range: None,
                            },
                        );

                        speakers.insert(
                            channel_name.clone(),
                            autoeq::roomeq::SpeakerConfig::Group(autoeq::roomeq::SpeakerGroup {
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

        let ctc = self
            .ctc_config
            .clone()
            .or_else(|| {
                self.ctc_measurements
                    .clone()
                    .map(|measurements| autoeq::roomeq::CtcConfig {
                        enabled: false,
                        matrix_source: "measured".to_string(),
                        measurements: Some(measurements),
                        ..Default::default()
                    })
            })
            .map(|mut ctc| {
                ctc.enabled = false;
                ctc
            });
        let ctc_enabled = ctc.as_ref().is_some_and(|ctc| ctc.enabled);
        let has_bass_output = speakers
            .keys()
            .any(|name| room_eq_channel_is_bass_output(name));
        let system = if let Some(system) = self.imported_system.clone() {
            Some(system)
        } else if ctc_enabled || has_bass_output {
            let bass_management_crossover = has_bass_output.then(|| {
                let xover_id = "bass_management".to_string();
                let crossover_type = match self.simple_preset.crossover {
                    SimpleCrossoverChoice::Lr24 => "LR24",
                    SimpleCrossoverChoice::Lr48 => "LR48",
                };
                crossovers.entry(xover_id.clone()).or_insert_with(|| {
                    autoeq::roomeq::CrossoverConfig {
                        crossover_type: crossover_type.to_string(),
                        frequency: Some(80.0),
                        frequencies: None,
                        frequency_range: None,
                    }
                });
                xover_id
            });
            ctc_system_config_for_speaker_names(
                speakers.keys().map(String::as_str),
                bass_management_crossover,
            )
        } else {
            None
        };

        autoeq::roomeq::RoomConfig {
            version: autoeq::roomeq::default_config_version(),
            system,
            speakers,
            crossovers: Some(crossovers),
            target_curve: None,
            optimizer,
            provenance: Default::default(),
            recording_config: None,
            ctc,
            reporting: None,
            cea2034_cache: None,
        }
    }

    /// Validate the current configuration.
    pub fn validate(&self) -> autoeq::roomeq::ValidationResult {
        let config = self.to_room_config();
        autoeq::roomeq::validate_room_config(&config)
    }

    /// Calculate the level offset needed to normalize a curve to 0 dB.
    /// Uses mean SPL in the 1 kHz to 2 kHz range by default.
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
        } else if spl.is_empty() {
            0.0
        } else {
            spl.iter().sum::<f64>() / spl.len() as f64
        }
    }

    /// Normalize a set of points by subtracting an offset.
    pub fn normalize_points(points: &[(f64, f64)], offset: f64) -> Vec<(f64, f64)> {
        points.iter().map(|&(f, db)| (f, db - offset)).collect()
    }

    /// Compute the average slope for L and R channels in dB/octave.
    pub fn compute_lr_slope(&self) -> Option<(f64, f64, f64)> {
        crate::room_eq_types::compute_lr_slope(&self.channel_measurements)
    }
}

/// Normalize completed recording measurements to their shared frequency range
/// before Room EQ combines channels. The recorder can legitimately emit
/// different analysis grids per channel, but downstream Room EQ must never
/// combine values by matching their vector indices.
fn normalize_channel_measurement_grids(measurements: &mut [ChannelMeasurement]) {
    let Some(first) = measurements.first() else {
        return;
    };
    let Some((&lower, &upper)) = first
        .measurement
        .frequencies
        .first()
        .zip(first.measurement.frequencies.last())
    else {
        return;
    };
    let (lower, upper) = measurements
        .iter()
        .skip(1)
        .fold((lower, upper), |range, channel| {
            let start = channel
                .measurement
                .frequencies
                .first()
                .copied()
                .unwrap_or(range.0);
            let end = channel
                .measurement
                .frequencies
                .last()
                .copied()
                .unwrap_or(range.1);
            (range.0.max(start), range.1.min(end))
        });
    let grid: Vec<f32> = first
        .measurement
        .frequencies
        .iter()
        .copied()
        .filter(|frequency| *frequency >= lower && *frequency <= upper)
        .collect();
    if grid.len() < 2 {
        return;
    }
    for channel in measurements {
        if channel.measurement.frequencies == grid {
            continue;
        }
        let frequencies = &channel.measurement.frequencies;
        channel.measurement.magnitude_db =
            interpolate_log_grid(frequencies, &channel.measurement.magnitude_db, &grid);
        channel.measurement.phase_deg =
            interpolate_log_grid(frequencies, &channel.measurement.phase_deg, &grid);
        channel.measurement.frequencies = grid.clone();
    }
}

fn interpolate_log_grid(frequencies: &[f32], values: &[f32], target: &[f32]) -> Vec<f32> {
    let count = frequencies.len().min(values.len());
    if count < 2 {
        return vec![0.0; target.len()];
    }
    let frequencies = &frequencies[..count];
    let values = &values[..count];
    target
        .iter()
        .map(|&frequency| {
            let upper = frequencies.partition_point(|value| *value < frequency);
            if upper == 0 {
                return values[0];
            }
            if upper == frequencies.len() {
                return values[values.len() - 1];
            }
            let lower = upper - 1;
            let (lo_frequency, hi_frequency) = (frequencies[lower], frequencies[upper]);
            let fraction = if lo_frequency > 0.0 && hi_frequency > lo_frequency {
                (frequency.ln() - lo_frequency.ln()) / (hi_frequency.ln() - lo_frequency.ln())
            } else {
                0.0
            };
            values[lower] + (values[upper] - values[lower]) * fraction
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recording_types::{ChannelRecording, ChannelRecordingState, RecordingResult};
    use crate::room_eq_types::ChannelMeasurement;

    fn make_recording_result(channel: usize) -> RecordingResult {
        RecordingResult {
            sample_rate_hz: None,
            channel,
            wav_path: None,
            csv_path: None,
            frequencies: vec![100.0, 1000.0, 10000.0],
            magnitude_db: vec![0.0, 0.0, 0.0],
            phase_deg: vec![],
            impulse_response: None,
            impulse_time_ms: None,
            thd_percent: None,
            harmonic_distortion_db: None,
            excess_group_delay_ms: None,
            rt60_ms: None,
            clarity_c50_db: None,
            clarity_c80_db: None,
            spectrogram_db: None,
            quality: None,
        }
    }

    fn make_measurement(channel_name: &str) -> ChannelMeasurement {
        ChannelMeasurement {
            acquisition: None,
            driver_measurement_sets: Vec::new(),
            provenance: Vec::new(),
            channel_name: channel_name.to_string(),
            measurement: make_recording_result(0),
            is_group: false,
            group_drivers: Vec::new(),
            multi_mic_measurements: Vec::new(),
        }
    }

    fn make_done_recording(channel_index: usize, channel_name: &str) -> ChannelRecording {
        let mut rec =
            ChannelRecording::with_mic_position(channel_index, channel_name.to_string(), 0, 0);
        rec.state = ChannelRecordingState::Done;
        rec.result = Some(make_recording_result(channel_index));
        rec
    }

    fn assert_effect_navigate_to_step(effect: &RoomEqEffect, expected: RoomEqStep) {
        assert!(
            matches!(effect, RoomEqEffect::NavigateToStep(step) if *step == expected),
            "expected NavigateToStep({:?}), got {:?}",
            expected,
            effect
        );
    }

    #[test]
    fn navigate_to_step_updates_step_and_emits_effect() {
        let mut model = RoomEqScreenModel::default();
        let effects = model.apply(RoomEqViewEvent::NavigateToStep(RoomEqStep::Configure));
        assert_eq!(model.step, RoomEqStep::Configure);
        assert_eq!(effects.len(), 1);
        assert_effect_navigate_to_step(&effects[0], RoomEqStep::Configure);
    }

    #[test]
    fn next_and_previous_step_navigate() {
        let mut model = RoomEqScreenModel {
            step: RoomEqStep::LoadData,
            ..Default::default()
        };
        let effects = model.apply(RoomEqViewEvent::NextStep);
        assert_eq!(model.step, RoomEqStep::Delay);
        assert_eq!(effects.len(), 1);
        assert_effect_navigate_to_step(&effects[0], RoomEqStep::Delay);

        let effects = model.apply(RoomEqViewEvent::PreviousStep);
        assert_eq!(model.step, RoomEqStep::LoadData);
        assert_eq!(effects.len(), 1);
        assert_effect_navigate_to_step(&effects[0], RoomEqStep::LoadData);
    }

    #[test]
    fn load_measurements_initializes_speaker_configs() {
        let mut model = RoomEqScreenModel::default();
        let measurements = vec![make_measurement("L"), make_measurement("R")];
        let effects = model.apply(RoomEqViewEvent::LoadMeasurements(measurements));
        assert!(effects.is_empty());
        assert_eq!(model.speaker_configs.len(), 2);
        assert_eq!(model.speaker_configs[0].channel_name, "L");
        assert_eq!(model.speaker_configs[1].channel_name, "R");
    }

    #[test]
    fn loading_and_clearing_measurements_refreshes_position_metadata() {
        let mut model = RoomEqScreenModel::default();
        let mut left = make_measurement("L");
        left.multi_mic_measurements.push(left.measurement.clone());
        model.apply(RoomEqViewEvent::LoadMeasurements(vec![left.clone()]));
        assert!(model.has_multi_position_data);
        assert_eq!(model.multi_position_counts, vec![("L".to_string(), 2)]);

        model.apply(RoomEqViewEvent::LoadMeasurements(vec![make_measurement(
            "R",
        )]));
        assert!(!model.has_multi_position_data);
        assert!(model.multi_position_counts.is_empty());

        model.apply(RoomEqViewEvent::LoadMeasurements(vec![left]));
        model.apply(RoomEqViewEvent::ClearMeasurements);
        assert!(model.channel_measurements.is_empty());
        assert!(model.speaker_configs.is_empty());
        assert!(!model.has_multi_position_data);
        assert!(model.multi_position_counts.is_empty());
    }

    #[test]
    fn start_optimization_with_no_measurements_sets_error() {
        let mut model = RoomEqScreenModel::default();
        let effects = model.apply(RoomEqViewEvent::StartOptimization);
        assert_eq!(
            model.error_message,
            Some("No measurements loaded".to_string())
        );
        assert_eq!(effects.len(), 1);
        assert!(
            matches!(&effects[0], RoomEqEffect::ShowError(msg) if msg == "No measurements loaded"),
            "expected ShowError, got {:?}",
            effects[0]
        );
    }

    #[test]
    fn start_optimization_with_measurements_starts() {
        let mut model = RoomEqScreenModel::default();
        model.apply(RoomEqViewEvent::LoadMeasurements(vec![make_measurement(
            "L",
        )]));
        let effects = model.apply(RoomEqViewEvent::StartOptimization);
        assert_eq!(model.optimization_status, OptimizationStatus::Running);
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], RoomEqEffect::StartOptimization(_)));
    }

    #[test]
    fn set_optimization_results_completes() {
        let mut model = RoomEqScreenModel::default();
        let effects = model.apply(RoomEqViewEvent::SetOptimizationResults {
            channel_results: vec![ChannelOptResult {
                channel_name: "L".to_string(),
                pre_score: 0.5,
                post_score: 0.9,
                eq_filters: Vec::new(),
                broadband_filters: Vec::new(),
                preamp_gain_db: 0.0,
                crossover_freqs: None,
                driver_gains: None,
                original_response: None,
                corrected_response: None,
                normalized_response: None,
                target_curve: None,
                group_delay_before: None,
                group_delay_after: None,
                phase_response_before: None,
                phase_response_after: None,
                impulse_response: None,
            }],
            dsp_output: Box::new(DspChainOutput {
                version: "1.0.0".to_string(),
                global_plugins: Vec::new(),
                channels: HashMap::new(),
                metadata: None,
                deployed_source_curves: Default::default(),
                correction_decisions: None,
            }),
            artifact_dir: PathBuf::from("/tmp/room_eq"),
        });
        assert!(effects.is_empty());
        assert_eq!(model.optimization_status, OptimizationStatus::Completed);
        assert_eq!(model.overall_progress, 1.0);
        assert_eq!(model.status_message, "Optimization completed");
        assert_eq!(model.average_pre_score(), 0.5);
        assert_eq!(model.average_post_score(), 0.9);
    }

    #[test]
    fn load_from_recording_groups_by_speaker() {
        let mut model = RoomEqScreenModel::default();
        let recording_state = RecordingState {
            channel_recordings: vec![make_done_recording(0, "L"), make_done_recording(1, "R")],
            ..Default::default()
        };
        let effects = model.apply(RoomEqViewEvent::LoadFromRecording(Box::new(
            recording_state,
        )));
        assert!(effects.is_empty());
        assert_eq!(model.channel_measurements.len(), 2);
        assert_eq!(model.channel_measurements[0].channel_name, "L");
        assert_eq!(model.channel_measurements[1].channel_name, "R");
        assert_eq!(model.speaker_configs.len(), 2);
        assert_eq!(model.data_source, RoomEqDataSource::FromRecording);
    }

    #[test]
    fn recording_provenance_survives_grouping_and_serialization() {
        let mut primary = make_done_recording(0, "L (Pos 1 / Mic 1)");
        primary.mic_position_index = 0;
        primary.mic_index = 0;
        let mut additional = make_done_recording(0, "L (Pos 3 / Mic 2)");
        additional.mic_position_index = 2;
        additional.mic_index = 1;
        let mut model = RoomEqScreenModel::default();
        model.apply(RoomEqViewEvent::LoadFromRecording(Box::new(
            RecordingState {
                channel_recordings: vec![primary, additional],
                ..Default::default()
            },
        )));
        let channel = &model.channel_measurements[0];
        assert_eq!(channel.channel_name, "L");
        assert_eq!(channel.multi_mic_measurements.len(), 1);
        assert_eq!(channel.provenance.len(), 2);
        assert_eq!(channel.provenance[0].mic_position_index, Some(0));
        assert_eq!(channel.provenance[1].mic_position_index, Some(2));
        assert_eq!(channel.provenance[1].mic_index, Some(1));
        assert_eq!(
            channel.provenance[1].name.as_deref(),
            Some("L (Pos 3 / Mic 2)")
        );
        let json = serde_json::to_value(channel).unwrap();
        let restored: ChannelMeasurement = serde_json::from_value(json).unwrap();
        assert_eq!(restored.provenance, channel.provenance);
        let config_json = serde_json::to_string(&model.to_room_config()).unwrap();
        let imported =
            crate::room_eq_types::RoomEqMeasurementsFile::load_from_json(&config_json, None)
                .unwrap();
        assert!(!imported.is_empty(), "{config_json}");
        assert_eq!(imported[0].provenance[0].name, channel.provenance[0].name);
        assert_eq!(imported[0].provenance[1].name, channel.provenance[1].name);
        assert_eq!(
            imported[0].measurement.frequencies,
            channel.measurement.frequencies
        );
        assert_eq!(
            imported[0].measurement.magnitude_db,
            channel.measurement.magnitude_db
        );
        assert_eq!(
            imported[0].multi_mic_measurements[0].magnitude_db,
            channel.multi_mic_measurements[0].magnitude_db
        );
    }

    #[test]
    fn legacy_measurement_has_unknown_provenance() {
        let mut json = serde_json::to_value(make_measurement("L")).unwrap();
        json.as_object_mut().unwrap().remove("provenance");
        let restored: ChannelMeasurement = serde_json::from_value(json).unwrap();
        assert!(restored.provenance.is_empty());
        assert!(restored.driver_measurement_sets.is_empty());
        assert!(restored.acquisition.is_none());
    }

    fn capture_model(
        bound_us: f64,
        grouped: bool,
    ) -> (RoomEqScreenModel, autoeq::MeasurementProvenance) {
        let takes: Vec<_> = (0..2)
            .map(|index| {
                serde_json::json!({
                    "microphone_id": format!("mic-{index}"), "device_id": format!("device-{index}"),
                    "offset_samples": 123.5, "skew_ppm": 80.0,
                    "residual_uncertainty_us": bound_us, "correction_applied": "resampled",
                    "timing_reference_id": "surveyed-emitter", "calibration_id": "cal-sha256",
                    "gain_db": 0.0, "calibration_orientation": "on_axis",
                    "position_m": [index as f64 * 0.06, 0.0, 0.0], "position_uncertainty_mm": 0.5,
                    "preserves_acoustic_delay": true, "quality_passed": true
                })
            })
            .collect();
        let provenance: autoeq::MeasurementProvenance = serde_json::from_value(serde_json::json!({
            "capture_kind": "stationary_ir", "timing_reference_id": "surveyed-emitter",
            "capture": {"geometry": "compact", "takes": takes,
                "reflection_report": {"source_id": "L", "direct_sound": null,
                    "early_reflections": [], "issues": ["No resolved direction"]}}
        }))
        .unwrap();
        let source = autoeq::MeasurementSource::Multiple(autoeq::read::MeasurementMultiple {
            measurements: (0..2)
                .map(|index| {
                    autoeq::read::MeasurementRef::Inline(autoeq::read::InlineMeasurement {
                        frequencies: vec![100.0, 1000.0, 10000.0],
                        magnitude_db: vec![index as f64; 3],
                        phase_deg: Some(vec![index as f64 * 5.0; 3]),
                        name: Some(format!("mic-{index}")),
                        wav_path: None,
                        csv_path: None,
                    })
                })
                .collect(),
            speaker_name: None,
            provenance: provenance.clone(),
        });
        let mut config = autoeq::RoomConfig::default();
        config.speakers.insert(
            "L".into(),
            if grouped {
                autoeq::SpeakerConfig::Group(autoeq::roomeq::SpeakerGroup {
                    measurements: vec![source.clone(), source],
                    name: "L".into(),
                    speaker_name: None,
                    crossover: None,
                })
            } else {
                autoeq::SpeakerConfig::Single(source)
            },
        );
        let channels = crate::room_eq_types::RoomEqMeasurementsFile::load_from_json(
            &serde_json::to_string(&config).unwrap(),
            None,
        )
        .unwrap();
        // Exercise saved frontend state as well as RoomConfig import/export.
        let channels = serde_json::from_str(&serde_json::to_string(&channels).unwrap()).unwrap();
        let mut model = RoomEqScreenModel::default();
        model.apply(RoomEqViewEvent::LoadMeasurements(channels));
        (model, provenance)
    }

    #[test]
    fn capture_acquisition_round_trips_without_upgrading_failed_clock_evidence() {
        for bound_us in [10.0, 55.0] {
            for grouped in [false, true] {
                let (model, expected) = capture_model(bound_us, grouped);
                let config = model.to_room_config();
                let sources = match &config.speakers["L"] {
                    autoeq::SpeakerConfig::Single(source) => vec![source],
                    autoeq::SpeakerConfig::Group(group) => group.measurements.iter().collect(),
                    _ => panic!("unexpected source type"),
                };
                for source in sources {
                    let autoeq::MeasurementSource::Multiple(source) = source else {
                        panic!("lost mic set")
                    };
                    assert_eq!(source.provenance, expected);
                    assert_eq!(
                        source
                            .provenance
                            .capture
                            .as_ref()
                            .unwrap()
                            .coherent_reference(2)
                            .is_ok(),
                        bound_us < 50.0
                    );
                }
            }
        }
    }

    #[test]
    fn changed_capture_responses_revoke_quality_but_retain_raw_clock_facts() {
        for mutation in 0..3 {
            let (mut model, expected) = capture_model(10.0, false);
            let channel = &mut model.channel_measurements[0];
            match mutation {
                0 => channel.measurement.phase_deg[0] += 1.0,
                1 => {
                    channel.multi_mic_measurements.clear();
                }
                _ => std::mem::swap(
                    &mut channel.measurement,
                    &mut channel.multi_mic_measurements[0],
                ),
            }
            let config = model.to_room_config();
            let autoeq::SpeakerConfig::Single(source) = &config.speakers["L"] else {
                panic!("lost source")
            };
            let provenance = match source {
                autoeq::MeasurementSource::Single(source) => &source.provenance,
                autoeq::MeasurementSource::Multiple(source) => &source.provenance,
                _ => panic!("lost declaration"),
            };
            assert!(provenance.timing_reference_id.is_none());
            let capture = provenance.capture.as_ref().unwrap();
            assert!(capture.takes.iter().all(|take| !take.quality_passed));
            assert_eq!(
                capture.takes[0].offset_samples,
                expected.capture.as_ref().unwrap().takes[0].offset_samples
            );
            assert!(capture.coherent_reference(2).is_err());
        }
    }

    #[test]
    fn malformed_response_cannot_preserve_acquisition_authorization() {
        let (_, provenance) = capture_model(10.0, false);
        let mut response = make_recording_result(0);
        response.phase_deg = vec![10.0];
        let acquisition =
            crate::room_eq_types::MeasurementAcquisition::new(provenance, [&response]);
        let exported = acquisition.for_responses([&response]);
        assert!(exported.timing_reference_id.is_none());
        assert!(
            exported
                .capture
                .unwrap()
                .takes
                .iter()
                .all(|take| !take.quality_passed)
        );
    }

    #[test]
    fn grouped_driver_positions_and_names_round_trip() {
        let response = |name: &str, gain: f64| {
            autoeq::read::MeasurementRef::Inline(autoeq::read::InlineMeasurement {
                name: Some(name.to_owned()),
                frequencies: vec![100.0, 1000.0, 10000.0],
                magnitude_db: vec![gain; 3],
                phase_deg: None,
                wav_path: None,
                csv_path: None,
            })
        };
        let mut config = autoeq::RoomConfig::default();
        config.speakers.insert(
            "L".into(),
            autoeq::SpeakerConfig::Group(autoeq::roomeq::SpeakerGroup {
                name: "Left speaker".into(),
                speaker_name: None,
                crossover: None,
                measurements: vec![
                    autoeq::MeasurementSource::Multiple(autoeq::read::MeasurementMultiple {
                        speaker_name: Some("Woofer".into()),
                        provenance: Default::default(),
                        measurements: vec![
                            response("Woofer seat A", 1.0),
                            response("Woofer seat B", 2.0),
                        ],
                    }),
                    autoeq::MeasurementSource::Single(autoeq::read::MeasurementSingle {
                        speaker_name: Some("Tweeter".into()),
                        provenance: Default::default(),
                        measurement: response("Tweeter seat A", 3.0),
                    }),
                ],
            }),
        );
        let json = serde_json::to_string(&config).unwrap();
        let channels =
            crate::room_eq_types::RoomEqMeasurementsFile::load_from_json(&json, None).unwrap();
        assert_eq!(channels.len(), 1);
        assert!(channels[0].is_group);
        let mut model = RoomEqScreenModel::default();
        model.apply(RoomEqViewEvent::LoadMeasurements(channels));
        assert_eq!(model.speaker_configs[0].driver_names, ["Woofer", "Tweeter"]);
        for hints in [vec![], vec![1800.0], vec![250.0, 2500.0]] {
            model.speaker_configs[0].crossover_freq_hints = hints.clone();
            let exported = model.to_room_config();
            let crossover = &exported.crossovers.as_ref().unwrap()["xover_L"];
            match hints.as_slice() {
                [] => {
                    assert!(crossover.frequency.is_none());
                    assert!(crossover.frequencies.is_none());
                }
                [frequency] => {
                    assert_eq!(crossover.frequency, Some(*frequency));
                    assert!(crossover.frequencies.is_none());
                }
                _ => {
                    assert!(crossover.frequency.is_none());
                    assert_eq!(crossover.frequencies.as_ref(), Some(&hints));
                }
            }
        }
        let saved = serde_json::to_string(&model.to_room_config()).unwrap();
        let restored =
            crate::room_eq_types::RoomEqMeasurementsFile::load_from_json(&saved, None).unwrap();
        let drivers = &restored[0].driver_measurement_sets;
        assert_eq!(drivers.len(), 2);
        assert_eq!(drivers[0].name.as_deref(), Some("Woofer"));
        assert_eq!(drivers[0].multi_mic_measurements.len(), 1);
        assert_eq!(
            drivers[0].provenance[1].name.as_deref(),
            Some("Woofer seat B")
        );
        assert_eq!(drivers[0].multi_mic_measurements[0].magnitude_db, [2.0; 3]);
        assert_eq!(drivers[1].name.as_deref(), Some("Tweeter"));
        assert!(drivers[1].multi_mic_measurements.is_empty());
        assert_eq!(drivers[1].measurement.magnitude_db, [3.0; 3]);
        // A broken extra position must reject the import, not silently reduce
        // the driver's measurement set while reporting a successful load.
        let autoeq::SpeakerConfig::Group(group) = config.speakers.get_mut("L").unwrap() else {
            unreachable!()
        };
        let autoeq::MeasurementSource::Multiple(woofer) = &mut group.measurements[0] else {
            unreachable!()
        };
        let autoeq::read::MeasurementRef::Inline(position) = &mut woofer.measurements[1] else {
            unreachable!()
        };
        position.frequencies.clear();
        position.magnitude_db.clear();
        let invalid = serde_json::to_string(&config).unwrap();
        let error = crate::room_eq_types::RoomEqMeasurementsFile::load_from_json(&invalid, None)
            .unwrap_err();
        assert!(error.contains("Speaker L, driver 1"), "{error}");
        assert!(error.contains("1 of 2"), "{error}");
    }

    #[test]
    fn set_crossover_freq_hint_validates_and_materializes() {
        use crate::room_eq_types::{RoomEqSpeakerConfig, SpeakerConfigType};
        let mut model = RoomEqScreenModel::default();
        model.speaker_configs = vec![
            RoomEqSpeakerConfig {
                channel_name: "L".into(),
                config_type: SpeakerConfigType::MultiDriver,
                driver_names: vec!["Woofer".into(), "Tweeter".into()],
                ..Default::default()
            },
            RoomEqSpeakerConfig {
                channel_name: "R".into(),
                config_type: SpeakerConfigType::Single,
                ..Default::default()
            },
        ];
        assert_eq!(model.expected_crossover_count(0), 1);
        assert_eq!(model.expected_crossover_count(1), 0);
        assert_eq!(model.expected_crossover_count(99), 0);

        assert!(model.set_crossover_freq_hint(0, 0, 2500.0));
        assert_eq!(model.speaker_configs[0].crossover_freq_hints, vec![2500.0]);

        for bad in [f64::NAN, f64::INFINITY, 10.0, 30000.0] {
            assert!(!model.set_crossover_freq_hint(0, 0, bad));
        }
        assert_eq!(model.speaker_configs[0].crossover_freq_hints, vec![2500.0]);

        assert!(!model.set_crossover_freq_hint(0, 1, 3000.0));
        assert!(!model.set_crossover_freq_hint(1, 0, 2500.0));
        assert!(!model.set_crossover_freq_hint(99, 0, 2500.0));
        assert_eq!(model.speaker_configs[0].crossover_freq_hints, vec![2500.0]);

        model.channel_measurements = vec![make_measurement("L")];
        let exported = model.to_room_config();
        let crossover = &exported.crossovers.as_ref().unwrap()["xover_L"];
        assert_eq!(crossover.frequency, Some(2500.0));
        assert!(crossover.frequencies.is_none());
    }

    #[test]
    fn helper_methods_detect_channel_layouts() {
        let mut model = RoomEqScreenModel::default();
        model.apply(RoomEqViewEvent::LoadMeasurements(vec![
            make_measurement("L"),
            make_measurement("R"),
            make_measurement("C"),
            make_measurement("LFE"),
            make_measurement("SL"),
            make_measurement("SR"),
        ]));
        assert!(model.has_subwoofer());
        assert!(model.is_surround());
        assert!(!model.has_height_channels());
        assert!(model.is_home_cinema());
        assert!(!model.has_multi_driver());
        assert!(!model.has_phase_data());
    }

    #[test]
    fn calculate_normalization_offset_uses_1k_to_2k_range() {
        let frequencies = vec![500.0, 1000.0, 1500.0, 2000.0, 4000.0];
        let spl = vec![10.0, 5.0, 7.0, 9.0, 20.0];
        let offset = RoomEqScreenModel::calculate_normalization_offset(&frequencies, &spl);
        assert!((offset - 7.0).abs() < 1e-9);
    }

    #[test]
    fn normalize_points_subtracts_offset() {
        let points = vec![(100.0, 5.0), (1000.0, 7.0)];
        let normalized = RoomEqScreenModel::normalize_points(&points, 2.0);
        assert_eq!(normalized, vec![(100.0, 3.0), (1000.0, 5.0)]);
    }
}
