// ============================================================================
// Audio Engine Types
// ============================================================================

use crate::decoder::AudioSource;
use sotf_plugins::PluginHost;
use std::any::Any;
use std::sync::Arc;

// Re-export shared types from sotf-types
pub use sotf_types::{
    AudioEngineState, AudioFrame, IsolatedExternalPluginWorkerStatus, PlaybackState, PluginConfig,
    PluginGraphConfig, PluginGraphEdgeConfig, PluginGraphNodeConfig,
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
    /// Update the plugin chain (hot reload)
    /// Receives a fully constructed PluginHost to avoid blocking audio thread
    UpdateHost(Box<PluginHost>),
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
            Self::UpdateHost(_) => f.debug_tuple("UpdateHost").field(&"...").finish(),
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
        latency_samples: usize,
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
    PlaybackChannelsChanged(usize),
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
