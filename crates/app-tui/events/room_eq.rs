//! Room EQ wizard event handlers

use super::PlayerCommand;
use crate::app::{App, FilePickerMode, FilePickerOrigin};
use crossterm::event::{KeyCode, KeyEvent};
use std::sync::{Arc, Mutex};


pub fn handle_room_eq_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    use sotf_audio_player::room_eq_types::{OptimizationStatus, RoomEqStep};

    // Esc goes up one level
    if key.code == KeyCode::Esc {
        match app.room_eq.step {
            RoomEqStep::LoadData => {
                if app.room_eq.editing_file_path {
                    app.room_eq.editing_file_path = false;
                } else {
                    app.configure_tab_focused = true;
                }
            }
            RoomEqStep::Configure => {
                app.room_eq.step = RoomEqStep::LoadData;
            }
            RoomEqStep::Optimize => {
                app.room_eq.step = RoomEqStep::Configure;
            }
            RoomEqStep::Review => {
                app.room_eq.step = RoomEqStep::Optimize;
            }
            RoomEqStep::Export => {
                if app.room_eq.editing_export_path {
                    app.room_eq.editing_export_path = false;
                } else {
                    app.room_eq.step = RoomEqStep::Review;
                }
            }
        }
        return None;
    }

    match app.room_eq.step {
        RoomEqStep::LoadData => {
            if app.room_eq.editing_file_path {
                match key.code {
                    KeyCode::Enter => {
                        app.room_eq.editing_file_path = false;
                        load_room_eq_measurements(app);
                    }
                    KeyCode::Backspace => {
                        app.room_eq.file_path.pop();
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
                    }
                    _ => {}
                }
                return None;
            }
            match key.code {
                KeyCode::Up => {
                    app.configure_tab_focused = true;
                }
                KeyCode::Enter => {
                    app.room_eq.editing_file_path = true;
                }
                KeyCode::Tab => {
                    if !app.room_eq.channel_measurements.is_empty() {
                        app.room_eq.step = RoomEqStep::Configure;
                    }
                }
                _ => {}
            }
            None
        }

        RoomEqStep::Configure => match key.code {
            KeyCode::Up => {
                if app.room_eq.selected_field > 0 {
                    app.room_eq.selected_field -= 1;
                } else {
                    app.configure_tab_focused = true;
                }
                None
            }
            KeyCode::Down => {
                if app.room_eq.selected_field < ROOM_EQ_FIELD_COUNT - 1 {
                    app.room_eq.selected_field += 1;
                }
                None
            }
            KeyCode::Left | KeyCode::Char('-') => {
                adjust_room_eq_field(app, -1);
                None
            }
            KeyCode::Right | KeyCode::Char('+') => {
                adjust_room_eq_field(app, 1);
                None
            }
            KeyCode::Enter | KeyCode::Tab => {
                app.room_eq.step = RoomEqStep::Optimize;
                None
            }
            KeyCode::BackTab => {
                app.room_eq.step = RoomEqStep::LoadData;
                None
            }
            _ => None,
        },

        RoomEqStep::Optimize => match key.code {
            KeyCode::Up => {
                app.configure_tab_focused = true;
                None
            }
            KeyCode::Enter => {
                match &app.room_eq.opt_status {
                    OptimizationStatus::Idle
                    | OptimizationStatus::Failed
                    | OptimizationStatus::Cancelled => {
                        spawn_room_eq_optimization(app);
                    }
                    OptimizationStatus::Completed => {
                        app.room_eq.step = RoomEqStep::Review;
                    }
                    OptimizationStatus::Running => {}
                }
                None
            }
            KeyCode::Tab => {
                if app.room_eq.opt_status == OptimizationStatus::Completed {
                    app.room_eq.step = RoomEqStep::Review;
                } else {
                    app.room_eq.step = RoomEqStep::Configure;
                }
                None
            }
            KeyCode::BackTab => {
                app.room_eq.step = RoomEqStep::Configure;
                None
            }
            _ => None,
        },

        RoomEqStep::Review => match key.code {
            KeyCode::Up => {
                if app.room_eq.selected_channel > 0 {
                    app.room_eq.selected_channel -= 1;
                } else {
                    app.configure_tab_focused = true;
                }
                None
            }
            KeyCode::Down => {
                if !app.room_eq.channel_results.is_empty()
                    && app.room_eq.selected_channel < app.room_eq.channel_results.len() - 1
                {
                    app.room_eq.selected_channel += 1;
                }
                None
            }
            KeyCode::Tab => {
                app.room_eq.step = RoomEqStep::Export;
                None
            }
            KeyCode::BackTab => {
                app.room_eq.step = RoomEqStep::Optimize;
                None
            }
            _ => None,
        },

        RoomEqStep::Export => {
            if app.room_eq.editing_export_path {
                match key.code {
                    KeyCode::Enter => {
                        app.room_eq.editing_export_path = false;
                        export_room_eq_results(app);
                    }
                    KeyCode::Backspace => {
                        app.room_eq.export_path.pop();
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
                    }
                    _ => {}
                }
                return None;
            }
            match key.code {
                KeyCode::Up => {
                    app.configure_tab_focused = true;
                }
                KeyCode::Enter => {
                    app.room_eq.editing_export_path = true;
                }
                KeyCode::Tab => {
                    app.room_eq.step = RoomEqStep::LoadData;
                }
                KeyCode::BackTab => {
                    app.room_eq.step = RoomEqStep::Review;
                }
                _ => {}
            }
            None
        }
    }
}

/// Total number of adjustable fields in the Room EQ configure step
const ROOM_EQ_FIELD_COUNT: usize = 24;

fn cycle_string(current: &str, options: &[&str], delta: i32) -> String {
    let idx = options.iter().position(|&o| o == current).unwrap_or(0);
    let new_idx = if delta > 0 {
        (idx + 1) % options.len()
    } else {
        (idx + options.len() - 1) % options.len()
    };
    options[new_idx].to_string()
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
            c.peq_model = cycle_string(
                &c.peq_model,
                &["pk", "hp-pk", "hp-pk-lp", "ls-pk", "ls-pk-hs"],
                delta,
            );
        }
        // Optimization
        8 => {
            let algos = ["cobyla", "autoeq:de", "nelder-mead"];
            c.algorithm = cycle_string(&c.algorithm, &algos, delta);
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
            c.local_algo = cycle_string(&c.local_algo, &["cobyla", "nelder-mead"], delta);
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
        // Target Tilt
        17 => c.target_tilt.enabled = !c.target_tilt.enabled,
        18 => c.target_tilt.slope = (c.target_tilt.slope + delta as f64 * 0.1).clamp(-3.0, 0.0),
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

    match std::fs::read_to_string(path) {
        Ok(contents) => match RoomEqMeasurementsFile::from_json_str(&contents) {
            Ok(file) => {
                app.room_eq.channel_measurements = file.channels;
                app.room_eq.load_error = None;
            }
            Err(e) => {
                app.room_eq.load_error = Some(format!("Parse error: {}", e));
                app.room_eq.channel_measurements.clear();
            }
        },
        Err(e) => {
            app.room_eq.load_error = Some(format!("Read error: {}", e));
            app.room_eq.channel_measurements.clear();
        }
    }
}

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

    if let Ok(mut guard) = result_slot.lock() {
        if let Some(result) = guard.take() {
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
                        })
                        .collect();
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
        if let Some(p) = guard.take() {
            app.room_eq.opt_progress = p.overall_progress as f32;
            app.room_eq.opt_iteration = p.iteration;
            app.room_eq.opt_max_iter = p.max_iterations;
            app.room_eq.opt_loss = p.loss;
            app.room_eq.loss_history.push((p.iteration, p.loss));
            return true;
        }
    }

    false
}

fn spawn_room_eq_optimization(app: &mut App) {
    use sotf_audio_player::room_eq_types::OptimizationStatus;

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
    app.room_eq.channel_results.clear();
    app.room_eq.loss_history.clear();

    // Build curves from loaded measurements
    let measurements = app.room_eq.channel_measurements.clone();
    let config = app.room_eq.config.clone();

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
        use autoeq::roomeq::{
            CallbackAction, ExcursionProtectionConfig as BackendExcursionProtectionConfig,
            FirConfig as BackendFirConfig, GroupDelayOptimizationConfig,
            HighFreqFilterConfig, HighpassType, LowFreqFilterConfig,
            MultiSeatConfig as BackendMultiSeatConfig, MultiSeatStrategy,
            OptimizerConfig, PhaseAlignmentConfig as BackendPhaseAlignmentConfig,
            ProcessingMode, RoomConfig, SchroederSplitConfig as BackendSchroederSplitConfig,
            SpeakerConfig, TargetTiltConfig as BackendTargetTiltConfig, TiltType,
            VoiceOfGodConfig, BroadbandTargetMatchingConfig as BackendBroadbandTargetMatchingConfig,
        };
        use autoeq::MeasurementSource;
        use sotf_audio_player::autoeq::run_room_optimization;
        use sotf_audio_player::room_eq_types::RoomEqOptimizationMode;

        // Convert measurements to speaker configs
        let mut speakers = std::collections::HashMap::new();
        for m in &measurements {
            let freq: Vec<f64> = m.measurement.frequencies.iter().map(|&f| f as f64).collect();
            let spl: Vec<f64> = m.measurement.magnitude_db.iter().map(|&db| db as f64).collect();
            let curve = autoeq::Curve {
                freq: ndarray::Array1::from(freq),
                spl: ndarray::Array1::from(spl),
                phase: None,
            };
            speakers.insert(
                m.channel_name.clone(),
                SpeakerConfig::Single(MeasurementSource::InMemory(curve)),
            );
        }

        // Build processing mode from optimization mode
        let processing_mode = match config.mode {
            RoomEqOptimizationMode::Iir => ProcessingMode::LowLatency,
            RoomEqOptimizationMode::Fir => ProcessingMode::PhaseLinear,
            RoomEqOptimizationMode::Mixed => ProcessingMode::Hybrid,
        };

        // Build OptimizerConfig directly (matching GPUI's to_room_config pattern)
        let optimizer = OptimizerConfig {
            loss_type: config.loss_type.clone(),
            algorithm: config.algorithm.clone(),
            num_filters: config.num_filters,
            min_q: config.min_q,
            max_q: config.max_q,
            min_db: config.min_db,
            max_db: config.max_db,
            min_freq: config.min_freq,
            max_freq: config.max_freq,
            max_iter: config.max_iter,
            population: config.population,
            peq_model: config.peq_model.clone(),
            mode: config.mode.to_code().to_string(),
            processing_mode,
            fir: Some(BackendFirConfig {
                taps: config.fir.taps,
                phase: config.fir.phase.clone(),
                correct_excess_phase: false,
                phase_smoothing: 0.167,
            }),
            seed: config.seed,
            mixed_config: if config.mode == RoomEqOptimizationMode::Mixed {
                Some(autoeq::roomeq::MixedModeConfig {
                    crossover_freq: config.mixed_config.crossover_freq,
                    crossover_type: config.mixed_config.crossover_type.clone(),
                    fir_band: config.mixed_config.fir_band.clone(),
                })
            } else {
                None
            },
            refine: config.refine,
            local_algo: config.local_algo.clone(),
            psychoacoustic: config.psychoacoustic,
            asymmetric_loss: config.asymmetric_loss,
            allow_delay: Some(config.allow_delay),
            target_tilt: if config.target_tilt.enabled {
                let tilt_type = match config.target_tilt.tilt_type.as_str() {
                    "harman" => TiltType::Harman,
                    "custom" => TiltType::Custom,
                    _ => TiltType::Flat,
                };
                Some(BackendTargetTiltConfig {
                    tilt_type,
                    slope_db_per_octave: config.target_tilt.slope,
                    reference_freq: config.target_tilt.reference_freq,
                    bass_shelf_db: config.target_tilt.bass_shelf_db,
                    bass_shelf_freq: config.target_tilt.bass_shelf_freq,
                })
            } else {
                None
            },
            excursion_protection: if config.excursion_protection.enabled {
                let filter_type = if config.excursion_protection.filter_type == "bw" {
                    HighpassType::Butterworth
                } else {
                    HighpassType::LinkwitzRiley
                };
                Some(BackendExcursionProtectionConfig {
                    enabled: true,
                    auto_detect_f3: config.excursion_protection.auto_detect_f3,
                    manual_f3_hz: Some(config.excursion_protection.manual_f3_hz),
                    filter_order: config.excursion_protection.filter_order,
                    filter_type,
                    margin_octaves: config.excursion_protection.margin_octaves,
                })
            } else {
                None
            },
            schroeder_split: if config.schroeder_split.enabled {
                Some(BackendSchroederSplitConfig {
                    enabled: true,
                    schroeder_freq: config.schroeder_split.schroeder_freq,
                    room_dimensions: None,
                    low_freq_config: LowFreqFilterConfig {
                        max_q: config.schroeder_split.low_freq_max_q,
                        min_q: 0.5,
                        allow_boost: config.schroeder_split.low_freq_allow_boost,
                    },
                    high_freq_config: HighFreqFilterConfig {
                        max_q: config.schroeder_split.high_freq_max_q,
                        shelving_only: config.schroeder_split.high_freq_shelving_only,
                    },
                })
            } else {
                None
            },
            phase_alignment: if config.phase_alignment.enabled {
                Some(BackendPhaseAlignmentConfig {
                    enabled: true,
                    min_freq: config.phase_alignment.min_freq,
                    max_freq: config.phase_alignment.max_freq,
                    optimize_polarity: config.phase_alignment.optimize_polarity,
                    max_delay_ms: config.phase_alignment.max_delay_ms,
                })
            } else {
                None
            },
            multi_seat: if config.multi_seat.enabled {
                let strategy = match config.multi_seat.strategy.as_str() {
                    "primary" => MultiSeatStrategy::PrimaryWithConstraints,
                    "average" => MultiSeatStrategy::Average,
                    _ => MultiSeatStrategy::MinimizeVariance,
                };
                Some(BackendMultiSeatConfig {
                    enabled: true,
                    strategy,
                    primary_seat: config.multi_seat.primary_seat,
                    max_deviation_db: config.multi_seat.max_deviation_db,
                })
            } else {
                None
            },
            gd_opt: if config.gd_opt.enabled {
                Some(GroupDelayOptimizationConfig {
                    enabled: true,
                    target_ms: config.gd_opt.target_ms,
                })
            } else {
                None
            },
            vog: if config.vog.enabled {
                Some(VoiceOfGodConfig {
                    enabled: true,
                    reference_channel: config.vog.reference_channel.clone(),
                })
            } else {
                None
            },
            broadband_target_matching: if config.broadband_target_matching.enabled {
                Some(BackendBroadbandTargetMatchingConfig { enabled: true })
            } else {
                None
            },
        };

        let room_config = RoomConfig {
            version: autoeq::roomeq::default_config_version(),
            system: None,
            speakers,
            crossovers: None,
            target_curve: None,
            optimizer,
            recording_config: None,
        };

        let progress_slot2 = progress_slot.clone();
        let callback: sotf_audio_player::autoeq::RoomOptimizationCallback = Box::new(move |p| {
            if let Ok(mut guard) = progress_slot2.lock() {
                *guard = Some(p.clone());
            }
            CallbackAction::Continue
        });

        let result = run_room_optimization(&room_config, 48000.0, Some(callback));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::tests::{key, make_app};
    use crate::app::Screen;
    use sotf_audio_player::room_eq_types::RoomEqStep;

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
        // Simulate progress calculation that could exceed 1.0
        let total_iters: usize = 100;
        let done_iters: usize = 150; // Exceeds total (e.g. extra iterations)
        let progress = if total_iters > 0 {
            (done_iters as f32 / total_iters as f32).min(1.0)
        } else {
            0.0
        };
        assert_eq!(progress, 1.0);
        assert!(progress <= 1.0);
    }

    #[test]
    fn room_eq_esc_at_load_data_returns_to_tab_bar() {
        let mut app = make_app();
        app.current_screen = Screen::Configure;
        app.configure_sub_screen = crate::app::ConfigureSubScreen::RoomEq;
        app.configure_tab_focused = false;
        app.room_eq.step = RoomEqStep::LoadData;

        handle_room_eq_keys(&mut app, key(KeyCode::Esc));
        assert!(app.configure_tab_focused);
    }

    #[test]
    fn room_eq_esc_at_configure_goes_back() {
        let mut app = make_app();
        app.room_eq.step = RoomEqStep::Configure;
        handle_room_eq_keys(&mut app, key(KeyCode::Esc));
        assert_eq!(app.room_eq.step, RoomEqStep::LoadData);
    }
}
