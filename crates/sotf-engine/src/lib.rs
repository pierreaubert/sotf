#![recursion_limit = "256"]

#[cfg(not(target_os = "ios"))]
pub mod devices;
#[cfg(not(target_os = "ios"))]
pub use devices::SharedAudioState;

#[cfg(target_os = "ios")]
pub mod devices_stub;
#[cfg(target_os = "ios")]
pub use devices_stub as devices;
#[cfg(target_os = "ios")]
pub use devices_stub::SharedAudioState;

pub mod decoder;
pub use decoder::{
    AudioDecoder, AudioDecoderError, AudioDecoderResult, AudioFormat, AudioStream, DecodedAudio,
    DsdDecodeCapability, StreamConfig, create_decoder, probe_file,
};

pub use decoder::core::AudioSpec;
pub use decoder::stream::{StreamEvent, StreamPosition, StreamState};

pub mod manager;
pub use manager::{
    AudioEngineManager, AudioFileInfo, StreamingCommand, StreamingEvent, StreamingState,
    clear_verified_rate_cache, select_output_sample_rate,
};

pub mod preflight;
pub use preflight::{PreflightError, run_preflight_checks};

pub mod project;

pub mod rate_limit;

pub mod replaygain;
pub mod signal_recorder;
pub use signal_recorder::{
    ChannelRecordingInfo, DeviceInfo, RecordingSession, reprocess_recordings,
};
pub mod timeline;
pub mod waveform;

// Re-export from math-dsp crate
pub use math_audio_dsp::analysis as signal_analysis;
pub use math_audio_dsp::signals;
pub use math_audio_dsp::{AnalysisResult, read_analysis_csv, write_analysis_csv};

pub mod engine;
#[cfg(not(target_os = "ios"))]
pub use engine::CpalSink;
pub use engine::{
    AudioEngine, AudioEngineState, AudioSink, DsdOutputBackend, DsdOutputPlan, EngineConfig,
    EngineFeaturePlan, NetworkEndpointBackend, NetworkEndpointPlan, OutputAccessBackend,
    OutputAccessPlan, PlaybackState, PluginConfig, PluginGraphConfig, PluginGraphEdgeConfig,
    PluginGraphNodeConfig, SinkConfig, SinkOpenResult, SinkType, plan_dsd_output,
    plan_engine_features, plan_network_endpoint, plan_output_access,
};
pub use sotf_types::{
    DsdOutputMode, DsdOutputStatus, EngineOversamplingPolicy, LatencyCompensationMode,
    NetworkEndpointConfig, NetworkEndpointMode, NetworkEndpointStatus, OutputAccessMode,
    OutputAccessStatus,
};

// Re-export driver-common types for daemon and other consumers
pub use driver_common::{self, AudioDriver, DriverConfig, DriverStatus};

mod plugin_param_accessors;
pub mod plugins;
pub use plugins::{
    ChannelConflict, EQFilter, Plugin, PluginChain, PluginSettings, PluginType,
    apply_matrix_preset, db_to_linear, detect_matrix_preset, get_channel_label,
    get_channel_label_from_config, linear_to_db_string, preset_file_to_path_config_json,
    resize_matrix,
};

// Re-export plugin types for convenience
pub use sotf_plugins::{
    HrtfData, LoudnessCompensation, LoudnessData, LoudnessInfo, SofaFile, SourcePosition,
    SpectrumData, SpectrumInfo, get_speaker_config_by_channels,
};
