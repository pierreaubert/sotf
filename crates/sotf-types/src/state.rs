//! Playback state and audio frame types.

use super::{
    AudioSource, DsdOutputMode, DsdOutputStatus, EngineOversamplingPolicy, NetworkEndpointConfig,
    NetworkEndpointStatus, OutputAccessMode, OutputAccessStatus,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    /// Create a new audio frame.
    ///
    /// # Panics
    /// Panics if `data.len()` does not equal `num_frames * num_channels`.
    /// This is a load-bearing invariant — downstream DSP code indexes the
    /// flat buffer with that arithmetic and a mismatch would silently corrupt
    /// audio in release builds, so we assert unconditionally (not just under
    /// debug_assertions).
    pub fn new(data: Vec<f32>, num_frames: usize, num_channels: usize, sample_rate: u32) -> Self {
        let expected = num_frames
            .checked_mul(num_channels)
            .expect("AudioFrame::new: num_frames * num_channels overflowed usize");
        assert_eq!(
            data.len(),
            expected,
            "AudioFrame::new: data length {} != num_frames ({}) * num_channels ({})",
            data.len(),
            num_frames,
            num_channels
        );
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

    /// Total number of samples (frames x channels)
    pub fn num_samples(&self) -> usize {
        self.num_frames * self.num_channels
    }

    /// Clear the frame (set to silence)
    pub fn clear(&mut self) {
        self.data.fill(0.0);
    }
}

/// Playback state
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

/// Lifecycle event reported for an isolated external plugin worker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IsolatedExternalPluginWorkerEvent {
    /// Worker process is already running.
    AlreadyRunning,
    /// Worker process was started.
    Started {
        /// Worker process ID.
        pid: u32,
    },
    /// Worker process has exited.
    Exited {
        /// Exit code if available.
        exit_code: Option<i32>,
    },
    /// Worker is not running.
    NotRunning,
}

/// Reported worker sandbox status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IsolatedExternalPluginSandboxStatus {
    #[default]
    Unknown,
    Disabled,
    Enforced,
    Unsupported,
}

/// Reported sandbox backend for a worker process.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IsolatedExternalPluginSandboxBackend {
    #[default]
    Unknown,
    LinuxLandlock,
    MacosProcessIsolation,
    WindowsProcessIsolation,
}

/// Snapshot of a single isolated external plugin worker status.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IsolatedExternalPluginWorkerStatus {
    /// Index of the plugin in the chain.
    pub plugin_index: usize,
    /// Host node id for the plugin.
    pub node_id: usize,
    /// Latest lifecycle event reported by the worker supervisor.
    pub event: Option<IsolatedExternalPluginWorkerEvent>,
    /// Last error while polling/ensuring worker state.
    pub error: Option<String>,
    /// Number of successful worker starts.
    pub worker_start_count: u64,
    /// Number of observed worker exits.
    pub worker_exit_count: u64,
    /// Number of launch failures.
    pub worker_launch_failure_count: u64,
    /// Number of block timeouts from the shared-memory proxy.
    pub block_timeout_count: u64,
    /// Number of worker failures while processing blocks.
    pub block_worker_failure_count: u64,
    /// Number of wrong-sequence block responses from the worker.
    pub block_wrong_sequence_count: u64,
    /// Runtime sandbox status observed by the worker.
    #[serde(default)]
    pub sandbox_status: IsolatedExternalPluginSandboxStatus,
    /// Runtime sandbox backend observed by the worker.
    #[serde(default)]
    pub sandbox_backend: IsolatedExternalPluginSandboxBackend,
    /// Optional reason text when sandboxing is unavailable/disabled/failed.
    #[serde(default)]
    pub sandbox_reason: Option<String>,
}

/// Complete audio engine state
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioEngineState {
    /// Current playback state
    pub playback_state: PlaybackState,
    /// Currently playing source
    pub current_source: Option<AudioSource>,
    /// Currently playing file path (convenience accessor, populated for File sources)
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
    /// Output device actually resolved by the playback stream.
    #[serde(default)]
    pub playback_output_device: Option<String>,
    /// Number of hardware output callbacks observed by the playback stream.
    #[serde(default)]
    pub playback_callback_count: u64,
    /// Last reported playback ring-buffer fill percentage.
    #[serde(default)]
    pub playback_buffer_fill_percent: u64,
    /// Number of output stream errors observed by the playback stream.
    #[serde(default)]
    pub playback_stream_error_count: u64,
    /// Number of processed frames received by the playback thread.
    #[serde(default)]
    pub playback_frames_received: u64,
    /// Number of processed frames written to the hardware ring buffer.
    #[serde(default)]
    pub playback_frames_written: u64,
    /// Number of processed frames dropped before reaching hardware.
    #[serde(default)]
    pub playback_frames_dropped: u64,
    /// Estimated callback sample rate from hardware consumption.
    #[serde(default)]
    pub playback_effective_sample_rate: u64,
    /// Total plugin chain latency in samples (for position compensation)
    pub plugin_latency_samples: usize,
    /// Whether transport position should compensate for plugin latency.
    #[serde(default)]
    pub latency_compensation_enabled: bool,
    /// Requested output access mode.
    #[serde(default)]
    pub output_access_mode: OutputAccessMode,
    /// Actual output access status reported by the backend.
    #[serde(default)]
    pub output_access_status: OutputAccessStatus,
    /// Requested DSD output behavior.
    #[serde(default)]
    pub dsd_output_mode: DsdOutputMode,
    /// Runtime DSD capability status.
    #[serde(default)]
    pub dsd_output_status: DsdOutputStatus,
    /// Active host oversampling policy.
    #[serde(default)]
    pub oversampling_policy: EngineOversamplingPolicy,
    /// Configured network endpoint.
    #[serde(default)]
    pub network_endpoint: NetworkEndpointConfig,
    /// Runtime network endpoint capability status.
    #[serde(default)]
    pub network_endpoint_status: NetworkEndpointStatus,
    /// Last error message, if any
    pub last_error: Option<String>,
    /// Seek in progress flag
    pub seeking: bool,
    /// Snapshot of isolated external plugin worker status.
    #[serde(default)]
    pub isolated_external_plugin_worker_statuses: Vec<IsolatedExternalPluginWorkerStatus>,
}

impl Default for AudioEngineState {
    fn default() -> Self {
        Self {
            playback_state: PlaybackState::Stopped,
            current_source: None,
            current_file: None,
            position: 0.0,
            duration: None,
            sample_rate: 48000,
            num_channels: 2,
            volume: 1.0,
            muted: false,
            processing_bypassed: false,
            underruns: 0,
            playback_output_device: None,
            playback_callback_count: 0,
            playback_buffer_fill_percent: 0,
            playback_stream_error_count: 0,
            playback_frames_received: 0,
            playback_frames_written: 0,
            playback_frames_dropped: 0,
            playback_effective_sample_rate: 0,
            plugin_latency_samples: 0,
            latency_compensation_enabled: true,
            output_access_mode: OutputAccessMode::Shared,
            output_access_status: OutputAccessStatus::Shared,
            dsd_output_mode: DsdOutputMode::Disabled,
            dsd_output_status: DsdOutputStatus::Disabled,
            oversampling_policy: EngineOversamplingPolicy::PluginPreferred,
            network_endpoint: NetworkEndpointConfig::default(),
            network_endpoint_status: NetworkEndpointStatus::Disabled,
            last_error: None,
            seeking: false,
            isolated_external_plugin_worker_statuses: Vec::new(),
        }
    }
}
