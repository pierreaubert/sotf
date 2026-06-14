use super::super::PlayerCommand;
use super::misc::adjust_spinorama_field;
use super::misc::is_spinorama_field_numerical;
use super::misc::set_spinorama_field_from_string;
use super::spawn::spawn_spinorama_optimization;
use super::spawn::spawn_spinorama_speaker_load;
use crate::app::{App, InputMode};
use crossterm::event::{KeyCode, KeyEvent};

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
