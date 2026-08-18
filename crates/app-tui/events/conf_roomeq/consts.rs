use crate::app::App;
use sotf_audio_player::room_eq_types::{
    OptimizationStatus, RoomEqWizardMode, apply_room_eq_easy_layout,
};
use std::sync::{Arc, Mutex};

/// Total number of adjustable fields in the Room EQ configure step
pub(super) const ROOM_EQ_FIELD_COUNT: usize = 29;

#[allow(clippy::type_complexity)]
pub(super) static ROOM_OPT_RESULT: std::sync::OnceLock<
    Arc<Mutex<Option<Result<sotf_audio_player::autoeq::RoomOptimizationResult, String>>>>,
> = std::sync::OnceLock::new();

pub(super) static ROOM_OPT_PROGRESS: std::sync::OnceLock<
    Arc<Mutex<Option<sotf_audio_player::autoeq::RoomOptimizationProgress>>>,
> = std::sync::OnceLock::new();

fn prepare_room_eq_config(app: &mut App) -> Result<autoeq::roomeq::RoomConfig, String> {
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
        use autoeq::roomeq::CallbackAction;
        use sotf_audio_player::autoeq::{
            run_room_optimization, run_room_optimization_with_probe_arrivals,
        };

        let progress_slot2 = progress_slot.clone();
        let callback: sotf_audio_player::autoeq::RoomOptimizationCallback = Box::new(move |p| {
            if let Ok(mut guard) = progress_slot2.lock() {
                *guard = Some(p.clone());
            }
            CallbackAction::Continue
        });

        let result = if let Some(arrivals) = probe_arrivals.as_ref() {
            run_room_optimization_with_probe_arrivals(
                &room_config,
                48000.0,
                Some(callback),
                arrivals,
            )
        } else {
            run_room_optimization(&room_config, 48000.0, Some(callback))
        };
        if let Ok(mut guard) = result_slot.lock() {
            *guard = Some(result);
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
            channel_name: name.to_string(),
            measurement: RecordingResult {
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
            },
            is_group: false,
            group_drivers: Vec::new(),
            multi_mic_measurements: Vec::new(),
        }
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
