//! Genelec GLM (Genelec Loudspeaker Manager) MIDI control profile
//!
//! Provides MIDI control for Genelec SAM monitors via GLM software (version 5.0+).
//!
//! GLM allows MIDI control of:
//! - System volume
//! - Mute/Dim functions
//! - Individual monitor solo/mute
//! - Monitor groups
//! - Volume presets
//! - Bass management
//!
//! **Important:** CC assignments are user-configurable in GLM's MIDI Settings.
//! The defaults provided here are common assignments, but you should verify
//! them in your GLM configuration.
//!
//! # Example
//!
//! ```no_run
//! use sotf_audio_player_midi::profiles::{GenelecGLMProfile, GLMControl};
//! use sotf_audio_player_midi::MidiManager;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut manager = MidiManager::new()?;
//! let mut glm = GLMControl::new(&mut manager);
//!
//! // Set system volume to 50%
//! glm.set_volume(64)?;
//!
//! // Activate dim (-20dB)
//! glm.dim(true)?;
//!
//! // Mute the system
//! glm.mute(true)?;
//!
//! // Solo a specific monitor (MIDI ID 1)
//! glm.solo_monitor(1)?;
//! # Ok(())
//! # }
//! ```

use crate::config::DeviceProfile;
use crate::error::Result;
use crate::manager::MidiManager;
use crate::message::MidiMessage;

/// Genelec GLM function types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GLMFunction {
    /// System volume control
    Volume,
    /// System mute toggle
    Mute,
    /// Dim function (-20dB)
    Dim,
    /// Solo function
    Solo,
    /// Monitor group selection
    MonitorGroup,
    /// Volume preset recall
    VolumePreset,
    /// Bass management toggle
    BassManagement,
    /// System power on/off (GLM 5.0+)
    SystemPower,
    /// Solo/mute individual monitor (GLM 5.0+)
    SoloMuteDevice,
}

/// Genelec GLM profile
pub struct GenelecGLMProfile;

impl GenelecGLMProfile {
    /// Default MIDI channel for GLM control
    pub const DEFAULT_CHANNEL: u8 = 0;

    /// Suggested default CC assignments (user-configurable in GLM)
    pub const VOLUME_CC: u8 = 7;           // Standard volume CC
    pub const MUTE_CC: u8 = 102;
    pub const DIM_CC: u8 = 103;
    pub const SOLO_CC: u8 = 104;
    pub const MONITOR_GROUP_CC: u8 = 105;
    pub const VOLUME_PRESET_CC: u8 = 106;
    pub const BASS_MGMT_CC: u8 = 107;
    pub const SYSTEM_POWER_CC: u8 = 108;
    pub const SOLO_MUTE_DEV_CC: u8 = 109;  // Value = MIDI ID of device

    /// Create a device profile for Genelec GLM
    pub fn create_profile() -> DeviceProfile {
        let mut profile = DeviceProfile::new("Genelec GLM".to_string());
        profile.description = Some(
            "MIDI control for Genelec SAM monitors via GLM 5.0+ software. \
             Note: CC assignments are configurable in GLM MIDI Settings."
                .to_string(),
        );

        // Add function mappings
        profile.add_mapping(Self::VOLUME_CC, "System Volume".to_string());
        profile.add_mapping(Self::MUTE_CC, "System Mute".to_string());
        profile.add_mapping(Self::DIM_CC, "Dim (-20dB)".to_string());
        profile.add_mapping(Self::SOLO_CC, "Solo".to_string());
        profile.add_mapping(Self::MONITOR_GROUP_CC, "Monitor Group".to_string());
        profile.add_mapping(Self::VOLUME_PRESET_CC, "Volume Preset".to_string());
        profile.add_mapping(Self::BASS_MGMT_CC, "Bass Management".to_string());
        profile.add_mapping(Self::SYSTEM_POWER_CC, "System Power".to_string());
        profile.add_mapping(Self::SOLO_MUTE_DEV_CC, "Solo/Mute Device".to_string());

        profile
    }

    /// Create a custom profile with user-defined CC assignments
    ///
    /// # Arguments
    /// * `volume_cc` - CC for volume control
    /// * `mute_cc` - CC for mute toggle
    /// * `dim_cc` - CC for dim function
    /// * `solo_cc` - CC for solo function
    pub fn create_custom_profile(
        volume_cc: u8,
        mute_cc: u8,
        dim_cc: u8,
        solo_cc: u8,
    ) -> DeviceProfile {
        let mut profile = DeviceProfile::new("Genelec GLM (Custom)".to_string());
        profile.description = Some("Custom MIDI control for Genelec GLM".to_string());

        profile.add_mapping(volume_cc, "System Volume".to_string());
        profile.add_mapping(mute_cc, "System Mute".to_string());
        profile.add_mapping(dim_cc, "Dim (-20dB)".to_string());
        profile.add_mapping(solo_cc, "Solo".to_string());

        profile
    }
}

/// High-level control interface for Genelec GLM
pub struct GLMControl<'a> {
    manager: &'a mut MidiManager,
    channel: u8,
    volume_cc: u8,
    mute_cc: u8,
    dim_cc: u8,
    solo_cc: u8,
    monitor_group_cc: u8,
    volume_preset_cc: u8,
    bass_mgmt_cc: u8,
    system_power_cc: u8,
    solo_mute_dev_cc: u8,
}

impl<'a> GLMControl<'a> {
    /// Create a new GLM control interface with default CC mappings
    pub fn new(manager: &'a mut MidiManager) -> Self {
        Self {
            manager,
            channel: GenelecGLMProfile::DEFAULT_CHANNEL,
            volume_cc: GenelecGLMProfile::VOLUME_CC,
            mute_cc: GenelecGLMProfile::MUTE_CC,
            dim_cc: GenelecGLMProfile::DIM_CC,
            solo_cc: GenelecGLMProfile::SOLO_CC,
            monitor_group_cc: GenelecGLMProfile::MONITOR_GROUP_CC,
            volume_preset_cc: GenelecGLMProfile::VOLUME_PRESET_CC,
            bass_mgmt_cc: GenelecGLMProfile::BASS_MGMT_CC,
            system_power_cc: GenelecGLMProfile::SYSTEM_POWER_CC,
            solo_mute_dev_cc: GenelecGLMProfile::SOLO_MUTE_DEV_CC,
        }
    }

    /// Create a GLM control interface with custom CC mappings
    pub fn new_custom(
        manager: &'a mut MidiManager,
        channel: u8,
        volume_cc: u8,
        mute_cc: u8,
        dim_cc: u8,
        solo_cc: u8,
    ) -> Self {
        Self {
            manager,
            channel,
            volume_cc,
            mute_cc,
            dim_cc,
            solo_cc,
            monitor_group_cc: GenelecGLMProfile::MONITOR_GROUP_CC,
            volume_preset_cc: GenelecGLMProfile::VOLUME_PRESET_CC,
            bass_mgmt_cc: GenelecGLMProfile::BASS_MGMT_CC,
            system_power_cc: GenelecGLMProfile::SYSTEM_POWER_CC,
            solo_mute_dev_cc: GenelecGLMProfile::SOLO_MUTE_DEV_CC,
        }
    }

    /// Set system volume
    ///
    /// # Arguments
    /// * `value` - Volume value (0-127)
    pub fn set_volume(&self, value: u8) -> Result<()> {
        self.manager.send_message(&MidiMessage::ControlChange {
            channel: self.channel,
            controller: self.volume_cc,
            value,
        })
    }

    /// Set volume as a percentage
    ///
    /// # Arguments
    /// * `percent` - Volume percentage (0.0-100.0)
    pub fn set_volume_percent(&self, percent: f32) -> Result<()> {
        let value = ((percent.clamp(0.0, 100.0) / 100.0) * 127.0) as u8;
        self.set_volume(value)
    }

    /// Toggle or set mute state
    ///
    /// # Arguments
    /// * `enabled` - true to mute, false to unmute
    pub fn mute(&self, enabled: bool) -> Result<()> {
        let value = if enabled { 127 } else { 0 };
        self.manager.send_message(&MidiMessage::ControlChange {
            channel: self.channel,
            controller: self.mute_cc,
            value,
        })
    }

    /// Toggle or set dim state (-20dB)
    ///
    /// # Arguments
    /// * `enabled` - true to enable dim, false to disable
    pub fn dim(&self, enabled: bool) -> Result<()> {
        let value = if enabled { 127 } else { 0 };
        self.manager.send_message(&MidiMessage::ControlChange {
            channel: self.channel,
            controller: self.dim_cc,
            value,
        })
    }

    /// Toggle or set solo state
    ///
    /// # Arguments
    /// * `enabled` - true to enable solo, false to disable
    pub fn solo(&self, enabled: bool) -> Result<()> {
        let value = if enabled { 127 } else { 0 };
        self.manager.send_message(&MidiMessage::ControlChange {
            channel: self.channel,
            controller: self.solo_cc,
            value,
        })
    }

    /// Select a monitor group
    ///
    /// # Arguments
    /// * `group` - Group number (0-127)
    pub fn select_monitor_group(&self, group: u8) -> Result<()> {
        self.manager.send_message(&MidiMessage::ControlChange {
            channel: self.channel,
            controller: self.monitor_group_cc,
            value: group,
        })
    }

    /// Recall a volume preset
    ///
    /// # Arguments
    /// * `preset` - Preset number (0-127)
    pub fn recall_volume_preset(&self, preset: u8) -> Result<()> {
        self.manager.send_message(&MidiMessage::ControlChange {
            channel: self.channel,
            controller: self.volume_preset_cc,
            value: preset,
        })
    }

    /// Toggle bass management
    ///
    /// # Arguments
    /// * `enabled` - true to enable, false to disable
    pub fn bass_management(&self, enabled: bool) -> Result<()> {
        let value = if enabled { 127 } else { 0 };
        self.manager.send_message(&MidiMessage::ControlChange {
            channel: self.channel,
            controller: self.bass_mgmt_cc,
            value,
        })
    }

    /// Control system power (GLM 5.0+)
    ///
    /// # Arguments
    /// * `on` - true for power on, false for power off
    pub fn system_power(&self, on: bool) -> Result<()> {
        let value = if on { 127 } else { 0 };
        self.manager.send_message(&MidiMessage::ControlChange {
            channel: self.channel,
            controller: self.system_power_cc,
            value,
        })
    }

    /// Solo a specific monitor by MIDI ID (GLM 5.0+)
    ///
    /// The MIDI ID can be found in the monitor info popup in GLM.
    ///
    /// # Arguments
    /// * `midi_id` - MIDI ID of the monitor (0-127)
    pub fn solo_monitor(&self, midi_id: u8) -> Result<()> {
        self.manager.send_message(&MidiMessage::ControlChange {
            channel: self.channel,
            controller: self.solo_mute_dev_cc,
            value: midi_id,
        })
    }

    /// Mute a specific monitor by MIDI ID (GLM 5.0+)
    ///
    /// # Arguments
    /// * `midi_id` - MIDI ID of the monitor (0-127)
    pub fn mute_monitor(&self, midi_id: u8) -> Result<()> {
        // Uses the same CC but different context
        self.manager.send_message(&MidiMessage::ControlChange {
            channel: self.channel,
            controller: self.solo_mute_dev_cc,
            value: midi_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_creation() {
        let profile = GenelecGLMProfile::create_profile();
        assert_eq!(profile.name, "Genelec GLM");
        assert!(profile.description.is_some());
        assert!(!profile.mappings.is_empty());
    }

    #[test]
    fn test_custom_profile() {
        let profile = GenelecGLMProfile::create_custom_profile(10, 11, 12, 13);
        assert_eq!(profile.mappings.get(&10), Some(&"System Volume".to_string()));
        assert_eq!(profile.mappings.get(&11), Some(&"System Mute".to_string()));
    }
}
