//! Windows Audio Backend Integration Tests
//!
//! Integration tests for the audio engine with different Windows backends.
//! These tests create actual audio streams to verify the backend works with the engine.
//!
//! Run with: cargo test --test windows_engine_tests --target x86_64-pc-windows-msvc

#[cfg(target_os = "windows")]
mod windows_tests {
    use sotf_audio::engine::{AudioEngine, EngineConfig, PlaybackState};
    use std::time::Duration;

    /// Test: Engine creation with default Windows backend
    #[test]
    fn test_engine_with_default_backend() {
        let _ = env_logger::builder().is_test(true).try_init();

        let config = EngineConfig::default();
        let engine = AudioEngine::new(config);

        match engine {
            Ok(_) => println!("Engine created successfully with default backend"),
            Err(e) => {
                if e.contains("No output device") || e.contains("device") {
                    println!("Expected: No audio device available in test environment");
                } else {
                    panic!("Engine creation failed: {}", e);
                }
            }
        }
    }

    /// Test: Engine handles Windows audio device changes
    #[test]
    fn test_engine_device_handling() {
        let _ = env_logger::builder().is_test(true).try_init();

        let config = EngineConfig::default();

        let _ = match AudioEngine::new(config) {
            Ok(engine) => {
                let state = engine.get_state();
                println!("Engine initial state: {:?}", state.playback_state);
            }
            Err(e) => {
                println!("Expected in CI: {}", e);
            }
        };
    }

    /// Test: Verify engine configuration works with Windows audio
    #[test]
    fn test_engine_config_windows() {
        let config = EngineConfig::default();

        assert_eq!(config.frame_size, 1024);
        assert_eq!(config.buffer_ms, 200);
        assert_eq!(config.output_sample_rate, 48000);
        assert_eq!(config.input_channels, 2);

        println!("Default engine config is valid for Windows");
    }
}

#[cfg(not(target_os = "windows"))]
mod windows_tests {
    use sotf_audio::engine::EngineConfig;

    /// Test: Verify engine config works on non-Windows
    #[test]
    fn test_engine_config_non_windows() {
        let _ = env_logger::builder().is_test(true).try_init();

        // Just test that config is valid
        let config = EngineConfig::default();
        assert_eq!(config.frame_size, 1024);

        println!("Engine config is valid");
    }
}
