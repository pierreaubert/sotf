use super::consts::HEADPHONE_DOWNLOAD_RESULT;
use super::consts::HEADPHONE_LIST_RESULT;
use super::consts::HEADPHONE_OPT_PROGRESS;
use super::consts::HEADPHONE_OPT_RESULT;
use crate::app::App;
use std::sync::{Arc, Mutex};

pub(super) fn spawn_headphone_eq_optimization(app: &mut App) {
    use sotf_audio_player::room_eq_types::OptimizationStatus;

    if app.headphone_eq.model.measurement_path.is_empty() {
        app.headphone_eq.model.optimization_status = OptimizationStatus::Failed;
        app.headphone_eq.model.error_message = Some("No measurement file selected".to_string());
        return;
    }

    app.headphone_eq.model.optimization_status = OptimizationStatus::Running;
    app.headphone_eq.model.error_message = None;
    app.headphone_eq.model.progress = 0.0;
    app.headphone_eq.model.current_iteration = 0;
    app.headphone_eq.model.current_loss = 0.0;
    app.headphone_eq.model.filters.clear();
    app.headphone_eq.model.progress_history.clear();

    let curve_path = app.headphone_eq.model.measurement_path.clone();
    let target = app.headphone_eq.model.target_preset.clone();
    let custom_target = app.headphone_eq.model.custom_target_path.clone();
    let c = &app.headphone_eq.model.optimizer_config;

    let mut args = autoeq::Args::headphone_defaults();
    args.num_filters = c.num_filters;
    args.min_freq = c.min_freq;
    args.max_freq = c.max_freq;
    args.min_db = c.min_db;
    args.max_db = c.max_db;
    args.min_q = c.min_q;
    args.max_q = c.max_q;
    args.maxeval = c.max_iter;
    args.algo = c.algorithm.to_autoeq_string().to_string();
    args.peq_model = sotf_audio_player::autoeq::parse_peq_model(&c.peq_model);
    args.population = c.population;
    args.recombination = c.de_cr;
    args.strategy = c.strategy.clone();
    args.tolerance = c.tolerance;
    args.refine = c.refine;
    args.local_algo = c.local_algo.clone();
    args.smooth = c.smooth;
    args.smooth_n = c.smooth_n;
    args.loss = sotf_audio_player::autoeq::parse_loss_type(&c.loss);

    let result_slot = HEADPHONE_OPT_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();
    let progress_slot = HEADPHONE_OPT_PROGRESS
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
        use sotf_audio_player::autoeq::CallbackAction;

        let progress_slot2 = progress_slot.clone();
        let callback = move |p: &sotf_audio_player::autoeq::ProgressUpdate| {
            let pct = if p.max_iterations > 0 {
                p.iteration as f32 / p.max_iterations as f32
            } else {
                0.0
            };
            if let Ok(mut guard) = progress_slot2.lock() {
                *guard = Some((p.iteration, p.max_iterations, p.loss, pct));
            }
            CallbackAction::Continue
        };

        let result = sotf_audio_player::autoeq::headphone::run_headphone_optimization_with_callback(
            &curve_path,
            &target,
            &custom_target,
            &args,
            Some(callback),
        );
        if let Ok(mut guard) = result_slot.lock() {
            *guard = Some(result);
        }
    });
}

pub(super) fn spawn_headphone_list_load() {
    let result_slot = HEADPHONE_LIST_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();

    if let Ok(mut g) = result_slot.lock() {
        *g = None;
    }

    let slot = result_slot.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let result = rt
            .block_on(async { autoeq::fetch_available_headphones().await })
            .map_err(|e| e.to_string());
        if let Ok(mut guard) = slot.lock() {
            *guard = Some(result);
        }
    });
}

pub(super) fn spawn_headphone_download(headphone_name: &str) {
    let result_slot = HEADPHONE_DOWNLOAD_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();

    if let Ok(mut g) = result_slot.lock() {
        *g = None;
    }

    let name = headphone_name.to_string();
    let slot = result_slot.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let result = rt.block_on(async {
            let (csv_path, _) = autoeq::fetch_headphone_frequency_response(&name)
                .await
                .map_err(|e| e.to_string())?;

            Ok::<String, String>(csv_path)
        });
        if let Ok(mut guard) = slot.lock() {
            *guard = Some(result);
        }
    });
}
