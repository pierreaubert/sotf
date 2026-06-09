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
    pub fn default_paths() -> Result<Self> {
        let config_dir = directories::BaseDirs::new()
            .ok_or_else(|| {
                crate::error::MidiError::ConfigError("Could not find config directory".to_string())
            })?
            .config_dir()
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
    use crate::MidiError;

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

    #[test]
    fn test_load_missing_file_is_io_error() {
        let result = MidiConfig::load("/definitely/does/not/exist.json");
        assert!(matches!(result, Err(MidiError::IoError(_))));
    }

    #[test]
    fn test_load_invalid_json_is_json_error() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut file = NamedTempFile::new().unwrap();
        write!(file, "not json").unwrap();
        let result = MidiConfig::load(file.path());
        assert!(matches!(result, Err(MidiError::JsonError(_))));
    }

    #[test]
    fn test_save_and_load_round_trip() {
        use tempfile::NamedTempFile;

        let mut config = MidiConfig::default();
        config.default_input = Some("input-1".to_string());
        config.default_output = Some("output-1".to_string());
        config.learn_mode = true;
        config.listen_channel = Some(5);

        let mut profile = DeviceProfile::new("foo".to_string());
        profile.description = Some("desc".to_string());
        profile.input_device = Some("dev-in".to_string());
        profile.output_device = Some("dev-out".to_string());
        profile.add_mapping(7, "volume".to_string());
        profile.add_init_message(vec![0xF0, 0x7D, 0x01, 0xF7]);
        profile
            .device_config
            .add_setting("sensitivity".to_string(), serde_json::json!(0.5));

        config.add_profile("foo".to_string(), profile);
        config.set_active_profile("foo".to_string());

        let file = NamedTempFile::new().unwrap();
        config.save(file.path()).unwrap();
        let loaded = MidiConfig::load(file.path()).unwrap();

        assert_eq!(loaded.active_profile, Some("foo".to_string()));
        assert_eq!(loaded.default_input, Some("input-1".to_string()));
        assert_eq!(loaded.default_output, Some("output-1".to_string()));
        assert!(loaded.learn_mode);
        assert_eq!(loaded.listen_channel, Some(5));
        let p = loaded.get_profile("foo").unwrap();
        assert_eq!(p.name, "foo");
        assert_eq!(p.description, Some("desc".to_string()));
        assert_eq!(p.get_mapping(7), Some(&"volume".to_string()));
        assert_eq!(p.init_messages, vec![vec![0xF0, 0x7D, 0x01, 0xF7]]);
        assert_eq!(
            p.device_config.get_setting("sensitivity"),
            Some(&serde_json::json!(0.5))
        );
    }

    #[test]
    fn test_active_profile_missing_returns_none() {
        let mut config = MidiConfig::default();
        config.set_active_profile("missing".to_string());
        assert!(config.active_profile().is_none());
    }

    #[test]
    fn test_add_profile_overwrites_existing() {
        let mut config = MidiConfig::default();
        let first = DeviceProfile::new("p".to_string());
        let mut second = DeviceProfile::new("p".to_string());
        second.description = Some("second".to_string());
        config.add_profile("p".to_string(), first);
        config.add_profile("p".to_string(), second);
        assert_eq!(
            config.get_profile("p").unwrap().description,
            Some("second".to_string())
        );
    }

    #[test]
    fn test_device_profile_init_messages() {
        let mut profile = DeviceProfile::new("test".to_string());
        profile.add_init_message(vec![0x90, 60, 100]);
        profile.add_init_message(vec![0x80, 60, 0]);
        assert_eq!(profile.init_messages.len(), 2);
        assert_eq!(profile.init_messages[1], vec![0x80, 60, 0]);
    }

    #[test]
    fn test_device_config_custom_settings() {
        let mut cfg = DeviceConfig::new();
        cfg.add_setting("key".to_string(), serde_json::json!(42));
        assert_eq!(cfg.get_setting("key"), Some(&serde_json::json!(42)));
        assert_eq!(cfg.get_setting("missing"), None);
    }

    #[test]
    fn test_midi_config_default_values() {
        let config = MidiConfig::default();
        assert!(config.profiles.is_empty());
        assert!(config.active_profile.is_none());
        assert!(config.default_input.is_none());
        assert!(config.default_output.is_none());
        assert!(!config.learn_mode);
        assert_eq!(config.listen_channel, None);
    }
}
