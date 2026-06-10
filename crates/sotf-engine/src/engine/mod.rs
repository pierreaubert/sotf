// ============================================================================
// Audio Engine - Native Multi-Threaded Audio Processing
// ============================================================================
//
// Replaces CamillaDSP with a native Rust implementation using the plugin system.
//
// Architecture:
//   Thread 1: Decoder + Resampler → Queue 1
//   Thread 2: Processing (PluginHost) → Queue 2
//   Thread 3: Playback (cpal output)
//   Thread 4: Manager (coordination + signals)

mod types;
pub use types::*;

mod audio_sink;
pub use audio_sink::{AudioSink, SinkConfig, SinkOpenResult, SinkType};

mod dsd_output;
pub use dsd_output::{DsdOutputBackend, DsdOutputPlan, plan_dsd_output};

mod feature_plan;
pub use feature_plan::{EngineFeaturePlan, plan_engine_features};

mod network_endpoint;
pub use network_endpoint::{NetworkEndpointBackend, NetworkEndpointPlan, plan_network_endpoint};

mod output_access;
pub use output_access::{OutputAccessBackend, OutputAccessPlan, plan_output_access};

#[cfg(not(target_os = "ios"))]
mod cpal_sink;
#[cfg(not(target_os = "ios"))]
pub use cpal_sink::CpalSink;

#[cfg(not(target_os = "ios"))]
mod playback_thread;
#[cfg(not(target_os = "ios"))]
pub use playback_thread::PlaybackThread;
#[cfg(not(target_os = "ios"))]
#[doc(hidden)]
pub mod playback_runtime_harness;

#[cfg(target_os = "ios")]
mod playback_thread_stub;
#[cfg(target_os = "ios")]
pub use playback_thread_stub::PlaybackThread;

mod decoder_thread;
pub use decoder_thread::DecoderThread;

mod processing_thread;
pub use processing_thread::{ProcessingThread, build_plugin_host};

mod manager_thread;
pub use manager_thread::ManagerThread;

mod audio_engine;
pub use audio_engine::AudioEngine;

mod config;
pub use config::EngineConfig;

#[cfg(not(target_os = "ios"))]
mod config_watcher;
#[cfg(not(target_os = "ios"))]
pub use config_watcher::{ConfigEvent, ConfigWatcher};

#[cfg(target_os = "ios")]
mod config_watcher_stub;
#[cfg(target_os = "ios")]
pub use config_watcher_stub::{ConfigEvent, ConfigWatcher};

mod gc_thread;
pub use gc_thread::{GcSender, GcThread};

mod rt_priority;
