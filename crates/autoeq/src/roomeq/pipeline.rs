//! Observable Room EQ optimization pipeline.
//!
//! This module exposes a structured event stream for callers that need more
//! detail than the legacy progress callback while keeping the optimization
//! implementation in `optimize`.

use crate::error::Result;
use std::collections::HashMap;
use std::path::Path;

use super::optimize::RoomOptimizationResult;
use super::types::RoomConfig;

/// Request data for a Room EQ pipeline run.
#[derive(Clone, Copy)]
pub struct RoomPipelineRequest<'a> {
    /// Complete room configuration.
    pub config: &'a RoomConfig,
    /// Sample rate for filter design.
    pub sample_rate: f64,
    /// Optional directory for generated artifacts.
    pub output_dir: Option<&'a Path>,
    /// Optional per-channel probe-based arrival times in milliseconds.
    pub probe_arrival_overrides: Option<&'a HashMap<String, f64>>,
}

/// Observable Room EQ optimization pipeline.
pub struct RoomPipeline<'a> {
    request: RoomPipelineRequest<'a>,
}

impl<'a> RoomPipeline<'a> {
    /// Create a new pipeline for the given request.
    pub fn new(request: RoomPipelineRequest<'a>) -> Self {
        Self { request }
    }

    /// Run the pipeline, optionally notifying an observer for each event.
    pub fn run(
        self,
        observer: Option<Box<dyn PipelineObserver>>,
    ) -> Result<RoomOptimizationResult> {
        super::optimize::optimize_room_pipeline_impl(self.request, observer)
    }
}

/// Stable identifier for a Room EQ pipeline step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PipelineStepId {
    /// Clone and normalize request configuration, including CEA2034 prefetch.
    ConfigPreparation,
    /// Validate the prepared configuration.
    Validation,
    /// Decide whether to use a topology-specific workflow or the generic path.
    TopologyRouteSelection,
    /// Execute a topology-specific workflow.
    TopologyWorkflowExecution,
    /// Optimize channels via the generic per-channel path.
    GenericChannelOptimization,
    /// Generate full FIR coefficients after IIR-only stages.
    FirGeneration,
    /// Generate short mixed-phase FIR coefficients.
    MixedPhaseFirGeneration,
    /// Apply standalone phase correction.
    PhaseCorrection,
    /// Align channels in time from measured or phase-estimated arrivals.
    TimeAlignment,
    /// Match broad spectral balance across channels.
    SpectralAlignment,
    /// Apply Voice of God timbre matching.
    VoiceOfGodAlignment,
    /// Optimize sub/main phase alignment.
    PhaseAlignment,
    /// Run group-delay optimization.
    GroupDelayOptimization,
    /// Compute pre/post impulse responses.
    ImpulseResponseComputation,
    /// Analyze and optionally correct inter-channel deviation.
    ChannelMatching,
    /// Refresh metadata and derived reports.
    MetadataRefresh,
    /// Check final result invariants.
    SanityCheck,
}

impl PipelineStepId {
    /// Canonical execution order of every pipeline step. Step indicators
    /// in UIs render this list left-to-right; later steps are not all
    /// always visited (TopologyWorkflowExecution vs
    /// GenericChannelOptimization, MixedPhaseFirGeneration, etc.) and
    /// will appear as `Skipped` in the live event stream.
    pub const ALL: &'static [PipelineStepId] = &[
        PipelineStepId::ConfigPreparation,
        PipelineStepId::Validation,
        PipelineStepId::TopologyRouteSelection,
        PipelineStepId::TopologyWorkflowExecution,
        PipelineStepId::GenericChannelOptimization,
        PipelineStepId::FirGeneration,
        PipelineStepId::MixedPhaseFirGeneration,
        PipelineStepId::PhaseCorrection,
        PipelineStepId::TimeAlignment,
        PipelineStepId::SpectralAlignment,
        PipelineStepId::VoiceOfGodAlignment,
        PipelineStepId::PhaseAlignment,
        PipelineStepId::GroupDelayOptimization,
        PipelineStepId::ImpulseResponseComputation,
        PipelineStepId::ChannelMatching,
        PipelineStepId::MetadataRefresh,
        PipelineStepId::SanityCheck,
    ];

    /// Short, human-readable label for status indicators. Matches the
    /// log lines emitted by the pipeline so users can correlate the UI
    /// step strip with the log scroll buffer.
    pub fn label(&self) -> &'static str {
        match self {
            PipelineStepId::ConfigPreparation => "Config",
            PipelineStepId::Validation => "Validate",
            PipelineStepId::TopologyRouteSelection => "Route",
            PipelineStepId::TopologyWorkflowExecution => "Topology",
            PipelineStepId::GenericChannelOptimization => "Channels",
            PipelineStepId::FirGeneration => "FIR",
            PipelineStepId::MixedPhaseFirGeneration => "Mixed-Phase FIR",
            PipelineStepId::PhaseCorrection => "Phase Corr.",
            PipelineStepId::TimeAlignment => "Time Align",
            PipelineStepId::SpectralAlignment => "Spectral Align",
            PipelineStepId::VoiceOfGodAlignment => "VoG Align",
            PipelineStepId::PhaseAlignment => "Phase Align",
            PipelineStepId::GroupDelayOptimization => "GD-Opt",
            PipelineStepId::ImpulseResponseComputation => "IR",
            PipelineStepId::ChannelMatching => "Match",
            PipelineStepId::MetadataRefresh => "Metadata",
            PipelineStepId::SanityCheck => "Sanity",
        }
    }
}

/// Lifecycle status for a pipeline step event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PipelineStepStatus {
    /// The step is starting.
    Started,
    /// The step produced an intermediate progress update.
    InProgress,
    /// The step completed and may have changed the result.
    Completed,
    /// The step was intentionally skipped.
    Skipped,
}

/// Observer decision after receiving a pipeline event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PipelineControl {
    /// Continue the pipeline.
    Continue,
    /// Stop the pipeline as soon as possible.
    Stop,
}

/// Structured event emitted by the Room EQ pipeline.
#[derive(Debug, Clone)]
pub struct PipelineEvent {
    /// Stable step identifier.
    pub step_id: PipelineStepId,
    /// Lifecycle status for this event.
    pub status: PipelineStepStatus,
    /// Current channel, if the event is channel-specific.
    pub channel: Option<String>,
    /// Current channel index, if available.
    pub channel_index: Option<usize>,
    /// Total channels or units in the current stage, if available.
    pub total_channels: Option<usize>,
    /// Current optimizer iteration, if available.
    pub iteration: Option<usize>,
    /// Maximum optimizer iterations, if available.
    pub max_iterations: Option<usize>,
    /// Current loss value, if available.
    pub loss: Option<f64>,
    /// Overall pipeline progress in the range 0.0..=1.0.
    pub overall_progress: f64,
    /// Optional display/log message.
    pub message: Option<String>,
    /// EPA preference score, computed periodically by the optimizer.
    pub epa_preference: Option<f64>,
}

impl PipelineEvent {
    /// Create a new event with default optional fields.
    pub fn new(step_id: PipelineStepId, status: PipelineStepStatus) -> Self {
        Self {
            step_id,
            status,
            channel: None,
            channel_index: None,
            total_channels: None,
            iteration: None,
            max_iterations: None,
            loss: None,
            overall_progress: 0.0,
            message: None,
            epa_preference: None,
        }
    }

    /// Convenience constructor for a started event.
    pub fn started(step_id: PipelineStepId, message: impl Into<String>) -> Self {
        Self::new(step_id, PipelineStepStatus::Started).with_message(message)
    }

    /// Convenience constructor for a completed event.
    pub fn completed(step_id: PipelineStepId, message: impl Into<String>) -> Self {
        Self::new(step_id, PipelineStepStatus::Completed).with_message(message)
    }

    /// Convenience constructor for a skipped event.
    pub fn skipped(step_id: PipelineStepId, message: impl Into<String>) -> Self {
        Self::new(step_id, PipelineStepStatus::Skipped).with_message(message)
    }

    /// Attach a message.
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Attach a channel name.
    pub fn with_channel(mut self, channel: impl Into<String>) -> Self {
        self.channel = Some(channel.into());
        self
    }

    /// Attach channel indexing.
    pub fn with_channels(mut self, channel_index: usize, total_channels: usize) -> Self {
        self.channel_index = Some(channel_index);
        self.total_channels = Some(total_channels);
        self
    }

    /// Attach optimizer iteration progress.
    pub fn with_iteration(mut self, iteration: usize, max_iterations: usize) -> Self {
        self.iteration = Some(iteration);
        self.max_iterations = Some(max_iterations);
        self
    }

    /// Attach a loss value.
    pub fn with_loss(mut self, loss: f64) -> Self {
        self.loss = Some(loss);
        self
    }

    /// Attach overall pipeline progress.
    pub fn with_overall_progress(mut self, overall_progress: f64) -> Self {
        self.overall_progress = overall_progress;
        self
    }

    /// Attach EPA preference progress.
    pub fn with_epa_preference(mut self, epa_preference: Option<f64>) -> Self {
        self.epa_preference = epa_preference;
        self
    }
}

/// Observer for structured pipeline events.
pub trait PipelineObserver: Send {
    /// Called for every pipeline event.
    fn on_event(&mut self, event: &PipelineEvent) -> PipelineControl;
}

impl<F> PipelineObserver for F
where
    F: FnMut(&PipelineEvent) -> PipelineControl + Send,
{
    fn on_event(&mut self, event: &PipelineEvent) -> PipelineControl {
        self(event)
    }
}
