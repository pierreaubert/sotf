// ============================================================================
// Audio Engine Types
// ============================================================================

use crate::decoder::AudioSource;
use sotf_plugins::PluginHost;
use std::any::Any;
use std::sync::Arc;

// Re-export shared types from the engine types module.
pub use crate::{
    AudioEngineState, AudioFrame, DsdOutputMode, DsdOutputStatus, EngineOversamplingPolicy,
    IsolatedExternalPluginSandboxBackend, IsolatedExternalPluginSandboxStatus,
    IsolatedExternalPluginWorkerEvent, IsolatedExternalPluginWorkerStatus, LatencyCompensationMode,
    NetworkEndpointConfig, NetworkEndpointMode, NetworkEndpointStatus, OutputAccessMode,
    OutputAccessStatus, PLUGIN_BUILD_DIAGNOSTIC_PREFIX, PlaybackState, PluginBuildDiagnostic,
    PluginBuildTarget, PluginConfig, PluginGraphConfig, PluginGraphEdgeConfig,
    PluginGraphNodeConfig, StreamMetadata,
};

// ============================================================================
// Plugin Data Cache - Lock-free(ish) shared cache for analyzer data
// ============================================================================

/// One snapshot of plugin analyzer data (one slot per plugin in the chain).
pub type PluginDataVec = Vec<Option<Arc<dyn Any + Send + Sync>>>;

/// Shared cache for plugin analyzer data.
/// The processing thread writes after each frame; the UI reads without
/// blocking the audio pipeline via lock-free ArcSwap.
pub type PluginDataCache = Arc<arc_swap::ArcSwap<PluginDataVec>>;

/// A complete plugin-host replacement prepared away from the processing thread.
///
/// Besides the built host, this owns every heap-backed object needed to commit
/// the change: analyzer-cache storage and both possible latency-alignment delay
/// lines. The processing thread only validates the base snapshot and moves
/// these allocations into its active state.
pub struct PreparedHostUpdate {
    pub(super) host: Box<PluginHost>,
    pub(super) expected_output_channels: usize,
    pub(super) expected_latency_samples: usize,
    pub(super) output_channels: usize,
    pub(super) output_sample_rate: u32,
    pub(super) latency_samples: usize,
    pub(super) analyzer_cache: Arc<PluginDataVec>,
    pub(super) old_path_delay: PreparedTransitionDelay,
    pub(super) new_path_delay: PreparedTransitionDelay,
}

impl PreparedHostUpdate {
    /// Validate and prepare a host replacement on a control/worker thread.
    pub fn prepare(
        host: PluginHost,
        input_sample_rate: u32,
        expected_output_channels: usize,
        expected_latency_samples: usize,
    ) -> Result<Self, String> {
        let output_channels = host.output_channels();
        if output_channels == 0 {
            return Err("prepared plugin host must expose at least one output channel".into());
        }
        if input_sample_rate == 0 {
            return Err("prepared plugin host requires a non-zero input sample rate".into());
        }
        let output_sample_rate = host.output_sample_rate(input_sample_rate);
        if output_sample_rate == 0 {
            return Err("prepared plugin host must expose a non-zero output sample rate".into());
        }
        let latency_samples = host.total_latency_samples();
        let delay_frames = latency_samples.abs_diff(expected_latency_samples);
        let delay_len = delay_frames
            .checked_mul(output_channels.max(expected_output_channels))
            .ok_or_else(|| "prepared host transition delay capacity overflow".to_string())?;
        let (old_path_delay, new_path_delay) = if expected_latency_samples < latency_samples {
            (
                PreparedTransitionDelay::new(delay_len),
                PreparedTransitionDelay::default(),
            )
        } else {
            (
                PreparedTransitionDelay::default(),
                PreparedTransitionDelay::new(delay_len),
            )
        };
        let analyzer_cache = Arc::new(vec![None; host.plugin_count()]);

        Ok(Self {
            host: Box::new(host),
            expected_output_channels,
            expected_latency_samples,
            output_channels,
            output_sample_rate,
            latency_samples,
            analyzer_cache,
            old_path_delay,
            new_path_delay,
        })
    }

    #[cfg(test)]
    pub(crate) fn prepared_analyzer_slots(&self) -> usize {
        self.analyzer_cache.len()
    }
}

impl std::fmt::Debug for PreparedHostUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedHostUpdate")
            .field("expected_output_channels", &self.expected_output_channels)
            .field("expected_latency_samples", &self.expected_latency_samples)
            .field("output_channels", &self.output_channels)
            .field("output_sample_rate", &self.output_sample_rate)
            .field("latency_samples", &self.latency_samples)
            .finish_non_exhaustive()
    }
}

/// Preallocated interleaved sample delay used only during a host transition.
#[derive(Default)]
pub struct PreparedTransitionDelay {
    samples: Vec<f32>,
    cursor: usize,
}

impl PreparedTransitionDelay {
    fn new(len: usize) -> Self {
        Self {
            samples: vec![0.0; len],
            cursor: 0,
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.samples.len()
    }

    pub(crate) fn process_in_place(&mut self, block: &mut [f32]) {
        if self.samples.is_empty() {
            return;
        }
        for sample in block {
            std::mem::swap(sample, &mut self.samples[self.cursor]);
            self.cursor += 1;
            if self.cursor == self.samples.len() {
                self.cursor = 0;
            }
        }
    }
}

#[cfg(test)]
mod prepared_host_update_tests {
    use super::PreparedTransitionDelay;

    #[test]
    fn transition_delay_is_sample_exact_across_block_partitions() {
        let mut delay = PreparedTransitionDelay::new(3);
        let mut first = [1.0, 2.0];
        let mut second = [3.0, 4.0, 5.0];

        delay.process_in_place(&mut first);
        delay.process_in_place(&mut second);

        assert_eq!(first, [0.0, 0.0]);
        assert_eq!(second, [0.0, 1.0, 2.0]);
    }
}

// ============================================================================
// Queue Messages - Messages passed through queues
// ============================================================================

/// Messages sent from decoder to processing
#[derive(Clone, Debug)]
pub enum DecoderMessage {
    /// Audio frame
    Frame(AudioFrame),
    /// End of stream reached
    EndOfStream,
    /// Flush the queue (used during seek)
    Flush,
}

/// Messages sent from processing to playback
#[derive(Clone, Debug)]
pub enum ProcessingMessage {
    /// Processed audio frame
    Frame(AudioFrame),
    /// End of stream reached
    EndOfStream,
    /// Flush the queue
    Flush,
}

// ============================================================================
// Control Commands - Commands sent to threads
// ============================================================================

/// Commands for the decoder thread
#[derive(Clone, Debug)]
pub enum DecoderCommand {
    /// Start playing an audio source
    Play(AudioSource),
    /// Start playing an audio source at a specific position in seconds
    PlayAt(AudioSource, f64),
    /// Start silent source (for HAL input plugins).
    /// Sends empty frames at regular intervals for source plugins using the
    /// configured pipeline input channel count.
    StartSilentSource(usize),
    /// Pause decoding
    Pause,
    /// Resume decoding
    Resume,
    /// Seek to position in seconds
    Seek(f64),
    /// Queue the next source for gapless playback.
    /// When the current source ends, the decoder seamlessly transitions to this source
    /// without sending EndOfStream or Flush, avoiding any gap in audio output.
    QueueNext(AudioSource),
    /// Cancel a previously queued next source.
    CancelNext,
    /// Stop decoding and cleanup
    Stop,
    /// Shutdown the thread
    Shutdown,
}

#[derive(Clone, Debug)]
pub enum DecoderResponse {
    Ok,
    Error(String),
}

/// Commands for the processing thread
pub enum ProcessingCommand {
    /// Commit a fully validated and allocation-prepared plugin-host replacement.
    CommitHostUpdate(PreparedHostUpdate),
    /// Set a plugin parameter
    SetParameter {
        plugin_index: usize,
        param_id: String,
        value: String, // Generic string value (JSON for complex types, or stringified primitives)
    },
    /// Bypass all processing (pass-through)
    Bypass(bool),
    /// Query plugin data (e.g. analyzer results)
    GetPluginData(usize),
    /// Poll isolated external plugin workers for process lifecycle events
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    PollIsolatedExternalPluginWorkers,
    /// Stop processing
    Stop,
    /// Shutdown the thread
    Shutdown,
}

impl std::fmt::Debug for ProcessingCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CommitHostUpdate(update) => {
                f.debug_tuple("CommitHostUpdate").field(update).finish()
            }
            Self::SetParameter {
                plugin_index,
                param_id,
                value,
            } => f
                .debug_struct("SetParameter")
                .field("plugin_index", plugin_index)
                .field("param_id", param_id)
                .field("value", value)
                .finish(),
            Self::Bypass(bypass) => f.debug_tuple("Bypass").field(bypass).finish(),
            Self::GetPluginData(index) => f.debug_tuple("GetPluginData").field(index).finish(),
            #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
            Self::PollIsolatedExternalPluginWorkers => {
                write!(f, "PollIsolatedExternalPluginWorkers")
            }
            Self::Stop => write!(f, "Stop"),
            Self::Shutdown => write!(f, "Shutdown"),
        }
    }
}

/// Response from processing thread
#[derive(Clone, Debug)]
pub enum ProcessingResponse {
    /// Ok response
    Ok,
    /// Plugin chain updated with new output channel count and latency
    PluginChainUpdated {
        output_channels: usize,
        previous_latency_samples: usize,
        latency_samples: usize,
        latency_changed: bool,
    },
    /// Plugin data
    PluginData(Arc<dyn Any + Send + Sync>),
    /// Error
    Error(String),
}

/// Commands for the playback thread
#[derive(Clone, Debug)]
pub enum PlaybackCommand {
    /// Set output volume (linear, 0.0 = silence, 1.0 = unity)
    SetVolume(f32),
    /// Mute/unmute
    Mute(bool),
    /// Update output channel count (requires rebuilding stream)
    UpdateChannels(usize),
    /// Update output sample rate (requires rebuilding stream)
    UpdateSampleRate(u32),
    /// Stop playback
    Stop,
    /// Shutdown the thread
    Shutdown,
}

/// Commands for the manager thread
#[derive(Clone, Debug)]
pub enum ManagerCommand {
    // Playback control
    Play(AudioSource),
    /// Play a source starting at a specific position
    PlayAt(AudioSource, f64),
    Pause,
    Resume,
    Stop,
    Seek(f64),
    /// Queue the next source for gapless playback.
    /// When the current track ends, the decoder seamlessly starts the queued source
    /// without any gap in audio output.
    QueueNext(AudioSource),
    /// Cancel a previously queued next source.
    CancelNext,

    // Volume control
    SetVolume(f32),
    Mute(bool),

    // Plugin control
    UpdatePluginChain(Vec<PluginConfig>),
    UpdatePluginGraph(PluginGraphConfig),
    SetPluginParameter {
        plugin_index: usize,
        param_id: String,
        value: String, // Generic string value (JSON for complex types, or stringified primitives)
    },
    BypassProcessing(bool),

    /// Poll isolated external plugin worker status without starting or restarting workers.
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    MaintainIsolatedExternalPluginWorkers,

    // Queries
    GetState,
    GetPosition,
    GetPluginData(usize),

    // Lifecycle
    ReloadConfig,
    Shutdown,
}

/// Response from manager thread
#[allow(
    clippy::large_enum_variant,
    reason = "boxing the state response would change the manager API and allocation behavior"
)]
#[derive(Clone)]
pub enum ManagerResponse {
    Ok,
    State(AudioEngineState),
    Position(f64),
    PluginData(Arc<dyn Any + Send + Sync>),
    Error(String),
    Shutdown,
}

// ============================================================================
// Thread Events - Events sent from threads to manager
// ============================================================================

/// Events sent from worker threads to manager
#[derive(Clone, Debug)]
pub enum ThreadEvent {
    /// Decoder reached end of stream
    DecoderEndOfStream,
    /// Decoder seamlessly transitioned to a queued next source (gapless playback)
    DecoderGaplessTransition(AudioSource),
    /// Decoder error
    DecoderError(String),
    /// Live stream metadata update (ICY/content-type/bitrate).
    StreamMetadataChanged(Option<StreamMetadata>),
    PlaybackChannelsChanged(usize),
    PlaybackOutputDeviceChanged(String),
    PlaybackOutputAccessChanged(crate::OutputAccessStatus),
    /// Playback thread hardware-consumption diagnostics changed.
    PlaybackStats {
        callback_count: u64,
        buffer_fill_percent: u64,
        stream_error_count: u64,
        frames_received: u64,
        frames_written: u64,
        frames_dropped: u64,
        effective_sample_rate: u64,
    },
    /// Post-volume, pre-clamp output level for the most recent meter window.
    PlaybackOutputMeter {
        peak_linear: f32,
        clipping_detected: bool,
    },
    /// Playback thread has fully drained its ring buffer after end-of-stream
    PlaybackDrained,
    /// Playback buffer underrun count update
    PlaybackUnderrun(u64),
    /// Processing error (fatal — sets PlaybackState::Stopped)
    ProcessingError(String),
    /// Non-fatal processing warning (sets last_error but does NOT change playback state)
    ProcessingWarning(String),
    /// Thread panicked
    ThreadPanic(String),
    /// Position update
    PositionUpdate(f64),
    /// Seek completed
    SeekComplete,
    /// Plugin chain total latency changed (in samples at the processing sample rate)
    PluginLatencyUpdate(usize),
    /// Isolated external plugin worker status snapshot.
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    IsolatedExternalPluginWorkerStatuses(Vec<IsolatedExternalPluginWorkerStatus>),
}
