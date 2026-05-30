//! Shared configuration types for the SOTF audio system.
//!
//! This crate contains lightweight, serializable types used across the SOTF
//! workspace without pulling in audio processing dependencies (cpal, symphonia, etc.).

mod audio_source;
mod config;
mod engine_features;
mod plugin_config;
mod sink;
mod state;

pub use audio_source::{AudioSource, ServiceId};
pub use config::EngineConfig;
pub use engine_features::{
    DsdOutputMode, DsdOutputStatus, EngineOversamplingPolicy, LatencyCompensationMode,
    NetworkEndpointConfig, NetworkEndpointMode, NetworkEndpointStatus, OutputAccessMode,
    OutputAccessStatus,
};
pub use plugin_config::{
    PluginConfig, PluginGraphConfig, PluginGraphEdgeConfig, PluginGraphNodeConfig,
};
pub use sink::{SinkConfig, SinkOpenResult, SinkType};
pub use state::{AudioEngineState, AudioFrame, PlaybackState};
pub use state::{
    IsolatedExternalPluginSandboxBackend, IsolatedExternalPluginSandboxStatus,
    IsolatedExternalPluginWorkerEvent, IsolatedExternalPluginWorkerStatus,
};
