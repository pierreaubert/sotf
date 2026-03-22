// ============================================================================
// Audio Engine Types
// ============================================================================

use serde::{Deserialize, Serialize};
use sotf_plugins::PluginHost;
use std::any::Any;
use std::path::PathBuf;
use std::sync::Arc;

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
// Audio Frame - The unit of audio data passed between threads
// ============================================================================

/// A chunk of interleaved audio samples
#[derive(Clone, Debug)]
pub struct AudioFrame {
    /// Interleaved samples: [L0, R0, L1, R1, ...] for stereo
    pub data: Vec<f32>,
    /// Number of frames (not samples!)
    pub num_frames: usize,
    /// Number of channels
    pub num_channels: usize,
    /// Sample rate
    pub sample_rate: u32,
}

impl AudioFrame {
    /// Create a new audio frame
    pub fn new(data: Vec<f32>, num_frames: usize, num_channels: usize, sample_rate: u32) -> Self {
        debug_assert_eq!(data.len(), num_frames * num_channels);
        Self {
            data,
            num_frames,
            num_channels,
            sample_rate,
        }
    }

    /// Create an empty (silent) audio frame
    pub fn silent(num_frames: usize, num_channels: usize, sample_rate: u32) -> Self {
        Self {
            data: vec![0.0; num_frames * num_channels],
            num_frames,
            num_channels,
            sample_rate,
        }
    }

    /// Total number of samples (frames × channels)
    pub fn num_samples(&self) -> usize {
        self.num_frames * self.num_channels
    }

    /// Clear the frame (set to silence)
    pub fn clear(&mut self) {
        self.data.fill(0.0);
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
    /// Start playing a file
    Play(PathBuf),
    /// Start playing a file at a specific position in seconds
    PlayAt(PathBuf, f64),
    /// Start silent source (for HAL input plugins)
    /// Sends empty frames at regular intervals for source plugins
    StartSilentSource,
    /// Pause decoding
    Pause,
    /// Resume decoding
    Resume,
    /// Seek to position in seconds
    Seek(f64),
    /// Queue the next file for gapless playback.
    /// When the current file ends, the decoder seamlessly transitions to this file
    /// without sending EndOfStream or Flush, avoiding any gap in audio output.
    QueueNext(PathBuf),
    /// Cancel a previously queued next file.
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
    Play(PathBuf),
    /// Play a file starting at a specific position
    PlayAt(PathBuf, f64),
    Pause,
    Resume,
    Stop,
    Seek(f64),
    /// Queue the next file for gapless playback.
    /// When the current track ends, the decoder seamlessly starts the queued file
    /// without any gap in audio output.
    QueueNext(PathBuf),
    /// Cancel a previously queued next file.
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
// State - Engine and playback state
// ============================================================================

/// Playback state
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

/// Complete audio engine state
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioEngineState {
    /// Current playback state
    pub playback_state: PlaybackState,
    /// Currently playing file
    pub current_file: Option<PathBuf>,
    /// Current position in seconds
    pub position: f64,
    /// Total duration in seconds (if known)
    pub duration: Option<f64>,
    /// Sample rate
    pub sample_rate: u32,
    /// Number of channels
    pub num_channels: usize,
    /// Output volume (linear)
    pub volume: f32,
    /// Muted flag
    pub muted: bool,
    /// Processing bypassed flag
    pub processing_bypassed: bool,
    /// Number of buffer underruns
    pub underruns: u64,
    /// Total plugin chain latency in samples (for position compensation)
    pub plugin_latency_samples: usize,
    /// Last error message, if any
    pub last_error: Option<String>,
    /// Seek in progress flag
    pub seeking: bool,
}

impl Default for AudioEngineState {
    fn default() -> Self {
        Self {
            playback_state: PlaybackState::Stopped,
            current_file: None,
            position: 0.0,
            duration: None,
            sample_rate: 48000,
            num_channels: 2,
            volume: 1.0,
            muted: false,
            processing_bypassed: false,
            underruns: 0,
            plugin_latency_samples: 0,
            last_error: None,
            seeking: false,
        }
    }
}

// ============================================================================
// Thread Events - Events sent from threads to manager
// ============================================================================

/// Events sent from worker threads to manager
#[derive(Clone, Debug)]
pub enum ThreadEvent {
    /// Decoder reached end of stream
    DecoderEndOfStream,
    /// Decoder seamlessly transitioned to a queued next file (gapless playback)
    DecoderGaplessTransition(PathBuf),
    /// Decoder error
    DecoderError(String),
    PlaybackChannelsChanged(usize),
    /// Playback thread has fully drained its ring buffer after end-of-stream
    PlaybackDrained,
    /// Playback buffer underrun count update
    PlaybackUnderrun(u64),
    /// Processing error
    ProcessingError(String),
    /// Thread panicked
    ThreadPanic(String),
    /// Position update
    PositionUpdate(f64),
    /// Seek completed
    SeekComplete,
    /// Plugin chain total latency changed (in samples at the processing sample rate)
    PluginLatencyUpdate(usize),
}

// ============================================================================
// Plugin Configuration
// ============================================================================

/// Plugin configuration for serialization/deserialization
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Plugin type identifier
    pub plugin_type: String,
    /// Plugin parameters
    pub parameters: serde_json::Value,
}

impl PluginConfig {
    /// Create a new plugin config
    pub fn new(plugin_type: impl Into<String>, parameters: serde_json::Value) -> Self {
        Self {
            plugin_type: plugin_type.into(),
            parameters,
        }
    }
}

/// Graph-based plugin configuration for DAG processing.
///
/// Unlike `Vec<PluginConfig>` (linear chain), this supports parallel paths
/// needed for multi-driver crossover setups.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginGraphConfig {
    pub nodes: Vec<PluginGraphNodeConfig>,
    pub edges: Vec<PluginGraphEdgeConfig>,
}

/// A node in the plugin graph
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginGraphNodeConfig {
    /// Unique node ID (used to reference in edges)
    pub id: usize,
    pub plugin_type: String,
    pub parameters: serde_json::Value,
    /// Number of input channels this node expects
    pub input_channels: usize,
}

/// An edge connecting two nodes in the plugin graph
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginGraphEdgeConfig {
    pub from_node: usize,
    pub to_node: usize,
}
