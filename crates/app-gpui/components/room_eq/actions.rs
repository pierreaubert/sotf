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
            state
                .app
                .measurement_state
                .room_eq_state
                .apply_smart_defaults();
            // Detect multi-mic recordings and set multi-position data
            let has_multi_mic = state
                .app
                .measurement_state
                .room_eq_state
                .channel_measurements
                .iter()
                .any(|m| !m.multi_mic_measurements.is_empty());

            if has_multi_mic {
                state
                    .app
                    .measurement_state
                    .room_eq_state
                    .has_multi_position_data = true;
                state
                    .app
                    .measurement_state
                    .room_eq_state
                    .multi_position_counts = state
                    .app
                    .measurement_state
                    .room_eq_state
                    .channel_measurements
                    .iter()
                    .map(|m| (m.channel_name.clone(), 1 + m.multi_mic_measurements.len()))
                    .collect();
                // Auto-enable multi-measurement optimization
                if !state
                    .app
                    .measurement_state
                    .room_eq_state
                    .optimizer_config
                    .multi_measurement
                    .enabled
                {
                    state
                        .app
                        .measurement_state
                        .room_eq_state
                        .optimizer_config
                        .multi_measurement
                        .enabled = true;
                    state
                        .app
                        .measurement_state
                        .room_eq_state
                        .optimizer_config
                        .multi_measurement
                        .strategy = "variance_penalized".to_string();
                }
            } else {
                state
                    .app
                    .measurement_state
                    .room_eq_state
                    .has_multi_position_data = false;
                state
                    .app
                    .measurement_state
                    .room_eq_state
                    .multi_position_counts = Vec::new();
            }

            let channel_count = state
                .app
                .measurement_state
                .room_eq_state
                .channel_measurements
                .len();

            let max_channels = state.app.max_room_eq_channels();
            if channel_count == 0 {
                state.app.measurement_state.room_eq_state.error_message =
                    Some("Failed to load measurements: no valid channel data found.".to_string());
                state
                    .app
                    .measurement_state
                    .room_eq_state
                    .status_message
                    .clear();
            } else if max_channels > 0 && channel_count > max_channels {
                // Truncate to max allowed channels at current release level
                state
                    .app
                    .measurement_state
                    .room_eq_state
                    .channel_measurements
                    .truncate(max_channels);
                state
                    .app
                    .measurement_state
                    .room_eq_state
                    .speaker_configs
                    .truncate(max_channels);
                state.app.measurement_state.room_eq_state.status_message = format!(
                    "Loaded {} channel(s) (truncated from {} — upgrade release channel for more)",
                    max_channels, channel_count
                );
                state.app.measurement_state.room_eq_state.error_message = None;
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
        #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
        {
            use crate::app::types::{
                ChannelMeasurement, RoomEqDataSource, RoomEqMeasurementsFile, RoomEqSpeakerConfig,
                SpeakerConfigType,
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

                            // Capture optimizer config before speakers are consumed
                            let backend_optimizer = room_config.optimizer.clone();

                            // Detect multi-position data before consuming speakers
                            let mut multi_position_counts: Vec<(String, usize)> = Vec::new();
                            for (channel_name, speaker_config) in &room_config.speakers {
                                match speaker_config {
                                    autoeq::SpeakerConfig::Single(autoeq::MeasurementSource::Multiple(m)) => {
                                        multi_position_counts.push((channel_name.clone(), m.measurements.len()));
                                    }
                                    autoeq::SpeakerConfig::Group(group) => {
                                        let has_multi = group.measurements.iter().any(|ms| {
                                            matches!(ms, autoeq::MeasurementSource::Multiple(_))
                                        });
                                        if has_multi {
                                            log::warn!(
                                                "Channel '{}' is a group speaker with multi-position measurements — \
                                                 multi-measurement optimization is not yet supported for group speakers",
                                                channel_name
                                            );
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            let has_multi_position_data = !multi_position_counts.is_empty();

                            // Use shared sotf-player parsing to convert RoomConfig → ChannelMeasurement.
                            // Use shared sotf-player parsing — handles all MeasurementRef
                            // variants (Inline, Named, Path) and both RoomConfig / legacy formats.
                            let mut channel_measurements: Vec<ChannelMeasurement> =
                                match RoomEqMeasurementsFile::load_from_json(&json, file_dir.as_deref()) {
                                    Ok(channels) => channels,
                                    Err(e) => {
                                        log::error!("Failed to parse measurements: {}", e);
                                        state_entity.update(cx, |state, _| {
                                            state.app.measurement_state.room_eq_state.error_message =
                                                Some(format!("Failed to parse measurements: {}", e));
                                        });
                                        return;
                                    }
                                };

                            // Enrich with extended metrics (THD, RT60, C50, C80) from CSV
                            for ch in &mut channel_measurements {
                                if let Some(metrics) = crate::components::migration::load_extended_metrics(
                                    ch.measurement.csv_path.as_deref(),
                                    file_dir.as_deref(),
                                ) {
                                    ch.measurement.thd_percent = metrics.thd_percent;
                                    ch.measurement.rt60_ms = metrics.rt60_ms;
                                    ch.measurement.clarity_c50_db = metrics.clarity_c50_db;
                                    ch.measurement.clarity_c80_db = metrics.clarity_c80_db;
                                    ch.measurement.excess_group_delay_ms = metrics.excess_group_delay_ms;
                                }
                            }

                            // Filter out channels with empty frequency data (can happen with
                            // older RoomConfig versions where CSV paths are unresolvable)
                            let empty_count = channel_measurements
                                .iter()
                                .filter(|m| m.measurement.frequencies.is_empty())
                                .count();
                            if empty_count > 0 {
                                log::warn!(
                                    "{} channel(s) have empty frequency data and will be skipped",
                                    empty_count
                                );
                            }
                            channel_measurements
                                .retain(|m| !m.measurement.frequencies.is_empty());

                            if channel_measurements.is_empty() {
                                log::error!("No valid inline measurements found in RoomConfig");
                                state_entity.update(cx, |state, _| {
                                    state.app.measurement_state.room_eq_state.error_message =
                                        Some("No valid measurement data found. The file may be an older format — try re-recording or re-exporting.".to_string());
                                });
                                return;
                            }

                            // Create speaker configs
                            let mut speaker_configs: Vec<RoomEqSpeakerConfig> = channel_measurements
                                .iter()
                                .map(|m| RoomEqSpeakerConfig {
                                    channel_name: m.channel_name.clone(),
                                    config_type: SpeakerConfigType::Single,
                                    driver_names: Vec::new(),
                                    ..Default::default()
                                })
                                .collect();

                            let channel_count = channel_measurements.len();
                            state_entity.update(cx, |state, _| {
                                let max_ch = state.app.max_room_eq_channels();
                                let truncated = max_ch > 0 && channel_count > max_ch;
                                if truncated {
                                    channel_measurements.truncate(max_ch);
                                    speaker_configs.truncate(max_ch);
                                }
                                state.app.measurement_state.room_eq_state.channel_measurements =
                                    channel_measurements;
                                state.app.measurement_state.room_eq_state.speaker_configs =
                                    speaker_configs;
                                state.app.measurement_state.room_eq_state.data_source =
                                    RoomEqDataSource::FromFile(file_path.clone());
                                if truncated {
                                    state.app.measurement_state.room_eq_state.status_message = format!(
                                        "Loaded {} channel(s) from {} (truncated from {} — upgrade release channel for more)",
                                        max_ch, file_path.display(), channel_count
                                    );
                                } else {
                                    state.app.measurement_state.room_eq_state.status_message = format!(
                                        "Successfully loaded {} channel(s) from {} (RoomConfig format)",
                                        channel_count,
                                        file_path.display()
                                    );
                                }
                                state.app.measurement_state.room_eq_state.error_message = None;
                                state.app.measurement_state.room_eq_state.has_multi_position_data = has_multi_position_data;
                                state.app.measurement_state.room_eq_state.multi_position_counts = multi_position_counts;
                                // Initialize equal weights for multi-measurement if detected
                                if has_multi_position_data {
                                    let max_count = state.app.measurement_state.room_eq_state.multi_position_counts
                                        .iter()
                                        .map(|(_, c)| *c)
                                        .max()
                                        .unwrap_or(0);
                                    if max_count > 0 {
                                        let equal_weight = 1.0 / max_count as f64;
                                        state.app.measurement_state.room_eq_state.optimizer_config
                                            .multi_measurement.weights = vec![equal_weight; max_count];
                                    }
                                }
                                // Import optimizer settings from the JSON so the GPUI
                                // uses the same parameters as the roomeq CLI.
                                state.app.measurement_state.room_eq_state.optimizer_config
                                    .import_from_backend(&backend_optimizer);
                                log::info!(
                                    "Imported optimizer config from file: algo={}, filters={}, pop={}, \
                                     max_db={}, max_q={}, max_iter={}, refine={}",
                                    backend_optimizer.algorithm,
                                    backend_optimizer.num_filters,
                                    backend_optimizer.population,
                                    backend_optimizer.max_db,
                                    backend_optimizer.max_q,
                                    backend_optimizer.max_iter,
                                    backend_optimizer.refine,
                                );
                                // Pre-seed the Delay Detection step from recording
                                // session metadata when the file carries it. Only
                                // untouched fields are overwritten so we don't stomp
                                // user overrides from an earlier session.
                                if let Some(hints) =
                                    RoomEqMeasurementsFile::extract_delay_detection_hints(&json)
                                {
                                    let dd = &mut state.app.measurement_state.room_eq_state.delay_detection;
                                    if let Some(sr) = hints.sample_rate {
                                        dd.sample_rate = sr;
                                    }
                                    if dd.output_device_name.is_none() {
                                        dd.output_device_name = hints.playback_device_name;
                                    }
                                    if dd.input_device_name.is_none() {
                                        dd.input_device_name = hints.recording_device_name;
                                    }
                                    // Populate probe results so the Delay step shows
                                    // arrival times from the recording session instead
                                    // of "no delay data available".
                                    if let Some(probe_results) = hints.probe_results {
                                        dd.apply_results(probe_results);
                                    }
                                }

                                state.app.measurement_state.room_eq_state.apply_smart_defaults();
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

                                    state_entity.update(cx, |state, _| {
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
                                    state_entity.update(cx, |state, _| {
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
                                        state_entity.update(cx, |state, _| {
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
                                let mut speaker_configs: Vec<RoomEqSpeakerConfig> = measurements_file
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
                                let mut channels = measurements_file.channels;
                                state_entity.update(cx, |state, _| {
                                    let max_ch = state.app.max_room_eq_channels();
                                    let truncated = max_ch > 0 && channel_count > max_ch;
                                    if truncated {
                                        channels.truncate(max_ch);
                                        speaker_configs.truncate(max_ch);
                                    }
                                    state.app.measurement_state.room_eq_state.channel_measurements =
                                        channels;
                                    state.app.measurement_state.room_eq_state.speaker_configs =
                                        speaker_configs;
                                    state.app.measurement_state.room_eq_state.data_source =
                                        RoomEqDataSource::FromFile(file_path.clone());
                                    if truncated {
                                        state.app.measurement_state.room_eq_state.status_message = format!(
                                            "Loaded {} channel(s) from {} (truncated from {} — upgrade release channel for more)",
                                            max_ch, file_path.display(), channel_count
                                        );
                                    } else {
                                        state.app.measurement_state.room_eq_state.status_message = format!(
                                            "Successfully loaded {} channel(s) from {}",
                                            channel_count,
                                            file_path.display()
                                        );
                                    }
                                    state.app.measurement_state.room_eq_state.error_message = None;
                                    // Legacy format: no multi-position data
                                    state.app.measurement_state.room_eq_state.has_multi_position_data = false;
                                    state.app.measurement_state.room_eq_state.multi_position_counts = Vec::new();
                                    state.app.measurement_state.room_eq_state.apply_smart_defaults();
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

                                state_entity.update(cx, |state, _| {
                                    state.app.measurement_state.room_eq_state.error_message =
                                        Some(error_msg);
                                });
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("File read error: {}", e);
                        state_entity.update(cx, |state, _| {
                            state.app.measurement_state.room_eq_state.error_message =
                                Some(format!("Failed to read file: {}", e));
                        });
                    }
                }
            }
            })
            .detach();
        }
    }

    /// Check if a legacy JSON file needs migration (large file with inline data)
    /// Uses the shared migration module
    fn check_legacy_needs_migration(json: &str, file_size: u64) -> bool {
        crate::components::migration::check_needs_migration(json, file_size)
    }
}
