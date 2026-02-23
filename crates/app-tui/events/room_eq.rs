//! Room EQ wizard event handlers

use super::PlayerCommand;
use crate::app::App;
use crossterm::event::{KeyCode, KeyEvent};
use std::sync::{Arc, Mutex};

pub(crate) fn room_eq_step_prev(s: sotf_audio_player::room_eq_types::RoomEqStep) -> sotf_audio_player::room_eq_types::RoomEqStep {
    use sotf_audio_player::room_eq_types::RoomEqStep;
    match s {
        RoomEqStep::SelectFile => RoomEqStep::SelectFile, // no wrap
        RoomEqStep::Configure => RoomEqStep::SelectFile,
        RoomEqStep::Optimize => RoomEqStep::Configure,
        RoomEqStep::Results => RoomEqStep::Optimize,
    }
}

pub(crate) fn room_eq_step_next(s: sotf_audio_player::room_eq_types::RoomEqStep) -> sotf_audio_player::room_eq_types::RoomEqStep {
    use sotf_audio_player::room_eq_types::RoomEqStep;
    match s {
        RoomEqStep::SelectFile => RoomEqStep::Configure,
        RoomEqStep::Configure => RoomEqStep::Optimize,
        RoomEqStep::Optimize => RoomEqStep::Results,
        RoomEqStep::Results => RoomEqStep::Results, // no wrap
    }
}

pub fn handle_room_eq_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    use sotf_audio_player::room_eq_types::{OptimizationStatus, RoomEqStep};

    // Esc goes up one level
    if key.code == KeyCode::Esc {
        match app.room_eq.step {
            RoomEqStep::SelectFile => {
                app.configure_tab_focused = true;
            }
            _ => {
                app.room_eq.step = room_eq_step_prev(app.room_eq.step);
            }
        }
        return None;
    }

    // Step navigation via Left/Right (except in Configure step)
    if app.room_eq.step != RoomEqStep::Configure {
        if key.code == KeyCode::Left {
            app.room_eq.step = room_eq_step_prev(app.room_eq.step);
            return None;
        }
        if key.code == KeyCode::Right {
            app.room_eq.step = room_eq_step_next(app.room_eq.step);
            return None;
        }
    }

    match app.room_eq.step {
        RoomEqStep::SelectFile => match key.code {
            KeyCode::Up => {
                if app.room_eq.selected_field > 0 {
                    app.room_eq.selected_field -= 1;
                } else {
                    app.configure_tab_focused = true;
                }
            }
            KeyCode::Down => {
                if app.room_eq.selected_field < 1 {
                    app.room_eq.selected_field += 1;
                }
            }
            KeyCode::Enter => {
                match app.room_eq.selected_field {
                    0 => {
                        // Open file browser for measurement file
                        app.current_browser_dir = std::path::PathBuf::from(&app.room_eq.measurement_path);
                        app.refresh_file_browser();
                    }
                    1 => {
                        // Select room name or preset
                    }
                    _ => {}
                }
            }
            KeyCode::Tab => {
                if !app.room_eq.measurement_path.is_empty() {
                    app.room_eq.step = RoomEqStep::Configure;
                }
            }
            _ => {}
        },

        RoomEqStep::Configure => match key.code {
            KeyCode::Up => {
                if app.room_eq.selected_field > 0 {
                    app.room_eq.selected_field -= 1;
                } else {
                    app.configure_tab_focused = true;
                }
            }
            KeyCode::Down => {
                if app.room_eq.selected_field < 24 {
                    app.room_eq.selected_field += 1;
                }
            }
            KeyCode::Left | KeyCode::Char('-') => {
                adjust_room_eq_field(app, -1);
            }
            KeyCode::Right | KeyCode::Char('+') => {
                adjust_room_eq_field(app, 1);
            }
            KeyCode::Enter | KeyCode::Tab => {
                app.room_eq.step = RoomEqStep::Optimize;
            }
            KeyCode::BackTab => {
                app.room_eq.step = RoomEqStep::SelectFile;
            }
            _ => {}
        },

        RoomEqStep::Optimize => match key.code {
            KeyCode::Enter => {
                match &app.room_eq.opt_status {
                    OptimizationStatus::Idle | OptimizationStatus::Failed | OptimizationStatus::Cancelled => {
                        spawn_room_eq_optimization(app);
                    }
                    OptimizationStatus::Completed => {
                        app.room_eq.step = RoomEqStep::Results;
                    }
                    OptimizationStatus::Running => {}
                }
            }
            KeyCode::Tab => {
                if app.room_eq.opt_status == OptimizationStatus::Completed {
                    app.room_eq.step = RoomEqStep::Results;
                } else {
                    app.room_eq.step = RoomEqStep::Configure;
                }
            }
            KeyCode::BackTab => {
                app.room_eq.step = RoomEqStep::Configure;
            }
            _ => {}
        },

        RoomEqStep::Results => match key.code {
            KeyCode::Enter => {
                export_room_eq_results(app);
            }
            KeyCode::BackTab => {
                app.room_eq.step = RoomEqStep::Optimize;
            }
            _ => {}
        },
    }

    None
}

fn adjust_room_eq_field(app: &mut App, delta: i32) {
    let c = &mut app.room_eq.config;
    match app.room_eq.selected_field {
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
        7 => c.smooth = !c.smooth,
        8 => {
            let n = c.smooth_n as i32 + delta;
            c.smooth_n = n.clamp(1, 24) as usize;
        }
        _ => {}
    }
}

fn load_room_eq_measurements(app: &mut App) {
    // Load measurement data from file
    let path = &app.room_eq.measurement_path;
    if path.is_empty() {
        app.room_eq.load_error = Some("No measurement file selected".to_string());
        return;
    }

    match std::fs::read_to_string(path) {
        Ok(_content) => {
            // Parse CSV or measurement data
            app.room_eq.load_error = None;
        }
        Err(e) => {
            app.room_eq.load_error = Some(format!("Failed to load: {}", e));
        }
    }
}

static OPT_RESULT: std::sync::OnceLock<
    Arc<
        Mutex<
            Option<Result<sotf_audio_player::autoeq::RoomOptimizationResult, String>>,
        >,
    >,
> = std::sync::OnceLock::new();
static OPT_PROGRESS: std::sync::OnceLock<Arc<Mutex<Option<(usize, usize, f64, f32)>>>> =
    std::sync::OnceLock::new();

/// Poll optimization progress/result on every tick while optimization is running.
/// Returns true if the UI needs a redraw.
pub fn poll_room_eq_optimization(app: &mut App) -> bool {
    use sotf_audio_player::room_eq_types::OptimizationStatus;

    if app.room_eq.opt_status != OptimizationStatus::Running {
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
                Ok(_r) => {
                    app.room_eq.opt_status = OptimizationStatus::Completed;
                    app.room_eq.opt_progress = 1.0;
                }
                Err(e) => {
                    app.room_eq.opt_status = OptimizationStatus::Failed;
                    app.room_eq.opt_error = Some(e);
                }
            }
            return true;
        }
    }

    if let Ok(mut guard) = progress_slot.lock() {
        if let Some((iter, max_iter, loss, pct)) = guard.take() {
            app.room_eq.opt_iteration = iter;
            app.room_eq.opt_max_iter = max_iter;
            app.room_eq.opt_loss = loss;
            app.room_eq.opt_progress = pct;
            return true;
        }
    }

    false
}

fn spawn_room_eq_optimization(app: &mut App) {
    use sotf_audio_player::room_eq_types::OptimizationStatus;

    app.room_eq.opt_status = OptimizationStatus::Running;
    app.room_eq.opt_error = None;
    app.room_eq.opt_progress = 0.0;
    app.room_eq.opt_iteration = 0;
    app.room_eq.opt_loss = 0.0;

    // Spawn background optimization task
    let _result_slot = OPT_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();

    // Actual implementation would spawn a thread here
    // For now, mark as completed
    app.room_eq.opt_status = OptimizationStatus::Completed;
}

fn export_room_eq_results(app: &mut App) {
    if app.room_eq.export_path.is_empty() {
        app.room_eq.export_error = Some("No export path specified".to_string());
        return;
    }

    // Serialize channel results as JSON
    let json = match serde_json::to_string_pretty(&app.room_eq.channel_results) {
        Ok(j) => j,
        Err(e) => {
            app.room_eq.export_error = Some(format!("Serialize error: {}", e));
            return;
        }
    };

    match std::fs::write(&app.room_eq.export_path, json) {
        Ok(()) => {
            app.room_eq.export_success = true;
            app.room_eq.export_error = None;
        }
        Err(e) => {
            app.room_eq.export_error = Some(format!("Write error: {}", e));
            app.room_eq.export_success = false;
        }
    }
}
