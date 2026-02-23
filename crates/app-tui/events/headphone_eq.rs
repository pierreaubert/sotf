//! Headphone EQ wizard event handlers

use super::PlayerCommand;
use crate::app::App;
use crossterm::event::{KeyCode, KeyEvent};
use std::sync::{Arc, Mutex};

pub(crate) fn headphone_eq_step_prev(s: crate::app::HeadphoneEqStep) -> crate::app::HeadphoneEqStep {
    use crate::app::HeadphoneEqStep;
    match s {
        HeadphoneEqStep::SelectFile => HeadphoneEqStep::SelectFile, // no wrap
        HeadphoneEqStep::Configure  => HeadphoneEqStep::SelectFile,
        HeadphoneEqStep::Optimize   => HeadphoneEqStep::Configure,
        HeadphoneEqStep::Results    => HeadphoneEqStep::Optimize,
    }
}

pub(crate) fn headphone_eq_step_next(s: crate::app::HeadphoneEqStep) -> crate::app::HeadphoneEqStep {
    use crate::app::HeadphoneEqStep;
    match s {
        HeadphoneEqStep::SelectFile => HeadphoneEqStep::Configure,
        HeadphoneEqStep::Configure  => HeadphoneEqStep::Optimize,
        HeadphoneEqStep::Optimize   => HeadphoneEqStep::Results,
        HeadphoneEqStep::Results    => HeadphoneEqStep::Results, // no wrap
    }
}

pub fn handle_headphone_eq_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    use crate::app::{HeadphoneEqStep, HEADPHONE_TARGET_PRESETS};
    use sotf_audio_player::room_eq_types::OptimizationStatus;

    // Esc goes up one level
    if key.code == KeyCode::Esc {
        match app.headphone_eq.step {
            HeadphoneEqStep::SelectFile => {
                if app.headphone_eq.editing_measurement {
                    app.headphone_eq.editing_measurement = false;
                } else if app.headphone_eq.editing_custom_target {
                    app.headphone_eq.editing_custom_target = false;
                } else {
                    app.configure_tab_focused = true;
                }
            }
            _ => {
                app.headphone_eq.step = headphone_eq_step_prev(app.headphone_eq.step);
            }
        }
        return None;
    }

    // Step navigation via Left/Right (except when editing text or in Configure step)
    let editing = app.headphone_eq.editing_measurement || app.headphone_eq.editing_custom_target;
    if !editing && app.headphone_eq.step != HeadphoneEqStep::Configure {
        if key.code == KeyCode::Left {
            app.headphone_eq.step = headphone_eq_step_prev(app.headphone_eq.step);
            return None;
        }
        if key.code == KeyCode::Right {
            app.headphone_eq.step = headphone_eq_step_next(app.headphone_eq.step);
            return None;
        }
    }

    match app.headphone_eq.step {
        HeadphoneEqStep::SelectFile => {
            if app.headphone_eq.editing_measurement {
                match key.code {
                    KeyCode::Enter => { app.headphone_eq.editing_measurement = false; }
                    KeyCode::Backspace => { app.headphone_eq.measurement_path.pop(); }
                    KeyCode::Char(c) => { app.headphone_eq.measurement_path.push(c); }
                    _ => {}
                }
                return None;
            }
            if app.headphone_eq.editing_custom_target {
                match key.code {
                    KeyCode::Enter => { app.headphone_eq.editing_custom_target = false; }
                    KeyCode::Backspace => { app.headphone_eq.custom_target_path.pop(); }
                    KeyCode::Char(c) => { app.headphone_eq.custom_target_path.push(c); }
                    _ => {}
                }
                return None;
            }
            match key.code {
                KeyCode::Up => {
                    if app.headphone_eq.selected_field > 0 {
                        app.headphone_eq.selected_field -= 1;
                    } else {
                        app.configure_tab_focused = true;
                    }
                }
                KeyCode::Down => {
                    let max = if app.headphone_eq.target_preset == "custom" { 2 } else { 1 };
                    if app.headphone_eq.selected_field < max {
                        app.headphone_eq.selected_field += 1;
                    }
                }
                KeyCode::Enter => {
                    match app.headphone_eq.selected_field {
                        0 => { app.headphone_eq.editing_measurement = true; }
                        1 => {} // target preset cycles with Left/Right
                        2 => { app.headphone_eq.editing_custom_target = true; }
                        _ => {}
                    }
                }
                KeyCode::Left | KeyCode::Right => {
                    if app.headphone_eq.selected_field == 1 {
                        let idx = HEADPHONE_TARGET_PRESETS.iter().position(|&p| p == &app.headphone_eq.target_preset).unwrap_or(0);
                        let new_idx = if key.code == KeyCode::Left {
                            (idx + HEADPHONE_TARGET_PRESETS.len() - 1) % HEADPHONE_TARGET_PRESETS.len()
                        } else {
                            (idx + 1) % HEADPHONE_TARGET_PRESETS.len()
                        };
                        app.headphone_eq.target_preset = HEADPHONE_TARGET_PRESETS[new_idx].to_string();
                    }
                }
                KeyCode::Tab => {
                    if !app.headphone_eq.measurement_path.is_empty() {
                        app.headphone_eq.step = HeadphoneEqStep::Configure;
                    }
                }
                _ => {}
            }
            None
        }

        HeadphoneEqStep::Configure => match key.code {
            KeyCode::Up => {
                if app.headphone_eq.selected_field > 0 {
                    app.headphone_eq.selected_field -= 1;
                } else {
                    app.configure_tab_focused = true;
                }
            }
            KeyCode::Down => {
                let max = if app.headphone_eq.target_preset == "custom" { 12 } else { 10 };
                if app.headphone_eq.selected_field < max {
                    app.headphone_eq.selected_field += 1;
                }
            }
            KeyCode::Left | KeyCode::Char('-') => {
                adjust_headphone_eq_field(app, -1);
            }
            KeyCode::Right | KeyCode::Char('+') => {
                adjust_headphone_eq_field(app, 1);
            }
            KeyCode::Enter | KeyCode::Tab => {
                app.headphone_eq.step = HeadphoneEqStep::Optimize;
            }
            KeyCode::BackTab => {
                app.headphone_eq.step = HeadphoneEqStep::SelectFile;
            }
            _ => {}
        },

        HeadphoneEqStep::Optimize => match key.code {
            KeyCode::Enter => {
                match &app.headphone_eq.opt_status {
                    OptimizationStatus::Idle | OptimizationStatus::Failed | OptimizationStatus::Cancelled => {
                        spawn_headphone_eq_optimization(app);
                    }
                    OptimizationStatus::Completed => {
                        app.headphone_eq.step = HeadphoneEqStep::Results;
                    }
                    OptimizationStatus::Running => {}
                }
            }
            KeyCode::Tab => {
                if app.headphone_eq.opt_status == OptimizationStatus::Completed {
                    app.headphone_eq.step = HeadphoneEqStep::Results;
                } else {
                    app.headphone_eq.step = HeadphoneEqStep::Configure;
                }
            }
            KeyCode::BackTab => {
                app.headphone_eq.step = HeadphoneEqStep::Configure;
            }
            _ => {}
        },

        HeadphoneEqStep::Results => match key.code {
            KeyCode::Tab => {
                app.headphone_eq.step = HeadphoneEqStep::SelectFile;
            }
            KeyCode::BackTab => {
                app.headphone_eq.step = HeadphoneEqStep::Optimize;
            }
            _ => {}
        },
    }

    None
}

fn adjust_headphone_eq_field(app: &mut App, delta: i32) {
    let c = &mut app.headphone_eq.config;
    let is_custom = app.headphone_eq.target_preset == "custom";

    match app.headphone_eq.selected_field {
        0 => c.num_filters = ((c.num_filters as i32 + delta).max(1).min(30)) as usize,
        1 => c.min_freq = (c.min_freq + delta as f64 * 10.0).clamp(20.0, 500.0),
        2 => c.max_freq = (c.max_freq + delta as f64 * 500.0).clamp(1000.0, 20000.0),
        3 => c.min_db = (c.min_db + delta as f64).clamp(-24.0, 0.0),
        4 => c.max_db = (c.max_db + delta as f64).clamp(0.0, 12.0),
        5 => c.min_q = (c.min_q + delta as f64 * 0.1).clamp(0.1, 2.0),
        6 => c.max_q = (c.max_q + delta as f64 * 0.5).clamp(1.0, 20.0),
        7 => c.smooth = !c.smooth,
        8 => {
            let n = c.smooth_n as i32 + delta;
            c.smooth_n = n.clamp(1, 24) as usize;
        }
        9 => c.window = if delta > 0 { "hann" } else { "blackman" }.to_string(),
        10 => c.preamp = (c.preamp + delta as f64).clamp(-24.0, 24.0),
        // Custom target fields
        11 => c.bass_boost = (c.bass_boost + delta as f64).clamp(0.0, 20.0),
        12 => c.treble_boost = (c.treble_boost + delta as f64).clamp(-6.0, 6.0),
        _ => {}
    }
}

static OPT_RESULT: std::sync::OnceLock<
    Arc<Mutex<Option<Result<Vec<sotf_audio_player::headphone_eq_types::HeadphoneBiquad>, String>>>>,
> = std::sync::OnceLock::new();
static OPT_PROGRESS: std::sync::OnceLock<Arc<Mutex<Option<(usize, usize, f64, f32)>>>> =
    std::sync::OnceLock::new();

/// Poll optimization progress/result on every tick while optimization is running.
/// Returns true if the UI needs a redraw.
pub fn poll_headphone_eq_optimization(app: &mut App) -> bool {
    use sotf_audio_player::room_eq_types::OptimizationStatus;

    if app.headphone_eq.opt_status != OptimizationStatus::Running {
        return false;
    }

    let result_slot = OPT_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();
    let progress_slot = OPT_PROGRESS
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();

    if let Ok(mut guard) = result_slot.lock() {
        if let Some(result) = guard.take() {
            match result {
                Ok(biquads) => {
                    app.headphone_eq.filters = biquads;
                    app.headphone_eq.opt_status = OptimizationStatus::Completed;
                    app.headphone_eq.opt_progress = 1.0;
                }
                Err(e) => {
                    app.headphone_eq.opt_status = OptimizationStatus::Failed;
                    app.headphone_eq.opt_error = Some(e);
                }
            }
            return true;
        }
    }

    if let Ok(mut guard) = progress_slot.lock() {
        if let Some((iter, max_iter, loss, pct)) = guard.take() {
            app.headphone_eq.opt_iteration = iter;
            app.headphone_eq.opt_max_iter = max_iter;
            app.headphone_eq.opt_loss = loss;
            app.headphone_eq.opt_progress = pct;
            return true;
        }
    }

    false
}

fn spawn_headphone_eq_optimization(app: &mut App) {
    use sotf_audio_player::room_eq_types::OptimizationStatus;

    app.headphone_eq.opt_status = OptimizationStatus::Running;
    app.headphone_eq.opt_error = None;
    app.headphone_eq.opt_progress = 0.0;
    app.headphone_eq.opt_iteration = 0;
    app.headphone_eq.opt_max_iter = 0;
    app.headphone_eq.opt_loss = 0.0;

    let curve_path = app.headphone_eq.measurement_path.clone();
    let target = app.headphone_eq.target_preset.clone();
    let custom_target = app.headphone_eq.custom_target_path.clone();
    let args = app.headphone_eq.config.clone();

    let result_slot = OPT_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();

    // Clear stale result
    if let Ok(mut g) = result_slot.lock() { *g = None; }

    std::thread::spawn(move || {
        let result = sotf_audio_player::autoeq::headphone::run_headphone_optimization(
            &curve_path,
            &target,
            &custom_target,
            &args,
            "json",
        );
        if let Ok(mut guard) = result_slot.lock() {
            *guard = Some(result);
        }
    });
}
