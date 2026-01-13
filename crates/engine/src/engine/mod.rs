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

mod playback_thread;
pub use playback_thread::PlaybackThread;

mod decoder_thread;
pub use decoder_thread::DecoderThread;

mod processing_thread;
pub use processing_thread::ProcessingThread;

mod manager_thread;
pub use manager_thread::ManagerThread;

mod audio_engine;
pub use audio_engine::AudioEngine;

mod config;
pub use config::EngineConfig;

mod config_watcher;
pub use config_watcher::{ConfigEvent, ConfigWatcher};
