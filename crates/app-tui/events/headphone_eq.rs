//! Headphone EQ wizard event handlers

use super::PlayerCommand;
use crate::app::{App, FilePickerMode, FilePickerOrigin};
use crossterm::event::{KeyCode, KeyEvent};
use std::sync::{Arc, Mutex};


pub(crate) fn headphone_eq_step_prev(s: crate::app::HeadphoneEqStep) -> crate::app::HeadphoneEqStep {
    use crate::app::HeadphoneEqStep;
    match s {
        HeadphoneEqStep::SelectFile   => HeadphoneEqStep::SelectFile, // no wrap
        HeadphoneEqStep::Configure    => HeadphoneEqStep::SelectFile,
        HeadphoneEqStep::Optimize     => HeadphoneEqStep::Configure,
        HeadphoneEqStep::Results      => HeadphoneEqStep::Optimize,
        HeadphoneEqStep::UpdatePlugin => HeadphoneEqStep::Results,
    }
}

#[cfg(test)]
pub(crate) fn headphone_eq_step_next(s: crate::app::HeadphoneEqStep) -> crate::app::HeadphoneEqStep {
    use crate::app::HeadphoneEqStep;
    match s {
        HeadphoneEqStep::SelectFile   => HeadphoneEqStep::Configure,
        HeadphoneEqStep::Configure    => HeadphoneEqStep::Optimize,
        HeadphoneEqStep::Optimize     => HeadphoneEqStep::Results,
        HeadphoneEqStep::Results      => HeadphoneEqStep::UpdatePlugin,
        HeadphoneEqStep::UpdatePlugin => HeadphoneEqStep::UpdatePlugin, // no wrap
    }
}

pub fn handle_headphone_eq_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    use crate::app::{HeadphoneEqStep, HEADPHONE_TARGET_PRESETS, SpinUpdateSubStep};
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
            HeadphoneEqStep::UpdatePlugin => {
                if app.headphone_eq.update_substep == SpinUpdateSubStep::ConfirmOverwrite {
                    app.headphone_eq.update_substep = SpinUpdateSubStep::Ready;
                    app.headphone_eq.update_existing_eq_info = None;
                } else {
                    app.headphone_eq.step = HeadphoneEqStep::Results;
                }
            }
            _ => {
                app.headphone_eq.step = headphone_eq_step_prev(app.headphone_eq.step);
            }
        }
        return None;
    }

    match app.headphone_eq.step {
        HeadphoneEqStep::SelectFile => {
            if app.headphone_eq.editing_measurement {
                match key.code {
                    KeyCode::Enter => {
                        app.headphone_eq.editing_measurement = false;
                    }
                    KeyCode::Backspace => {
                        app.headphone_eq.measurement_path.pop();
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
                    }
                    _ => {}
                }
                return None;
            }
            if app.headphone_eq.editing_custom_target {
                match key.code {
                    KeyCode::Enter => {
                        app.headphone_eq.editing_custom_target = false;
                    }
                    KeyCode::Backspace => {
                        app.headphone_eq.custom_target_path.pop();
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
                        app.configure_tab_focused = true;
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
                KeyCode::Left | KeyCode::Right => {
                    if app.headphone_eq.selected_field == 1 {
                        let delta = if key.code == KeyCode::Right { 1i32 } else { -1 };
                        app.headphone_eq.target_preset = cycle_string(
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
                if app.headphone_eq.config_selected_field > 0 {
                    app.headphone_eq.config_selected_field -= 1;
                } else {
                    app.configure_tab_focused = true;
                }
                None
            }
            KeyCode::Down => {
                if app.headphone_eq.config_selected_field < 17 {
                    app.headphone_eq.config_selected_field += 1;
                }
                None
            }
            KeyCode::Left | KeyCode::Char('-') => {
                adjust_headphone_eq_field(app, -1);
                None
            }
            KeyCode::Right | KeyCode::Char('+') => {
                adjust_headphone_eq_field(app, 1);
                None
            }
            KeyCode::Enter | KeyCode::Tab => {
                app.headphone_eq.step = HeadphoneEqStep::Optimize;
                None
            }
            KeyCode::BackTab => {
                app.headphone_eq.step = HeadphoneEqStep::SelectFile;
                None
            }
            _ => None,
        },

        HeadphoneEqStep::Optimize => match key.code {
            KeyCode::Up => {
                app.configure_tab_focused = true;
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
            KeyCode::Tab => {
                if app.headphone_eq.opt_status == OptimizationStatus::Completed {
                    app.headphone_eq.step = HeadphoneEqStep::Results;
                } else {
                    app.headphone_eq.step = HeadphoneEqStep::Configure;
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
                app.configure_tab_focused = true;
                None
            }
            KeyCode::Tab => {
                app.headphone_eq.step = HeadphoneEqStep::UpdatePlugin;
                None
            }
            KeyCode::BackTab => {
                app.headphone_eq.step = HeadphoneEqStep::Optimize;
                None
            }
            _ => None,
        },

        HeadphoneEqStep::UpdatePlugin => {
            match app.headphone_eq.update_substep {
                SpinUpdateSubStep::Ready => match key.code {
                    KeyCode::Up => {
                        app.configure_tab_focused = true;
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
                    KeyCode::BackTab => {
                        app.headphone_eq.step = HeadphoneEqStep::Results;
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
                                    log::info!("Auto-saved preset before headphone EQ overwrite: {}", filename);
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
            }
        },
    }
}

fn cycle_string(current: &str, options: &[&str], delta: i32) -> String {
    let idx = options.iter().position(|&o| o == current).unwrap_or(0);
    let new_idx = if delta > 0 {
        (idx + 1) % options.len()
    } else {
        (idx + options.len() - 1) % options.len()
    };
    options[new_idx].to_string()
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
            c.peq_model = cycle_string(
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
            c.strategy = cycle_string(
                &c.strategy,
                &["currenttobest1bin", "best1bin", "rand1bin", "best2bin"],
                delta,
            );
        }
        12 => c.de_f = (c.de_f + delta as f64 * 0.1).clamp(0.1, 2.0),
        13 => c.de_cr = (c.de_cr + delta as f64 * 0.1).clamp(0.1, 1.0),
        14 => c.refine = !c.refine,
        15 => {
            c.local_algo = cycle_string(&c.local_algo, &["cobyla", "nelder-mead"], delta);
        }
        16 => c.smooth = !c.smooth,
        17 => {
            let n = c.smooth_n as i32 + delta;
            c.smooth_n = n.clamp(1, 24) as usize;
        }
        _ => {}
    }
}

static HEADPHONE_OPT_RESULT: std::sync::OnceLock<
    Arc<Mutex<Option<Result<sotf_audio_player::autoeq::HeadphoneOptimizationResult, String>>>>,
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

    if let Ok(mut guard) = result_slot.lock() {
        if let Some(result) = guard.take() {
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
                    app.headphone_eq.loss_history = r.optimization_history.clone();
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

    // Clear stale result
    if let Ok(mut g) = result_slot.lock() {
        *g = None;
    }

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
    fn headphone_eq_up_at_top_returns_to_tab_bar() {
        let mut app = make_app();
        app.current_screen = Screen::Configure;
        app.configure_sub_screen = crate::app::ConfigureSubScreen::HeadphoneEq;
        app.configure_tab_focused = false;
        app.headphone_eq.step = HeadphoneEqStep::SelectFile;
        app.headphone_eq.selected_field = 0;

        handle_headphone_eq_keys(&mut app, key(KeyCode::Up));
        assert!(app.configure_tab_focused);
    }

    #[test]
    fn headphone_eq_up_decrements_field_when_not_at_top() {
        let mut app = make_app();
        app.current_screen = Screen::Configure;
        app.configure_sub_screen = crate::app::ConfigureSubScreen::HeadphoneEq;
        app.configure_tab_focused = false;
        app.headphone_eq.step = HeadphoneEqStep::SelectFile;
        app.headphone_eq.selected_field = 1;

        handle_headphone_eq_keys(&mut app, key(KeyCode::Up));
        assert_eq!(app.headphone_eq.selected_field, 0);
        assert!(!app.configure_tab_focused);
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
    fn headphone_eq_esc_at_configure_goes_to_select() {
        let mut app = make_app();
        app.headphone_eq.step = HeadphoneEqStep::Configure;
        handle_headphone_eq_keys(&mut app, key(KeyCode::Esc));
        assert_eq!(app.headphone_eq.step, HeadphoneEqStep::SelectFile);
    }

    #[test]
    fn headphone_eq_esc_at_select_returns_to_tab_bar() {
        let mut app = make_app();
        app.headphone_eq.step = HeadphoneEqStep::SelectFile;
        app.configure_tab_focused = false;
        handle_headphone_eq_keys(&mut app, key(KeyCode::Esc));
        assert!(app.configure_tab_focused);
    }

    #[test]
    fn headphone_eq_tab_without_path_stays_on_select() {
        let mut app = make_app();
        app.headphone_eq.step = HeadphoneEqStep::SelectFile;
        app.headphone_eq.measurement_path = String::new();
        handle_headphone_eq_keys(&mut app, key(KeyCode::Tab));
        assert_eq!(app.headphone_eq.step, HeadphoneEqStep::SelectFile);
    }

    #[test]
    fn headphone_eq_tab_with_path_advances_to_configure() {
        let mut app = make_app();
        app.headphone_eq.step = HeadphoneEqStep::SelectFile;
        app.headphone_eq.measurement_path = "/some/path.csv".to_string();
        handle_headphone_eq_keys(&mut app, key(KeyCode::Tab));
        assert_eq!(app.headphone_eq.step, HeadphoneEqStep::Configure);
    }
}
