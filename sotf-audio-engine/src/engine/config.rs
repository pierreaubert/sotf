// ============================================================================
// Engine Configuration
// ============================================================================

use super::PluginConfig;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Audio engine configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EngineConfig {
    /// Configuration version for migration support
    #[serde(default = "default_engine_config_version")]
    pub version: u32,

    /// Processing frame size (number of frames per block)
    pub frame_size: usize,

    /// Queue buffer size in milliseconds
    pub buffer_ms: u32,

    /// Target output sample rate (hardware sample rate)
    pub output_sample_rate: u32,

    /// Input channel count (from decoder/source)
    pub input_channels: usize,

    /// Target output channels (for hardware/validation)
    pub output_channels: usize,

    /// Output device name (None = default device)
    #[serde(skip)]
    pub output_device: Option<String>,

    /// Initial plugin chain
    pub plugins: Vec<PluginConfig>,

    /// Initial volume (linear, 0.0-1.0)
    pub volume: f32,

    /// Start muted
    pub muted: bool,

    /// Optional path to config file for watching/reloading
    #[serde(skip)]
    pub config_path: Option<PathBuf>,

    /// Watch config file and Unix signals for reload/shutdown
    #[serde(skip)]
    pub watch_config: bool,
}

fn default_engine_config_version() -> u32 {
    1
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            version: default_engine_config_version(),
            frame_size: 1024,
            buffer_ms: 200,
            output_sample_rate: 48000,
            input_channels: 2,
            output_channels: 2,
            output_device: None,
            plugins: Vec::new(),
            volume: 1.0,
            muted: false,
            config_path: None,
            watch_config: false,
        }
    }
}

impl EngineConfig {
    /// Calculate queue capacity in frames
    pub fn queue_capacity_frames(&self) -> usize {
        let total_frames = (self.output_sample_rate as u64 * self.buffer_ms as u64) / 1000;
        (total_frames as usize).div_ceil(self.frame_size)
    }

    /// Calculate total buffer size in frames
    pub fn total_buffer_frames(&self) -> usize {
        (self.output_sample_rate as u64 * self.buffer_ms as u64 / 1000) as usize
    }

    /// Load configuration from a JSON file, applying migrations if needed
    pub fn load_from_file(path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let json = std::fs::read_to_string(path)?;
        let mut config: EngineConfig = serde_json::from_str(&json)?;

        // Check if migration is needed
        const LATEST_VERSION: u32 = 1;
        let original_version = config.version;

        if config.version < LATEST_VERSION {
            log::info!(
                "Migrating EngineConfig from version {} to {}",
                original_version,
                LATEST_VERSION
            );

            // Apply migrations
            config = Self::migrate(config)?;

            // Save upgraded config back to disk
            config.save_to_file(path)?;

            log::info!(
                "Successfully migrated EngineConfig from version {} to {}",
                original_version,
                LATEST_VERSION
            );
        }

        Ok(config)
    }

    /// Save configuration to a JSON file
    pub fn save_to_file(&self, path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Apply all necessary migrations to bring EngineConfig to the latest version
    fn migrate(config: EngineConfig) -> Result<EngineConfig, Box<dyn std::error::Error>> {
        const LATEST_VERSION: u32 = 1;

        // Apply migrations sequentially
        while config.version < LATEST_VERSION {
            match config.version {
                // Example migration from version 1 to 2:
                // 1 => {
                //     log::info!("Applying EngineConfig migration: v1 -> v2");
                //     // Apply migration logic here
                //     // e.g., add new field with default value, transform plugin configs, etc.
                //     config.version = 2;
                // }

                // If we reach here with no match, we have an unknown version
                v => {
                    return Err(format!("Unknown EngineConfig version: {}", v).into());
                }
            }
        }

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_capacity_calculation() {
        let config = EngineConfig {
            frame_size: 1024,
            buffer_ms: 200,
            output_sample_rate: 48000,
            ..Default::default()
        };

        // 200ms at 48kHz = 9600 frames
        // 9600 / 1024 = ~9.375, rounds up to 10 chunks
        let capacity = config.queue_capacity_frames();
        assert_eq!(capacity, 10);

        let total_frames = config.total_buffer_frames();
        assert_eq!(total_frames, 9600);
    }

    #[test]
    fn test_queue_capacity_different_rates() {
        let config = EngineConfig {
            frame_size: 512,
            buffer_ms: 100,
            output_sample_rate: 44100,
            ..Default::default()
        };

        // 100ms at 44.1kHz = 4410 frames
        // 4410 / 512 = ~8.6, rounds up to 9 chunks
        let capacity = config.queue_capacity_frames();
        assert_eq!(capacity, 9);
    }
}
