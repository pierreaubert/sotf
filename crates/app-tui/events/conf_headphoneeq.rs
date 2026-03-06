//! Headphone EQ wizard event handlers

use super::PlayerCommand;
use crate::app::{App, FilePickerMode, FilePickerOrigin, InputMode};
use crossterm::event::{KeyCode, KeyEvent};
use std::sync::{Arc, Mutex};

#[cfg(test)]
pub(crate) fn headphone_eq_step_prev(
    s: crate::app::HeadphoneEqStep,
) -> crate::app::HeadphoneEqStep {
    use crate::app::HeadphoneEqStep;
    match s {
        HeadphoneEqStep::SelectFile => HeadphoneEqStep::SelectFile, // no wrap
        HeadphoneEqStep::Configure => HeadphoneEqStep::SelectFile,
        HeadphoneEqStep::Optimize => HeadphoneEqStep::Configure,
        HeadphoneEqStep::Results => HeadphoneEqStep::Optimize,
        HeadphoneEqStep::UpdatePlugin => HeadphoneEqStep::Results,
    }
}

#[cfg(test)]
pub(crate) fn headphone_eq_step_next(
    s: crate::app::HeadphoneEqStep,
) -> crate::app::HeadphoneEqStep {
    use crate::app::HeadphoneEqStep;
    match s {
        HeadphoneEqStep::SelectFile => HeadphoneEqStep::Configure,
        HeadphoneEqStep::Configure => HeadphoneEqStep::Optimize,
        HeadphoneEqStep::Optimize => HeadphoneEqStep::Results,
        HeadphoneEqStep::Results => HeadphoneEqStep::UpdatePlugin,
        HeadphoneEqStep::UpdatePlugin => HeadphoneEqStep::UpdatePlugin, // no wrap
    }
}

fn headphone_eq_step_prev_wrap(s: crate::app::HeadphoneEqStep) -> crate::app::HeadphoneEqStep {
    use crate::app::HeadphoneEqStep;
    match s {
        HeadphoneEqStep::SelectFile => HeadphoneEqStep::UpdatePlugin,
        HeadphoneEqStep::Configure => HeadphoneEqStep::SelectFile,
        HeadphoneEqStep::Optimize => HeadphoneEqStep::Configure,
        HeadphoneEqStep::Results => HeadphoneEqStep::Optimize,
        HeadphoneEqStep::UpdatePlugin => HeadphoneEqStep::Results,
    }
}

fn headphone_eq_step_next_wrap(s: crate::app::HeadphoneEqStep) -> crate::app::HeadphoneEqStep {
    use crate::app::HeadphoneEqStep;
    match s {
        HeadphoneEqStep::SelectFile => HeadphoneEqStep::Configure,
        HeadphoneEqStep::Configure => HeadphoneEqStep::Optimize,
        HeadphoneEqStep::Optimize => HeadphoneEqStep::Results,
        HeadphoneEqStep::Results => HeadphoneEqStep::UpdatePlugin,
        HeadphoneEqStep::UpdatePlugin => HeadphoneEqStep::SelectFile,
    }
}

pub fn handle_headphone_eq_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    use crate::app::{HEADPHONE_TARGET_PRESETS, HeadphoneEqStep, SpinUpdateSubStep};
    use sotf_audio_player::room_eq_types::OptimizationStatus;

    // Esc: exit editing if active, then two-level focus (content → step tab → configure tab)
    if key.code == KeyCode::Esc {
        // First dismiss numerical direct-edit mode
        if app.headphone_eq.editing_value {
            app.headphone_eq.editing_value = false;
            app.headphone_eq.edit_buffer.clear();
            return None;
        }
        // Then dismiss text editing
        if app.headphone_eq.editing_measurement {
            app.headphone_eq.editing_measurement = false;
            app.clear_autocomplete();
            return None;
        }
        if app.headphone_eq.editing_custom_target {
            app.headphone_eq.editing_custom_target = false;
            app.clear_autocomplete();
            return None;
        }
        // UpdatePlugin confirm substep has its own Esc handling
        if app.headphone_eq.step == HeadphoneEqStep::UpdatePlugin
            && app.headphone_eq.update_substep == SpinUpdateSubStep::ConfirmOverwrite
        {
            app.headphone_eq.update_substep = SpinUpdateSubStep::Ready;
            app.headphone_eq.update_existing_eq_info = None;
            return None;
        }
        // Two-level: content → step tab bar → configure tab bar
        if app.headphone_eq.step_tab_focused {
            app.headphone_eq.step_tab_focused = false;
            app.input_mode = InputMode::Configure;
        } else {
            app.headphone_eq.step_tab_focused = true;
        }
        return None;
    }

    // When the step tab bar has focus, Left/Right change step, Up goes to
    // the top-level configure tab bar, Down/Enter returns to step content.
    if app.headphone_eq.step_tab_focused {
        match key.code {
            KeyCode::Left | KeyCode::BackTab => {
                app.headphone_eq.step = headphone_eq_step_prev_wrap(app.headphone_eq.step);
                return None;
            }
            KeyCode::Right | KeyCode::Tab => {
                app.headphone_eq.step = headphone_eq_step_next_wrap(app.headphone_eq.step);
                return None;
            }
            KeyCode::Up => {
                app.headphone_eq.step_tab_focused = false;
                app.input_mode = InputMode::Configure;
                return None;
            }
            KeyCode::Down | KeyCode::Enter => {
                app.headphone_eq.step_tab_focused = false;
                // Auto-start optimization when entering the Optimize step content
                if app.headphone_eq.step == HeadphoneEqStep::Optimize
                    && app.headphone_eq.opt_status == OptimizationStatus::Idle
                {
                    spawn_headphone_eq_optimization(app);
                }
                return None;
            }
            _ => return None,
        }
    }

    match app.headphone_eq.step {
        HeadphoneEqStep::SelectFile => {
            if app.headphone_eq.editing_measurement {
                match key.code {
                    KeyCode::Enter => {
                        app.headphone_eq.editing_measurement = false;
                        app.clear_autocomplete();
                    }
                    KeyCode::Tab => {
                        app.zsh_tab_complete(
                            crate::app::app_autocomplete::get_headphone_measurement_path,
                            crate::app::app_autocomplete::set_headphone_measurement_path,
                            crate::app::app_autocomplete::AutocompleteKind::FilePath,
                        );
                    }
                    KeyCode::BackTab => {
                        app.zsh_backtab_complete(
                            crate::app::app_autocomplete::set_headphone_measurement_path,
                        );
                    }
                    KeyCode::Down => {
                        app.autocomplete_down(
                            crate::app::app_autocomplete::set_headphone_measurement_path,
                        );
                    }
                    KeyCode::Up => {
                        app.autocomplete_up(
                            crate::app::app_autocomplete::set_headphone_measurement_path,
                        );
                    }
                    KeyCode::Backspace => {
                        app.headphone_eq.measurement_path.pop();
                        app.refresh_autocomplete_inline(
                            crate::app::app_autocomplete::get_headphone_measurement_path,
                            crate::app::app_autocomplete::AutocompleteKind::FilePath,
                        );
                    }
                    KeyCode::F(2) => {
                        let start = app.headphone_eq.measurement_path.clone();
                        app.open_file_explorer(
                            FilePickerOrigin::HeadphoneMeasurement,
                            FilePickerMode::File,
                            "Select Measurement CSV",
                            Some(&start),
                            Some("csv"),
                        );
                    }
                    KeyCode::Char(c) => {
                        app.headphone_eq.measurement_path.push(c);
                        app.refresh_autocomplete_inline(
                            crate::app::app_autocomplete::get_headphone_measurement_path,
                            crate::app::app_autocomplete::AutocompleteKind::FilePath,
                        );
                    }
                    _ => {}
                }
                return None;
            }
            if app.headphone_eq.editing_custom_target {
                match key.code {
                    KeyCode::Enter => {
                        app.headphone_eq.editing_custom_target = false;
                        app.clear_autocomplete();
                    }
                    KeyCode::Tab => {
                        app.zsh_tab_complete(
                            crate::app::app_autocomplete::get_headphone_custom_target_path,
                            crate::app::app_autocomplete::set_headphone_custom_target_path,
                            crate::app::app_autocomplete::AutocompleteKind::FilePath,
                        );
                    }
                    KeyCode::BackTab => {
                        app.zsh_backtab_complete(
                            crate::app::app_autocomplete::set_headphone_custom_target_path,
                        );
                    }
                    KeyCode::Down => {
                        app.autocomplete_down(
                            crate::app::app_autocomplete::set_headphone_custom_target_path,
                        );
                    }
                    KeyCode::Up => {
                        app.autocomplete_up(
                            crate::app::app_autocomplete::set_headphone_custom_target_path,
                        );
                    }
                    KeyCode::Backspace => {
                        app.headphone_eq.custom_target_path.pop();
                        app.refresh_autocomplete_inline(
                            crate::app::app_autocomplete::get_headphone_custom_target_path,
                            crate::app::app_autocomplete::AutocompleteKind::FilePath,
                        );
                    }
                    KeyCode::F(2) => {
                        let start = app.headphone_eq.custom_target_path.clone();
                        app.open_file_explorer(
                            FilePickerOrigin::HeadphoneCustomTarget,
                            FilePickerMode::File,
                            "Select Custom Target CSV",
                            Some(&start),
                            Some("csv"),
                        );
                    }
                    KeyCode::Char(c) => {
                        app.headphone_eq.custom_target_path.push(c);
                        app.refresh_autocomplete_inline(
                            crate::app::app_autocomplete::get_headphone_custom_target_path,
                            crate::app::app_autocomplete::AutocompleteKind::FilePath,
                        );
                    }
                    _ => {}
                }
                return None;
            }
            match key.code {
                KeyCode::Up => {
                    if app.headphone_eq.selected_field > 0 {
                        app.headphone_eq.selected_field -= 1;
                    } else {
                        app.headphone_eq.step_tab_focused = true;
                    }
                }
                KeyCode::Down => {
                    let max = if app.headphone_eq.target_preset == "custom" {
                        2
                    } else {
                        1
                    };
                    if app.headphone_eq.selected_field < max {
                        app.headphone_eq.selected_field += 1;
                    }
                }
                KeyCode::Enter => {
                    match app.headphone_eq.selected_field {
                        0 => {
                            app.headphone_eq.editing_measurement = true;
                        }
                        1 => {} // target preset cycles with Left/Right
                        2 => {
                            app.headphone_eq.editing_custom_target = true;
                        }
                        _ => {}
                    }
                }
                KeyCode::Left | KeyCode::Right | KeyCode::Char('+') | KeyCode::Char('-') => {
                    if app.headphone_eq.selected_field == 1 {
                        let delta = match key.code {
                            KeyCode::Char('+') | KeyCode::Right => 1i32,
                            _ => -1,
                        };
                        app.headphone_eq.target_preset = super::cycle_string(
                            &app.headphone_eq.target_preset,
                            HEADPHONE_TARGET_PRESETS,
                            delta,
                        );
                        // Clamp selected_field if "custom" row disappeared
                        let max = if app.headphone_eq.target_preset == "custom" {
                            2
                        } else {
                            1
                        };
                        if app.headphone_eq.selected_field > max {
                            app.headphone_eq.selected_field = max;
                        }
                    }
                }
                KeyCode::Tab => {
                    // Cycle through fields
                    let max = if app.headphone_eq.target_preset == "custom" {
                        2
                    } else {
                        1
                    };
                    if app.headphone_eq.selected_field < max {
                        app.headphone_eq.selected_field += 1;
                    } else {
                        app.headphone_eq.selected_field = 0;
                    }
                }
                _ => {}
            }
            None
        }

        HeadphoneEqStep::Configure => {
            // Numerical direct-edit mode
            if app.headphone_eq.editing_value {
                match key.code {
                    KeyCode::Enter => {
                        set_headphone_eq_field_from_string(app);
                        app.headphone_eq.editing_value = false;
                        app.headphone_eq.edit_buffer.clear();
                    }
                    KeyCode::Esc => {
                        app.headphone_eq.editing_value = false;
                        app.headphone_eq.edit_buffer.clear();
                    }
                    KeyCode::Backspace => {
                        app.headphone_eq.edit_buffer.pop();
                    }
                    KeyCode::Char(c) if c.is_ascii_digit() || c == '.' || c == '-' => {
                        app.headphone_eq.edit_buffer.push(c);
                    }
                    _ => {}
                }
                return None;
            }
            match key.code {
                KeyCode::Up => {
                    if app.headphone_eq.config_selected_field > 0 {
                        app.headphone_eq.config_selected_field -= 1;
                    } else {
                        app.headphone_eq.step_tab_focused = true;
                    }
                }
                KeyCode::Down => {
                    if app.headphone_eq.config_selected_field < 17 {
                        app.headphone_eq.config_selected_field += 1;
                    }
                }
                KeyCode::Left | KeyCode::Char('-') => {
                    adjust_headphone_eq_field(app, -1);
                }
                KeyCode::Right | KeyCode::Char('+') => {
                    adjust_headphone_eq_field(app, 1);
                }
                KeyCode::Tab => {
                    if app.headphone_eq.config_selected_field < 17 {
                        app.headphone_eq.config_selected_field += 1;
                    } else {
                        app.headphone_eq.config_selected_field = 0;
                    }
                }
                KeyCode::Enter => {
                    let f = app.headphone_eq.config_selected_field;
                    if is_headphone_eq_field_numerical(f) {
                        app.headphone_eq.edit_buffer =
                            headphone_eq_field_value_string(app, f);
                        app.headphone_eq.editing_value = true;
                    }
                    // Booleans: toggle
                    else if matches!(f, 14 | 16) {
                        adjust_headphone_eq_field(app, 1);
                    }
                }
                KeyCode::BackTab => {
                    app.headphone_eq.step = HeadphoneEqStep::SelectFile;
                }
                _ => {}
            }
            None
        }

        HeadphoneEqStep::Optimize => match key.code {
            KeyCode::Up => {
                app.headphone_eq.step_tab_focused = true;
                None
            }
            KeyCode::Enter => {
                match &app.headphone_eq.opt_status {
                    OptimizationStatus::Idle
                    | OptimizationStatus::Failed
                    | OptimizationStatus::Cancelled => {
                        spawn_headphone_eq_optimization(app);
                    }
                    OptimizationStatus::Completed => {
                        app.headphone_eq.step = HeadphoneEqStep::Results;
                    }
                    OptimizationStatus::Running => {}
                }
                None
            }
            KeyCode::BackTab => {
                app.headphone_eq.step = HeadphoneEqStep::Configure;
                None
            }
            _ => None,
        },

        HeadphoneEqStep::Results => match key.code {
            KeyCode::Up => {
                app.headphone_eq.step_tab_focused = true;
                None
            }
            KeyCode::BackTab => {
                app.headphone_eq.step = HeadphoneEqStep::Optimize;
                None
            }
            _ => None,
        },

        HeadphoneEqStep::UpdatePlugin => match app.headphone_eq.update_substep {
            SpinUpdateSubStep::Ready => match key.code {
                KeyCode::Up => {
                    app.headphone_eq.step_tab_focused = true;
                    None
                }
                KeyCode::BackTab => {
                    app.headphone_eq.step = HeadphoneEqStep::Results;
                    None
                }
                KeyCode::Enter => {
                    if let Some((slot, count)) = app.find_last_eq_info() {
                        if count > 0 {
                            app.headphone_eq.update_existing_eq_info = Some((slot, count));
                            app.headphone_eq.update_substep = SpinUpdateSubStep::ConfirmOverwrite;
                        } else {
                            match app.apply_headphone_to_plugin_chain() {
                                Ok(msg) => app.status_message = Some(msg),
                                Err(e) => app.status_message = Some(format!("Error: {}", e)),
                            }
                        }
                    } else {
                        match app.apply_headphone_to_plugin_chain() {
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
                    if let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir() {
                        let timestamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
                        let filename = format!("pre-headphone-eq-{}.json", timestamp);
                        match app.plugin_chain.save_to_file(&presets_dir, &filename) {
                            Ok(_) => {
                                app.status_message = Some(format!("Saved backup: {}", filename));
                                log::info!(
                                    "Auto-saved preset before headphone EQ overwrite: {}",
                                    filename
                                );
                            }
                            Err(e) => {
                                app.status_message = Some(format!("Backup failed: {}", e));
                                log::error!("Failed to auto-save preset: {}", e);
                                app.headphone_eq.update_substep = SpinUpdateSubStep::Ready;
                                app.headphone_eq.update_existing_eq_info = None;
                                return None;
                            }
                        }
                    }
                    match app.apply_headphone_to_plugin_chain() {
                        Ok(msg) => app.status_message = Some(msg),
                        Err(e) => app.status_message = Some(format!("Error: {}", e)),
                    }
                    app.headphone_eq.update_substep = SpinUpdateSubStep::Ready;
                    app.headphone_eq.update_existing_eq_info = None;
                    None
                }
                KeyCode::Char('n') => {
                    match app.apply_headphone_to_plugin_chain() {
                        Ok(msg) => app.status_message = Some(msg),
                        Err(e) => app.status_message = Some(format!("Error: {}", e)),
                    }
                    app.headphone_eq.update_substep = SpinUpdateSubStep::Ready;
                    app.headphone_eq.update_existing_eq_info = None;
                    None
                }
                KeyCode::Esc => {
                    app.headphone_eq.update_substep = SpinUpdateSubStep::Ready;
                    app.headphone_eq.update_existing_eq_info = None;
                    None
                }
                _ => None,
            },
        },
    }
}

fn is_headphone_eq_field_numerical(field: usize) -> bool {
    matches!(field, 0..=6 | 9 | 10 | 12 | 13 | 17)
}

fn headphone_eq_field_value_string(app: &App, field: usize) -> String {
    let c = &app.headphone_eq.config;
    match field {
        0 => c.num_filters.to_string(),
        1 => format!("{:.0}", c.min_freq),
        2 => format!("{:.0}", c.max_freq),
        3 => format!("{:.1}", c.min_db),
        4 => format!("{:.1}", c.max_db),
        5 => format!("{:.1}", c.min_q),
        6 => format!("{:.1}", c.max_q),
        9 => c.max_iter.to_string(),
        10 => c.population.to_string(),
        12 => format!("{:.1}", c.de_f),
        13 => format!("{:.1}", c.de_cr),
        17 => c.smooth_n.to_string(),
        _ => String::new(),
    }
}

fn set_headphone_eq_field_from_string(app: &mut App) {
    let c = &mut app.headphone_eq.config;
    let buf = &app.headphone_eq.edit_buffer;
    match app.headphone_eq.config_selected_field {
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
                c.population = v.clamp(10, 200);
            }
        }
        12 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.de_f = v.clamp(0.1, 2.0);
            }
        }
        13 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.de_cr = v.clamp(0.1, 1.0);
            }
        }
        17 => {
            if let Ok(v) = buf.parse::<usize>() {
                c.smooth_n = v.clamp(1, 24);
            }
        }
        _ => {}
    }
}

fn adjust_headphone_eq_field(app: &mut App, delta: i32) {
    let c = &mut app.headphone_eq.config;
    match app.headphone_eq.config_selected_field {
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
            c.peq_model = super::cycle_string(
                &c.peq_model,
                &["pk", "hp-pk", "hp-pk-lp", "ls-pk", "ls-pk-hs"],
                delta,
            );
        }
        8 => {
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
        9 => {
            let n = c.max_iter as i32 + delta * 1000;
            c.max_iter = n.clamp(1000, 100000) as usize;
        }
        10 => {
            let n = c.population as i32 + delta * 10;
            c.population = n.clamp(10, 200) as usize;
        }
        11 => {
            c.strategy = super::cycle_string(
                &c.strategy,
                &["currenttobest1bin", "best1bin", "rand1bin", "best2bin"],
                delta,
            );
        }
        12 => c.de_f = (c.de_f + delta as f64 * 0.1).clamp(0.1, 2.0),
        13 => c.de_cr = (c.de_cr + delta as f64 * 0.1).clamp(0.1, 1.0),
        14 => c.refine = !c.refine,
        15 => {
            c.local_algo = super::cycle_string(&c.local_algo, &["cobyla", "nelder-mead"], delta);
        }
        16 => c.smooth = !c.smooth,
        17 => {
            let n = c.smooth_n as i32 + delta;
            c.smooth_n = n.clamp(1, 24) as usize;
        }
        _ => {}
    }
}

#[allow(clippy::type_complexity)]
static HEADPHONE_OPT_RESULT: std::sync::OnceLock<
    Arc<Mutex<Option<Result<sotf_audio_player::autoeq::HeadphoneOptimizationResult, String>>>>,
> = std::sync::OnceLock::new();
#[allow(clippy::type_complexity)]
static HEADPHONE_OPT_PROGRESS: std::sync::OnceLock<
    Arc<Mutex<Option<(usize, usize, f64, f32)>>>,
> = std::sync::OnceLock::new();

pub fn poll_headphone_eq_optimization(app: &mut App) -> bool {
    use sotf_audio_player::headphone_eq_types::HeadphoneEqBiquad;
    use sotf_audio_player::room_eq_types::OptimizationStatus;

    if app.headphone_eq.opt_status != OptimizationStatus::Running {
        return false;
    }

    let result_slot = HEADPHONE_OPT_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();
    let progress_slot = HEADPHONE_OPT_PROGRESS
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();

    if let Ok(mut guard) = result_slot.lock()
        && let Some(result) = guard.take() {
            match result {
                Ok(r) => {
                    app.headphone_eq.pre_loss = r.initial_loss;
                    app.headphone_eq.post_loss = r.final_loss;
                    app.headphone_eq.filters = r
                        .biquads
                        .iter()
                        .map(|b| HeadphoneEqBiquad {
                            filter_type: format!("{:?}", b.filter_type),
                            freq: b.freq,
                            q: b.q,
                            db_gain: b.db_gain,
                        })
                        .collect();
                    app.headphone_eq.curve_frequencies = r.frequencies.clone();
                    app.headphone_eq.curve_input = r.input_curve.clone();
                    app.headphone_eq.curve_target = r.target_curve.clone();
                    app.headphone_eq.curve_corrected = r.corrected_curve.clone();
                    app.headphone_eq.curve_filter_response = r.filter_response.clone();
                    // Keep the progress-based loss_history; only override if empty
                    if app.headphone_eq.loss_history.is_empty() {
                        app.headphone_eq.loss_history = r.optimization_history.clone();
                    }
                    app.headphone_eq.opt_status = OptimizationStatus::Completed;
                    app.headphone_eq.opt_progress = 1.0;
                    // Auto-advance to Results
                    app.headphone_eq.step = crate::app::HeadphoneEqStep::Results;
                }
                Err(e) => {
                    app.headphone_eq.opt_status = OptimizationStatus::Failed;
                    app.headphone_eq.opt_error = Some(e);
                }
            }
            return true;
        }

    if let Ok(mut guard) = progress_slot.lock()
        && let Some((iter, max_iter, loss, pct)) = guard.take() {
            app.headphone_eq.opt_iteration = iter;
            app.headphone_eq.opt_max_iter = max_iter;
            app.headphone_eq.opt_loss = loss;
            app.headphone_eq.opt_progress = pct;
            app.headphone_eq.loss_history.push((iter, loss));
            return true;
        }

    false
}

fn spawn_headphone_eq_optimization(app: &mut App) {
    use sotf_audio_player::room_eq_types::OptimizationStatus;

    if app.headphone_eq.measurement_path.is_empty() {
        app.headphone_eq.opt_status = OptimizationStatus::Failed;
        app.headphone_eq.opt_error = Some("No measurement file selected".to_string());
        return;
    }

    app.headphone_eq.opt_status = OptimizationStatus::Running;
    app.headphone_eq.opt_error = None;
    app.headphone_eq.opt_progress = 0.0;
    app.headphone_eq.opt_iteration = 0;
    app.headphone_eq.opt_loss = 0.0;
    app.headphone_eq.filters.clear();
    app.headphone_eq.loss_history.clear();

    let curve_path = app.headphone_eq.measurement_path.clone();
    let target = app.headphone_eq.target_preset.clone();
    let custom_target = app.headphone_eq.custom_target_path.clone();
    let c = &app.headphone_eq.config;

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

        let result =
            sotf_audio_player::autoeq::headphone::run_headphone_optimization_with_callback(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{HeadphoneEqStep, Screen};
    use crate::events::tests::{key, make_app};

    #[test]
    fn headphone_eq_step_prev_does_not_wrap() {
        assert_eq!(
            headphone_eq_step_prev(HeadphoneEqStep::SelectFile),
            HeadphoneEqStep::SelectFile,
        );
    }

    #[test]
    fn headphone_eq_step_next_does_not_wrap() {
        assert_eq!(
            headphone_eq_step_next(HeadphoneEqStep::UpdatePlugin),
            HeadphoneEqStep::UpdatePlugin,
        );
    }

    #[test]
    fn headphone_eq_step_prev_advances_backwards() {
        assert_eq!(
            headphone_eq_step_prev(HeadphoneEqStep::Configure),
            HeadphoneEqStep::SelectFile,
        );
        assert_eq!(
            headphone_eq_step_prev(HeadphoneEqStep::Optimize),
            HeadphoneEqStep::Configure,
        );
        assert_eq!(
            headphone_eq_step_prev(HeadphoneEqStep::Results),
            HeadphoneEqStep::Optimize,
        );
    }

    #[test]
    fn headphone_eq_step_next_advances_forward() {
        assert_eq!(
            headphone_eq_step_next(HeadphoneEqStep::SelectFile),
            HeadphoneEqStep::Configure,
        );
        assert_eq!(
            headphone_eq_step_next(HeadphoneEqStep::Configure),
            HeadphoneEqStep::Optimize,
        );
        assert_eq!(
            headphone_eq_step_next(HeadphoneEqStep::Optimize),
            HeadphoneEqStep::Results,
        );
    }

    #[test]
    fn headphone_eq_up_at_top_goes_to_step_tab() {
        let mut app = make_app();
        app.current_screen = Screen::Configure;
        app.configure_sub_screen = crate::app::ConfigureSubScreen::HeadphoneEq;
        app.input_mode = InputMode::ConfigureHeadphoneEq;
        app.headphone_eq.step = HeadphoneEqStep::SelectFile;
        app.headphone_eq.selected_field = 0;

        handle_headphone_eq_keys(&mut app, key(KeyCode::Up));
        assert!(app.headphone_eq.step_tab_focused, "Up at field 0 should focus step tab bar");
        assert!(app.input_mode.is_configure_sub_screen(), "should NOT jump to configure tab");
    }

    #[test]
    fn headphone_eq_up_decrements_field_when_not_at_top() {
        let mut app = make_app();
        app.current_screen = Screen::Configure;
        app.configure_sub_screen = crate::app::ConfigureSubScreen::HeadphoneEq;
        app.input_mode = InputMode::ConfigureHeadphoneEq;
        app.headphone_eq.step = HeadphoneEqStep::SelectFile;
        app.headphone_eq.selected_field = 1;

        handle_headphone_eq_keys(&mut app, key(KeyCode::Up));
        assert_eq!(app.headphone_eq.selected_field, 0);
        assert!(app.input_mode.is_configure_sub_screen());
    }

    #[test]
    fn headphone_eq_down_clamps_at_non_custom_max() {
        let mut app = make_app();
        app.headphone_eq.step = HeadphoneEqStep::SelectFile;
        app.headphone_eq.target_preset = "harman-over-ear-2018".to_string();
        app.headphone_eq.selected_field = 1;

        // Down should NOT go past 1 for non-custom presets
        handle_headphone_eq_keys(&mut app, key(KeyCode::Down));
        assert_eq!(app.headphone_eq.selected_field, 1);
    }

    #[test]
    fn headphone_eq_down_allows_field_2_for_custom() {
        let mut app = make_app();
        app.headphone_eq.step = HeadphoneEqStep::SelectFile;
        app.headphone_eq.target_preset = "custom".to_string();
        app.headphone_eq.selected_field = 1;

        // Down should go to 2 when preset is "custom"
        handle_headphone_eq_keys(&mut app, key(KeyCode::Down));
        assert_eq!(app.headphone_eq.selected_field, 2);
    }

    #[test]
    fn headphone_eq_esc_from_content_goes_to_step_tab() {
        let mut app = make_app();
        app.input_mode = InputMode::ConfigureHeadphoneEq;
        app.headphone_eq.step = HeadphoneEqStep::Configure;
        handle_headphone_eq_keys(&mut app, key(KeyCode::Esc));
        assert!(app.headphone_eq.step_tab_focused, "Esc from content should focus step tab bar");
        assert!(app.input_mode.is_configure_sub_screen(), "should NOT jump to configure tab");
    }

    #[test]
    fn headphone_eq_esc_from_step_tab_goes_to_configure_tab() {
        let mut app = make_app();
        app.input_mode = InputMode::ConfigureHeadphoneEq;
        app.headphone_eq.step = HeadphoneEqStep::SelectFile;
        app.headphone_eq.step_tab_focused = true;
        handle_headphone_eq_keys(&mut app, key(KeyCode::Esc));
        assert_eq!(app.input_mode, InputMode::Configure);
        assert!(!app.headphone_eq.step_tab_focused);
    }

    #[test]
    fn headphone_eq_esc_chain_content_to_step_to_configure() {
        let mut app = make_app();
        app.input_mode = InputMode::ConfigureHeadphoneEq;
        app.headphone_eq.step = HeadphoneEqStep::Optimize;

        // First Esc → step tab bar
        handle_headphone_eq_keys(&mut app, key(KeyCode::Esc));
        assert!(app.headphone_eq.step_tab_focused);
        assert!(app.input_mode.is_configure_sub_screen());

        // Second Esc → configure tab bar
        handle_headphone_eq_keys(&mut app, key(KeyCode::Esc));
        assert_eq!(app.input_mode, InputMode::Configure);
        assert!(!app.headphone_eq.step_tab_focused);
    }

    #[test]
    fn headphone_eq_step_tab_right_changes_step() {
        let mut app = make_app();
        app.input_mode = InputMode::ConfigureHeadphoneEq;
        app.headphone_eq.step = HeadphoneEqStep::SelectFile;
        app.headphone_eq.step_tab_focused = true;
        handle_headphone_eq_keys(&mut app, key(KeyCode::Right));
        assert_eq!(app.headphone_eq.step, HeadphoneEqStep::Configure);
        assert!(app.headphone_eq.step_tab_focused, "should stay on step tab bar");
    }

    #[test]
    fn headphone_eq_step_tab_up_goes_to_configure_tab() {
        let mut app = make_app();
        app.input_mode = InputMode::ConfigureHeadphoneEq;
        app.headphone_eq.step_tab_focused = true;
        handle_headphone_eq_keys(&mut app, key(KeyCode::Up));
        assert_eq!(app.input_mode, InputMode::Configure);
        assert!(!app.headphone_eq.step_tab_focused);
    }

    #[test]
    fn headphone_eq_step_tab_down_enters_content() {
        let mut app = make_app();
        app.input_mode = InputMode::ConfigureHeadphoneEq;
        app.headphone_eq.step_tab_focused = true;
        handle_headphone_eq_keys(&mut app, key(KeyCode::Down));
        assert!(!app.headphone_eq.step_tab_focused);
        assert!(app.input_mode.is_configure_sub_screen());
    }

    #[test]
    fn headphone_eq_tab_cycles_select_file_fields() {
        let mut app = make_app();
        app.headphone_eq.step = HeadphoneEqStep::SelectFile;
        app.headphone_eq.selected_field = 0;
        handle_headphone_eq_keys(&mut app, key(KeyCode::Tab));
        assert_eq!(app.headphone_eq.selected_field, 1);
        assert_eq!(app.headphone_eq.step, HeadphoneEqStep::SelectFile);
    }

    #[test]
    fn headphone_eq_right_on_select_file_cycles_preset() {
        let mut app = make_app();
        app.headphone_eq.step = HeadphoneEqStep::SelectFile;
        app.headphone_eq.selected_field = 1; // target preset field
        let old_preset = app.headphone_eq.target_preset.clone();
        handle_headphone_eq_keys(&mut app, key(KeyCode::Right));
        // Right on field 1 should cycle the target preset
        assert_ne!(app.headphone_eq.target_preset, old_preset);
        // Should stay on SelectFile step (not navigate)
        assert_eq!(app.headphone_eq.step, HeadphoneEqStep::SelectFile);
    }

    #[test]
    fn headphone_eq_left_right_adjusts_configure_field() {
        let mut app = make_app();
        app.input_mode = InputMode::ConfigureHeadphoneEq;
        app.headphone_eq.step = HeadphoneEqStep::Configure;
        app.headphone_eq.config_selected_field = 0; // num_filters
        let before = app.headphone_eq.config.num_filters;
        handle_headphone_eq_keys(&mut app, key(KeyCode::Right));
        assert_eq!(app.headphone_eq.config.num_filters, before + 1);
        handle_headphone_eq_keys(&mut app, key(KeyCode::Left));
        assert_eq!(app.headphone_eq.config.num_filters, before);
        // Should stay on Configure step
        assert_eq!(app.headphone_eq.step, HeadphoneEqStep::Configure);
    }

    #[test]
    fn headphone_eq_enter_on_numerical_enters_edit_mode() {
        let mut app = make_app();
        app.input_mode = InputMode::ConfigureHeadphoneEq;
        app.headphone_eq.step = HeadphoneEqStep::Configure;
        app.headphone_eq.config_selected_field = 0; // num_filters (numerical)
        handle_headphone_eq_keys(&mut app, key(KeyCode::Enter));
        assert!(app.headphone_eq.editing_value);
        assert!(!app.headphone_eq.edit_buffer.is_empty());
    }

    #[test]
    fn headphone_eq_edit_mode_enter_commits() {
        let mut app = make_app();
        app.input_mode = InputMode::ConfigureHeadphoneEq;
        app.headphone_eq.step = HeadphoneEqStep::Configure;
        app.headphone_eq.config_selected_field = 0;
        app.headphone_eq.editing_value = true;
        app.headphone_eq.edit_buffer = "15".to_string();
        handle_headphone_eq_keys(&mut app, key(KeyCode::Enter));
        assert!(!app.headphone_eq.editing_value);
        assert_eq!(app.headphone_eq.config.num_filters, 15);
    }
}
