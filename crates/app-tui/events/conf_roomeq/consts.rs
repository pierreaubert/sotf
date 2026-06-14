use crate::app::App;
use sotf_audio_player::room_eq_types::{OptimizationStatus, ctc_system_config_for_speaker_names};
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

pub(super) fn spawn_room_eq_optimization(app: &mut App) {
    if app.room_eq.model.channel_measurements.is_empty() {
        app.room_eq.model.optimization_status = OptimizationStatus::Failed;
        app.room_eq.model.error_message = Some("No measurements loaded".to_string());
        return;
    }

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
    let measurements = app.room_eq.model.channel_measurements.clone();
    let config = app.room_eq.model.optimizer_config.clone();
    // Probe-based arrival times from the Delay Detection step (None if the
    // user skipped that step; in that case the optimizer falls back to
    // WAV-onset detection for each channel).
    let probe_arrivals = app.room_eq.model.delay_detection.probe_arrival_map();
    let ctc_config = app.room_eq.model.ctc_config.clone();
    let ctc_measurements = app.room_eq.model.ctc_measurements.clone();

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
        use autoeq::MeasurementSource;
        use autoeq::roomeq::{CallbackAction, RoomConfig, SpeakerConfig};
        use sotf_audio_player::autoeq::{
            run_room_optimization, run_room_optimization_with_probe_arrivals,
        };

        // Convert measurements to speaker configs. Multi-mic / multi-
        // position takes ride along as `InMemoryMultiple` so the
        // optimizer averages every position into one EQ chain per
        // channel; single-take channels stay on `InMemory`.
        let curve_from_result =
            |result: &sotf_audio_player::recording_types::RecordingResult| -> autoeq::Curve {
                let freq: Vec<f64> = result.frequencies.iter().map(|&f| f as f64).collect();
                let spl: Vec<f64> = result.magnitude_db.iter().map(|&db| db as f64).collect();
                autoeq::Curve {
                    freq: ndarray::Array1::from(freq),
                    spl: ndarray::Array1::from(spl),
                    phase: None,
                    ..Default::default()
                }
            };

        let mut speakers = std::collections::HashMap::new();
        for m in &measurements {
            let primary_curve = curve_from_result(&m.measurement);
            let source = if m.multi_mic_measurements.is_empty() {
                MeasurementSource::InMemory(primary_curve)
            } else {
                let mut curves = Vec::with_capacity(m.multi_mic_measurements.len() + 1);
                curves.push(primary_curve);
                curves.extend(m.multi_mic_measurements.iter().map(curve_from_result));
                MeasurementSource::InMemoryMultiple(curves)
            };
            speakers.insert(m.channel_name.clone(), SpeakerConfig::Single(source));
        }

        let optimizer = config.to_optimizer_config();

        let ctc = ctc_config.or_else(|| {
            ctc_measurements.map(|measurements| autoeq::roomeq::CtcConfig {
                enabled: true,
                matrix_source: "measured".to_string(),
                measurements: Some(measurements),
                ..Default::default()
            })
        });
        let system = ctc.as_ref().filter(|ctc| ctc.enabled).and_then(|_| {
            ctc_system_config_for_speaker_names(speakers.keys().map(String::as_str), None)
        });

        let room_config = RoomConfig {
            version: autoeq::roomeq::default_config_version(),
            system,
            speakers,
            crossovers: None,
            target_curve: None,
            optimizer,
            recording_config: None,
            ctc,
            cea2034_cache: None,
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
