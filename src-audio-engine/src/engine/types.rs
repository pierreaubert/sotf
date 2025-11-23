// ============================================================================
// Audio Engine Types
// ============================================================================

use serde::{Deserialize, Serialize};
use std::any::Any;
use std::path::PathBuf;
use std::sync::Arc;

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
    /// Start silent source (for HAL input plugins)
    /// Sends empty frames at regular intervals for source plugins
    StartSilentSource,
    /// Pause decoding
    Pause,
    /// Resume decoding
    Resume,
    /// Seek to position in seconds
    Seek(f64),
    /// Stop decoding and cleanup
    Stop,
    /// Shutdown the thread
    Shutdown,
}

/// Commands for the processing thread
#[derive(Clone, Debug)]
pub enum ProcessingCommand {
    /// Update the plugin chain (hot reload)
    UpdatePlugins(Vec<PluginConfig>),
    /// Set a plugin parameter
    SetParameter {
        plugin_index: usize,
        param_id: String,
        value: f32,
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

/// Response from processing thread
#[derive(Clone)]
pub enum ProcessingResponse {
    /// Ok response
    Ok,
    /// Plugin chain updated with new output channel count
    PluginChainUpdated { output_channels: usize },
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
    Pause,
    Resume,
    Stop,
    Seek(f64),

    // Volume control
    SetVolume(f32),
    Mute(bool),

    // Plugin control
    UpdatePluginChain(Vec<PluginConfig>),
    SetPluginParameter {
        plugin_index: usize,
        param_id: String,
        value: f32,
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
    /// Decoder error
    DecoderError(String),
    /// Playback buffer underrun
    PlaybackUnderrun,
    /// Processing error
    ProcessingError(String),
    /// Thread panicked
    ThreadPanic(String),
    /// Position update
    PositionUpdate(f64),
    /// Seek completed
    SeekComplete,
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
