//! Committed audio preferences and transient edits, independent of the application shell.

use crate::ReplayGainMode;

pub const INPUT_SAMPLE_RATES_HZ: &[u32] = &[44100, 48000, 88200, 96000, 176400, 192000];
pub const INPUT_CHANNEL_COUNTS: &[u32] = &[2, 4, 6, 8];
pub const INPUT_BUFFER_FRAMES: &[u32] = &[128, 256, 512, 1024, 2048, 4096];

/// Committed audio preferences. Draft values must never be serialized here.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AudioPreferences {
    pub output_device: Option<String>,
    /// Explicit OS-default route; false preserves legacy smart-device selection.
    pub follow_system_default: bool,
    pub systemwide_input: bool,
    pub sample_rate_hz: u32,
    pub channel_count: u32,
    pub buffer_frames: u32,
    pub replay_gain_enabled: bool,
    pub replay_gain_mode: ReplayGainMode,
}

impl Default for AudioPreferences {
    fn default() -> Self {
        Self {
            output_device: None,
            follow_system_default: false,
            systemwide_input: false,
            sample_rate_hz: 48000,
            channel_count: 2,
            buffer_frames: 1024,
            replay_gain_enabled: true,
            replay_gain_mode: ReplayGainMode::Track,
        }
    }
}

impl AudioPreferences {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !INPUT_SAMPLE_RATES_HZ.contains(&self.sample_rate_hz) {
            return Err("Unsupported systemwide sample rate");
        }
        if !INPUT_CHANNEL_COUNTS.contains(&self.channel_count) {
            return Err("Unsupported systemwide channel count");
        }
        if !INPUT_BUFFER_FRAMES.contains(&self.buffer_frames) {
            return Err("Unsupported systemwide buffer size");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AudioOutputDraft {
    pub device_name: Option<String>,
    /// An explicit default-route edit, distinct from no edit.
    pub system_default: bool,
    pub replay_gain_enabled: Option<bool>,
    pub replay_gain_mode: Option<ReplayGainMode>,
    pub systemwide_input: Option<bool>,
    pub sample_rate_hz: Option<u32>,
    pub channel_count: Option<u32>,
    pub buffer_frames: Option<u32>,
    pub error: Option<String>,
}

impl AudioOutputDraft {
    /// Preserve the user's selected values when an in-flight Apply changes the
    /// committed baseline. An absent edit still means the old baseline value.
    pub fn rebase_after_apply(&mut self, before: &AudioPreferences, after: &AudioPreferences) {
        self.set_systemwide_input(
            self.systemwide_input.unwrap_or(before.systemwide_input),
            after.systemwide_input,
        );
        self.set_sample_rate_hz(
            self.sample_rate_hz.unwrap_or(before.sample_rate_hz),
            after.sample_rate_hz,
        );
        self.set_channel_count(
            self.channel_count.unwrap_or(before.channel_count),
            after.channel_count,
        );
        self.set_buffer_frames(
            self.buffer_frames.unwrap_or(before.buffer_frames),
            after.buffer_frames,
        );
        self.set_replay_gain_enabled(
            self.replay_gain_enabled
                .unwrap_or(before.replay_gain_enabled),
            after.replay_gain_enabled,
        );
        self.set_replay_gain_mode(
            self.replay_gain_mode.unwrap_or(before.replay_gain_mode),
            after.replay_gain_mode,
        );
        let selected = self
            .selected_name(before.output_device.as_deref())
            .map(str::to_owned);
        if self.system_default || (self.device_name.is_none() && before.follow_system_default) {
            self.select_system_default(after.follow_system_default);
        } else if let Some(name) = selected {
            self.select(name, after.output_device.as_deref());
        }
    }

    /// Inactive systemwide format edits must not interrupt file playback.
    pub fn requires_restart(&self, active_systemwide: bool) -> bool {
        self.system_default
            || self.device_name.is_some()
            || self.replay_gain_enabled.is_some()
            || self.replay_gain_mode.is_some()
            || self.systemwide_input.is_some()
            || (self.systemwide_input.unwrap_or(active_systemwide)
                && (self.sample_rate_hz.is_some()
                    || self.channel_count.is_some()
                    || self.buffer_frames.is_some()))
    }

    pub fn is_dirty(&self) -> bool {
        self.system_default
            || self.device_name.is_some()
            || self.replay_gain_enabled.is_some()
            || self.replay_gain_mode.is_some()
            || self.systemwide_input.is_some()
            || self.sample_rate_hz.is_some()
            || self.channel_count.is_some()
            || self.buffer_frames.is_some()
    }

    pub fn set_systemwide_input(&mut self, value: bool, active: bool) {
        self.systemwide_input = (value != active).then_some(value);
        self.error = None;
    }

    pub fn set_sample_rate_hz(&mut self, value: u32, active: u32) {
        self.sample_rate_hz = (value != active).then_some(value);
        self.error = None;
    }

    pub fn set_channel_count(&mut self, value: u32, active: u32) {
        self.channel_count = (value != active).then_some(value);
        self.error = None;
    }

    pub fn set_buffer_frames(&mut self, value: u32, active: u32) {
        self.buffer_frames = (value != active).then_some(value);
        self.error = None;
    }

    pub fn set_replay_gain_enabled(&mut self, enabled: bool, active: bool) {
        self.replay_gain_enabled = (enabled != active).then_some(enabled);
        self.error = None;
    }

    pub fn set_replay_gain_mode(&mut self, mode: ReplayGainMode, active: ReplayGainMode) {
        self.replay_gain_mode = (mode != active).then_some(mode);
        self.error = None;
    }

    pub fn select(&mut self, name: String, active: Option<&str>) {
        self.system_default = false;
        self.device_name = (Some(name.as_str()) != active).then_some(name);
        self.error = None;
    }

    pub fn select_system_default(&mut self, active: bool) {
        self.device_name = None;
        self.system_default = !active;
        self.error = None;
    }

    pub fn discard(&mut self) {
        *self = Self::default();
    }

    pub fn selected_name<'a>(&'a self, active: Option<&'a str>) -> Option<&'a str> {
        if self.system_default {
            None
        } else {
            self.device_name.as_deref().or(active)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_default_is_an_explicit_reversible_route_edit() {
        let mut draft = AudioOutputDraft::default();
        draft.select_system_default(false);
        assert!(draft.is_dirty());
        assert!(draft.requires_restart(false));
        assert_eq!(draft.selected_name(Some("DAC")), None);
        draft.select("DAC".into(), Some("DAC"));
        assert!(!draft.is_dirty());
        draft.select_system_default(true);
        assert!(!draft.is_dirty());
        draft.select("DAC".into(), None);
        assert_eq!(draft.selected_name(None), Some("DAC"));
        draft.discard();
        assert!(!draft.is_dirty());
    }

    #[test]
    fn pending_apply_rebase_preserves_default_and_named_route_edits() {
        let default_route = AudioPreferences {
            follow_system_default: true,
            ..Default::default()
        };
        let named = AudioPreferences {
            output_device: Some("DAC".into()),
            ..Default::default()
        };
        let mut draft = AudioOutputDraft::default();
        draft.rebase_after_apply(&default_route, &named);
        assert!(draft.system_default);
        assert_eq!(draft.selected_name(Some("DAC")), None);
        draft.rebase_after_apply(&named, &default_route);
        assert!(!draft.is_dirty());
        draft.select("Headphones".into(), Some("DAC"));
        draft.rebase_after_apply(&named, &default_route);
        assert_eq!(draft.selected_name(None), Some("Headphones"));
    }

    #[test]
    fn system_default_preference_is_backward_compatible_and_round_trips() {
        let old: AudioPreferences = serde_json::from_str(r#"{"output_device":null}"#).unwrap();
        assert!(!old.follow_system_default);
        let selected = AudioPreferences {
            follow_system_default: true,
            ..old
        };
        let restored: AudioPreferences =
            serde_json::from_str(&serde_json::to_string(&selected).unwrap()).unwrap();
        assert_eq!(restored, selected);
    }

    #[test]
    fn rebase_preserves_controls_reset_while_apply_was_pending() {
        let before = AudioPreferences {
            output_device: Some("Speakers".into()),
            ..Default::default()
        };
        let after = AudioPreferences {
            output_device: Some("DAC".into()),
            follow_system_default: false,
            systemwide_input: true,
            sample_rate_hz: 96000,
            channel_count: 6,
            buffer_frames: 512,
            replay_gain_enabled: false,
            replay_gain_mode: ReplayGainMode::Album,
        };
        // Resetting controls to the old baseline removes their draft fields.
        let mut draft = AudioOutputDraft::default();
        draft.rebase_after_apply(&before, &after);
        assert_eq!(draft.device_name.as_deref(), Some("Speakers"));
        assert_eq!(draft.systemwide_input, Some(false));
        assert_eq!(draft.sample_rate_hz, Some(48000));
        assert_eq!(draft.channel_count, Some(2));
        assert_eq!(draft.buffer_frames, Some(1024));
        assert_eq!(draft.replay_gain_enabled, Some(true));
        assert_eq!(draft.replay_gain_mode, Some(ReplayGainMode::Track));
    }

    #[test]
    fn rebase_removes_applied_values_and_preserves_newer_edits() {
        let before = AudioPreferences::default();
        let after = AudioPreferences {
            sample_rate_hz: 96000,
            channel_count: 6,
            ..before.clone()
        };
        let mut draft = AudioOutputDraft::default();
        draft.set_sample_rate_hz(192000, before.sample_rate_hz);
        draft.set_channel_count(6, before.channel_count);
        draft.rebase_after_apply(&before, &after);
        assert_eq!(draft.sample_rate_hz, Some(192000));
        assert_eq!(draft.channel_count, None);
        assert!(draft.is_dirty());
    }

    #[test]
    fn audio_edits_are_independent_and_reversible() {
        let mut draft = AudioOutputDraft::default();
        draft.select("DAC".into(), Some("Speakers"));
        draft.set_replay_gain_enabled(false, true);
        draft.set_replay_gain_mode(ReplayGainMode::Album, ReplayGainMode::Track);
        assert!(draft.is_dirty());
        draft.select("Speakers".into(), Some("Speakers"));
        assert!(draft.is_dirty());
        assert_eq!(draft.replay_gain_enabled, Some(false));
        draft.set_replay_gain_enabled(true, true);
        assert!(draft.is_dirty());
        draft.set_replay_gain_mode(ReplayGainMode::Track, ReplayGainMode::Track);
        assert!(!draft.is_dirty());
    }

    #[test]
    fn discard_clears_every_audio_edit_and_error() {
        let mut draft = AudioOutputDraft::default();
        draft.select("DAC".into(), None);
        draft.set_replay_gain_enabled(false, true);
        draft.set_replay_gain_mode(ReplayGainMode::Album, ReplayGainMode::Track);
        draft.set_systemwide_input(true, false);
        draft.set_sample_rate_hz(96000, 48000);
        draft.set_channel_count(6, 2);
        draft.set_buffer_frames(512, 1024);
        draft.error = Some("Output disappeared".into());
        draft.discard();
        assert!(!draft.is_dirty());
        assert!(draft.error.is_none());
        assert_eq!(draft.selected_name(Some("Speakers")), Some("Speakers"));
    }

    #[test]
    fn input_format_edits_survive_switching_back_to_file_source() {
        let mut draft = AudioOutputDraft::default();
        draft.set_systemwide_input(true, false);
        draft.set_sample_rate_hz(96000, 48000);
        draft.set_channel_count(6, 2);
        draft.set_buffer_frames(512, 1024);
        draft.set_systemwide_input(false, false);
        assert!(draft.is_dirty());
        assert_eq!(draft.systemwide_input, None);
        assert_eq!(draft.sample_rate_hz, Some(96000));
        assert_eq!(draft.channel_count, Some(6));
        assert_eq!(draft.buffer_frames, Some(512));
        assert!(!draft.requires_restart(false));
        assert!(draft.requires_restart(true));
        draft.set_sample_rate_hz(48000, 48000);
        draft.set_channel_count(2, 2);
        draft.set_buffer_frames(1024, 1024);
        assert!(!draft.is_dirty());
    }
}
