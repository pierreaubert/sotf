use crate::app::types::RoomEqOptimizationMode;
use crate::components::autoeq::{AutoEqConfig, AutoEqForm, AutoEqFormUiState, AutoEqLayoutMode};
use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Card, HStack, StackSpacing, Text, TextSize, TextWeight,
    VStack,
};

use super::render::render_channel_config_row;

impl PlayerView {
    pub(crate) fn render_room_eq_configure(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let room_eq = &state.app.measurement_state.room_eq_state;
        let release_channel = state.app.ui_state.release_channel;
        let has_phase_data = room_eq.has_phase_data();
        let has_multi_driver = room_eq.has_multi_driver();
        let is_iir_mode = room_eq.optimizer_config.mode == RoomEqOptimizationMode::Iir;

        // Build AutoEqConfig from our RoomEqOptimizerConfig
        let config = &room_eq.optimizer_config;
        let autoeq_config = AutoEqConfig {
            opt_mode: config.mode.to_code().to_string(),
            fir_taps: config.fir.taps,
            fir_phase: config.fir.phase.clone(),
            num_filters: config.num_filters,
            // sample_rate removed from RoomEqOptimizerConfig (CLI-specific)
            sample_rate: 48000,
            min_db: config.min_db,
            max_db: config.max_db,
            min_q: config.min_q,
            max_q: config.max_q,
            min_freq: config.min_freq,
            max_freq: config.max_freq,
            peq_model: config.peq_model.clone(),
            algo: config.algorithm.clone(),
            population: config.population,
            maxeval: config.max_iter,
            // DE-specific params use defaults (hidden in room EQ form)
            de_f: 0.8,
            de_cr: 0.9,
            strategy: "currenttobest1bin".to_string(),
            adaptive_weight_f: 0.8,
            adaptive_weight_cr: 0.7,
            refine: config.refine,
            local_algo: config.local_algo.clone(),
            smooth: config.smooth,
            smooth_n: config.smooth_n,
            // Spacing uses defaults (hidden in room EQ form)
            spacing_weight: 1.0,
            min_spacing_oct: 0.08,
            tolerance: config.tolerance,
            atolerance: config.atolerance,
            psychoacoustic: config.psychoacoustic,
            asymmetric_loss: config.asymmetric_loss,
            loss_type: config.loss_type.clone(),
            target_curve: config.target_curve.clone(),
            system_type: config.system_type.clone(),

            // v2 fields
            allow_delay: config.allow_delay,
            seed_enabled: config.seed.is_some(),
            seed: config.seed.unwrap_or(42),
            gd_opt_enabled: config.gd_opt.enabled,
            gd_opt_target_ms: config.gd_opt.target_ms,
            vog_enabled: config.vog.enabled,
            vog_reference_channel: config.vog.reference_channel.clone(),
            broadband_target_matching: config.broadband_target_matching.enabled,
            mixed_crossover_freq: config.mixed_config.crossover_freq,
            mixed_crossover_type: config.mixed_config.crossover_type.clone(),
            mixed_fir_band: config.mixed_config.fir_band.clone(),

            // Scenario B
            use_target_tilt: config.target_tilt.enabled,
            tilt_type: config.target_tilt.tilt_type.clone(),
            tilt_slope: config.target_tilt.slope,
            tilt_reference_freq: config.target_tilt.reference_freq,
            tilt_bass_shelf_db: config.target_tilt.bass_shelf_db,
            tilt_bass_shelf_freq: config.target_tilt.bass_shelf_freq,

            use_excursion_protection: config.excursion_protection.enabled,
            excursion_auto_detect_f3: config.excursion_protection.auto_detect_f3,
            excursion_manual_f3: config.excursion_protection.manual_f3_hz,
            excursion_filter_order: config.excursion_protection.filter_order,
            excursion_filter_type: config.excursion_protection.filter_type.clone(),
            excursion_margin_octaves: config.excursion_protection.margin_octaves,

            use_schroeder_split: config.schroeder_split.enabled,
            schroeder_freq: config.schroeder_split.schroeder_freq,
            schroeder_low_max_q: config.schroeder_split.low_freq_max_q,
            schroeder_low_allow_boost: config.schroeder_split.low_freq_allow_boost,
            schroeder_high_max_q: config.schroeder_split.high_freq_max_q,
            schroeder_high_shelving_only: config.schroeder_split.high_freq_shelving_only,
            schroeder_low_max_db: config.schroeder_split.low_freq_max_db,

            use_sub_config: config.sub_config.enabled,
            sub_num_filters: config.sub_config.num_filters,
            sub_max_db: config.sub_config.max_db,
            sub_min_db: config.sub_config.min_db,
            sub_min_q: config.sub_config.min_q,
            sub_max_q: config.sub_config.max_q,
            use_channel_matching: config.channel_matching.enabled,
            channel_matching_threshold_db: config.channel_matching.threshold_db,
            channel_matching_max_filters: config.channel_matching.max_filters,

            // Scenario A
            use_phase_alignment: config.phase_alignment.enabled,
            phase_min_freq: config.phase_alignment.min_freq,
            phase_max_freq: config.phase_alignment.max_freq,
            phase_optimize_polarity: config.phase_alignment.optimize_polarity,
            phase_max_delay_ms: config.phase_alignment.max_delay_ms,

            use_multi_seat: config.multi_seat.enabled,
            multi_seat_strategy: config.multi_seat.strategy.clone(),
            multi_seat_primary_seat: config.multi_seat.primary_seat,
            multi_seat_max_deviation_db: config.multi_seat.max_deviation_db,

            use_multi_measurement: config.multi_measurement.enabled,
            multi_measurement_strategy: config.multi_measurement.strategy.clone(),
            multi_measurement_variance_lambda: config.multi_measurement.variance_lambda,
            multi_measurement_weights: config.multi_measurement.weights.clone(),
            multi_measurement_labels: {
                let max_count = room_eq
                    .multi_position_counts
                    .iter()
                    .map(|(_, c)| *c)
                    .max()
                    .unwrap_or(0);
                (0..max_count)
                    .map(|i| format!("Position {}", i + 1))
                    .collect()
            },
        };

        // Build AutoEqFormUiState from our dropdowns
        let autoeq_ui_state = AutoEqFormUiState {
            detail_level: room_eq.detail_level,
            selected_preset: Some(room_eq.selected_preset.clone()),
            opt_mode_open: room_eq.dropdowns.opt_mode_open,
            fir_phase_open: room_eq.dropdowns.fir_phase_open,
            algo_open: room_eq.dropdowns.algorithm_open,
            peq_model_open: room_eq.dropdowns.peq_model_open,
            strategy_open: room_eq.dropdowns.strategy_open,
            local_algo_open: room_eq.dropdowns.local_algo_open,
            loss_type_open: room_eq.dropdowns.loss_type_open,
            target_curve_open: room_eq.dropdowns.target_curve_open,
            system_type_open: room_eq.dropdowns.system_type_open,
            tilt_type_open: room_eq.dropdowns.tilt_type_open,
            excursion_filter_type_open: room_eq.dropdowns.excursion_filter_type_open,
            multi_seat_strategy_open: room_eq.dropdowns.multi_seat_strategy_open,
            mixed_crossover_type_open: room_eq.dropdowns.mixed_crossover_type_open,
            mixed_fir_band_open: room_eq.dropdowns.mixed_fir_band_open,
            vog_reference_channel_open: room_eq.dropdowns.vog_reference_channel_open,
            multi_measurement_strategy_open: room_eq.dropdowns.multi_measurement_strategy_open,
            focused_block: None,
            ..Default::default()
        };

        // Compute available modes: if no phase data, only IIR
        let available_modes: Vec<String> = if has_phase_data {
            RoomEqOptimizationMode::available(release_channel)
                .iter()
                .map(|m| m.to_code().to_string())
                .collect()
        } else {
            vec!["iir".to_string()]
        };

        let window_width = state.app.ui_state.window_width;
        let show_advanced = room_eq.show_advanced_config;

        // Build the AutoEQ form with handlers
        // In Basic mode, hide most params so users only see mode + algorithm + num_filters + target_curve
        let autoeq_form = AutoEqForm::new("room-eq-optimizer-form")
            .layout_mode(AutoEqLayoutMode::RoomEq)
            .available_width(window_width)
            .config(autoeq_config)
            .ui_state(autoeq_ui_state)
            .show_goals(false) // Goals hidden: loss is always flat, system type is auto-detected
            .show_optimization_tuning(show_advanced) // Only in advanced mode
            .hide_de_params(true) // Always hidden (internal to DE algorithm)
            .hide_smoothing(true) // Smoothing is handled directly in the room config card
            .hide_spacing(true) // Always hidden (internal)
            .hide_tolerance(!show_advanced) // Only in advanced mode
            .hide_sample_rate(true) // Always hidden (auto-detected)
            .hide_phase_alignment(is_iir_mode || !show_advanced)
            .hide_scenario_a_text(true) // Remove "Scenario A" subtitle
            .hide_multi_measurement(!room_eq.has_multiple_measurements() || !show_advanced)
            .allowed_opt_modes(available_modes)
            .on_opt_mode_change({
                let state = self.state.clone();
                move |mode, _window, cx| {
                    use crate::app::types::room_eq::RoomEqOptimizationMode;
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .mode = RoomEqOptimizationMode::from_code(mode);
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .opt_mode_open = false;
                        cx.notify();
                    });
                }
            })
            .on_opt_mode_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .opt_mode_open = open;
                        cx.notify();
                    });
                }
            })
            .on_fir_taps_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .fir
                            .taps = value;
                        cx.notify();
                    });
                }
            })
            .on_fir_phase_change({
                let state = self.state.clone();
                move |phase, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .fir
                            .phase = phase.to_string();
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .fir_phase_open = false;
                        cx.notify();
                    });
                }
            })
            .on_fir_phase_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .fir_phase_open = open;
                        cx.notify();
                    });
                }
            })
            .on_algo_change({
                let state = self.state.clone();
                move |algo, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .algorithm = algo.to_string();
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .algorithm_open = false;
                        cx.notify();
                    });
                }
            })
            .on_algo_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .algorithm_open = open;
                        cx.notify();
                    });
                }
            })
            .on_peq_model_change({
                let state = self.state.clone();
                move |model, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .peq_model = model.to_string();
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .peq_model_open = false;
                        cx.notify();
                    });
                }
            })
            .on_peq_model_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .peq_model_open = open;
                        cx.notify();
                    });
                }
            })
            .on_num_filters_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .num_filters = value;
                        cx.notify();
                    });
                }
            })
            .on_min_q_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .min_q = value;
                        cx.notify();
                    });
                }
            })
            .on_max_q_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .max_q = value;
                        cx.notify();
                    });
                }
            })
            .on_min_db_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .min_db = value;
                        cx.notify();
                    });
                }
            })
            .on_max_db_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .max_db = value;
                        cx.notify();
                    });
                }
            })
            .on_min_freq_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .min_freq = value;
                        cx.notify();
                    });
                }
            })
            .on_max_freq_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .max_freq = value;
                        cx.notify();
                    });
                }
            })
            .on_maxeval_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .max_iter = value;
                        cx.notify();
                    });
                }
            })
            .on_population_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .population = value;
                        cx.notify();
                    });
                }
            })
            .on_tolerance_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .tolerance = value;
                        cx.notify();
                    });
                }
            })
            .on_atolerance_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .atolerance = value;
                        cx.notify();
                    });
                }
            })
            .on_refine_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .refine = value;
                        cx.notify();
                    });
                }
            })
            .on_local_algo_change({
                let state = self.state.clone();
                move |algo, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .local_algo = algo.to_string();
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .local_algo_open = false;
                        cx.notify();
                    });
                }
            })
            .on_local_algo_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .local_algo_open = open;
                        cx.notify();
                    });
                }
            })
            .on_psychoacoustic_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, cx| {
                        let cfg = &mut state.app.measurement_state.room_eq_state.optimizer_config;
                        cfg.psychoacoustic = value;
                        if value {
                            cfg.smooth = false;
                        }
                        cx.notify();
                    });
                }
            })
            .on_smooth_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, cx| {
                        let cfg = &mut state.app.measurement_state.room_eq_state.optimizer_config;
                        cfg.smooth = value;
                        if value {
                            cfg.psychoacoustic = false;
                        }
                        cx.notify();
                    });
                }
            })
            .on_smooth_n_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .smooth_n = value;
                        cx.notify();
                    });
                }
            })
            .on_asymmetric_loss_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .asymmetric_loss = value;
                        cx.notify();
                    });
                }
            })
            .on_loss_type_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .loss_type = value.to_string();
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .loss_type_open = false;
                        cx.notify();
                    });
                }
            })
            .on_loss_type_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .loss_type_open = open;
                        cx.notify();
                    });
                }
            })
            .on_target_curve_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .target_curve = value.to_string();
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .target_curve_open = false;

                        // Open custom target curve modal when "custom" is selected
                        if value == "custom" {
                            state
                                .app
                                .measurement_state
                                .room_eq_state
                                .dropdowns
                                .custom_target_modal_open = true;
                        }
                        cx.notify();
                    });
                }
            })
            .on_target_curve_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .target_curve_open = open;
                        cx.notify();
                    });
                }
            })
            .on_edit_custom_target({
                let state = self.state.clone();
                move |_window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .custom_target_modal_open = true;
                        cx.notify();
                    });
                }
            })
            .on_system_type_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .system_type = value.to_string();
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .system_type_open = false;
                        cx.notify();
                    });
                }
            })
            .on_system_type_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .system_type_open = open;
                        cx.notify();
                    });
                }
            })
            // Target Tilt
            .on_use_target_tilt_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .target_tilt
                            .enabled = v;
                        cx.notify();
                    });
                }
            })
            .on_tilt_type_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .target_tilt
                            .tilt_type = v.to_string();
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .tilt_type_open = false;
                        cx.notify();
                    });
                }
            })
            .on_tilt_type_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .tilt_type_open = open;
                        cx.notify();
                    });
                }
            })
            .on_tilt_slope_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .target_tilt
                            .slope = v;
                        cx.notify();
                    });
                }
            })
            .on_tilt_reference_freq_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .target_tilt
                            .reference_freq = v;
                        cx.notify();
                    });
                }
            })
            .on_tilt_bass_shelf_db_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .target_tilt
                            .bass_shelf_db = v;
                        cx.notify();
                    });
                }
            })
            .on_tilt_bass_shelf_freq_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .target_tilt
                            .bass_shelf_freq = v;
                        cx.notify();
                    });
                }
            })
            // Excursion Protection
            .on_use_excursion_protection_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .excursion_protection
                            .enabled = v;
                        cx.notify();
                    });
                }
            })
            .on_excursion_auto_detect_f3_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .excursion_protection
                            .auto_detect_f3 = v;
                        cx.notify();
                    });
                }
            })
            .on_excursion_manual_f3_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .excursion_protection
                            .manual_f3_hz = v;
                        cx.notify();
                    });
                }
            })
            .on_excursion_filter_order_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .excursion_protection
                            .filter_order = v;
                        cx.notify();
                    });
                }
            })
            .on_excursion_filter_type_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .excursion_protection
                            .filter_type = v.to_string();
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .excursion_filter_type_open = false;
                        cx.notify();
                    });
                }
            })
            .on_excursion_filter_type_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .excursion_filter_type_open = open;
                        cx.notify();
                    });
                }
            })
            .on_excursion_margin_octaves_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .excursion_protection
                            .margin_octaves = v;
                        cx.notify();
                    });
                }
            })
            // Schroeder Split
            .on_use_schroeder_split_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .schroeder_split
                            .enabled = v;
                        cx.notify();
                    });
                }
            })
            .on_schroeder_freq_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .schroeder_split
                            .schroeder_freq = v;
                        cx.notify();
                    });
                }
            })
            .on_schroeder_low_max_q_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .schroeder_split
                            .low_freq_max_q = v;
                        cx.notify();
                    });
                }
            })
            .on_schroeder_low_allow_boost_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .schroeder_split
                            .low_freq_allow_boost = v;
                        cx.notify();
                    });
                }
            })
            .on_schroeder_high_max_q_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .schroeder_split
                            .high_freq_max_q = v;
                        cx.notify();
                    });
                }
            })
            .on_schroeder_high_shelving_only_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .schroeder_split
                            .high_freq_shelving_only = v;
                        cx.notify();
                    });
                }
            })
            // Phase Alignment
            .on_use_phase_alignment_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .phase_alignment
                            .enabled = v;
                        cx.notify();
                    });
                }
            })
            .on_phase_min_freq_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .phase_alignment
                            .min_freq = v;
                        cx.notify();
                    });
                }
            })
            .on_phase_max_freq_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .phase_alignment
                            .max_freq = v;
                        cx.notify();
                    });
                }
            })
            .on_phase_optimize_polarity_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .phase_alignment
                            .optimize_polarity = v;
                        cx.notify();
                    });
                }
            })
            .on_phase_max_delay_ms_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .phase_alignment
                            .max_delay_ms = v;
                        cx.notify();
                    });
                }
            })
            // Multi-Seat
            .on_use_multi_seat_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .multi_seat
                            .enabled = v;
                        cx.notify();
                    });
                }
            })
            .on_multi_seat_strategy_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .multi_seat
                            .strategy = v.to_string();
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .multi_seat_strategy_open = false;
                        cx.notify();
                    });
                }
            })
            .on_multi_seat_strategy_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .multi_seat_strategy_open = open;
                        cx.notify();
                    });
                }
            })
            .on_multi_seat_primary_seat_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .multi_seat
                            .primary_seat = v;
                        cx.notify();
                    });
                }
            })
            .on_multi_seat_max_deviation_db_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .multi_seat
                            .max_deviation_db = v;
                        cx.notify();
                    });
                }
            })
            // Multi-Measurement
            .on_use_multi_measurement_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .multi_measurement
                            .enabled = v;
                        cx.notify();
                    });
                }
            })
            .on_multi_measurement_strategy_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .multi_measurement
                            .strategy = v.to_string();
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .multi_measurement_strategy_open = false;
                        cx.notify();
                    });
                }
            })
            .on_multi_measurement_strategy_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .multi_measurement_strategy_open = open;
                        cx.notify();
                    });
                }
            })
            .on_multi_measurement_variance_lambda_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .multi_measurement
                            .variance_lambda = v;
                        cx.notify();
                    });
                }
            })
            .on_multi_measurement_weight_change({
                let state = self.state.clone();
                move |idx, v, _window, cx| {
                    state.update(cx, |state, cx| {
                        let weights = &mut state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .multi_measurement
                            .weights;
                        if idx < weights.len() {
                            weights[idx] = v;
                        }
                        cx.notify();
                    });
                }
            })
            // v2 features
            .on_allow_delay_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .allow_delay = v;
                        cx.notify();
                    });
                }
            })
            .on_seed_enabled_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        let config =
                            &mut state.app.measurement_state.room_eq_state.optimizer_config;
                        if v {
                            config.seed = Some(config.seed.unwrap_or(42));
                        } else {
                            config.seed = None;
                        }
                        cx.notify();
                    });
                }
            })
            .on_seed_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .seed = Some(v as u64);
                        cx.notify();
                    });
                }
            })
            .on_gd_opt_enabled_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .gd_opt
                            .enabled = v;
                        cx.notify();
                    });
                }
            })
            .on_gd_opt_target_ms_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .gd_opt
                            .target_ms = v;
                        cx.notify();
                    });
                }
            })
            .on_vog_enabled_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .vog
                            .enabled = v;
                        cx.notify();
                    });
                }
            })
            .on_vog_reference_channel_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .vog
                            .reference_channel = v.to_string();
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .vog_reference_channel_open = false;
                        cx.notify();
                    });
                }
            })
            .on_vog_reference_channel_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .vog_reference_channel_open = open;
                        cx.notify();
                    });
                }
            })
            .on_broadband_target_matching_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .broadband_target_matching
                            .enabled = v;
                        cx.notify();
                    });
                }
            })
            .on_mixed_crossover_freq_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .mixed_config
                            .crossover_freq = v;
                        cx.notify();
                    });
                }
            })
            .on_mixed_crossover_type_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .mixed_config
                            .crossover_type = v.to_string();
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .mixed_crossover_type_open = false;
                        cx.notify();
                    });
                }
            })
            .on_mixed_crossover_type_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .mixed_crossover_type_open = open;
                        cx.notify();
                    });
                }
            })
            .on_mixed_fir_band_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .mixed_config
                            .fir_band = v.to_string();
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .mixed_fir_band_open = false;
                        cx.notify();
                    });
                }
            })
            .on_mixed_fir_band_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .mixed_fir_band_open = open;
                        cx.notify();
                    });
                }
            })
            .on_detail_level_change({
                let state = self.state.clone();
                move |level, _window, cx| {
                    use sotf_audio_player::autoeq::DetailLevel;
                    state.update(cx, |state, cx| {
                        state.app.measurement_state.room_eq_state.detail_level = match level {
                            "simple" => DetailLevel::Simple,
                            "intermediate" => DetailLevel::Intermediate,
                            "expert" => DetailLevel::Expert,
                            _ => DetailLevel::Simple,
                        };
                        cx.notify();
                    });
                }
            })
            .on_preset_change({
                let state = self.state.clone();
                move |preset_id, _window, cx| {
                    use sotf_audio_player::autoeq::{DetailLevel, EqWorkflow, find_preset};
                    state.update(cx, |state, cx| {
                        let req = &mut state.app.measurement_state.room_eq_state;
                        req.selected_preset = preset_id.to_string();
                        if let Some(preset) = find_preset(EqWorkflow::RoomEq, preset_id) {
                            if preset.is_custom() {
                                req.detail_level = DetailLevel::Expert;
                            } else if let Some(params) = preset.apply() {
                                let c = &mut req.optimizer_config;
                                c.num_filters = params.num_filters;
                                c.min_freq = params.min_freq;
                                c.max_freq = params.max_freq;
                                c.min_db = params.min_db;
                                c.max_db = params.max_db;
                                c.min_q = params.min_q;
                                c.max_q = params.max_q;
                                c.peq_model = params.peq_model;
                                c.population = params.population;
                                c.max_iter = params.maxeval;
                                c.refine = params.refine;
                            }
                        }
                        cx.notify();
                    });
                }
            })
            .on_preset_toggle({
                let state = self.state.clone();
                move |_open, _window, cx| {
                    state.update(cx, |state, cx| {
                        cx.notify();
                        let _ = state;
                    });
                }
            });

        // Build compact mode selector buttons (75px each)
        let current_mode = room_eq.optimizer_config.mode;
        let mode_selector = {
            let modes: Vec<RoomEqOptimizationMode> = if has_phase_data {
                RoomEqOptimizationMode::available(release_channel)
            } else {
                vec![RoomEqOptimizationMode::Iir]
            };

            HStack::new()
                .spacing(StackSpacing::Xs)
                .children(modes.iter().map(|mode| {
                    let is_selected = current_mode == *mode;
                    let mode_val = *mode;
                    let short_name = match mode_val {
                        RoomEqOptimizationMode::Iir => "IIR",
                        RoomEqOptimizationMode::Fir => "FIR",
                        RoomEqOptimizationMode::Mixed => "Mixed",
                        RoomEqOptimizationMode::MixedPhase => "MixedΦ",
                    };

                    Button::new(
                        SharedString::from(format!("cfg-select-mode-{:?}", mode)),
                        short_name,
                    )
                    .variant(if is_selected {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::Secondary
                    })
                    .size(ButtonSize::Xs)
                    .theme(theme.to_button_theme())
                    .build()
                    .w(px(75.0))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |view, _, _, cx| {
                            view.state.update(cx, |state, _| {
                                state
                                    .app
                                    .measurement_state
                                    .room_eq_state
                                    .optimizer_config
                                    .mode = mode_val;
                            });
                            cx.notify();
                        }),
                    )
                    .into_any_element()
                }))
        };

        // Basic/Advanced toggle
        let toggle_state = self.state.clone();
        let toggle_label = if show_advanced { "Basic" } else { "Advanced" };

        let mut content = VStack::new()
            .spacing(StackSpacing::Md)
            .child(
                HStack::new()
                    .spacing(StackSpacing::Md)
                    .child(
                        Text::new("Configure Optimization")
                            .weight(TextWeight::Bold)
                            .size(TextSize::Md),
                    )
                    .child(
                        Button::new("toggle-advanced", toggle_label)
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Xs)
                            .theme(theme.to_button_theme())
                            .build()
                            .on_click(move |_event: &ClickEvent, _window, cx| {
                                toggle_state.update(cx, |state, cx| {
                                    let adv = &mut state
                                        .app
                                        .measurement_state
                                        .room_eq_state
                                        .show_advanced_config;
                                    *adv = !*adv;
                                    cx.notify();
                                });
                            }),
                    ),
            )
            .child(
                Text::new(if show_advanced {
                    "All optimizer parameters are shown."
                } else {
                    "Showing basic settings. Click Advanced for full control."
                })
                .size(TextSize::Xs)
                .color(theme.text_secondary),
            )
            // Mode selector inline
            .child(mode_selector)
            .child(autoeq_form)
            .child(self.render_slope_recommendation(cx));

        // Only show channel configuration for multi-driver measurements
        if has_multi_driver {
            content = content.child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(
                        Text::new("Channel Configuration")
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
                    .content(self.render_channel_config_list(cx)),
            );
        }

        content.child(self.render_room_eq_validation_summary(cx))
    }

    /// Render slope recommendation based on L and R channel measurements.
    /// Shows computed slope and recommended range.
    fn render_slope_recommendation(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let room_eq = &state.app.measurement_state.room_eq_state;

        let (slope, rec_min, rec_max) = match room_eq.compute_lr_slope() {
            Some((s, rmin, rmax)) => (s, rmin, rmax),
            None => return div().into_any_element(),
        };

        let slope_text = format!("Current slope: {:.2} dB/oct", slope);
        let rec_text = format!(
            "Slope recommendation: [{:.2}, {:.2}] dB/oct",
            rec_min, rec_max
        );

        VStack::new()
            .spacing(StackSpacing::Xs)
            .child(
                Text::new(slope_text.clone())
                    .size(TextSize::Sm)
                    .color(theme.text_primary),
            )
            .child(
                Text::new(rec_text.clone())
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
            .into_any_element()
    }

    /// Render validation summary based on current config
    fn render_room_eq_validation_summary(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let validation = state.app.measurement_state.room_eq_state.validate();

        if validation.is_valid && validation.warnings.is_empty() {
            return div().into_any_element();
        }

        let mut content = VStack::new().spacing(StackSpacing::Xs);

        for error in &validation.errors {
            content = content.child(
                HStack::new()
                    .spacing(StackSpacing::Xs)
                    .child(Text::new("!").color(theme.error).weight(TextWeight::Bold))
                    .child(
                        Text::new(error.clone())
                            .size(TextSize::Xs)
                            .color(theme.error),
                    ),
            );
        }

        for warning in &validation.warnings {
            content = content.child(
                HStack::new()
                    .spacing(StackSpacing::Xs)
                    .child(Text::new("?").color(theme.warning).weight(TextWeight::Bold))
                    .child(
                        Text::new(warning.clone())
                            .size(TextSize::Xs)
                            .color(theme.warning),
                    ),
            );
        }

        Card::new()
            .background(theme.surface)
            .header_background(theme.background_secondary)
            .border(if !validation.is_valid {
                theme.error
            } else {
                theme.warning
            })
            .header(
                Text::new("Configuration Check")
                    .color(theme.text_primary)
                    .weight(TextWeight::Semibold),
            )
            .content(content)
            .into_any_element()
    }

    /// Render the list of channel configurations
    fn render_channel_config_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let speaker_configs = state
            .app
            .measurement_state
            .room_eq_state
            .speaker_configs
            .clone();

        if speaker_configs.is_empty() {
            return VStack::new()
                .spacing(StackSpacing::Sm)
                .child(
                    Text::new("No channels configured. Load measurement data first.")
                        .size(TextSize::Xs)
                        .color(theme.text_muted),
                )
                .into_any_element();
        }

        let view = cx.entity().clone();

        // Collect rows before returning to avoid closure lifetime issues
        let rows: Vec<_> = speaker_configs
            .iter()
            .enumerate()
            .map(|(idx, config)| render_channel_config_row(idx, config, &theme, &view, d))
            .collect();

        VStack::new()
            .spacing(StackSpacing::Sm)
            .children(rows)
            .into_any_element()
    }
}
