//! Room EQ wizard event handlers

use super::PlayerCommand;
use crate::app::{App, FilePickerMode, FilePickerOrigin, InputMode};
use crossterm::event::{KeyCode, KeyEvent};
use sotf_audio_player::room_eq_types::{OptimizationStatus, RoomEqStep};
use std::sync::{Arc, Mutex};

fn room_eq_step_prev_wrap(s: RoomEqStep) -> RoomEqStep {
    match s {
        RoomEqStep::LoadData => RoomEqStep::Export,
        RoomEqStep::Delay => RoomEqStep::LoadData,
        RoomEqStep::Process => RoomEqStep::Delay,
        RoomEqStep::Configure => RoomEqStep::Process,
        RoomEqStep::Optimize => RoomEqStep::Configure,
        RoomEqStep::Review => RoomEqStep::Optimize,
        RoomEqStep::Export => RoomEqStep::Review,
    }
}

fn room_eq_step_next_wrap(s: RoomEqStep) -> RoomEqStep {
    match s {
        RoomEqStep::LoadData => RoomEqStep::Delay,
        RoomEqStep::Delay => RoomEqStep::Process,
        RoomEqStep::Process => RoomEqStep::Configure,
        RoomEqStep::Configure => RoomEqStep::Optimize,
        RoomEqStep::Optimize => RoomEqStep::Review,
        RoomEqStep::Review => RoomEqStep::Export,
        RoomEqStep::Export => RoomEqStep::LoadData,
    }
}

/// Auto-start optimization when entering the Optimize step, if data is loaded and not already running.
pub fn auto_start_optimization(app: &mut App) {
    if app.room_eq.opt_status == OptimizationStatus::Idle
        && !app.room_eq.channel_measurements.is_empty()
    {
        spawn_room_eq_optimization(app);
    }
}

/// Open the file explorer for Room EQ measurement selection if no data is loaded.
pub fn auto_open_load_data(app: &mut App) {
    if app.room_eq.file_path.is_empty() && app.room_eq.channel_measurements.is_empty() {
        app.open_file_explorer(
            FilePickerOrigin::RoomEqFilePath,
            FilePickerMode::File,
            "Select Room EQ Measurements (JSON)",
            Some(&app.room_eq.file_path.clone()),
            Some("json"),
        );
    }
}

pub fn handle_room_eq_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    // Esc: exit editing if active, then two-level focus (content → step tab → configure tab)
    if key.code == KeyCode::Esc {
        // First dismiss numerical direct-edit mode
        if app.room_eq.editing_value {
            app.room_eq.editing_value = false;
            app.room_eq.edit_buffer.clear();
            return None;
        }
        match app.room_eq.step {
            RoomEqStep::LoadData if app.room_eq.editing_file_path => {
                app.room_eq.editing_file_path = false;
                app.clear_autocomplete();
                return None;
            }
            RoomEqStep::Export if app.room_eq.editing_export_path => {
                app.room_eq.editing_export_path = false;
                app.clear_autocomplete();
                return None;
            }
            _ => {}
        }
        if app.room_eq.step_tab_focused {
            app.room_eq.step_tab_focused = false;
            app.input_mode = InputMode::Configure;
        } else {
            app.room_eq.step_tab_focused = true;
        }
        return None;
    }

    // When the step tab bar has focus, Left/Right change step, Up goes to
    // the top-level configure tab bar, Down/Enter returns to step content.
    if app.room_eq.step_tab_focused {
        match key.code {
            KeyCode::Left | KeyCode::BackTab => {
                app.room_eq.step = room_eq_step_prev_wrap(app.room_eq.step);
            }
            KeyCode::Right | KeyCode::Tab => {
                app.room_eq.step = room_eq_step_next_wrap(app.room_eq.step);
            }
            KeyCode::Up => {
                app.room_eq.step_tab_focused = false;
                app.input_mode = InputMode::Configure;
            }
            KeyCode::Down | KeyCode::Enter => {
                app.room_eq.step_tab_focused = false;
            }
            _ => {}
        }
        if app.room_eq.step == RoomEqStep::Optimize {
            auto_start_optimization(app);
        }
        return None;
    }

    match app.room_eq.step {
        RoomEqStep::LoadData => handle_load_data_keys(app, key),
        RoomEqStep::Delay => handle_delay_detection_keys(app, key),
        RoomEqStep::Process => {
            use sotf_audio_player::room_eq_types::RoomEqWizardMode;
            match key.code {
                KeyCode::Char('1') => {
                    app.room_eq.wizard_mode = RoomEqWizardMode::Simple;
                }
                KeyCode::Char('2') => {
                    app.room_eq.wizard_mode = RoomEqWizardMode::Full;
                }
                _ => {}
            }
            None
        }
        RoomEqStep::Configure => handle_configure_keys(app, key),
        RoomEqStep::Optimize => handle_optimize_keys(app, key),
        RoomEqStep::Review => handle_review_keys(app, key),
        RoomEqStep::Export => handle_export_keys(app, key),
    }
}

fn handle_load_data_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    if app.room_eq.editing_file_path {
        match key.code {
            KeyCode::Enter => {
                app.room_eq.editing_file_path = false;
                app.clear_autocomplete();
                load_room_eq_measurements(app);
            }
            KeyCode::Tab => {
                app.zsh_tab_complete(
                    crate::app::app_autocomplete::get_room_eq_file_path,
                    crate::app::app_autocomplete::set_room_eq_file_path,
                    crate::app::app_autocomplete::AutocompleteKind::FilePath,
                );
            }
            KeyCode::BackTab => {
                app.zsh_backtab_complete(crate::app::app_autocomplete::set_room_eq_file_path);
            }
            KeyCode::Down => {
                app.autocomplete_down(crate::app::app_autocomplete::set_room_eq_file_path);
            }
            KeyCode::Up => {
                app.autocomplete_up(crate::app::app_autocomplete::set_room_eq_file_path);
            }
            KeyCode::Backspace => {
                app.room_eq.file_path.pop();
                app.refresh_autocomplete_inline(
                    crate::app::app_autocomplete::get_room_eq_file_path,
                    crate::app::app_autocomplete::AutocompleteKind::FilePath,
                );
            }
            KeyCode::F(2) => {
                let start = app.room_eq.file_path.clone();
                app.open_file_explorer(
                    FilePickerOrigin::RoomEqFilePath,
                    FilePickerMode::File,
                    "Select Room EQ Measurements (JSON)",
                    Some(&start),
                    Some("json"),
                );
            }
            KeyCode::Char(c) => {
                app.room_eq.file_path.push(c);
                app.refresh_autocomplete_inline(
                    crate::app::app_autocomplete::get_room_eq_file_path,
                    crate::app::app_autocomplete::AutocompleteKind::FilePath,
                );
            }
            _ => {}
        }
        return None;
    }
    match key.code {
        KeyCode::Up => {
            app.room_eq.step_tab_focused = true;
        }
        KeyCode::Enter => {
            let start = app.room_eq.file_path.clone();
            app.open_file_explorer(
                FilePickerOrigin::RoomEqFilePath,
                FilePickerMode::File,
                "Select Room EQ Measurements (JSON)",
                Some(&start),
                Some("json"),
            );
        }
        _ => {}
    }
    None
}

fn handle_configure_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    // Numerical direct-edit mode
    if app.room_eq.editing_value {
        match key.code {
            KeyCode::Enter => {
                set_room_eq_field_from_string(app);
                app.room_eq.editing_value = false;
                app.room_eq.edit_buffer.clear();
            }
            KeyCode::Esc => {
                app.room_eq.editing_value = false;
                app.room_eq.edit_buffer.clear();
            }
            KeyCode::Backspace => {
                app.room_eq.edit_buffer.pop();
            }
            KeyCode::Char(c) if c.is_ascii_digit() || c == '.' || c == '-' => {
                app.room_eq.edit_buffer.push(c);
            }
            _ => {}
        }
        return None;
    }
    match key.code {
        KeyCode::Up => {
            if app.room_eq.selected_field > 0 {
                app.room_eq.selected_field -= 1;
            } else {
                app.room_eq.step_tab_focused = true;
            }
        }
        KeyCode::Down => {
            if app.room_eq.selected_field < ROOM_EQ_FIELD_COUNT - 1 {
                app.room_eq.selected_field += 1;
            }
        }
        KeyCode::Left | KeyCode::Char('-') => {
            adjust_room_eq_field(app, -1);
        }
        KeyCode::Right | KeyCode::Char('+') => {
            adjust_room_eq_field(app, 1);
        }
        KeyCode::Tab => {
            if app.room_eq.selected_field < ROOM_EQ_FIELD_COUNT - 1 {
                app.room_eq.selected_field += 1;
            } else {
                app.room_eq.selected_field = 0;
            }
        }
        KeyCode::Enter => {
            let f = app.room_eq.selected_field;
            if is_room_eq_field_numerical(f) {
                app.room_eq.edit_buffer = room_eq_field_value_string(app, f);
                app.room_eq.editing_value = true;
            }
            // Booleans: toggle
            else if matches!(f, 11 | 13 | 14 | 17 | 19 | 21 | 23) {
                adjust_room_eq_field(app, 1);
            }
        }
        KeyCode::BackTab => {
            app.room_eq.step = RoomEqStep::LoadData;
        }
        _ => {}
    }
    None
}

fn handle_optimize_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            if !app.room_eq.opt_log_lines.is_empty()
                && app.room_eq.opt_log_scroll < app.room_eq.opt_log_lines.len().saturating_sub(1)
            {
                app.room_eq.opt_log_scroll += 1;
            } else {
                app.room_eq.step_tab_focused = true;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.room_eq.opt_log_scroll > 0 {
                app.room_eq.opt_log_scroll -= 1;
            }
        }
        KeyCode::Home => {
            app.room_eq.opt_log_scroll = app.room_eq.opt_log_lines.len().saturating_sub(1);
        }
        KeyCode::End => {
            app.room_eq.opt_log_scroll = 0;
        }
        KeyCode::Enter => match &app.room_eq.opt_status {
            OptimizationStatus::Idle
            | OptimizationStatus::Failed
            | OptimizationStatus::Cancelled
            | OptimizationStatus::Completed => {
                spawn_room_eq_optimization(app);
            }
            OptimizationStatus::Running => {}
        },
        KeyCode::BackTab => {
            app.room_eq.step = RoomEqStep::Configure;
        }
        _ => {}
    }
    None
}

fn handle_review_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Up => {
            if app.room_eq.selected_channel > 0 {
                app.room_eq.selected_channel -= 1;
            } else {
                app.room_eq.step_tab_focused = true;
            }
        }
        KeyCode::Down => {
            if !app.room_eq.channel_results.is_empty()
                && app.room_eq.selected_channel < app.room_eq.channel_results.len() - 1
            {
                app.room_eq.selected_channel += 1;
            }
        }
        KeyCode::BackTab => {
            app.room_eq.step = RoomEqStep::Optimize;
        }
        _ => {}
    }
    None
}

fn handle_export_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    if app.room_eq.editing_export_path {
        match key.code {
            KeyCode::Enter => {
                app.room_eq.editing_export_path = false;
                app.clear_autocomplete();
                export_room_eq_results(app);
            }
            KeyCode::Tab => {
                app.zsh_tab_complete(
                    crate::app::app_autocomplete::get_room_eq_export_path,
                    crate::app::app_autocomplete::set_room_eq_export_path,
                    crate::app::app_autocomplete::AutocompleteKind::FilePath,
                );
            }
            KeyCode::BackTab => {
                app.zsh_backtab_complete(crate::app::app_autocomplete::set_room_eq_export_path);
            }
            KeyCode::Down => {
                app.autocomplete_down(crate::app::app_autocomplete::set_room_eq_export_path);
            }
            KeyCode::Up => {
                app.autocomplete_up(crate::app::app_autocomplete::set_room_eq_export_path);
            }
            KeyCode::Backspace => {
                app.room_eq.export_path.pop();
                app.refresh_autocomplete_inline(
                    crate::app::app_autocomplete::get_room_eq_export_path,
                    crate::app::app_autocomplete::AutocompleteKind::FilePath,
                );
            }
            KeyCode::F(2) => {
                let start = app.room_eq.export_path.clone();
                app.open_file_explorer(
                    FilePickerOrigin::RoomEqExportPath,
                    FilePickerMode::File,
                    "Select Export Path (JSON)",
                    Some(&start),
                    Some("json"),
                );
            }
            KeyCode::Char(c) => {
                app.room_eq.export_path.push(c);
                app.refresh_autocomplete_inline(
                    crate::app::app_autocomplete::get_room_eq_export_path,
                    crate::app::app_autocomplete::AutocompleteKind::FilePath,
                );
            }
            _ => {}
        }
        return None;
    }
    match key.code {
        KeyCode::Up => {
            app.room_eq.step_tab_focused = true;
        }
        KeyCode::Enter => {
            app.room_eq.editing_export_path = true;
        }
        KeyCode::BackTab => {
            app.room_eq.step = RoomEqStep::Review;
        }
        // 'a' = Apply optimization output to the live plugin chain.
        // Auto-detects rack vs graph mode (matches the GPUI buttons).
        KeyCode::Char('a') => match app.apply_room_eq_to_chain() {
            Ok(msg) => {
                app.room_eq.apply_status = Some(msg);
                app.room_eq.apply_error = None;
            }
            Err(e) => {
                app.room_eq.apply_error = Some(e);
                app.room_eq.apply_status = None;
            }
        },
        _ => {}
    }
    None
}

/// Total number of adjustable fields in the Room EQ configure step
const ROOM_EQ_FIELD_COUNT: usize = 24;

fn is_room_eq_field_numerical(field: usize) -> bool {
    matches!(field, 0..=6 | 9 | 10 | 18 | 20 | 22)
}

fn room_eq_field_value_string(app: &App, field: usize) -> String {
    let c = &app.room_eq.config;
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
        18 => format!("{:.1}", c.target_response.slope_db_per_octave),
        20 => format!("{:.0}", c.excursion_protection.manual_f3_hz),
        22 => format!("{:.0}", c.schroeder_split.schroeder_freq),
        _ => String::new(),
    }
}

fn set_room_eq_field_from_string(app: &mut App) {
    let c = &mut app.room_eq.config;
    let buf = &app.room_eq.edit_buffer;
    match app.room_eq.selected_field {
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
        18 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.target_response.slope_db_per_octave = v.clamp(-3.0, 0.0);
            }
        }
        20 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.excursion_protection.manual_f3_hz = v.clamp(20.0, 200.0);
            }
        }
        22 => {
            if let Ok(v) = buf.parse::<f64>() {
                c.schroeder_split.schroeder_freq = v.clamp(100.0, 1000.0);
            }
        }
        _ => {}
    }
}

fn adjust_room_eq_field(app: &mut App, delta: i32) {
    use sotf_audio_player::room_eq_types::{MultiSpeakerMode, RoomEqOptimizationMode};

    let c = &mut app.room_eq.config;
    match app.room_eq.selected_field {
        // Basic
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
        // Optimization
        8 => {
            let algos = ["cobyla", "autoeq:de", "nelder-mead"];
            c.algorithm = super::cycle_string(&c.algorithm, &algos, delta);
        }
        9 => {
            let n = c.max_iter as i32 + delta * 1000;
            c.max_iter = n.clamp(1000, 100000) as usize;
        }
        10 => {
            let n = c.population as i32 + delta * 10;
            c.population = n.clamp(10, 200) as usize;
        }
        11 => c.refine = !c.refine,
        12 => {
            c.local_algo = super::cycle_string(&c.local_algo, &["cobyla", "nelder-mead"], delta);
        }
        13 => c.psychoacoustic = !c.psychoacoustic,
        14 => c.asymmetric_loss = !c.asymmetric_loss,
        // Mode
        15 => {
            let modes = RoomEqOptimizationMode::all();
            let idx = modes.iter().position(|m| *m == c.mode).unwrap_or(0);
            let new_idx = if delta > 0 {
                (idx + 1) % modes.len()
            } else {
                (idx + modes.len() - 1) % modes.len()
            };
            c.mode = modes[new_idx];
        }
        16 => {
            let modes = MultiSpeakerMode::all();
            let idx = modes
                .iter()
                .position(|m| *m == c.multi_speaker_mode)
                .unwrap_or(0);
            let new_idx = if delta > 0 {
                (idx + 1) % modes.len()
            } else {
                (idx + modes.len() - 1) % modes.len()
            };
            c.multi_speaker_mode = modes[new_idx];
        }
        // Target Response
        17 => c.target_response.enabled = !c.target_response.enabled,
        18 => {
            c.target_response.slope_db_per_octave =
                (c.target_response.slope_db_per_octave + delta as f64 * 0.1).clamp(-3.0, 0.0)
        }
        // Excursion Protection
        19 => c.excursion_protection.enabled = !c.excursion_protection.enabled,
        20 => {
            c.excursion_protection.manual_f3_hz =
                (c.excursion_protection.manual_f3_hz + delta as f64 * 5.0).clamp(20.0, 200.0)
        }
        // Schroeder Split
        21 => c.schroeder_split.enabled = !c.schroeder_split.enabled,
        22 => {
            c.schroeder_split.schroeder_freq =
                (c.schroeder_split.schroeder_freq + delta as f64 * 10.0).clamp(100.0, 1000.0)
        }
        // Phase Alignment
        23 => c.phase_alignment.enabled = !c.phase_alignment.enabled,
        _ => {}
    }
}

pub(crate) fn load_room_eq_measurements(app: &mut App) {
    use sotf_audio_player::room_eq_types::RoomEqMeasurementsFile;

    let path = &app.room_eq.file_path;
    if path.is_empty() {
        app.room_eq.load_error = Some("No file path specified".to_string());
        return;
    }

    let base_dir = std::path::Path::new(path).parent();

    match std::fs::read_to_string(path) {
        Ok(contents) => match RoomEqMeasurementsFile::load_from_json(&contents, base_dir) {
            Ok(channels) => {
                app.room_eq.channel_measurements = channels;
                app.room_eq.load_error = None;
                // Pre-seed Delay Detection form from the recording
                // session metadata when the file carries it. Only fields
                // the user hasn't already customised this session are
                // touched so we don't stomp on an active override.
                if let Some(hints) =
                    RoomEqMeasurementsFile::extract_delay_detection_hints(&contents)
                {
                    let dd = &mut app.room_eq.delay_detection;
                    if let Some(sr) = hints.sample_rate {
                        dd.sample_rate = sr;
                    }
                    if dd.output_device_name.is_none() {
                        dd.output_device_name = hints.playback_device_name;
                    }
                    if dd.input_device_name.is_none() {
                        dd.input_device_name = hints.recording_device_name;
                    }
                }
            }
            Err(e) => {
                app.room_eq.load_error = Some(e);
                app.room_eq.channel_measurements.clear();
            }
        },
        Err(e) => {
            app.room_eq.load_error = Some(format!("Read error: {}", e));
            app.room_eq.channel_measurements.clear();
        }
    }
}

#[allow(clippy::type_complexity)]
static ROOM_OPT_RESULT: std::sync::OnceLock<
    Arc<Mutex<Option<Result<sotf_audio_player::autoeq::RoomOptimizationResult, String>>>>,
> = std::sync::OnceLock::new();
static ROOM_OPT_PROGRESS: std::sync::OnceLock<
    Arc<Mutex<Option<sotf_audio_player::autoeq::RoomOptimizationProgress>>>,
> = std::sync::OnceLock::new();

pub fn poll_room_eq_optimization(app: &mut App) -> bool {
    use sotf_audio_player::room_eq_types::{ChannelOptResult, EqFilterConfig, OptimizationStatus};

    if app.room_eq.opt_status != OptimizationStatus::Running {
        return false;
    }

    let result_slot = ROOM_OPT_RESULT
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();
    let progress_slot = ROOM_OPT_PROGRESS
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone();

    if let Ok(mut guard) = result_slot.lock()
        && let Some(result) = guard.take()
    {
        match result {
            Ok(r) => {
                // Convert autoeq results to TUI ChannelOptResult
                app.room_eq.channel_results = r
                    .channel_results
                    .iter()
                    .map(|(name, ch)| ChannelOptResult {
                        channel_name: name.clone(),
                        pre_score: ch.pre_score,
                        post_score: ch.post_score,
                        eq_filters: ch
                            .biquads
                            .iter()
                            .map(|b| EqFilterConfig {
                                filter_type: format!("{:?}", b.filter_type),
                                frequency: b.freq,
                                q: b.q,
                                gain_db: b.db_gain,
                            })
                            .collect(),
                        broadband_filters: vec![],
                        crossover_freqs: None,
                        driver_gains: None,
                        original_response: Some(
                            ch.initial_curve
                                .freq
                                .iter()
                                .zip(ch.initial_curve.spl.iter())
                                .map(|(&f, &s)| (f, s))
                                .collect(),
                        ),
                        corrected_response: Some(
                            ch.final_curve
                                .freq
                                .iter()
                                .zip(ch.final_curve.spl.iter())
                                .map(|(&f, &s)| (f, s))
                                .collect(),
                        ),
                        normalized_response: None,
                        target_curve: None,
                        group_delay_before: None,
                        group_delay_after: None,
                        phase_response_before: None,
                        phase_response_after: None,
                        impulse_response: None,
                    })
                    .collect();
                // Capture the full DSP chain output so the "Apply to
                // chain" action (rack/graph) can run the same algorithm
                // the GPUI app uses.
                app.room_eq.dsp_output = Some(r.to_dsp_chain_output());
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

    if let Ok(mut guard) = progress_slot.lock()
        && let Some(p) = guard.take()
    {
        app.room_eq.opt_progress = p.overall_progress as f32;
        app.room_eq.opt_iteration = p.iteration;
        app.room_eq.opt_max_iter = p.max_iterations;
        app.room_eq.opt_loss = p.loss;
        app.room_eq.opt_current_speaker = p.current_speaker.clone();
        app.room_eq.opt_total_speakers = p.total_speakers;
        app.room_eq.opt_status_message = p.message.clone();
        if p.loss > 0.0 {
            // Use running index as X so the chart shows all iterations across all speakers
            let idx = app.room_eq.loss_history.len();
            app.room_eq.loss_history.push((idx, p.loss));
        }
        if let Some(msg) = p.message {
            for line in msg.lines() {
                app.room_eq.opt_log_lines.push_back(line.to_string());
            }
            while app.room_eq.opt_log_lines.len() > 300 {
                app.room_eq.opt_log_lines.pop_front();
            }
        }
        // Auto-generate log entries for iteration milestones
        if p.iteration > 0 && p.iteration % 100 == 0 {
            let msg = format!(
                "[{}] iter {}/{} loss={:.6}",
                p.current_speaker, p.iteration, p.max_iterations, p.loss
            );
            app.room_eq.opt_log_lines.push_back(msg);
            while app.room_eq.opt_log_lines.len() > 300 {
                app.room_eq.opt_log_lines.pop_front();
            }
        }
        return true;
    }

    false
}

fn spawn_room_eq_optimization(app: &mut App) {
    if app.room_eq.channel_measurements.is_empty() {
        app.room_eq.opt_status = OptimizationStatus::Failed;
        app.room_eq.opt_error = Some("No measurements loaded".to_string());
        return;
    }

    app.room_eq.opt_status = OptimizationStatus::Running;
    app.room_eq.opt_error = None;
    app.room_eq.opt_progress = 0.0;
    app.room_eq.opt_iteration = 0;
    app.room_eq.opt_loss = 0.0;
    app.room_eq.opt_current_speaker = String::new();
    app.room_eq.opt_total_speakers = 0;
    app.room_eq.opt_status_message = None;
    app.room_eq.channel_results.clear();
    app.room_eq.dsp_output = None;
    app.room_eq.apply_status = None;
    app.room_eq.apply_error = None;
    app.room_eq.loss_history.clear();
    app.room_eq.opt_log_lines.clear();
    app.room_eq.opt_log_scroll = 0;

    // Build curves from loaded measurements
    let measurements = app.room_eq.channel_measurements.clone();
    let config = app.room_eq.config.clone();
    // Probe-based arrival times from the Delay Detection step (None if the
    // user skipped that step; in that case the optimizer falls back to
    // WAV-onset detection for each channel).
    let probe_arrivals = app.room_eq.delay_detection.probe_arrival_map();

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

        // Convert measurements to speaker configs
        let mut speakers = std::collections::HashMap::new();
        for m in &measurements {
            let freq: Vec<f64> = m
                .measurement
                .frequencies
                .iter()
                .map(|&f| f as f64)
                .collect();
            let spl: Vec<f64> = m
                .measurement
                .magnitude_db
                .iter()
                .map(|&db| db as f64)
                .collect();
            let curve = autoeq::Curve {
                freq: ndarray::Array1::from(freq),
                spl: ndarray::Array1::from(spl),
                phase: None,
                ..Default::default()
            };
            speakers.insert(
                m.channel_name.clone(),
                SpeakerConfig::Single(MeasurementSource::InMemory(curve)),
            );
        }

        let optimizer = config.to_optimizer_config();

        let room_config = RoomConfig {
            version: autoeq::roomeq::default_config_version(),
            system: None,
            speakers,
            crossovers: None,
            target_curve: None,
            optimizer,
            recording_config: None,
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

pub(crate) fn export_room_eq_results(app: &mut App) {
    if app.room_eq.export_path.is_empty() {
        app.room_eq.export_error = Some("No export path specified".to_string());
        return;
    }

    let formats = sotf_audio_player::autoeq::EQ_EXPORT_FORMAT_OPTIONS;
    let (format_id, _, _) = formats
        .get(app.room_eq.export_format)
        .copied()
        .unwrap_or(("json", "JSON", ".json"));

    // Collect all EQ filters from channel results and convert to Biquad
    let biquads: Vec<math_audio_iir_fir::Biquad> = app
        .room_eq
        .channel_results
        .iter()
        .flat_map(|ch| {
            ch.eq_filters.iter().map(|f| {
                let ft = match f.filter_type.as_str() {
                    "peak" => math_audio_iir_fir::BiquadFilterType::Peak,
                    "lowshelf" => math_audio_iir_fir::BiquadFilterType::Lowshelf,
                    "highshelf" => math_audio_iir_fir::BiquadFilterType::Highshelf,
                    "lowpass" => math_audio_iir_fir::BiquadFilterType::Lowpass,
                    "highpass" => math_audio_iir_fir::BiquadFilterType::Highpass,
                    _ => math_audio_iir_fir::BiquadFilterType::Peak,
                };
                math_audio_iir_fir::Biquad::new(ft, f.frequency, 48000.0, f.q, f.gain_db)
            })
        })
        .collect();

    let content =
        match sotf_audio_player::autoeq::format_peq_export(format_id, "Room EQ", &biquads, 48000) {
            Ok(c) => c,
            Err(e) => {
                app.room_eq.export_error = Some(format!("Format error: {}", e));
                return;
            }
        };

    match std::fs::write(&app.room_eq.export_path, content) {
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

// ============================================================================
// Delay step (simplified — just table + editing, no probe form)
// ============================================================================

/// Simplified delay step handler — just row navigation + editing.
/// The probe form has moved to the Recording wizard's Probe step.
fn handle_delay_detection_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    // --- Row edit sub-mode: typing a new arrival time for a row ---
    if let Some(row) = app.room_eq.dd_edit_row {
        match key.code {
            KeyCode::Esc => {
                app.room_eq.editing_value = false;
                app.room_eq.edit_buffer.clear();
                app.room_eq.dd_edit_row = None;
            }
            KeyCode::Enter => {
                if let Ok(v) = app.room_eq.edit_buffer.parse::<f64>()
                    && row < app.room_eq.delay_detection.edited_arrival_ms.len()
                {
                    app.room_eq.delay_detection.edited_arrival_ms[row] = v;
                }
                app.room_eq.editing_value = false;
                app.room_eq.edit_buffer.clear();
                app.room_eq.dd_edit_row = None;
            }
            KeyCode::Char(c) if c.is_ascii_digit() || c == '.' || c == '-' => {
                app.room_eq.edit_buffer.push(c);
            }
            KeyCode::Backspace => {
                app.room_eq.edit_buffer.pop();
            }
            _ => {}
        }
        return None;
    }

    let num_rows = app
        .room_eq
        .delay_detection
        .results
        .as_ref()
        .map(|r| r.channels.len())
        .unwrap_or(0);

    match key.code {
        // j / k: navigate results table row cursor
        KeyCode::Char('j') | KeyCode::Down if num_rows > 0 => {
            app.room_eq.dd_selected_row = (app.room_eq.dd_selected_row + 1) % num_rows;
        }
        KeyCode::Char('k') | KeyCode::Up if num_rows > 0 => {
            app.room_eq.dd_selected_row = (app.room_eq.dd_selected_row + num_rows - 1) % num_rows;
        }
        // e: start editing the selected row's delay override
        KeyCode::Char('e') if num_rows > 0 => {
            let row = app.room_eq.dd_selected_row.min(num_rows - 1);
            app.room_eq.dd_edit_row = Some(row);
            app.room_eq.editing_value = true;
            app.room_eq.edit_buffer.clear();
        }
        _ => {}
    }
    None
}

/// No-op poll — the delay step no longer runs probes (that moved to
/// the Recording wizard's Probe step). Kept as a public function
/// because `main.rs` still calls it on every tick.
pub fn poll_delay_detection(_app: &mut App) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Screen;
    use crate::events::tests::{key, make_app};

    fn app_on_room_eq_content() -> App {
        let mut app = make_app();
        app.current_screen = Screen::Configure;
        app.configure_sub_screen = crate::app::ConfigureSubScreen::RoomEq;
        app.input_mode = InputMode::ConfigureRoomEq;
        app.room_eq.step_tab_focused = false;
        app
    }

    #[test]
    fn room_eq_step_prev_does_not_wrap() {
        assert_eq!(RoomEqStep::LoadData.previous(), None);
    }

    #[test]
    fn room_eq_step_next_does_not_wrap() {
        assert_eq!(RoomEqStep::Export.next(), None);
    }

    #[test]
    fn room_eq_progress_clamped_to_one() {
        let total_iters: usize = 100;
        let done_iters: usize = 150;
        let progress = if total_iters > 0 {
            (done_iters as f32 / total_iters as f32).min(1.0)
        } else {
            0.0
        };
        assert_eq!(progress, 1.0);
        assert!(progress <= 1.0);
    }

    // ── Esc: two-level focus (content → step tab → configure tab) ────────

    #[test]
    fn room_eq_esc_from_content_goes_to_step_tab() {
        let mut app = app_on_room_eq_content();
        app.room_eq.step = RoomEqStep::LoadData;

        handle_room_eq_keys(&mut app, key(KeyCode::Esc));
        assert!(
            app.room_eq.step_tab_focused,
            "Esc from content should focus step tab bar"
        );
        assert!(
            app.input_mode.is_configure_sub_screen(),
            "should NOT jump to configure tab bar"
        );
    }

    #[test]
    fn room_eq_esc_from_step_tab_goes_to_configure_tab() {
        let mut app = app_on_room_eq_content();
        app.room_eq.step = RoomEqStep::Configure;
        app.room_eq.step_tab_focused = true;

        handle_room_eq_keys(&mut app, key(KeyCode::Esc));
        assert_eq!(app.input_mode, InputMode::Configure);
        assert!(!app.room_eq.step_tab_focused);
    }

    #[test]
    fn room_eq_esc_chain_content_to_step_to_configure() {
        let mut app = app_on_room_eq_content();
        app.room_eq.step = RoomEqStep::Optimize;

        // First Esc → step tab bar
        handle_room_eq_keys(&mut app, key(KeyCode::Esc));
        assert!(app.room_eq.step_tab_focused);
        assert!(app.input_mode.is_configure_sub_screen());

        // Second Esc → configure tab bar
        handle_room_eq_keys(&mut app, key(KeyCode::Esc));
        assert_eq!(app.input_mode, InputMode::Configure);
        assert!(!app.room_eq.step_tab_focused);
    }

    // ── Step tab bar navigation ──────────────────────────────────────────

    #[test]
    fn room_eq_step_tab_right_changes_step() {
        let mut app = app_on_room_eq_content();
        app.room_eq.step = RoomEqStep::LoadData;
        app.room_eq.step_tab_focused = true;

        handle_room_eq_keys(&mut app, key(KeyCode::Right));
        assert_eq!(app.room_eq.step, RoomEqStep::Delay);
        assert!(app.room_eq.step_tab_focused, "should stay on step tab bar");
    }

    #[test]
    fn room_eq_step_tab_left_changes_step() {
        let mut app = app_on_room_eq_content();
        app.room_eq.step = RoomEqStep::Configure;
        app.room_eq.step_tab_focused = true;

        handle_room_eq_keys(&mut app, key(KeyCode::Left));
        assert_eq!(app.room_eq.step, RoomEqStep::Process);
        assert!(app.room_eq.step_tab_focused);
    }

    #[test]
    fn room_eq_step_tab_wraps_forward() {
        let mut app = app_on_room_eq_content();
        app.room_eq.step = RoomEqStep::Export;
        app.room_eq.step_tab_focused = true;

        handle_room_eq_keys(&mut app, key(KeyCode::Right));
        assert_eq!(app.room_eq.step, RoomEqStep::LoadData);
    }

    #[test]
    fn room_eq_step_tab_wraps_backward() {
        let mut app = app_on_room_eq_content();
        app.room_eq.step = RoomEqStep::LoadData;
        app.room_eq.step_tab_focused = true;

        handle_room_eq_keys(&mut app, key(KeyCode::Left));
        assert_eq!(app.room_eq.step, RoomEqStep::Export);
    }

    #[test]
    fn room_eq_step_tab_up_goes_to_configure_tab() {
        let mut app = app_on_room_eq_content();
        app.room_eq.step_tab_focused = true;

        handle_room_eq_keys(&mut app, key(KeyCode::Up));
        assert_eq!(app.input_mode, InputMode::Configure);
        assert!(!app.room_eq.step_tab_focused);
    }

    #[test]
    fn room_eq_step_tab_down_enters_content() {
        let mut app = app_on_room_eq_content();
        app.room_eq.step_tab_focused = true;

        handle_room_eq_keys(&mut app, key(KeyCode::Down));
        assert!(!app.room_eq.step_tab_focused);
        assert!(app.input_mode.is_configure_sub_screen());
    }

    #[test]
    fn room_eq_step_tab_enter_enters_content() {
        let mut app = app_on_room_eq_content();
        app.room_eq.step_tab_focused = true;

        handle_room_eq_keys(&mut app, key(KeyCode::Enter));
        assert!(!app.room_eq.step_tab_focused);
        assert!(app.input_mode.is_configure_sub_screen());
    }

    // ── Content-level: Up at top goes to step tab bar ────────────────────

    #[test]
    fn room_eq_up_on_load_data_goes_to_step_tab() {
        let mut app = app_on_room_eq_content();
        app.room_eq.step = RoomEqStep::LoadData;

        handle_room_eq_keys(&mut app, key(KeyCode::Up));
        assert!(
            app.room_eq.step_tab_focused,
            "Up from LoadData should go to step tab"
        );
        assert!(
            app.input_mode.is_configure_sub_screen(),
            "should NOT jump to configure tab"
        );
    }

    #[test]
    fn room_eq_up_on_configure_first_field_goes_to_step_tab() {
        let mut app = app_on_room_eq_content();
        app.room_eq.step = RoomEqStep::Configure;
        app.room_eq.selected_field = 0;

        handle_room_eq_keys(&mut app, key(KeyCode::Up));
        assert!(app.room_eq.step_tab_focused);
        assert!(app.input_mode.is_configure_sub_screen());
    }

    #[test]
    fn room_eq_up_on_optimize_goes_to_step_tab() {
        let mut app = app_on_room_eq_content();
        app.room_eq.step = RoomEqStep::Optimize;

        handle_room_eq_keys(&mut app, key(KeyCode::Up));
        assert!(app.room_eq.step_tab_focused);
        assert!(app.input_mode.is_configure_sub_screen());
    }

    #[test]
    fn room_eq_up_on_review_first_channel_goes_to_step_tab() {
        let mut app = app_on_room_eq_content();
        app.room_eq.step = RoomEqStep::Review;
        app.room_eq.selected_channel = 0;

        handle_room_eq_keys(&mut app, key(KeyCode::Up));
        assert!(app.room_eq.step_tab_focused);
        assert!(app.input_mode.is_configure_sub_screen());
    }

    #[test]
    fn room_eq_up_on_export_goes_to_step_tab() {
        let mut app = app_on_room_eq_content();
        app.room_eq.step = RoomEqStep::Export;

        handle_room_eq_keys(&mut app, key(KeyCode::Up));
        assert!(app.room_eq.step_tab_focused);
        assert!(app.input_mode.is_configure_sub_screen());
    }

    // ── Content-level: Left/Right adjusts values, BackTab goes back ────

    #[test]
    fn room_eq_content_left_right_adjusts_configure_field() {
        let mut app = app_on_room_eq_content();
        app.room_eq.step = RoomEqStep::Configure;
        app.room_eq.selected_field = 0; // num_filters
        let before = app.room_eq.config.num_filters;
        handle_room_eq_keys(&mut app, key(KeyCode::Right));
        assert_eq!(app.room_eq.config.num_filters, before + 1);
        handle_room_eq_keys(&mut app, key(KeyCode::Left));
        assert_eq!(app.room_eq.config.num_filters, before);
        assert_eq!(app.room_eq.step, RoomEqStep::Configure);
    }

    #[test]
    fn room_eq_enter_on_numerical_enters_edit_mode() {
        let mut app = app_on_room_eq_content();
        app.room_eq.step = RoomEqStep::Configure;
        app.room_eq.selected_field = 0; // num_filters (numerical)
        handle_room_eq_keys(&mut app, key(KeyCode::Enter));
        assert!(app.room_eq.editing_value);
        assert!(!app.room_eq.edit_buffer.is_empty());
    }

    #[test]
    fn room_eq_edit_mode_enter_commits() {
        let mut app = app_on_room_eq_content();
        app.room_eq.step = RoomEqStep::Configure;
        app.room_eq.selected_field = 0;
        app.room_eq.editing_value = true;
        app.room_eq.edit_buffer = "10".to_string();
        handle_room_eq_keys(&mut app, key(KeyCode::Enter));
        assert!(!app.room_eq.editing_value);
        assert_eq!(app.room_eq.config.num_filters, 10);
    }

    #[test]
    fn room_eq_content_backtab_goes_back() {
        let mut app = app_on_room_eq_content();
        app.room_eq.step = RoomEqStep::Optimize;

        handle_room_eq_keys(&mut app, key(KeyCode::BackTab));
        assert_eq!(app.room_eq.step, RoomEqStep::Configure);

        handle_room_eq_keys(&mut app, key(KeyCode::BackTab));
        assert_eq!(app.room_eq.step, RoomEqStep::LoadData);
    }
}
