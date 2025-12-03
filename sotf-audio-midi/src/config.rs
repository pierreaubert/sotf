//! MIDI configuration and device profiles

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Complete MIDI configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MidiConfig {
    /// Device profiles keyed by profile name
    pub profiles: HashMap<String, DeviceProfile>,

    /// Currently active profile
    pub active_profile: Option<String>,

    /// Default input device (device name)
    pub default_input: Option<String>,

    /// Default output device (device name)
    pub default_output: Option<String>,

    /// Enable MIDI learning mode
    pub learn_mode: bool,

    /// MIDI channel for listening (0-15, None = all channels)
    pub listen_channel: Option<u8>,
}

impl MidiConfig {
    /// Load configuration from a JSON file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let contents = fs::read_to_string(path)?;
        let config = serde_json::from_str(&contents)?;
        Ok(config)
    }

    /// Save configuration to a JSON file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    /// Add a device profile
    pub fn add_profile(&mut self, name: String, profile: DeviceProfile) {
        self.profiles.insert(name, profile);
    }

    /// Get a device profile by name
    pub fn get_profile(&self, name: &str) -> Option<&DeviceProfile> {
        self.profiles.get(name)
    }

    /// Set the active profile
    pub fn set_active_profile(&mut self, name: String) {
        self.active_profile = Some(name);
    }

    /// Get the active profile
    pub fn active_profile(&self) -> Option<&DeviceProfile> {
        self.active_profile
            .as_ref()
            .and_then(|name| self.profiles.get(name))
    }
}

/// A device profile defining MIDI mappings and settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceProfile {
    /// Profile name
    pub name: String,

    /// Description of this profile
    pub description: Option<String>,

    /// Input device name (if any)
    pub input_device: Option<String>,

    /// Output device name (if any)
    pub output_device: Option<String>,

    /// Device-specific configuration
    pub device_config: DeviceConfig,

    /// MIDI mappings (control number -> function name)
    pub mappings: HashMap<u8, String>,

    /// Custom initialization messages to send on connection
    pub init_messages: Vec<Vec<u8>>,
}

impl DeviceProfile {
    /// Create a new device profile
    pub fn new(name: String) -> Self {
        Self {
            name,
            description: None,
            input_device: None,
            output_device: None,
            device_config: DeviceConfig::default(),
            mappings: HashMap::new(),
            init_messages: Vec::new(),
        }
    }

    /// Add a MIDI control mapping
    pub fn add_mapping(&mut self, control: u8, function: String) {
        self.mappings.insert(control, function);
    }

    /// Get the function mapped to a control number
    pub fn get_mapping(&self, control: u8) -> Option<&String> {
        self.mappings.get(&control)
    }

    /// Add an initialization message
    pub fn add_init_message(&mut self, message: Vec<u8>) {
        self.init_messages.push(message);
    }
}

/// Device-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
    /// Device manufacturer
    pub manufacturer: Option<String>,

    /// Device model
    pub model: Option<String>,

    /// MIDI channel (0-15, None = omni)
    pub channel: Option<u8>,

    /// Enable sysex messages
    pub sysex_enabled: bool,

    /// Enable active sensing
    pub active_sensing: bool,

    /// Custom settings as key-value pairs
    pub custom_settings: HashMap<String, serde_json::Value>,
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            manufacturer: None,
            model: None,
            channel: None,
            sysex_enabled: true,
            active_sensing: false,
            custom_settings: HashMap::new(),
        }
    }
}

impl DeviceConfig {
    /// Create a new device configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the MIDI channel
    pub fn with_channel(mut self, channel: u8) -> Self {
        self.channel = Some(channel);
        self
    }

    /// Enable/disable sysex
    pub fn with_sysex(mut self, enabled: bool) -> Self {
        self.sysex_enabled = enabled;
        self
    }

    /// Set manufacturer
    pub fn with_manufacturer(mut self, manufacturer: String) -> Self {
        self.manufacturer = Some(manufacturer);
        self
    }

    /// Set model
    pub fn with_model(mut self, model: String) -> Self {
        self.model = Some(model);
        self
    }

    /// Add a custom setting
    pub fn add_setting(&mut self, key: String, value: serde_json::Value) {
        self.custom_settings.insert(key, value);
    }

    /// Get a custom setting
    pub fn get_setting(&self, key: &str) -> Option<&serde_json::Value> {
        self.custom_settings.get(key)
    }
}

/// Configuration file paths
pub struct ConfigPaths {
    /// User config directory
    pub config_dir: PathBuf,

    /// MIDI config file
    pub midi_config: PathBuf,

    /// Profiles directory
    pub profiles_dir: PathBuf,
}

impl ConfigPaths {
    /// Get default configuration paths
    pub fn default() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| {
                crate::error::MidiError::ConfigError("Could not find config directory".to_string())
            })?
            .join("sotf")
            .join("midi");

        let midi_config = config_dir.join("config.json");
        let profiles_dir = config_dir.join("profiles");

        Ok(Self {
            config_dir,
            midi_config,
            profiles_dir,
        })
    }

    /// Ensure all directories exist
    pub fn ensure_directories(&self) -> Result<()> {
        fs::create_dir_all(&self.config_dir)?;
        fs::create_dir_all(&self.profiles_dir)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_profile_creation() {
        let mut profile = DeviceProfile::new("Test Profile".to_string());
        profile.add_mapping(7, "volume".to_string());
        profile.add_mapping(1, "modulation".to_string());

        assert_eq!(profile.get_mapping(7), Some(&"volume".to_string()));
        assert_eq!(profile.get_mapping(1), Some(&"modulation".to_string()));
        assert_eq!(profile.get_mapping(99), None);
    }

    #[test]
    fn test_midi_config_serialization() {
        let mut config = MidiConfig::default();
        let profile = DeviceProfile::new("Test".to_string());
        config.add_profile("test".to_string(), profile);
        config.set_active_profile("test".to_string());

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: MidiConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.active_profile, Some("test".to_string()));
        assert!(deserialized.profiles.contains_key("test"));
    }

    #[test]
    fn test_device_config_builder() {
        let config = DeviceConfig::new()
            .with_channel(0)
            .with_manufacturer("ACME".to_string())
            .with_model("MK-1000".to_string())
            .with_sysex(true);

        assert_eq!(config.channel, Some(0));
        assert_eq!(config.manufacturer, Some("ACME".to_string()));
        assert_eq!(config.model, Some("MK-1000".to_string()));
        assert!(config.sysex_enabled);
    }
}
