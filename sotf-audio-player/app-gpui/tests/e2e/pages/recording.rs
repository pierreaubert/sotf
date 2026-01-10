use crate::driver::AppDriver;
use gpui::*;
use sotf_audio_player_gpui::app::types::{
    RecordingSignalType, SpeakerConfiguration,
};
use sotf_audio_player_gpui::app::Screen;

pub struct RecordingPage<'a, 'b> {
    driver: &'a mut AppDriver<'b>,
}

impl<'a, 'b> RecordingPage<'a, 'b> {
    pub fn new(driver: &'a mut AppDriver<'b>) -> Self {
        Self { driver }
    }

    pub fn navigate_to(&mut self) {
        self.driver.navigate_to(Screen::Recording);
    }

    // === Setters ===

    pub fn set_playback_device(&mut self, name: &str) {
        let name = name.to_string();
        self.driver.update_app(|app, _| {
            // Simulate device selection logic (simplified for test)
            app.recording_state.playback_config.device_name = name;
        });
    }

    pub fn set_recording_device(&mut self, name: &str) {
        let name = name.to_string();
        self.driver.update_app(|app, _| {
            app.recording_state.recording_config.device_name = name;
        });
    }

    pub fn set_sample_rate(&mut self, rate: u32) {
        self.driver.update_app(|app, _| {
            app.recording_state.playback_config.sample_rate = rate;
            app.recording_state.recording_config.sample_rate = rate;
        });
    }

    pub fn set_speaker_config(&mut self, config: SpeakerConfiguration) {
        self.driver.update_app(|app, _| {
            app.recording_state.playback_config.speaker_configuration = config;
            app.recording_state.playback_config.num_channels = config.channel_count();
            // Re-init channel recordings when speaker config changes
            app.recording_state.init_channel_recordings();
        });
    }

    pub fn set_signal_type(&mut self, signal_type: RecordingSignalType) {
        self.driver.update_app(|app, _| {
            app.recording_state.signal_type = signal_type;
        });
    }

    pub fn set_signal_duration(&mut self, duration_secs: f32) {
        self.driver.update_app(|app, _| {
            app.recording_state.signal_duration_secs = duration_secs;
        });
    }

    pub fn set_signal_level(&mut self, level_db: f32) {
        self.driver.update_app(|app, _| {
            app.recording_state.signal_level_db = level_db;
        });
    }

    pub fn set_sweep_range(&mut self, start_freq: f32, end_freq: f32) {
        self.driver.update_app(|app, _| {
            app.recording_state.sweep_start_freq = start_freq;
            app.recording_state.sweep_end_freq = end_freq;
        });
    }

    // === Getters / Verifiers ===

    pub fn get_playback_device(&mut self) -> String {
        self.driver
            .read_app(|app| app.recording_state.playback_config.device_name.clone())
    }

    pub fn get_recording_device(&mut self) -> String {
        self.driver
            .read_app(|app| app.recording_state.recording_config.device_name.clone())
    }

    pub fn get_sample_rate(&mut self) -> u32 {
        self.driver
            .read_app(|app| app.recording_state.playback_config.sample_rate)
    }

    pub fn get_speaker_config(&mut self) -> SpeakerConfiguration {
        self.driver
            .read_app(|app| app.recording_state.playback_config.speaker_configuration)
    }

    pub fn get_signal_type(&mut self) -> RecordingSignalType {
        self.driver.read_app(|app| app.recording_state.signal_type)
    }

    pub fn get_signal_duration(&mut self) -> f32 {
        self.driver
            .read_app(|app| app.recording_state.signal_duration_secs)
    }

    pub fn get_channel_count(&mut self) -> usize {
        self.driver
            .read_app(|app| app.recording_state.playback_config.num_channels)
    }
    
    pub fn get_sweep_range(&mut self) -> (f32, f32) {
        self.driver.read_app(|app| {
            (
                app.recording_state.sweep_start_freq,
                app.recording_state.sweep_end_freq,
            )
        })
    }
}
