use crate::app::App;
use sotf_audio_player::room_eq_types::{
    OptimizationStatus, RoomEqWizardMode, apply_room_eq_easy_layout,
};
use std::sync::{Arc, Mutex};

/// Total number of adjustable fields in the Room EQ configure step
pub(super) const ROOM_EQ_FIELD_COUNT: usize = 29;

#[allow(clippy::type_complexity)]
pub(super) static ROOM_OPT_RESULT: std::sync::OnceLock<
    Arc<
        Mutex<
            Option<(
                u64,
                Result<sotf_audio_player::autoeq::RoomOptimizationResult, String>,
            )>,
        >,
    >,
> = std::sync::OnceLock::new();

pub(super) static ROOM_OPT_PROGRESS: std::sync::OnceLock<
    Arc<Mutex<Option<sotf_audio_player::autoeq::RoomOptimizationProgress>>>,
> = std::sync::OnceLock::new();

fn prepare_room_eq_config(app: &mut App) -> Result<roomeq_model::RoomConfig, String> {
    if app.room_eq.model.channel_measurements.is_empty() {
        return Err("No measurements loaded".to_string());
    }

    if app.room_eq.model.wizard_mode == RoomEqWizardMode::Simple {
        let channel_names = app
            .room_eq
            .model
            .channel_measurements
            .iter()
            .map(|measurement| measurement.channel_name.clone())
            .collect::<Vec<_>>();
        let mut preset = app.room_eq.model.simple_preset.clone();
        if let Err(error) = apply_room_eq_easy_layout(
            app.room_eq.easy_layout,
            &channel_names,
            &mut preset,
            &mut app.room_eq.model.optimizer_config,
        ) {
            return Err(error.to_string());
        }
        app.room_eq.model.simple_preset = preset;
    }

    Ok(app.room_eq.model.to_room_config())
}

pub(super) fn spawn_room_eq_optimization(app: &mut App) {
    let room_config = match prepare_room_eq_config(app) {
        Ok(config) => config,
        Err(error) => {
            app.room_eq.model.optimization_status = OptimizationStatus::Failed;
            app.room_eq.model.error_message = Some(error);
            return;
        }
    };

    app.room_eq.model.optimization_status = OptimizationStatus::Running;
    app.room_eq.model.error_message = None;
    app.room_eq.model.overall_progress = 0.0;
    app.room_eq.model.current_iteration = 0;
    app.room_eq.model.current_loss = 0.0;
    app.room_eq.model.current_channel = None;
    app.room_eq.model.channel_results.clear();
    app.room_eq.model.dsp_output = None;
    app.room_eq.model.status_message = String::new();
    // Capture the input revision this run belongs to and install a
    // fresh cancellation flag shared with the worker callback.
    let run_revision = app.room_eq.model.begin_run();
    app.room_eq.model.cancel_requested = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let cancel_flag = app.room_eq.model.cancel_requested.clone();
    // Design the filters at the configured rate, not a fixed 48 kHz.
    let sample_rate = app.room_eq.model.optimizer_config.sample_rate as f64;

    app.room_eq.opt_max_iter = 0;
    app.room_eq.apply_status = None;
    app.room_eq.apply_error = None;
    app.room_eq.loss_history.clear();
    app.room_eq.opt_log_lines.clear();
    app.room_eq.opt_log_scroll = 0;

    // Build curves from loaded measurements
    // Probe-based arrival times from the Delay Detection step (None if the
    // user skipped that step; in that case the optimizer falls back to
    // WAV-onset detection for each channel).
    let probe_arrivals = app.room_eq.model.delay_detection.probe_arrival_map();

    let result_slot = ROOM_OPT_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();
    let progress_slot = ROOM_OPT_PROGRESS
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();

    // Clear stale results
    if let Ok(mut g) = result_slot.lock() {
        *g = None;
    }
    if let Ok(mut g) = progress_slot.lock() {
        *g = None;
    }

    std::thread::spawn(move || {
        use roomeq_workflow::CallbackAction;
        use sotf_audio_player::autoeq::{
            run_room_optimization, run_room_optimization_with_probe_arrivals,
        };

        let progress_slot2 = progress_slot.clone();
        let cancel_flag2 = cancel_flag.clone();
        let callback: sotf_audio_player::autoeq::RoomOptimizationCallback = Box::new(move |p| {
            if let Ok(mut guard) = progress_slot2.lock() {
                *guard = Some(p.clone());
            }
            if cancel_flag2.load(std::sync::atomic::Ordering::Relaxed) {
                CallbackAction::Stop
            } else {
                CallbackAction::Continue
            }
        });

        let result = if let Some(arrivals) = probe_arrivals.as_ref() {
            run_room_optimization_with_probe_arrivals(
                &room_config,
                sample_rate,
                Some(callback),
                arrivals,
            )
        } else {
            run_room_optimization(&room_config, sample_rate, Some(callback))
        };
        if let Ok(mut guard) = result_slot.lock() {
            *guard = Some((run_revision, result));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::prepare_room_eq_config;
    use crate::events::tests::make_app;
    use sotf_audio_player::recording_types::RecordingResult;
    use sotf_audio_player::room_eq_types::{
        ChannelMeasurement, RoomEqEasyLayout, RoomEqWizardMode,
    };

    fn measurement(channel: usize, name: &str) -> ChannelMeasurement {
        ChannelMeasurement {
            acquisition: None,
            driver_measurement_sets: Vec::new(),
            provenance: Vec::new(),
            channel_name: name.to_string(),
            measurement: RecordingResult {
                sample_rate_hz: None,
                channel,
                wav_path: None,
                csv_path: None,
                frequencies: vec![20.0, 1_000.0, 20_000.0],
                magnitude_db: vec![0.0; 3],
                phase_deg: vec![0.0; 3],
                impulse_response: None,
                impulse_time_ms: None,
                thd_percent: None,
                harmonic_distortion_db: None,
                excess_group_delay_ms: None,
                rt60_ms: None,
                clarity_c50_db: None,
                clarity_c80_db: None,
                spectrogram_db: None,
                quality: None,
                stimulus_config: None,
            },
            is_group: false,
            group_drivers: Vec::new(),
            multi_mic_measurements: Vec::new(),
        }
    }

    fn exportable_app() -> crate::app::App {
        let mut app = make_app();
        let dsp_output: sotf_audio_player::room_eq_types::DspChainOutput =
            serde_json::from_value(serde_json::json!({
                "version": "1.0.0",
                "channels": {
                    "L": {
                        "channel": "L",
                        "plugins": [{
                            "plugin_type": "EQ",
                            "parameters": {"filters": [{
                                "filter_type": "lowshelf",
                                "frequency": 100.0,
                                "q": 0.7,
                                "gain_db": 3.0
                            }]}
                        }]
                    },
                    "R": {
                        "channel": "R",
                        "plugins": [{
                            "plugin_type": "EQ",
                            "parameters": {"filters": [{
                                "filter_type": "highpass",
                                "frequency": 40.0,
                                "q": 0.7,
                                "gain_db": 0.0
                            }]}
                        }]
                    }
                }
            }))
            .unwrap();
        app.room_eq.model.dsp_output = Some(dsp_output);
        let run = app.room_eq.model.begin_run();
        assert!(app.room_eq.model.complete_run(run));
        app.room_eq.model.export_format_index = 0;
        app
    }

    #[test]
    fn tui_export_writes_the_full_chain_per_channel() {
        use crate::events::conf_roomeq::misc::export_room_eq_results;

        let mut app = exportable_app();
        let path =
            std::env::temp_dir().join(format!("sotf-tui-export-full-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        app.room_eq.export_path = path.to_string_lossy().into_owned();

        export_room_eq_results(&mut app);

        assert!(app.room_eq.export_success, "{:?}", app.room_eq.export_error);
        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let channels = written.get("channels").unwrap();
        // Both channels survive as separate chains (no merged PEQ bank),
        // with their non-peak filter types intact.
        let l_filters = channels["L"]["plugins"][0]["parameters"]["filters"].clone();
        let r_filters = channels["R"]["plugins"][0]["parameters"]["filters"].clone();
        assert_eq!(l_filters[0]["filter_type"], "lowshelf");
        assert_eq!(r_filters[0]["filter_type"], "highpass");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn tui_export_rejects_stale_results() {
        use crate::events::conf_roomeq::misc::export_room_eq_results;

        let mut app = exportable_app();
        let path =
            std::env::temp_dir().join(format!("sotf-tui-export-stale-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        app.room_eq.export_path = path.to_string_lossy().into_owned();
        // Inputs changed after the run: the result must not export.
        app.room_eq.model.note_inputs_changed();

        export_room_eq_results(&mut app);

        assert!(!app.room_eq.export_success);
        assert!(
            app.room_eq
                .export_error
                .as_deref()
                .is_some_and(|e| e.contains("stale")),
            "{:?}",
            app.room_eq.export_error
        );
        assert!(!path.exists());
    }

    #[test]
    fn tui_export_rejects_peq_only_formats() {
        use crate::events::conf_roomeq::misc::export_room_eq_results;

        let mut app = exportable_app();
        app.room_eq.model.export_format_index = sotf_audio_player::autoeq::EQ_EXPORT_FORMAT_OPTIONS
            .iter()
            .position(|(id, _, _)| *id == "rme-channel")
            .unwrap();
        let path =
            std::env::temp_dir().join(format!("sotf-tui-export-peq-{}.xml", std::process::id()));
        let _ = std::fs::remove_file(&path);
        app.room_eq.export_path = path.to_string_lossy().into_owned();

        export_room_eq_results(&mut app);

        assert!(!app.room_eq.export_success);
        assert!(
            app.room_eq
                .export_error
                .as_deref()
                .is_some_and(|e| e.contains("cannot represent the full RoomEQ chain")),
            "{:?}",
            app.room_eq.export_error
        );
        assert!(!path.exists());
    }

    #[test]
    fn beginner_layout_is_validated_before_tui_optimization() {
        let mut app = make_app();
        app.room_eq.model.wizard_mode = RoomEqWizardMode::Simple;
        app.room_eq.easy_layout = RoomEqEasyLayout::Stereo21;
        app.room_eq.model.channel_measurements = vec![measurement(0, "L")];

        let error = prepare_room_eq_config(&mut app).unwrap_err();
        assert!(error.contains("missing"));
        assert!(app.room_eq.model.simple_preset.bass_management.is_empty());
    }

    #[test]
    fn tui_beginner_stereo_21_uses_shared_bass_managed_room_config() {
        let mut app = make_app();
        app.room_eq.model.wizard_mode = RoomEqWizardMode::Simple;
        app.room_eq.easy_layout = RoomEqEasyLayout::Stereo21;
        app.room_eq.model.channel_measurements = vec![
            measurement(0, "L"),
            measurement(1, "R"),
            measurement(2, "LFE"),
        ];
        app.room_eq.model.init_speaker_configs();

        let config = prepare_room_eq_config(&mut app).unwrap();

        assert_eq!(app.room_eq.model.simple_preset.bass_management, "Standard");
        assert!(config.optimizer.schroeder_split.is_some());
        assert!(
            config
                .crossovers
                .as_ref()
                .is_some_and(|crossovers| crossovers.contains_key("bass_management"))
        );
        assert!(
            config
                .system
                .as_ref()
                .and_then(|system| system.subwoofers.as_ref())
                .is_some()
        );
    }
}
