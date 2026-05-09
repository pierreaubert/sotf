//! Spinorama EQ wizard event handlers

use super::PlayerCommand;
use crate::app::{App, InputMode};
use crossterm::event::{KeyCode, KeyEvent};
use std::sync::{Arc, Mutex};

#[cfg(test)]
pub(crate) fn spinorama_step_prev(s: crate::app::SpinoramaStep) -> crate::app::SpinoramaStep {
    use crate::app::SpinoramaStep;
    match s {
        SpinoramaStep::Select => SpinoramaStep::Select, // no wrap
        SpinoramaStep::Configure => SpinoramaStep::Select,
        SpinoramaStep::Optimize => SpinoramaStep::Configure,
        SpinoramaStep::Results => SpinoramaStep::Optimize,
        SpinoramaStep::UpdatePlugin => SpinoramaStep::Results,
    }
}

#[cfg(test)]
pub(crate) fn spinorama_step_next(s: crate::app::SpinoramaStep) -> crate::app::SpinoramaStep {
    use crate::app::SpinoramaStep;
    match s {
        SpinoramaStep::Select => SpinoramaStep::Configure,
        SpinoramaStep::Configure => SpinoramaStep::Optimize,
        SpinoramaStep::Optimize => SpinoramaStep::Results,
        SpinoramaStep::Results => SpinoramaStep::UpdatePlugin,
        SpinoramaStep::UpdatePlugin => SpinoramaStep::UpdatePlugin, // no wrap
    }
}

fn spinorama_step_prev_wrap(s: crate::app::SpinoramaStep) -> crate::app::SpinoramaStep {
    use crate::app::SpinoramaStep;
    match s {
        SpinoramaStep::Select => SpinoramaStep::UpdatePlugin,
        SpinoramaStep::Configure => SpinoramaStep::Select,
        SpinoramaStep::Optimize => SpinoramaStep::Configure,
        SpinoramaStep::Results => SpinoramaStep::Optimize,
        SpinoramaStep::UpdatePlugin => SpinoramaStep::Results,
    }
}

fn spinorama_step_next_wrap(s: crate::app::SpinoramaStep) -> crate::app::SpinoramaStep {
    use crate::app::SpinoramaStep;
    match s {
        SpinoramaStep::Select => SpinoramaStep::Configure,
        SpinoramaStep::Configure => SpinoramaStep::Optimize,
        SpinoramaStep::Optimize => SpinoramaStep::Results,
        SpinoramaStep::Results => SpinoramaStep::UpdatePlugin,
        SpinoramaStep::UpdatePlugin => SpinoramaStep::Select,
    }
}

pub fn handle_spinorama_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    use crate::app::SpinoramaStep;
    use sotf_audio_player::room_eq_types::OptimizationStatus;

    // Esc goes up one level within the wizard
    if key.code == KeyCode::Esc {
        // First dismiss numerical direct-edit mode
        if app.spinorama_eq.editing_value {
            app.spinorama_eq.editing_value = false;
            app.spinorama_eq.edit_buffer.clear();
            return None;
        }
        if app.spinorama_eq.step_tab_focused {
            app.spinorama_eq.step_tab_focused = false;
            app.input_mode = InputMode::Configure;
        } else {
            app.spinorama_eq.step_tab_focused = true;
        }
        return None;
    }

    // When the step tab bar has focus, Left/Right change step, Up goes to
    // the top-level configure tab bar, Down/Enter returns to step content.
    if app.spinorama_eq.step_tab_focused {
        match key.code {
            KeyCode::Left | KeyCode::BackTab => {
                app.spinorama_eq.step = spinorama_step_prev_wrap(app.spinorama_eq.step);
                return None;
            }
            KeyCode::Right | KeyCode::Tab => {
                app.spinorama_eq.step = spinorama_step_next_wrap(app.spinorama_eq.step);
                return None;
            }
            KeyCode::Up => {
                app.spinorama_eq.step_tab_focused = false;
                app.input_mode = InputMode::Configure;
                return None;
            }
            KeyCode::Down | KeyCode::Enter => {
                app.spinorama_eq.step_tab_focused = false;
                return None;
            }
            _ => return None,
        }
    }

    match app.spinorama_eq.step {
        SpinoramaStep::Select => match key.code {
            KeyCode::Up => {
                if app.spinorama_eq.selected_speaker_idx > 0 {
                    app.spinorama_eq.selected_speaker_idx -= 1;
                } else {
                    app.spinorama_eq.step_tab_focused = true;
                }
                None
            }
            KeyCode::Down => {
                let max = app.spinorama_eq.filtered_speakers.len().saturating_sub(1);
                if app.spinorama_eq.selected_speaker_idx < max {
                    app.spinorama_eq.selected_speaker_idx += 1;
                }
                None
            }
            KeyCode::Enter => {
                let idx = app.spinorama_eq.selected_speaker_idx;
                if let Some(name) = app.spinorama_eq.filtered_speakers.get(idx).cloned() {
                    app.spinorama_eq.selected_speaker = Some(name);
                    app.spinorama_eq.step = SpinoramaStep::Configure;
                }
                None
            }
            KeyCode::Char('r') => {
                // Retry speaker list load (e.g. after error)
                app.spinorama_eq.speakers_error = None;
                app.spinorama_eq.available_speakers.clear();
                app.spinorama_eq.loading_speakers = true;
                spawn_spinorama_speaker_load();
                None
            }
            KeyCode::Backspace => {
                app.spinorama_eq.search_query.pop();
                app.spinorama_eq.update_filter();
                None
            }
            KeyCode::Char(c) => {
                app.spinorama_eq.search_query.push(c);
                app.spinorama_eq.update_filter();
                None
            }
            _ => None,
        },

        SpinoramaStep::Configure => {
            // Numerical direct-edit mode
            if app.spinorama_eq.editing_value {
                match key.code {
                    KeyCode::Enter => {
                        set_spinorama_field_from_string(app);
                        app.spinorama_eq.editing_value = false;
                        app.spinorama_eq.edit_buffer.clear();
                    }
                    KeyCode::Esc => {
                        app.spinorama_eq.editing_value = false;
                        app.spinorama_eq.edit_buffer.clear();
                    }
                    KeyCode::Backspace => {
                        app.spinorama_eq.edit_buffer.pop();
                    }
                    KeyCode::Char(c) if c.is_ascii_digit() || c == '.' || c == '-' => {
                        app.spinorama_eq.edit_buffer.push(c);
                    }
                    _ => {}
                }
                return None;
            }
            match key.code {
                KeyCode::Up => {
                    if app.spinorama_eq.selected_field > 0 {
                        app.spinorama_eq.selected_field -= 1;
                    } else {
                        app.spinorama_eq.step_tab_focused = true;
                    }
                }
                KeyCode::Down if app.spinorama_eq.selected_field < 24 => {
                    app.spinorama_eq.selected_field += 1;
                }
                KeyCode::Left | KeyCode::Char('-') => {
                    adjust_spinorama_field(app, -1);
                }
                KeyCode::Right | KeyCode::Char('+') => {
                    adjust_spinorama_field(app, 1);
                }
                KeyCode::Tab => {
                    if app.spinorama_eq.selected_field < 24 {
                        app.spinorama_eq.selected_field += 1;
                    } else {
                        app.spinorama_eq.selected_field = 0;
                    }
                }
                KeyCode::Enter => {
                    let f = app.spinorama_eq.selected_field;
                    if is_spinorama_field_numerical(f) {
                        app.spinorama_eq.edit_buffer = spinorama_field_value_string(app, f);
                        app.spinorama_eq.editing_value = true;
                    }
                    // Booleans: toggle
                    else if matches!(f, 15 | 17 | 19) {
                        adjust_spinorama_field(app, 1);
                    }
                }
                KeyCode::BackTab => {
                    app.spinorama_eq.step = SpinoramaStep::Select;
                }
                _ => {}
            }
            None
        }

        SpinoramaStep::Optimize => match key.code {
            KeyCode::Up => {
                app.spinorama_eq.step_tab_focused = true;
                None
            }
            KeyCode::Enter => {
                match &app.spinorama_eq.opt_status {
                    OptimizationStatus::Idle
                    | OptimizationStatus::Failed
                    | OptimizationStatus::Cancelled
                    | OptimizationStatus::Completed => {
                        spawn_spinorama_optimization(app);
                    }
                    OptimizationStatus::Running => {}
                }
                None
            }
            KeyCode::BackTab => {
                app.spinorama_eq.step = SpinoramaStep::Configure;
                None
            }
            _ => None,
        },

        SpinoramaStep::Results => match key.code {
            KeyCode::Up => {
                app.spinorama_eq.step_tab_focused = true;
                None
            }
            KeyCode::BackTab => {
                app.spinorama_eq.step = SpinoramaStep::Optimize;
                None
            }
            _ => None,
        },

        SpinoramaStep::UpdatePlugin => {
            use crate::app::SpinUpdateSubStep;
            match app.spinorama_eq.update_substep {
                SpinUpdateSubStep::Ready => match key.code {
                    KeyCode::Up => {
                        app.spinorama_eq.step_tab_focused = true;
                        None
                    }
                    KeyCode::BackTab => {
                        app.spinorama_eq.step = SpinoramaStep::Results;
                        None
                    }
                    KeyCode::Enter => {
                        // Check if an existing EQ has filters
                        if let Some((slot, count)) = app.find_last_eq_info() {
                            if count > 0 {
                                app.spinorama_eq.update_existing_eq_info = Some((slot, count));
                                app.spinorama_eq.update_substep =
                                    SpinUpdateSubStep::ConfirmOverwrite;
                            } else {
                                // Existing EQ but empty — apply directly
                                match app.apply_spinorama_to_plugins() {
                                    Ok(msg) => app.status_message = Some(msg),
                                    Err(e) => app.status_message = Some(format!("Error: {}", e)),
                                }
                            }
                        } else {
                            // No existing EQ — apply directly (will insert one)
                            match app.apply_spinorama_to_plugins() {
                                Ok(msg) => app.status_message = Some(msg),
                                Err(e) => app.status_message = Some(format!("Error: {}", e)),
                            }
                        }
                        None
                    }
                    _ => None,
                },
                SpinUpdateSubStep::ConfirmOverwrite => match key.code {
                    KeyCode::Char('y') => {
                        // Auto-save preset before overwriting
                        if let Some(presets_dir) =
                            sotf_audio_player::config::get_plugin_presets_dir()
                        {
                            let timestamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
                            let filename = format!("pre-spinorama-{}.json", timestamp);
                            match app.plugin_graph.save_to_file(&presets_dir, &filename) {
                                Ok(_) => {
                                    app.status_message =
                                        Some(format!("Saved backup: {}", filename));
                                    log::info!(
                                        "Auto-saved preset before spinorama overwrite: {}",
                                        filename
                                    );
                                }
                                Err(e) => {
                                    app.status_message = Some(format!("Backup failed: {}", e));
                                    log::error!("Failed to auto-save preset: {}", e);
                                    // Reset and don't apply
                                    app.spinorama_eq.update_substep = SpinUpdateSubStep::Ready;
                                    app.spinorama_eq.update_existing_eq_info = None;
                                    return None;
                                }
                            }
                        }
                        // Apply
                        match app.apply_spinorama_to_plugins() {
                            Ok(msg) => app.status_message = Some(msg),
                            Err(e) => app.status_message = Some(format!("Error: {}", e)),
                        }
                        app.spinorama_eq.update_substep = SpinUpdateSubStep::Ready;
                        app.spinorama_eq.update_existing_eq_info = None;
                        None
                    }
                    KeyCode::Char('n') => {
                        // Apply without saving
                        match app.apply_spinorama_to_plugins() {
                            Ok(msg) => app.status_message = Some(msg),
                            Err(e) => app.status_message = Some(format!("Error: {}", e)),
                        }
                        app.spinorama_eq.update_substep = SpinUpdateSubStep::Ready;
                        app.spinorama_eq.update_existing_eq_info = None;
                        None
                    }
                    KeyCode::Esc => {
                        app.spinorama_eq.update_substep = SpinUpdateSubStep::Ready;
                        app.spinorama_eq.update_existing_eq_info = None;
                        None
                    }
                    _ => None,
                },
            }
        }
    }
}

fn is_spinorama_field_numerical(field: usize) -> bool {
    matches!(field, 1..=7 | 10 | 11 | 13 | 14 | 18 | 20 | 21 | 22 | 23)
}

fn spinorama_field_value_string(app: &App, field: usize) -> String {
    let c = &app.spinorama_eq.config;
    match field {
        1 => c.num_filters.to_string(),
        2 => format!("{:.0}", c.min_freq),
        3 => format!("{:.0}", c.max_freq),
        4 => format!("{:.1}", c.min_db),
        5 => format!("{:.1}", c.max_db),
        6 => format!("{:.1}", c.min_q),
        7 => format!("{:.1}", c.max_q),
        10 => c.max_iter.to_string(),
        11 => c.population.to_string(),
        13 => format!("{:.1}", c.de_f),
        14 => format!("{:.1}", c.de_cr),
        18 => c.smooth_n.to_string(),
        20 => format!("{:.0}", c.spacing_weight),
        21 => format!("{:.2}", c.min_spacing_oct),
        22 => format!("{:.6}", c.tolerance),
        23 => format!("{:.6}", c.atolerance),
        _ => String::new(),
    }
}

fn set_spinorama_field_from_string(app: &mut App) {
    let c = &mut app.spinorama_eq.config;
    let buf = &app.spinorama_eq.edit_buffer;
    match app.spinorama_eq.selected_field {
        1 => {
            if let Ok(v) = buf.parse::<usize>() {
                c.num_filters = v.clamp(1, 30);
            }
        }
        2 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.min_freq = v.clamp(20.0, 500.0);
            }
        }
        3 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.max_freq = v.clamp(1000.0, 20000.0);
            }
        }
        4 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.min_db = v.clamp(-24.0, 0.0);
            }
        }
        5 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.max_db = v.clamp(0.0, 12.0);
            }
        }
        6 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.min_q = v.clamp(0.1, 2.0);
            }
        }
        7 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.max_q = v.clamp(1.0, 20.0);
            }
        }
        10 => {
            if let Ok(v) = buf.parse::<usize>() {
                c.max_iter = v.clamp(1000, 100000);
            }
        }
        11 => {
            if let Ok(v) = buf.parse::<usize>() {
                c.population = v.clamp(10, 200);
            }
        }
        13 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.de_f = v.clamp(0.1, 2.0);
            }
        }
        14 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.de_cr = v.clamp(0.1, 1.0);
            }
        }
        18 => {
            if let Ok(v) = buf.parse::<usize>() {
                c.smooth_n = v.clamp(1, 24);
            }
        }
        20 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.spacing_weight = v.clamp(0.0, 1000.0);
            }
        }
        21 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.min_spacing_oct = v.clamp(0.01, 1.0);
            }
        }
        22 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.tolerance = v.clamp(1e-6, 1e-1);
            }
        }
        23 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.atolerance = v.clamp(1e-6, 1e-1);
            }
        }
        _ => {}
    }
}

fn adjust_spinorama_field(app: &mut App, delta: i32) {
    let c = &mut app.spinorama_eq.config;
    match app.spinorama_eq.selected_field {
        // ── Loss ──
        0 => {
            c.loss_function = super::cycle_string(
                &c.loss_function,
                &["flat", "flat-asymmetric", "score"],
                delta,
            );
        }
        // ── Filters ──
        1 => {
            let n = c.num_filters as i32 + delta;
            c.num_filters = n.clamp(1, 30) as usize;
        }
        2 => c.min_freq = (c.min_freq + delta as f64 * 10.0).clamp(20.0, 500.0),
        3 => c.max_freq = (c.max_freq + delta as f64 * 500.0).clamp(1000.0, 20000.0),
        4 => c.min_db = (c.min_db + delta as f64).clamp(-24.0, 0.0),
        5 => c.max_db = (c.max_db + delta as f64).clamp(0.0, 12.0),
        6 => c.min_q = (c.min_q + delta as f64 * 0.1).clamp(0.1, 2.0),
        7 => c.max_q = (c.max_q + delta as f64 * 0.5).clamp(1.0, 20.0),
        8 => {
            c.peq_model = super::cycle_string(
                &c.peq_model,
                &["pk", "hp-pk", "hp-pk-lp", "ls-pk", "ls-pk-hs"],
                delta,
            );
        }
        // ── Optimization ──
        9 => {
            use sotf_audio_player::room_eq_types::RoomEqAlgorithm;
            let algos = RoomEqAlgorithm::all();
            let idx = algos.iter().position(|a| *a == c.algorithm).unwrap_or(0);
            let new_idx = if delta > 0 {
                (idx + 1) % algos.len()
            } else {
                (idx + algos.len() - 1) % algos.len()
            };
            c.algorithm = algos[new_idx];
        }
        10 => {
            let n = c.max_iter as i32 + delta * 1000;
            c.max_iter = n.clamp(1000, 100000) as usize;
        }
        11 => {
            let n = c.population as i32 + delta * 10;
            c.population = n.clamp(10, 200) as usize;
        }
        12 => {
            c.strategy = super::cycle_string(
                &c.strategy,
                &["currenttobest1bin", "best1bin", "rand1bin", "best2bin"],
                delta,
            );
        }
        13 => c.de_f = (c.de_f + delta as f64 * 0.1).clamp(0.1, 2.0),
        14 => c.de_cr = (c.de_cr + delta as f64 * 0.1).clamp(0.1, 1.0),
        // ── Refinement ──
        15 => c.refine = !c.refine,
        16 => {
            c.local_algo = super::cycle_string(&c.local_algo, &["cobyla"], delta);
        }
        // ── Smoothing ──
        17 => c.smooth = !c.smooth,
        18 => {
            let n = c.smooth_n as i32 + delta;
            c.smooth_n = n.clamp(1, 24) as usize;
        }
        19 => c.psychoacoustic = !c.psychoacoustic,
        // ── Constraints ──
        20 => c.spacing_weight = (c.spacing_weight + delta as f64 * 10.0).clamp(0.0, 1000.0),
        21 => c.min_spacing_oct = (c.min_spacing_oct + delta as f64 * 0.01).clamp(0.01, 1.0),
        // ── Convergence ──
        22 => {
            c.tolerance = if delta > 0 {
                (c.tolerance * 10.0).min(1e-1)
            } else {
                (c.tolerance / 10.0).max(1e-6)
            };
        }
        23 => {
            c.atolerance = if delta > 0 {
                (c.atolerance * 10.0).min(1e-1)
            } else {
                (c.atolerance / 10.0).max(1e-6)
            };
        }
        24 => {
            c.sample_rate = match (c.sample_rate, delta > 0) {
                (44100, true) => 48000,
                (48000, true) => 96000,
                (96000, true) => 44100,
                (96000, false) => 48000,
                (48000, false) => 44100,
                (44100, false) => 96000,
                _ => 48000,
            };
        }
        _ => {}
    }
}

#[allow(clippy::type_complexity)]
static SPEAKERS_RESULT: std::sync::OnceLock<Arc<Mutex<Option<Result<Vec<String>, String>>>>> =
    std::sync::OnceLock::new();

#[allow(clippy::type_complexity)]
static OPT_RESULT: std::sync::OnceLock<
    Arc<Mutex<Option<Result<sotf_audio_player::autoeq::SpeakerOptimizationResult, String>>>>,
> = std::sync::OnceLock::new();
#[allow(clippy::type_complexity)]
static OPT_PROGRESS: std::sync::OnceLock<
    Arc<Mutex<Option<(usize, usize, f64, f32, Option<f64>)>>>,
> = std::sync::OnceLock::new();

/// Poll speaker-load result on every tick. Returns true if the UI needs a redraw.
/// Also auto-triggers speaker list loading when entering the Select step.
pub fn poll_spinorama_speaker_load(app: &mut App) -> bool {
    use crate::app::{ConfigureSubScreen, Screen, SpinoramaStep};

    // Auto-load speakers when on Select step with empty list
    if !app.spinorama_eq.loading_speakers
        && app.spinorama_eq.available_speakers.is_empty()
        && app.spinorama_eq.speakers_error.is_none()
        && app.current_screen == Screen::Configure
        && app.configure_sub_screen == ConfigureSubScreen::SpinoramaEq
        && app.spinorama_eq.step == SpinoramaStep::Select
    {
        app.spinorama_eq.loading_speakers = true;
        spawn_spinorama_speaker_load();
    }

    if !app.spinorama_eq.loading_speakers {
        return false;
    }
    let result_slot = SPEAKERS_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();
    if let Ok(mut guard) = result_slot.lock()
        && let Some(result) = guard.take()
    {
        app.spinorama_eq.loading_speakers = false;
        match result {
            Ok(speakers) => {
                app.spinorama_eq.available_speakers = speakers;
                app.spinorama_eq.update_filter();
            }
            Err(e) => {
                app.spinorama_eq.speakers_error = Some(e);
            }
        }
        return true;
    }
    false
}

/// Kick off a spinorama speaker-catalog fetch if one isn't already
/// in flight and the cache is empty. Used by the Recording wizard's
/// save step to pre-warm the per-channel autocomplete without having
/// to visit the spinorama EQ screen first.
pub(crate) fn ensure_spinorama_speakers_loading(app: &mut App) {
    if app.spinorama_eq.loading_speakers
        || !app.spinorama_eq.available_speakers.is_empty()
        || app.spinorama_eq.speakers_error.is_some()
    {
        return;
    }
    app.spinorama_eq.loading_speakers = true;
    spawn_spinorama_speaker_load();
}

fn spawn_spinorama_speaker_load() {
    let result_slot = SPEAKERS_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();

    // Clear any stale result from a previous load
    if let Ok(mut g) = result_slot.lock() {
        *g = None;
    }

    // Spawn background thread
    let slot = result_slot.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let result = rt
            .block_on(async { autoeq::fetch_available_speakers().await })
            .map_err(|e| e.to_string());
        if let Ok(mut guard) = slot.lock() {
            *guard = Some(result);
        }
    });
}

/// Poll optimization progress/result on every tick while optimization is running.
/// Returns true if the UI needs a redraw.
pub fn poll_spinorama_optimization(app: &mut App) -> bool {
    use sotf_audio_player::room_eq_types::OptimizationStatus;
    use sotf_audio_player::spinorama_eq_types::SpinoramaBiquad;

    if app.spinorama_eq.opt_status != OptimizationStatus::Running {
        return false;
    }

    let result_slot = OPT_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();
    let progress_slot = OPT_PROGRESS
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();

    if let Ok(mut guard) = result_slot.lock()
        && let Some(result) = guard.take()
    {
        match result {
            Ok(r) => {
                app.spinorama_eq.pre_loss = r.initial_loss;
                app.spinorama_eq.post_loss = r.final_loss;
                app.spinorama_eq.filters = r
                    .biquads
                    .iter()
                    .map(|b| SpinoramaBiquad {
                        filter_type: format!("{:?}", b.filter_type),
                        freq: b.freq,
                        q: b.q,
                        db_gain: b.db_gain,
                    })
                    .collect();
                app.spinorama_eq.curve_frequencies = r.frequencies.clone();
                app.spinorama_eq.curve_input = r.input_curve.clone();
                app.spinorama_eq.curve_target = r.target_curve.clone();
                app.spinorama_eq.curve_corrected = r.corrected_curve.clone();
                app.spinorama_eq.curve_filter_response = r.filter_response.clone();
                // Keep the progress-based loss_history (which includes scores)
                // Only override if empty (e.g. if the callback wasn't called)
                if app.spinorama_eq.loss_history.is_empty() {
                    app.spinorama_eq.loss_history = r
                        .optimization_history
                        .iter()
                        .map(|(iter, loss)| (*iter, *loss, None))
                        .collect();
                }
                app.spinorama_eq.opt_status = OptimizationStatus::Completed;
                app.spinorama_eq.opt_progress = 1.0;
            }
            Err(e) => {
                app.spinorama_eq.opt_status = OptimizationStatus::Failed;
                app.spinorama_eq.opt_error = Some(e);
            }
        }
        return true;
    }

    if let Ok(mut guard) = progress_slot.lock()
        && let Some((iter, max_iter, loss, pct, score)) = guard.take()
    {
        app.spinorama_eq.opt_iteration = iter;
        app.spinorama_eq.opt_max_iter = max_iter;
        app.spinorama_eq.opt_loss = loss;
        app.spinorama_eq.opt_progress = pct;
        app.spinorama_eq.loss_history.push((iter, loss, score));
        return true;
    }

    false
}

fn spawn_spinorama_optimization(app: &mut App) {
    use sotf_audio_player::room_eq_types::OptimizationStatus;

    // Start new optimization
    let speaker = match &app.spinorama_eq.selected_speaker {
        Some(s) => s.clone(),
        None => {
            app.spinorama_eq.opt_status = OptimizationStatus::Failed;
            app.spinorama_eq.opt_error = Some("No speaker selected".to_string());
            return;
        }
    };

    app.spinorama_eq.opt_status = OptimizationStatus::Running;
    app.spinorama_eq.opt_error = None;
    app.spinorama_eq.opt_progress = 0.0;
    app.spinorama_eq.opt_iteration = 0;
    app.spinorama_eq.opt_loss = 0.0;
    app.spinorama_eq.filters.clear();

    let c = &app.spinorama_eq.config;
    let num_filters = c.num_filters;
    let min_freq = c.min_freq;
    let max_freq = c.max_freq;
    let min_db = c.min_db;
    let max_db = c.max_db;
    let min_q = c.min_q;
    let max_q = c.max_q;
    let max_iter = c.max_iter;
    let peq_model_str = c.peq_model.clone();
    let algorithm = c.algorithm;
    let population = c.population;
    let strategy = c.strategy.clone();
    let de_f = c.de_f;
    let de_cr = c.de_cr;
    let refine = c.refine;
    let local_algo = c.local_algo.clone();
    let smooth = c.smooth;
    let smooth_n = c.smooth_n;
    let psychoacoustic = c.psychoacoustic;
    let spacing_weight = c.spacing_weight;
    let min_spacing_oct = c.min_spacing_oct;
    let loss_function = c.loss_function.clone();
    let tolerance = c.tolerance;
    let atolerance = c.atolerance;
    let sample_rate = c.sample_rate;

    let result_slot2 = OPT_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();
    let progress_slot2 = OPT_PROGRESS
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();

    // Clear any stale result from a previous run
    if let Ok(mut g) = result_slot2.lock() {
        *g = None;
    }
    if let Ok(mut g) = progress_slot2.lock() {
        *g = None;
    }

    std::thread::spawn(move || {
        use sotf_audio_player::autoeq::{
            CallbackAction, CallbackConfig, MeasurementInput, SpeakerOptimizationConfig,
            run_speaker_optimization_with_callback,
        };

        let mut args = autoeq::Args::speaker_defaults();
        args.num_filters = num_filters;
        args.min_freq = min_freq;
        args.max_freq = max_freq;
        args.min_db = min_db;
        args.max_db = max_db;
        args.min_q = min_q;
        args.max_q = max_q;
        args.maxeval = max_iter;
        args.sample_rate = sample_rate as f64;
        args.population = population;
        args.strategy = strategy;
        args.adaptive_weight_f = de_f;
        args.recombination = de_cr;
        args.refine = refine;
        args.local_algo = local_algo;
        args.smooth = smooth;
        args.smooth_n = smooth_n;
        args.spacing_weight = spacing_weight;
        args.min_spacing_oct = min_spacing_oct;
        args.tolerance = tolerance;
        args.atolerance = atolerance;
        // Map algorithm enum to autoeq algo format
        args.algo = algorithm.to_autoeq_string().to_string();
        // Map PEQ model string to enum
        args.peq_model = match peq_model_str.as_str() {
            "pk" => autoeq::PeqModel::Pk,
            "hp-pk" => autoeq::PeqModel::HpPk,
            "hp-pk-lp" => autoeq::PeqModel::HpPkLp,
            "ls-pk" => autoeq::PeqModel::LsPk,
            "ls-pk-hs" => autoeq::PeqModel::LsPkHs,
            _ => autoeq::PeqModel::Pk,
        };
        // Map loss function string to LossType enum
        args.loss = match loss_function.as_str() {
            "flat" => autoeq::LossType::SpeakerFlat,
            "flat-asymmetric" => autoeq::LossType::SpeakerFlatAsymmetric,
            "score" => autoeq::LossType::SpeakerScore,
            other => panic!("Unknown loss function: {}", other),
        };
        // Psychoacoustic smoothing not directly on Args — handled via smooth settings
        let _ = psychoacoustic; // TODO: map when autoeq supports it directly

        let config = SpeakerOptimizationConfig {
            main_measurement: Some(MeasurementInput::Spinorama {
                speaker: speaker.clone(),
                version: "asr".to_string(),
                measurement: "CEA2034".to_string(),
                curve_name: args.curve_name.clone(),
            }),
            args,
            callback_config: Some(CallbackConfig {
                interval: 50,
                include_biquads: false,
                include_filter_response: false,
            }),
            ..Default::default()
        };

        let progress_slot3 = progress_slot2.clone();
        let callback: sotf_audio_player::autoeq::SpeakerOptimizationCallback = Box::new(move |p| {
            let pct = if p.max_iterations > 0 {
                p.iteration as f32 / p.max_iterations as f32
            } else {
                0.0
            };
            if let Ok(mut guard) = progress_slot3.lock() {
                *guard = Some((p.iteration, p.max_iterations, p.loss, pct, p.score));
            }
            CallbackAction::Continue
        });

        let result = run_speaker_optimization_with_callback(&config, Some(callback));
        if let Ok(mut guard) = result_slot2.lock() {
            *guard = Some(result);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::SpinoramaStep;

    #[test]
    fn spinorama_step_prev_does_not_wrap() {
        assert_eq!(
            spinorama_step_prev(SpinoramaStep::Select),
            SpinoramaStep::Select,
        );
    }

    #[test]
    fn spinorama_step_next_does_not_wrap() {
        assert_eq!(
            spinorama_step_next(SpinoramaStep::UpdatePlugin),
            SpinoramaStep::UpdatePlugin,
        );
    }

    #[test]
    fn spinorama_step_round_trip() {
        let steps = [
            SpinoramaStep::Select,
            SpinoramaStep::Configure,
            SpinoramaStep::Optimize,
            SpinoramaStep::Results,
            SpinoramaStep::UpdatePlugin,
        ];
        for i in 0..steps.len() - 1 {
            assert_eq!(spinorama_step_next(steps[i]), steps[i + 1]);
            assert_eq!(spinorama_step_prev(steps[i + 1]), steps[i]);
        }
    }
}
