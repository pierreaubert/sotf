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

#[cfg(not(target_os = "ios"))]
mod cpal_sink;
#[cfg(not(target_os = "ios"))]
pub use cpal_sink::CpalSink;

#[cfg(not(target_os = "ios"))]
mod playback_thread;
#[cfg(not(target_os = "ios"))]
pub use playback_thread::PlaybackThread;

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
