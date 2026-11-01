use crate::app::App;

pub(super) fn is_room_eq_field_numerical(field: usize) -> bool {
    matches!(field, 0..=6 | 9..=13 | 23 | 25 | 27)
}

pub(super) fn set_room_eq_field_from_string(app: &mut App) {
    let c = &mut app.room_eq.model.optimizer_config;
    let buf = &app.room_eq.edit_buffer;
    match app.room_eq.selected_field {
        0 => {
            if let Ok(v) = buf.parse::<usize>() {
                c.num_filters = v.clamp(1, 30);
            }
        }
        1 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.min_freq = v.clamp(20.0, 500.0);
            }
        }
        2 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.max_freq = v.clamp(1000.0, 20000.0);
            }
        }
        3 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.min_db = v.clamp(-24.0, 0.0);
            }
        }
        4 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.max_db = v.clamp(0.0, 12.0);
            }
        }
        5 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.min_q = v.clamp(0.1, 2.0);
            }
        }
        6 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.max_q = v.clamp(1.0, 20.0);
            }
        }
        9 => {
            if let Ok(v) = buf.parse::<usize>() {
                c.max_iter = v.clamp(1000, 100000);
            }
        }
        10 => {
            if let Ok(v) = buf.parse::<usize>() {
                c.population = v.clamp(10, 10000);
            }
        }
        11 => {
            if let Ok(v) = buf.parse::<usize>() {
                c.bo_initial_samples = v.clamp(0, 10000);
            }
        }
        12 => {
            if let Ok(v) = buf.parse::<usize>() {
                c.bo_batch_size = v.clamp(0, 64);
            }
        }
        13 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.bo_posterior_std_threshold = v.clamp(0.0, 1.0);
            }
        }
        23 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.target_response.slope_db_per_octave = v.clamp(-3.0, 0.0);
            }
        }
        25 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.excursion_protection.manual_f3_hz = v.clamp(20.0, 200.0);
            }
        }
        27 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.schroeder_split.schroeder_freq = v.clamp(100.0, 1000.0);
            }
        }
        _ => {}
    }
    // Any optimizer-config edit makes stored results inapplicable.
    app.room_eq.model.note_inputs_changed();
}

pub(super) fn adjust_room_eq_field(app: &mut App, delta: i32) {
    use sotf_audio_player::room_eq_types::{MultiSpeakerMode, RoomEqOptimizationMode};

    let c = &mut app.room_eq.model.optimizer_config;
    match app.room_eq.selected_field {
        // Basic
        0 => {
            let n = c.num_filters as i32 + delta;
            c.num_filters = n.clamp(1, 30) as usize;
        }
        1 => c.min_freq = (c.min_freq + delta as f64 * 10.0).clamp(20.0, 500.0),
        2 => c.max_freq = (c.max_freq + delta as f64 * 500.0).clamp(1000.0, 20000.0),
        3 => c.min_db = (c.min_db + delta as f64).clamp(-24.0, 0.0),
        4 => c.max_db = (c.max_db + delta as f64).clamp(0.0, 12.0),
        5 => c.min_q = (c.min_q + delta as f64 * 0.1).clamp(0.1, 2.0),
        6 => c.max_q = (c.max_q + delta as f64 * 0.5).clamp(1.0, 20.0),
        7 => {
            c.peq_model = super::super::cycle_string(
                &c.peq_model,
                &["pk", "hp-pk", "hp-pk-lp", "ls-pk", "ls-pk-hs"],
                delta,
            );
        }
        // Optimization
        8 => {
            let algos = ["autoeq:cobyla", "autoeq:de", "autoeq:bo", "autoeq:cmaes"];
            c.algorithm = super::super::cycle_string(&c.algorithm, &algos, delta);
        }
        9 => {
            let n = c.max_iter as i32 + delta * 1000;
            c.max_iter = n.clamp(1000, 100000) as usize;
        }
        10 => {
            let n = c.population as i32 + delta * 10;
            c.population = n.clamp(10, 10000) as usize;
        }
        11 => {
            let n = c.bo_initial_samples as i32 + delta;
            c.bo_initial_samples = n.clamp(0, 10000) as usize;
        }
        12 => {
            let n = c.bo_batch_size as i32 + delta;
            c.bo_batch_size = n.clamp(0, 64) as usize;
        }
        13 => {
            c.bo_posterior_std_threshold =
                (c.bo_posterior_std_threshold + delta as f64 * 0.001).clamp(0.0, 1.0);
        }
        14 => {
            c.bo_acquisition =
                super::super::cycle_string(&c.bo_acquisition, &["qei", "ei", "thompson"], delta);
        }
        15 => c.bo_ehvi = !c.bo_ehvi,
        16 => c.refine = !c.refine,
        17 => {
            c.local_algo = super::super::cycle_string(&c.local_algo, &["cobyla"], delta);
        }
        18 => c.psychoacoustic = !c.psychoacoustic,
        19 => c.asymmetric_loss = !c.asymmetric_loss,
        // Mode
        20 => {
            let modes = RoomEqOptimizationMode::all();
            let idx = modes.iter().position(|m| *m == c.mode).unwrap_or(0);
            let new_idx = if delta > 0 {
                (idx + 1) % modes.len()
            } else {
                (idx + modes.len() - 1) % modes.len()
            };
            c.mode = modes[new_idx];
        }
        21 => {
            let modes = MultiSpeakerMode::all();
            let idx = modes
                .iter()
                .position(|m| *m == c.multi_speaker_mode)
                .unwrap_or(0);
            let new_idx = if delta > 0 {
                (idx + 1) % modes.len()
            } else {
                (idx + modes.len() - 1) % modes.len()
            };
            c.multi_speaker_mode = modes[new_idx];
        }
        // Target Response
        22 => c.target_response.enabled = !c.target_response.enabled,
        23 => {
            c.target_response.slope_db_per_octave =
                (c.target_response.slope_db_per_octave + delta as f64 * 0.1).clamp(-3.0, 0.0)
        }
        // Excursion Protection
        24 => c.excursion_protection.enabled = !c.excursion_protection.enabled,
        25 => {
            c.excursion_protection.manual_f3_hz =
                (c.excursion_protection.manual_f3_hz + delta as f64 * 5.0).clamp(20.0, 200.0)
        }
        // Schroeder Split
        26 => c.schroeder_split.enabled = !c.schroeder_split.enabled,
        27 => {
            c.schroeder_split.schroeder_freq =
                (c.schroeder_split.schroeder_freq + delta as f64 * 10.0).clamp(100.0, 1000.0)
        }
        // Phase Alignment
        28 => c.phase_alignment.enabled = !c.phase_alignment.enabled,
        _ => {}
    }
    // Any optimizer-config edit makes stored results inapplicable.
    app.room_eq.model.note_inputs_changed();
}

pub(crate) fn load_room_eq_measurements(app: &mut App) {
    use sotf_audio_player::room_eq_types::RoomEqMeasurementsFile;

    app.room_eq.quality_review_entries.clear();
    app.room_eq.quality_sidecar_warning = None;
    let path = &app.room_eq.file_path;
    if path.is_empty() {
        app.room_eq.load_error = Some("No file path specified".to_string());
        return;
    }

    let base_dir = std::path::Path::new(path).parent();

    match std::fs::read_to_string(path) {
        Ok(contents) => match RoomEqMeasurementsFile::load_from_json(&contents, base_dir) {
            Ok(mut channels) => {
                let quality_sidecar =
                    sotf_audio_player::recording_helpers::load_recording_quality_sidecar(
                        std::path::Path::new(path),
                        contents.as_bytes(),
                    );
                let sidecar = match quality_sidecar {
                    Ok(Some(sidecar)) => Some(sidecar),
                    Ok(None) => {
                        app.room_eq.quality_sidecar_warning = Some(
                            "Quality evidence unavailable (unknown): no current recording-quality.json sidecar".into(),
                        );
                        None
                    }
                    Err(error) => {
                        app.room_eq.quality_sidecar_warning = Some(format!(
                            "Quality evidence unavailable (unknown): could not read recording-quality.json: {error}"
                        ));
                        None
                    }
                };
                app.room_eq.quality_review_entries =
                    attach_quality_sidecar(&mut channels, sidecar.as_ref());
                app.room_eq.model.ctc_config =
                    serde_json::from_str::<roomeq_model::RoomConfig>(&contents)
                        .ok()
                        .and_then(|room_config| room_config.ctc);
                if let (Some(ctc), Some(dir)) = (app.room_eq.model.ctc_config.as_mut(), base_dir) {
                    ctc.resolve_paths(dir);
                }
                app.room_eq.model.ctc_measurements = app
                    .room_eq
                    .model
                    .ctc_config
                    .as_ref()
                    .and_then(|ctc| ctc.measurements.clone());
                app.room_eq.model.channel_measurements = channels;
                // Replaced measurements own no results yet: drop any
                // result produced from the previous inputs.
                app.room_eq.model.note_inputs_changed();
                app.room_eq.model.invalidate_results();
                app.room_eq.model.init_speaker_configs();
                app.room_eq.model.apply_smart_defaults(None);
                app.room_eq.infer_easy_layout();
                app.room_eq.load_error = None;
                // Pre-seed Delay Detection form from the recording
                // session metadata when the file carries it. Only fields
                // the user hasn't already customised this session are
                // touched so we don't stomp on an active override.
                if let Some(hints) =
                    RoomEqMeasurementsFile::extract_delay_detection_hints(&contents)
                {
                    let dd = &mut app.room_eq.model.delay_detection;
                    if let Some(sr) = hints.sample_rate {
                        dd.sample_rate = sr;
                    }
                    if dd.output_device_name.is_none() {
                        dd.output_device_name = hints.playback_device_name;
                    }
                    if dd.input_device_name.is_none() {
                        dd.input_device_name = hints.recording_device_name;
                    }
                    if let Some(probe_results) = hints.probe_results {
                        dd.apply_results(probe_results);
                    }
                }
            }
            Err(e) => {
                app.room_eq.load_error = Some(e);
                app.room_eq.model.channel_measurements.clear();
                app.room_eq.model.ctc_measurements = None;
                app.room_eq.model.ctc_config = None;
                app.room_eq.model.speaker_configs.clear();
                // A failed replacement load must not leave the previous
                // run's results applicable under the new (empty) inputs.
                app.room_eq.model.note_inputs_changed();
                app.room_eq.model.invalidate_results();
            }
        },
        Err(e) => {
            app.room_eq.load_error = Some(format!("Read error: {}", e));
            app.room_eq.model.channel_measurements.clear();
            app.room_eq.model.ctc_measurements = None;
            app.room_eq.model.ctc_config = None;
            app.room_eq.model.speaker_configs.clear();
            app.room_eq.model.note_inputs_changed();
            app.room_eq.model.invalidate_results();
        }
    }
}

fn attach_quality_sidecar(
    channels: &mut [sotf_audio_player::room_eq_types::ChannelMeasurement],
    sidecar: Option<&sotf_audio_player::recording_helpers::RecordingQualitySidecar>,
) -> Vec<crate::app::RoomEqQualityReviewEntry> {
    use sotf_audio_player::recording_types::RecordingSourceIdentity;

    let mut entries = Vec::new();
    for channel in channels {
        if channel.is_group {
            for (driver_index, driver) in channel.driver_measurement_sets.iter_mut().enumerate() {
                let driver_label = driver.name.clone();
                let measurements = std::iter::once(&mut driver.measurement)
                    .chain(driver.multi_mic_measurements.iter_mut());
                for (measurement_index, measurement) in measurements.enumerate() {
                    let source = RecordingSourceIdentity {
                        speaker: channel.channel_name.clone(),
                        driver_index: Some(driver_index),
                        measurement_index,
                    };
                    let quality = sidecar
                        .and_then(|sidecar| sidecar.quality_for(&source))
                        .cloned();
                    let stimulus_config = sidecar
                        .and_then(|sidecar| sidecar.stimulus_for(&source))
                        .cloned();
                    measurement.quality = quality.clone();
                    measurement.stimulus_config = stimulus_config.clone();
                    let source_label = driver
                        .provenance
                        .get(measurement_index)
                        .and_then(|item| item.name.clone())
                        .or_else(|| driver_label.clone());
                    entries.push(crate::app::RoomEqQualityReviewEntry {
                        source,
                        source_label,
                        quality,
                        stimulus_config,
                    });
                }
            }
            if let Some(first_driver) = channel.driver_measurement_sets.first() {
                channel.measurement.quality = first_driver.measurement.quality.clone();
                channel.measurement.stimulus_config =
                    first_driver.measurement.stimulus_config.clone();
            }
            continue;
        }

        let channel_name = channel.channel_name.clone();
        let measurements = std::iter::once(&mut channel.measurement)
            .chain(channel.multi_mic_measurements.iter_mut());
        for (measurement_index, measurement) in measurements.enumerate() {
            let source = RecordingSourceIdentity {
                speaker: channel_name.clone(),
                driver_index: None,
                measurement_index,
            };
            let quality = sidecar
                .and_then(|sidecar| sidecar.quality_for(&source))
                .cloned();
            let stimulus_config = sidecar
                .and_then(|sidecar| sidecar.stimulus_for(&source))
                .cloned();
            measurement.quality = quality.clone();
            measurement.stimulus_config = stimulus_config.clone();
            let source_label = channel
                .provenance
                .get(measurement_index)
                .and_then(|item| item.name.clone());
            entries.push(crate::app::RoomEqQualityReviewEntry {
                source,
                source_label,
                quality,
                stimulus_config,
            });
        }
    }
    entries
}

pub(crate) fn export_room_eq_results(app: &mut App) {
    if app.room_eq.export_path.is_empty() {
        app.room_eq.export_error = Some("No export path specified".to_string());
        app.room_eq.export_success = false;
        return;
    }

    // Export the full DSP chain output, exactly like the GPUI app.
    // Flattening per-channel `eq_filters` into one PEQ bank (the old
    // behavior) lost channel routing, filter types, gains, delays, FIR
    // and crossover stages, and silently substituted Peak filters.
    let Some(dsp_output) = app.room_eq.model.dsp_output.clone() else {
        app.room_eq.export_error =
            Some("No optimization results to export. Run the optimizer first.".to_string());
        app.room_eq.export_success = false;
        return;
    };
    if !app.room_eq.model.results_are_current() {
        app.room_eq.export_error = Some(
            "Optimization results are stale — inputs changed since the run. Re-run the optimizer before exporting."
                .to_string(),
        );
        app.room_eq.export_success = false;
        return;
    }

    let formats = sotf_audio_player::autoeq::EQ_EXPORT_FORMAT_OPTIONS;
    let (format_id, _, _) = formats
        .get(app.room_eq.model.export_format_index)
        .copied()
        .unwrap_or(("json", "JSON", ".json"));

    let sample_rate = app.room_eq.model.optimizer_config.sample_rate as f64;
    let path = std::path::Path::new(&app.room_eq.export_path).to_path_buf();
    let artifact_dir = app.room_eq.model.artifact_dir.clone().unwrap_or_else(|| {
        path.parent().map_or_else(
            || std::path::PathBuf::from("."),
            std::path::Path::to_path_buf,
        )
    });

    let result: Result<(), String> = (|| {
        // `room_eq_types::DspChainOutput` is the autoeq type the chain
        // exporter expects, re-exported by the player crate.
        let autoeq_output = &dsp_output;

        match format_id {
            // SotF JSON: the complete chain, pretty-printed.
            "json" => {
                let content =
                    serde_json::to_string_pretty(autoeq_output).map_err(|e| e.to_string())?;
                std::fs::write(&path, content).map_err(|e| format!("Write error: {e}"))
            }
            "camilladsp" | "apo" | "easyeffects" | "wavelet" | "pipewire" | "roon" => {
                let format = match format_id {
                    "camilladsp" => roomeq_workflow::ExportFormat::CamillaDsp,
                    "apo" => roomeq_workflow::ExportFormat::EqualizerApo,
                    "easyeffects" => roomeq_workflow::ExportFormat::EasyEffects,
                    "wavelet" => roomeq_workflow::ExportFormat::Wavelet,
                    "pipewire" => roomeq_workflow::ExportFormat::PipeWire,
                    _ => roomeq_workflow::ExportFormat::RoonDsp,
                };
                roomeq_workflow::export_dsp_chain_with_convolution_sidecars(
                    autoeq_output,
                    format,
                    &path,
                    sample_rate,
                    &artifact_dir,
                )
                .map_err(|e| format!("Export failed: {e}"))
            }
            // PEQ-only formats cannot represent the full chain; reject
            // explicitly instead of silently dropping stages.
            other => Err(format!(
                "Format '{other}' cannot represent the full RoomEQ chain (channel routing, gains, delays, FIR and crossover stages would be dropped). Export as JSON or CamillaDSP instead."
            )),
        }
    })();

    match result {
        Ok(()) => {
            app.room_eq.export_success = true;
            app.room_eq.export_error = None;
        }
        Err(e) => {
            app.room_eq.export_error = Some(e);
            app.room_eq.export_success = false;
        }
    }
}
