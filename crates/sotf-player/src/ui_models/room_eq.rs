//! Shared, UI-agnostic view model for the Room EQ wizard.
//!
//! This module intentionally separates *domain* state (measurements, optimizer
//! config, optimization results) from *view* state (selected rows, edit buffers,
//! focus flags). The latter stays in `app-gpui` / `app-tui`.

use crate::autoeq::{PipelineStepId, PipelineStepStatus};
use crate::room_eq_types::{
    ChannelMeasurement, ChannelMetadata, ChannelOptResult, DelayDetectionState, DspChainOutput,
    OptimizationStatus, RoomEqDataSource, RoomEqOptimizerConfig, RoomEqSpeakerConfig, RoomEqStep,
    RoomEqWizardMode, SimplePresetConfig, apply_simple_preset,
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
                self.has_multi_position_data = false;
                self.multi_position_counts.clear();
            }
            RoomEqViewEvent::ClearMeasurements => {
                self.channel_measurements.clear();
                self.speaker_configs.clear();
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
                ..Default::default()
            })
            .collect();
    }

    /// Apply smart defaults based on loaded measurement metadata.
    pub fn apply_smart_defaults(&mut self, playback_sample_rate: Option<u32>) {
        let meta = ChannelMetadata {
            channel_names: self.channel_names(),
            playback_sample_rate,
        };
        self.optimizer_config.apply_smart_defaults(&meta);
    }
}
