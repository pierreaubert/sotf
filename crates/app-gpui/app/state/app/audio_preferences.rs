use super::App;
use sotf_audio_player::ui_models::audio_preferences::AudioPreferences;

impl App {
    /// Snapshot committed values only, even when Preferences has unsaved edits.
    pub fn audio_preferences(&self) -> AudioPreferences {
        let devices = &self.audio_device_state;
        AudioPreferences {
            output_device: devices.current_output_device_name.clone(),
            follow_system_default: devices.follow_system_default,
            systemwide_input: !matches!(
                devices.playback_source,
                crate::app::types::PlaybackSource::File
            ),
            sample_rate_hz: devices.hal_config.sample_rate,
            channel_count: devices.hal_config.channel_count,
            buffer_frames: devices.hal_config.buffer_frames,
            replay_gain_enabled: self.playback.replay_gain_enabled,
            replay_gain_mode: self.playback.replay_gain_mode,
        }
    }

    pub fn restore_audio_preferences(
        &mut self,
        preferences: &AudioPreferences,
    ) -> Result<(), &'static str> {
        preferences.validate()?;
        let devices = &mut self.audio_device_state;
        devices.follow_system_default = preferences.follow_system_default;
        devices.current_output_device_name = if preferences.follow_system_default {
            None
        } else {
            preferences.output_device.clone()
        };
        if let Some(index) = devices.output_devices.iter().position(|device| {
            if preferences.follow_system_default {
                device.is_default
            } else {
                Some(device.name.as_str()) == preferences.output_device.as_deref()
            }
        }) {
            devices.selected_output_device_index = index;
        }
        devices.hal_config.sample_rate = preferences.sample_rate_hz;
        devices.hal_config.channel_count = preferences.channel_count;
        devices.hal_config.buffer_frames = preferences.buffer_frames;
        #[cfg(all(target_os = "macos", feature = "hal"))]
        {
            devices.playback_source = if preferences.systemwide_input {
                crate::app::types::PlaybackSource::HalDevice
            } else {
                crate::app::types::PlaybackSource::File
            };
        }
        self.playback.replay_gain_enabled = preferences.replay_gain_enabled;
        self.playback.replay_gain_mode = preferences.replay_gain_mode;
        Ok(())
    }
}
