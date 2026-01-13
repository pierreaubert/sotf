use crate::driver::AppDriver;
use sotf_audio_player_gpui::app::types::{
    RecordingSignalType, SpeakerConfiguration, ChannelRecordingState
};

pub struct RecordingPage<'a, 'b> {
    driver: &'a mut AppDriver<'b>,
}

impl<'a, 'b> RecordingPage<'a, 'b> {
    pub fn new(driver: &'a mut AppDriver<'b>) -> Self {
        Self { driver }
    }
    
    pub fn run_until_parked(&mut self) {
        self.driver.run_until_parked();
    }

    pub fn is_all_channels_recorded(&mut self) -> bool {
         self.driver.read_app(|app| {
             app.measurement_state.recording_state.all_channels_recorded()
        })
    }

    pub fn verify_recording_results(&mut self) {
        self.driver.read_app(|app| {
             for recording in &app.measurement_state.recording_state.channel_recordings {
                 assert_eq!(recording.state, ChannelRecordingState::Done, "Channel {} failed", recording.channel_name);
                 assert!(recording.result.is_some(), "Channel {} has no result", recording.channel_name);
                 
                 let res = recording.result.as_ref().unwrap();
                 assert!(res.wav_path.is_some(), "WAV path missing");
                 assert!(res.csv_path.is_some(), "CSV path missing");
                 
                 // Verify file existence
                 if let Some(path) = &res.wav_path {
                     assert!(std::path::Path::new(path).exists(), "WAV file not created: {}", path);
                 }
             }
        });
    }

    pub fn navigate_to_recording(&mut self) {
        self.driver.update_app(|app, _| {
            // Trigger the action to switch to recording view
            // Using the action directly is cleaner, but for now we can simulate state change
            // if we assume actions work.
            // Better to trigger the action:
            use sotf_audio_player_gpui::app::types::Screen;
            app.ui_state.current_screen = Screen::Recording;
        });
    }

    pub fn scan_audio_devices(&mut self) {
        self.driver.update_app(|app, _| {
            // Force a refresh of audio devices
             app.load_audio_devices();
        });
    }

    pub fn get_available_devices(&mut self) -> (Vec<String>, Vec<String>) {
        self.driver.read_app(|app| {
            let input_names = app.audio_device_state.input_devices.iter().map(|d| d.name.clone()).collect();
            let output_names = app.audio_device_state.output_devices.iter().map(|d| d.name.clone()).collect();
            (input_names, output_names)
        })
    }

    /// Tries to find a suitable loopback device name
    pub fn find_loopback_device(&mut self) -> Option<String> {
        let (inputs, outputs) = self.get_available_devices();
        
        let candidates = [
            "BlackHole", "Loopback", "VB-Cable", "Cable", "Null", "Dummy", "Stereo Mix"
        ];

        for candidate in candidates {
             // Find one that exists in BOTH lists
             let in_match = inputs.iter().find(|n| n.contains(candidate));
             let out_match = outputs.iter().find(|n| n.contains(candidate));
             
             if let (Some(in_name), Some(out_name)) = (in_match, out_match) {
                 // They must be somewhat similar or the same device
                 return Some(in_name.clone());
             }
        }
        None
    }

    pub fn select_devices(&mut self, device_name: &str) {
        self.driver.update_app(move |app, _| {
            // Select for output
            if let Some(idx) = app.audio_device_state.output_devices.iter().position(|d| d.name == device_name) {
                 app.audio_device_state.selected_output_device_index = idx;
                 app.audio_device_state.current_output_device_name = Some(device_name.to_string());
                 
                 // Trigger config update logic (simulation)
                 // In real app, selecting device updates config. 
                 // We need to manually populate the config if the event handler isn't running in this atomic update
                 if let Some(device) = app.audio_device_state.output_devices.get(idx).cloned() {
                     app.measurement_state.recording_state.playback_config.device_name = device.name.clone();
                     app.measurement_state.recording_state.playback_config.device_id = device.device_id.unwrap_or(device.name);
                     // Set max channels
                     if let Some(config) = device.default_config { 
                        app.measurement_state.recording_state.playback_config.num_channels = config.channels as usize;
                        app.measurement_state.recording_state.playback_config.sample_rate = config.sample_rate;
                     }
                 }
            }

            // Select for input
            if let Some(idx) = app.audio_device_state.input_devices.iter().position(|d| d.name == device_name) {
                 app.audio_device_state.selected_input_device_index = idx;
                 app.audio_device_state.current_input_device_name = Some(device_name.to_string());

                 if let Some(device) = app.audio_device_state.input_devices.get(idx).cloned() {
                     app.measurement_state.recording_state.recording_config.device_name = device.name.clone();
                     app.measurement_state.recording_state.recording_config.device_id = device.device_id.unwrap_or(device.name);
                     if let Some(config) = device.default_config { 
                        app.measurement_state.recording_state.recording_config.num_channels = config.channels as usize;
                        app.measurement_state.recording_state.recording_config.sample_rate = config.sample_rate;
                     }
                 }
            }
        });
    }

    pub fn get_device_max_channels(&mut self) -> usize {
        self.driver.read_app(|app| {
             app.measurement_state.recording_state.playback_config.num_channels
        })
    }

    pub fn set_speaker_config(&mut self, config: SpeakerConfiguration) {
         self.driver.update_app(move |app, _| {
             app.measurement_state.recording_state.playback_config.speaker_configuration = config;
             // Update channel count
             app.measurement_state.recording_state.playback_config.num_channels = config.channel_count();
             // Update channel mappings based on config
             let names = config.default_channel_names();
             app.measurement_state.recording_state.playback_config.channel_mappings = names.iter().enumerate().map(|(i, name)| {
                 sotf_audio_player_gpui::app::types::ChannelMapping {
                     interface_channel: i + 1,
                     group_name: name.to_string(),
                 }
             }).collect();
             
             // Initialize recordings
             app.measurement_state.recording_state.init_channel_recordings();
         });
    }

    pub fn configure_signal(&mut self, signal_type: RecordingSignalType, duration: f32, level_db: f32) {
        self.driver.update_app(move |app, _| {
            app.measurement_state.recording_state.signal_type = signal_type;
            app.measurement_state.recording_state.signal_duration_secs = duration;
            app.measurement_state.recording_state.signal_level_db = level_db;
            
            // If sweep, set reasonable defaults
            if signal_type == RecordingSignalType::Sweep {
                app.measurement_state.recording_state.sweep_start_freq = 100.0;
                app.measurement_state.recording_state.sweep_end_freq = 1000.0; // Short sweep
            }
        });
    }

    pub fn start_recording_all(&mut self) {
         self.driver.view.update(self.driver.cx, |view, _window, cx| {
             view.start_recording_all_channels(cx);
         });
    }

    // === Parameter Check Helpers ===

    pub fn set_playback_device(&mut self, name: &str) {
        self.driver.update_app(move |app, _| {
            // Update config directly to simulate selection
            app.measurement_state.recording_state.playback_config.device_name = name.to_string();
            app.measurement_state.recording_state.playback_config.device_id = name.to_string();
        });
    }

    pub fn get_playback_device(&mut self) -> String {
        self.driver.read_app(|app| {
            app.measurement_state.recording_state.playback_config.device_name.clone()
        })
    }

    pub fn set_recording_device(&mut self, name: &str) {
         self.driver.update_app(move |app, _| {
            app.measurement_state.recording_state.recording_config.device_name = name.to_string();
             app.measurement_state.recording_state.recording_config.device_id = name.to_string();
        });
    }

    pub fn get_recording_device(&mut self) -> String {
        self.driver.read_app(|app| {
            app.measurement_state.recording_state.recording_config.device_name.clone()
        })
    }

    pub fn set_sample_rate(&mut self, rate: u32) {
        self.driver.update_app(move |app, _| {
            app.measurement_state.recording_state.playback_config.sample_rate = rate;
            app.measurement_state.recording_state.recording_config.sample_rate = rate;
        });
    }

    pub fn get_sample_rate(&mut self) -> u32 {
        self.driver.read_app(|app| {
            app.measurement_state.recording_state.playback_config.sample_rate
        })
    }

    pub fn get_speaker_config(&mut self) -> SpeakerConfiguration {
        self.driver.read_app(|app| {
            app.measurement_state.recording_state.playback_config.speaker_configuration
        })
    }

    pub fn get_channel_count(&mut self) -> usize {
        self.driver.read_app(|app| {
            app.measurement_state.recording_state.playback_config.num_channels
        })
    }

    pub fn set_signal_type(&mut self, signal_type: RecordingSignalType) {
        self.driver.update_app(move |app, _| {
            app.measurement_state.recording_state.signal_type = signal_type;
        });
    }

    pub fn get_signal_type(&mut self) -> RecordingSignalType {
        self.driver.read_app(|app| {
            app.measurement_state.recording_state.signal_type
        })
    }

    pub fn set_signal_duration(&mut self, duration: f32) {
        self.driver.update_app(move |app, _| {
            app.measurement_state.recording_state.signal_duration_secs = duration;
        });
    }

    pub fn get_signal_duration(&mut self) -> f32 {
        self.driver.read_app(|app| {
            app.measurement_state.recording_state.signal_duration_secs
        })
    }
    
    pub fn set_signal_level(&mut self, level_db: f32) {
        self.driver.update_app(move |app, _| {
            app.measurement_state.recording_state.signal_level_db = level_db;
        });
    }
    
    pub fn get_signal_level(&mut self) -> f32 {
        self.driver.read_app(|app| {
            app.measurement_state.recording_state.signal_level_db
        })
    }

    pub fn set_sweep_range(&mut self, start: f32, end: f32) {
        self.driver.update_app(move |app, _| {
            app.measurement_state.recording_state.sweep_start_freq = start;
            app.measurement_state.recording_state.sweep_end_freq = end;
        });
    }

    pub fn get_sweep_range(&mut self) -> (f32, f32) {
        self.driver.read_app(|app| {
            (
                app.measurement_state.recording_state.sweep_start_freq,
                app.measurement_state.recording_state.sweep_end_freq
            )
        })
    }
    
    pub fn set_mic_calibration(&mut self, path: &str) {
        self.driver.update_app(move |app, _| {
            app.measurement_state.recording_state.mic_calibration_path = Some(path.to_string());
        });
    }
    
    pub fn get_mic_calibration(&mut self) -> Option<String> {
        self.driver.read_app(|app| {
            app.measurement_state.recording_state.mic_calibration_path.clone()
        })
    }
    
    pub fn set_recording_directory(&mut self, path: &str) {
         self.driver.update_app(move |app, _| {
            app.measurement_state.recording_state.recording_base_directory = Some(path.to_string());
        });
    }
    
    pub fn get_recording_directory(&mut self) -> Option<String> {
        self.driver.read_app(|app| {
            app.measurement_state.recording_state.recording_base_directory.clone()
        })
    }
}
