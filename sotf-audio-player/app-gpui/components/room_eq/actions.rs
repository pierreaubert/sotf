use crate::ui::PlayerView;
use gpui::prelude::*;

impl PlayerView {

    pub(crate) fn load_room_eq_from_recording(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .room_eq_state
                .load_from_recording(&state.app.recording_state);
            state.app.room_eq_state.init_speaker_configs();
            let channel_count = state.app.room_eq_state.channel_measurements.len();
            state.app.room_eq_state.status_message = format!(
                "Successfully loaded {} channel(s) from recording session",
                channel_count
            );
            state.app.room_eq_state.error_message = None;
        });
    }

    pub(crate) fn load_room_eq_from_file(&mut self, cx: &mut Context<Self>) {
        use crate::app::types::{
            RoomEqDataSource, RoomEqMeasurementsFile, RoomEqSpeakerConfig, SpeakerConfigType,
        };

        let state_entity = self.state.clone();

        cx.spawn(async move |_, cx| {
            // Open file dialog
            let file = rfd::AsyncFileDialog::new()
                .add_filter("JSON", &["json"])
                .set_title("Load Room EQ Measurements")
                .pick_file()
                .await;

            if let Some(file) = file {
                let file_path = file.path().to_path_buf();
                log::info!("Loading measurements from {:?}", file_path);

                // Read file content
                match std::fs::read_to_string(&file_path) {
                    Ok(json) => {
                        log::debug!("File read successfully, size: {} bytes", json.len());

                        // Deserialize measurements file
                        match serde_json::from_str::<RoomEqMeasurementsFile>(&json) {
                            Ok(measurements_file) => {
                                log::info!(
                                    "Successfully parsed {} channel measurements from {:?}",
                                    measurements_file.channels.len(),
                                    file_path
                                );

                                // Validate that we have at least one channel
                                if measurements_file.channels.is_empty() {
                                    log::error!("No channels found in measurements file");
                                    let _ = state_entity.update(cx, |state, _| {
                                        state.app.room_eq_state.error_message =
                                            Some("No channels found in the measurement file".to_string());
                                    });
                                    return;
                                }

                                // Validate each channel has data
                                for (idx, channel) in measurements_file.channels.iter().enumerate() {
                                    if channel.measurement.frequencies.is_empty() {
                                        log::error!("Channel {} '{}' has no frequency data", idx, channel.channel_name);
                                        let _ = state_entity.update(cx, |state, _| {
                                            state.app.room_eq_state.error_message =
                                                Some(format!("Channel '{}' has no frequency data", channel.channel_name));
                                        });
                                        return;
                                    }
                                    log::debug!(
                                        "Channel {}: {} freq points, is_group: {}",
                                        channel.channel_name,
                                        channel.measurement.frequencies.len(),
                                        channel.is_group
                                    );
                                }

                                // Create speaker configs from loaded measurements
                                let speaker_configs: Vec<RoomEqSpeakerConfig> = measurements_file
                                    .channels
                                    .iter()
                                    .map(|m| {
                                        let config_type = if m.is_group {
                                            SpeakerConfigType::MultiDriver
                                        } else {
                                            SpeakerConfigType::Single
                                        };
                                        RoomEqSpeakerConfig {
                                            channel_name: m.channel_name.clone(),
                                            config_type,
                                            driver_names: m
                                                .group_drivers
                                                .iter()
                                                .enumerate()
                                                .map(|(i, _)| format!("Driver {}", i + 1))
                                                .collect(),
                                            ..Default::default()
                                        }
                                    })
                                    .collect();

                                let channel_count = measurements_file.channels.len();
                                let _ = state_entity.update(cx, |state, _| {
                                    state.app.room_eq_state.channel_measurements =
                                        measurements_file.channels;
                                    state.app.room_eq_state.speaker_configs = speaker_configs;
                                    state.app.room_eq_state.data_source =
                                        RoomEqDataSource::FromFile(file_path.clone());
                                    state.app.room_eq_state.status_message = format!(
                                        "Successfully loaded {} channel(s) from {}",
                                        channel_count,
                                        file_path.display()
                                    );
                                    state.app.room_eq_state.error_message = None;
                                });
                            }
                            Err(e) => {
                                log::error!("JSON parse error: {}", e);
                                // Try to provide more helpful error messages
                                let error_msg = if json.contains("\"channel\"") && !json.contains("\"version\"") {
                                    format!(
                                        "File format error: Missing 'version' field. This may be an old format file. Error: {}",
                                        e
                                    )
                                } else if !json.contains("\"channels\"") {
                                    format!(
                                        "File format error: Missing 'channels' field. This doesn't appear to be a valid measurement file. Error: {}",
                                        e
                                    )
                                } else {
                                    format!("Failed to parse JSON: {}", e)
                                };

                                let _ = state_entity.update(cx, |state, _| {
                                    state.app.room_eq_state.error_message = Some(error_msg);
                                });
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("File read error: {}", e);
                        let _ = state_entity.update(cx, |state, _| {
                            state.app.room_eq_state.error_message =
                                Some(format!("Failed to read file: {}", e));
                        });
                    }
                }
            }
        })
        .detach();
    }

}
