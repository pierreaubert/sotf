use crate::ui::PlayerView;
use gpui::prelude::*;

impl PlayerView {
    pub(crate) fn load_room_eq_from_recording(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            // Check if there are valid recordings first
            let has_valid_recordings = state
                .app
                .measurement_state
                .recording_state
                .channel_recordings
                .iter()
                .any(|r| r.state == crate::app::types::ChannelRecordingState::Done);

            if !has_valid_recordings {
                state.app.measurement_state.room_eq_state.error_message = Some(
                    "No completed recordings found. Please record measurements first.".to_string(),
                );
                state
                    .app
                    .measurement_state
                    .room_eq_state
                    .status_message
                    .clear();
                return;
            }

            state
                .app
                .measurement_state
                .room_eq_state
                .load_from_recording(&state.app.measurement_state.recording_state);
            state
                .app
                .measurement_state
                .room_eq_state
                .init_speaker_configs();

            let channel_count = state
                .app
                .measurement_state
                .room_eq_state
                .channel_measurements
                .len();

            if channel_count == 0 {
                state.app.measurement_state.room_eq_state.error_message =
                    Some("Failed to load measurements: no valid channel data found.".to_string());
                state
                    .app
                    .measurement_state
                    .room_eq_state
                    .status_message
                    .clear();
            } else {
                state.app.measurement_state.room_eq_state.status_message = format!(
                    "Successfully loaded {} channel(s) from recording session",
                    channel_count
                );
                state.app.measurement_state.room_eq_state.error_message = None;
            }
        });
    }

    pub(crate) fn load_room_eq_from_file(&mut self, cx: &mut Context<Self>) {
        use crate::app::types::{
            ChannelMeasurement, RecordingResult, RoomEqDataSource, RoomEqMeasurementsFile,
            RoomEqSpeakerConfig, SpeakerConfigType,
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
                let file_dir = file_path.parent().map(|p| p.to_path_buf());
                log::info!("Loading measurements from {:?}", file_path);

                // Get file size for migration detection
                let file_size = std::fs::metadata(&file_path)
                    .map(|m| m.len())
                    .unwrap_or(0);

                // Read file content
                match std::fs::read_to_string(&file_path) {
                    Ok(json) => {
                        log::debug!("File read successfully, size: {} bytes", json.len());

                        // First try to parse as new RoomConfig format
                        if let Ok(room_config) = serde_json::from_str::<autoeq::RoomConfig>(&json) {
                            log::info!(
                                "Successfully parsed {} speakers from {:?} (RoomConfig format)",
                                room_config.speakers.len(),
                                file_path
                            );

                            // Convert RoomConfig speakers to ChannelMeasurement
                            let channel_measurements: Vec<ChannelMeasurement> = room_config
                                .speakers
                                .into_iter()
                                .enumerate()
                                .filter_map(|(idx, (channel_name, speaker_config))| {
                                    // Extract inline measurement from speaker config
                                    let inline = match speaker_config {
                                        autoeq::SpeakerConfig::Single(source) => match source {
                                            autoeq::MeasurementSource::Single(ref_) => {
                                                ref_.inline_data().cloned()
                                            }
                                            autoeq::MeasurementSource::Multiple(refs) => {
                                                refs.first().and_then(|r| r.inline_data()).cloned()
                                            }
                                            autoeq::MeasurementSource::InMemory(_) => None,
                                        },
                                        _ => None, // Groups not yet supported
                                    };

                                    inline.map(|inline_data| {
                                        // Check if inline data is empty - if so, load from CSV
                                        let (frequencies, magnitude_db, phase_deg) = if inline_data.frequencies.is_empty() {
                                            // Try to load from CSV file using autoeq's reader
                                            if let Some(csv_path) = &inline_data.csv_path {
                                                let csv_full_path = file_dir
                                                    .as_ref()
                                                    .map(|d| d.join(csv_path))
                                                    .unwrap_or_else(|| std::path::PathBuf::from(csv_path));

                                                if let Ok(curve) = autoeq::read::read_curve_from_csv(&csv_full_path) {
                                                    log::info!(
                                                        "Loaded {} frequency points from CSV for channel '{}'",
                                                        curve.freq.len(),
                                                        channel_name
                                                    );
                                                    (
                                                        curve.freq.iter().map(|&f| f as f32).collect(),
                                                        curve.spl.iter().map(|&s| s as f32).collect(),
                                                        curve.phase.map(|p| p.iter().map(|&v| v as f32).collect()).unwrap_or_default(),
                                                    )
                                                } else {
                                                    log::warn!("Failed to load CSV for channel '{}': {:?}", channel_name, csv_full_path);
                                                    (Vec::new(), Vec::new(), Vec::new())
                                                }
                                            } else {
                                                log::warn!("No CSV path and empty inline data for channel '{}'", channel_name);
                                                (Vec::new(), Vec::new(), Vec::new())
                                            }
                                        } else {
                                            // Use inline data
                                            (
                                                inline_data.frequencies.iter().map(|&f| f as f32).collect(),
                                                inline_data.magnitude_db.iter().map(|&m| m as f32).collect(),
                                                inline_data.phase_deg.unwrap_or_default().iter().map(|&p| p as f32).collect(),
                                            )
                                        };

                                        // Try to load extended metrics from CSV file
                                        let extended_metrics = crate::components::migration::load_extended_metrics(
                                            inline_data.csv_path.as_deref(),
                                            file_dir.as_deref(),
                                        );

                                        let (thd_percent, rt60_ms, clarity_c50_db, clarity_c80_db, excess_group_delay_ms) =
                                            if let Some(metrics) = extended_metrics {
                                                (
                                                    metrics.thd_percent,
                                                    metrics.rt60_ms,
                                                    metrics.clarity_c50_db,
                                                    metrics.clarity_c80_db,
                                                    metrics.excess_group_delay_ms,
                                                )
                                            } else {
                                                (None, None, None, None, None)
                                            };

                                        ChannelMeasurement {
                                            channel_name: channel_name.clone(),
                                            measurement: RecordingResult {
                                                channel: idx,
                                                wav_path: inline_data.wav_path,
                                                csv_path: inline_data.csv_path,
                                                frequencies,
                                                magnitude_db,
                                                phase_deg,
                                                impulse_response: None,
                                                impulse_time_ms: None,
                                                excess_group_delay_ms,
                                                thd_percent,
                                                harmonic_distortion_db: None,
                                                rt60_ms,
                                                clarity_c50_db,
                                                clarity_c80_db,
                                                spectrogram_db: None,
                                            },
                                            is_group: false,
                                            group_drivers: Vec::new(),
                                        }
                                    })
                                })
                                .collect();

                            if channel_measurements.is_empty() {
                                log::error!("No valid inline measurements found in RoomConfig");
                                let _ = state_entity.update(cx, |state, _| {
                                    state.app.measurement_state.room_eq_state.error_message =
                                        Some("No valid inline measurements found in file".to_string());
                                });
                                return;
                            }

                            // Create speaker configs
                            let speaker_configs: Vec<RoomEqSpeakerConfig> = channel_measurements
                                .iter()
                                .map(|m| RoomEqSpeakerConfig {
                                    channel_name: m.channel_name.clone(),
                                    config_type: SpeakerConfigType::Single,
                                    driver_names: Vec::new(),
                                    ..Default::default()
                                })
                                .collect();

                            let channel_count = channel_measurements.len();
                            let _ = state_entity.update(cx, |state, _| {
                                state.app.measurement_state.room_eq_state.channel_measurements =
                                    channel_measurements;
                                state.app.measurement_state.room_eq_state.speaker_configs =
                                    speaker_configs;
                                state.app.measurement_state.room_eq_state.data_source =
                                    RoomEqDataSource::FromFile(file_path.clone());
                                state.app.measurement_state.room_eq_state.status_message = format!(
                                    "Successfully loaded {} channel(s) from {} (RoomConfig format)",
                                    channel_count,
                                    file_path.display()
                                );
                                state.app.measurement_state.room_eq_state.error_message = None;
                            });
                            return;
                        }

                        // Fall back to legacy RoomEqMeasurementsFile format
                        match serde_json::from_str::<RoomEqMeasurementsFile>(&json) {
                            Ok(measurements_file) => {
                                log::info!(
                                    "Successfully parsed {} channel measurements from {:?} (legacy format)",
                                    measurements_file.channels.len(),
                                    file_path
                                );

                                // Check if this legacy file needs migration (large file with inline data)
                                let needs_migration = Self::check_legacy_needs_migration(&json, file_size);

                                if needs_migration {
                                    // Show migration modal instead of loading directly
                                    log::info!(
                                        "Detected legacy format ({:.2} MB), showing migration modal",
                                        file_size as f64 / 1_000_000.0
                                    );

                                    let channel_count = measurements_file.channels.len();

                                    let _ = state_entity.update(cx, |state, _| {
                                        // Use the recording state's migration modal
                                        let rec_state = &mut state.app.measurement_state.recording_state;
                                        rec_state.migration_modal_open = true;
                                        rec_state.migration_file_path =
                                            Some(file_path.to_string_lossy().to_string());
                                        rec_state.migration_file_dir =
                                            file_dir.map(|d| d.to_string_lossy().to_string());
                                        rec_state.migration_file_size = Some(file_size);
                                        rec_state.migration_channel_count = channel_count;
                                        rec_state.migration_pending_json = Some(json);
                                    });
                                    return;
                                }

                                // Validate that we have at least one channel
                                if measurements_file.channels.is_empty() {
                                    log::error!("No channels found in measurements file");
                                    let _ = state_entity.update(cx, |state, _| {
                                        state.app.measurement_state.room_eq_state.error_message =
                                            Some("No channels found in the measurement file".to_string());
                                    });
                                    return;
                                }

                                // Validate each channel has data
                                for (idx, channel) in measurements_file.channels.iter().enumerate() {
                                    if channel.measurement.frequencies.is_empty() {
                                        log::error!(
                                            "Channel {} '{}' has no frequency data",
                                            idx,
                                            channel.channel_name
                                        );
                                        let _ = state_entity.update(cx, |state, _| {
                                            state.app.measurement_state.room_eq_state.error_message =
                                                Some(format!(
                                                    "Channel '{}' has no frequency data",
                                                    channel.channel_name
                                                ));
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
                                    state.app.measurement_state.room_eq_state.channel_measurements =
                                        measurements_file.channels;
                                    state.app.measurement_state.room_eq_state.speaker_configs =
                                        speaker_configs;
                                    state.app.measurement_state.room_eq_state.data_source =
                                        RoomEqDataSource::FromFile(file_path.clone());
                                    state.app.measurement_state.room_eq_state.status_message = format!(
                                        "Successfully loaded {} channel(s) from {}",
                                        channel_count,
                                        file_path.display()
                                    );
                                    state.app.measurement_state.room_eq_state.error_message = None;
                                });
                            }
                            Err(e) => {
                                log::error!("JSON parse error: {}", e);
                                // Try to provide more helpful error messages
                                let error_msg = if json.contains("\"speakers\"") {
                                    format!(
                                        "File appears to be in RoomConfig format but failed to parse: {}. \
                                        Make sure all speakers have inline measurement data.",
                                        e
                                    )
                                } else if json.contains("\"channel\"")
                                    && !json.contains("\"version\"")
                                {
                                    format!(
                                        "File format error: Missing 'version' field. This may be an old format file. Error: {}",
                                        e
                                    )
                                } else if !json.contains("\"channels\"")
                                    && !json.contains("\"speakers\"")
                                {
                                    format!(
                                        "File format error: Missing 'channels' or 'speakers' field. \
                                        This doesn't appear to be a valid measurement file. Error: {}",
                                        e
                                    )
                                } else {
                                    format!("Failed to parse JSON: {}", e)
                                };

                                let _ = state_entity.update(cx, |state, _| {
                                    state.app.measurement_state.room_eq_state.error_message =
                                        Some(error_msg);
                                });
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("File read error: {}", e);
                        let _ = state_entity.update(cx, |state, _| {
                            state.app.measurement_state.room_eq_state.error_message =
                                Some(format!("Failed to read file: {}", e));
                        });
                    }
                }
            }
        })
        .detach();
    }

    /// Check if a legacy JSON file needs migration (large file with inline data)
    /// Uses the shared migration module
    fn check_legacy_needs_migration(json: &str, file_size: u64) -> bool {
        crate::components::migration::check_needs_migration(json, file_size)
    }
}
