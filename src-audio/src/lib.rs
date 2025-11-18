pub mod devices;
pub use devices::SharedAudioState;

pub mod decoder;
pub use decoder::{
    AudioDecoder, AudioDecoderError, AudioDecoderResult, AudioFormat, AudioStream, DecodedAudio,
    StreamConfig, create_decoder, probe_file,
};

pub use decoder::core::AudioSpec;
pub use decoder::stream::{StreamEvent, StreamPosition, StreamState};

pub mod manager;
pub use manager::{
    AudioFileInfo, AudioStreamingManager, StreamingCommand, StreamingEvent, StreamingState,
};

pub mod replaygain;
pub mod signal_recorder;
pub mod signals;

pub mod signal_analysis;

pub mod sofa;
pub use sofa::{HrtfData, SofaFile, SourcePosition};

pub mod plugins;
pub use plugins::{
    AnalyzerData, AnalyzerPlugin, CompressorPlugin, EqPlugin, GainPlugin, GatePlugin,
    InPlacePlugin, InPlacePluginAdapter, LimiterPlugin, LoudnessCompensation,
    LoudnessCompensationPlugin, LoudnessData, LoudnessInfo, LoudnessMonitorPlugin, Parameter,
    ParameterId, ParameterValue, Plugin, PluginHost, PluginInfo, ProcessContext, ResamplerPlugin,
    SharedPluginHost, SpectrumAnalyzerPlugin, SpectrumData, SpectrumInfo, UpmixerPlugin,
};

// HAL plugins (macOS only)
#[cfg(target_os = "macos")]
pub use plugins::{HalInputPlugin, HalInputPluginParams, HalOutputPlugin, HalOutputPluginParams};

pub mod engine;
pub use engine::{AudioEngine, AudioEngineState, EngineConfig, PlaybackState, PluginConfig};

// pub mod audio_playback;
// pub use audio_playback::{PlaybackRecorder, PlaybackRecordingConfig, AudioPlaybackError};
