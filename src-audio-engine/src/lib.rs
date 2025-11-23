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
    AudioEngineManager, AudioFileInfo, StreamingCommand, StreamingEvent, StreamingState,
};

pub mod preflight;
pub use preflight::{run_preflight_checks, PreflightError};

pub mod replaygain;
pub mod signal_recorder;
pub mod signals;

pub mod signal_analysis;

pub mod engine;
pub use engine::{AudioEngine, AudioEngineState, EngineConfig, PlaybackState, PluginConfig};

// Re-export plugin types for convenience
pub use sotf_plugins::{
    HrtfData, LoudnessCompensation, LoudnessData, LoudnessInfo, SofaFile, SourcePosition,
    SpectrumData, SpectrumInfo, get_speaker_config_by_channels,
};
