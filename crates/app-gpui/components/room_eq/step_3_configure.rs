use crate::app::types::RoomEqOptimizationMode;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_autoeq::{AutoEqConfig, AutoEqForm, AutoEqFormUiState, AutoEqLayoutMode};
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Card, HStack, StackAlign, StackSpacing, Text, TextSize,
    TextWeight, VStack,
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
            refine: config.refine,
            local_algo: config.local_algo.clone(),
            // Smoothing uses defaults (hidden in room EQ form, uses psychoacoustic instead)
            smooth: false,
            smooth_n: 6,
            // Spacing uses defaults (hidden in room EQ form)
            spacing_weight: 1.0,
            min_spacing_oct: 0.08,
            // Tolerance uses defaults (hidden in room EQ form)
            tolerance: 0.00001,
            atolerance: 0.00001,
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
        };

        // Build AutoEqFormUiState from our dropdowns
        let autoeq_ui_state = AutoEqFormUiState {
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

        // Build the AutoEQ form with handlers
        let autoeq_form = AutoEqForm::new("room-eq-optimizer-form")
            .layout_mode(AutoEqLayoutMode::RoomEq)
            .available_width(window_width)
            .config(autoeq_config)
            .ui_state(autoeq_ui_state)
            .show_goals(false) // Goals hidden: loss is always flat, system type is auto-detected
            .show_optimization_tuning(true)
            .hide_de_params(true)
            .hide_smoothing(true)
            .hide_spacing(true)
            .hide_tolerance(true)
            .hide_sample_rate(true)
            .hide_phase_alignment(is_iir_mode) // Phase alignment only for non-IIR modes
            .hide_scenario_a_text(true) // Remove "Scenario A" subtitle
            .allowed_opt_modes(available_modes)
            .on_opt_mode_change({
                let state = self.state.clone();
                move |mode, _window, cx| {
                    use crate::app::types::room_eq::RoomEqOptimizationMode;
                    state.update(cx, |state, _cx| {
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
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .fir
                            .taps = value;
                    });
                }
            })
            .on_fir_phase_change({
                let state = self.state.clone();
                move |phase, _window, cx| {
                    state.update(cx, |state, _cx| {
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
                    state.update(cx, |state, _cx| {
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
                    state.update(cx, |state, _cx| {
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
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .num_filters = value;
                    });
                }
            })
            .on_min_q_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .min_q = value;
                    });
                }
            })
            .on_max_q_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .max_q = value;
                    });
                }
            })
            .on_min_db_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .min_db = value;
                    });
                }
            })
            .on_max_db_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .max_db = value;
                    });
                }
            })
            .on_min_freq_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .min_freq = value;
                    });
                }
            })
            .on_max_freq_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .max_freq = value;
                    });
                }
            })
            .on_maxeval_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .max_iter = value;
                    });
                }
            })
            .on_population_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .population = value;
                    });
                }
            })
            .on_refine_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .refine = value;
                    });
                }
            })
            .on_local_algo_change({
                let state = self.state.clone();
                move |algo, _window, cx| {
                    state.update(cx, |state, _cx| {
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
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .psychoacoustic = value;
                    });
                }
            })
            .on_asymmetric_loss_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .asymmetric_loss = value;
                    });
                }
            })
            .on_loss_type_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
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
            .on_system_type_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
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
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .target_tilt
                            .enabled = v;
                    });
                }
            })
            .on_tilt_type_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
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
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .target_tilt
                            .slope = v;
                    });
                }
            })
            .on_tilt_reference_freq_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .target_tilt
                            .reference_freq = v;
                    });
                }
            })
            .on_tilt_bass_shelf_db_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .target_tilt
                            .bass_shelf_db = v;
                    });
                }
            })
            .on_tilt_bass_shelf_freq_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .target_tilt
                            .bass_shelf_freq = v;
                    });
                }
            })
            // Excursion Protection
            .on_use_excursion_protection_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .excursion_protection
                            .enabled = v;
                    });
                }
            })
            .on_excursion_auto_detect_f3_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .excursion_protection
                            .auto_detect_f3 = v;
                    });
                }
            })
            .on_excursion_manual_f3_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .excursion_protection
                            .manual_f3_hz = v;
                    });
                }
            })
            .on_excursion_filter_order_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .excursion_protection
                            .filter_order = v;
                    });
                }
            })
            .on_excursion_filter_type_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
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
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .excursion_protection
                            .margin_octaves = v;
                    });
                }
            })
            // Schroeder Split
            .on_use_schroeder_split_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .schroeder_split
                            .enabled = v;
                    });
                }
            })
            .on_schroeder_freq_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .schroeder_split
                            .schroeder_freq = v;
                    });
                }
            })
            .on_schroeder_low_max_q_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .schroeder_split
                            .low_freq_max_q = v;
                    });
                }
            })
            .on_schroeder_low_allow_boost_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .schroeder_split
                            .low_freq_allow_boost = v;
                    });
                }
            })
            .on_schroeder_high_max_q_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .schroeder_split
                            .high_freq_max_q = v;
                    });
                }
            })
            .on_schroeder_high_shelving_only_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .schroeder_split
                            .high_freq_shelving_only = v;
                    });
                }
            })
            // Phase Alignment
            .on_use_phase_alignment_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .phase_alignment
                            .enabled = v;
                    });
                }
            })
            .on_phase_min_freq_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .phase_alignment
                            .min_freq = v;
                    });
                }
            })
            .on_phase_max_freq_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .phase_alignment
                            .max_freq = v;
                    });
                }
            })
            .on_phase_optimize_polarity_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .phase_alignment
                            .optimize_polarity = v;
                    });
                }
            })
            .on_phase_max_delay_ms_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .phase_alignment
                            .max_delay_ms = v;
                    });
                }
            })
            // Multi-Seat
            .on_use_multi_seat_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .multi_seat
                            .enabled = v;
                    });
                }
            })
            .on_multi_seat_strategy_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
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
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .multi_seat
                            .primary_seat = v;
                    });
                }
            })
            .on_multi_seat_max_deviation_db_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .multi_seat
                            .max_deviation_db = v;
                    });
                }
            })
            // v2 features
            .on_allow_delay_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .allow_delay = v;
                    });
                }
            })
            .on_seed_enabled_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        let config =
                            &mut state.app.measurement_state.room_eq_state.optimizer_config;
                        if v {
                            config.seed = Some(config.seed.unwrap_or(42));
                        } else {
                            config.seed = None;
                        }
                    });
                }
            })
            .on_seed_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .seed = Some(v as u64);
                    });
                }
            })
            .on_gd_opt_enabled_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .gd_opt
                            .enabled = v;
                    });
                }
            })
            .on_gd_opt_target_ms_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .gd_opt
                            .target_ms = v;
                    });
                }
            })
            .on_vog_enabled_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .vog
                            .enabled = v;
                    });
                }
            })
            .on_vog_reference_channel_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
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
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .broadband_target_matching
                            .enabled = v;
                    });
                }
            })
            .on_mixed_crossover_freq_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .mixed_config
                            .crossover_freq = v;
                    });
                }
            })
            .on_mixed_crossover_type_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
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
                    state.update(cx, |state, _cx| {
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
            });

        // Build mode selector cards
        let current_mode = room_eq.optimizer_config.mode;
        let mode_selector = {
            let modes: Vec<RoomEqOptimizationMode> = if has_phase_data {
                RoomEqOptimizationMode::available(release_channel)
            } else {
                vec![RoomEqOptimizationMode::Iir]
            };

            Card::new()
                .background(theme.surface)
                .header_background(theme.background_secondary)
                .border(theme.border)
                .header(
                    Text::new("Optimization Mode")
                        .color(theme.text_primary)
                        .weight(TextWeight::Semibold),
                )
                .content(
                    VStack::new()
                        .spacing(StackSpacing::Sm)
                        .child(if !has_phase_data {
                            Text::new("Only IIR mode is available (no phase data in measurements).")
                                .size(TextSize::Xs)
                                .color(theme.text_muted)
                                .into_any_element()
                        } else {
                            Text::new(
                                "Choose the type of filters to generate for your room correction.",
                            )
                            .size(TextSize::Xs)
                            .color(theme.text_secondary)
                            .into_any_element()
                        })
                        .child(
                            HStack::new()
                                .spacing(StackSpacing::Sm)
                                .align(StackAlign::Stretch)
                                .children(modes.iter().map(|mode| {
                                    let is_selected = current_mode == *mode;
                                    let mode_val = *mode;

                                    div().flex_1().child(
                                        Card::new()
                                            .background(if is_selected {
                                                theme.surface_selected
                                            } else {
                                                theme.surface
                                            })
                                            .border(if is_selected {
                                                theme.accent
                                            } else {
                                                theme.border
                                            })
                                            .content(
                                                VStack::new()
                                                    .spacing(StackSpacing::Xs)
                                                    .child(
                                                        Text::new(mode.as_str())
                                                            .weight(TextWeight::Semibold)
                                                            .color(if is_selected {
                                                                theme.accent
                                                            } else {
                                                                theme.text_primary
                                                            }),
                                                    )
                                                    .child(
                                                        Text::new(mode.description())
                                                            .size(TextSize::Xs)
                                                            .color(theme.text_secondary),
                                                    )
                                                    .child(
                                                        div().mt_2().child(
                                                            Button::new(
                                                                SharedString::from(format!(
                                                                    "cfg-select-mode-{:?}",
                                                                    mode
                                                                )),
                                                                if is_selected {
                                                                    "Selected"
                                                                } else {
                                                                    "Select"
                                                                },
                                                            )
                                                            .variant(if is_selected {
                                                                ButtonVariant::Primary
                                                            } else {
                                                                ButtonVariant::Secondary
                                                            })
                                                            .size(ButtonSize::Xs)
                                                            .full_width(true)
                                                            .theme(theme.to_button_theme())
                                                            .build()
                                                            .on_mouse_up(
                                                                MouseButton::Left,
                                                                cx.listener(
                                                                    move |view, _, _, cx| {
                                                                        view.state.update(
                                                                            cx,
                                                                            |state, _| {
                                                                                state
                                                                            .app
                                                                            .measurement_state
                                                                            .room_eq_state
                                                                            .optimizer_config
                                                                            .mode = mode_val;
                                                                            },
                                                                        );
                                                                        cx.notify();
                                                                    },
                                                                ),
                                                            ),
                                                        ),
                                                    ),
                                            ),
                                    )
                                })),
                        ),
                )
        };

        let mut content = VStack::new()
            .spacing(StackSpacing::Md)
            .child(
                Text::new("Configure Optimization")
                    .weight(TextWeight::Bold)
                    .size(TextSize::Md),
            )
            .child(
                Text::new("Configure per-channel settings and optimizer parameters.")
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            )
            // Mode selector inline
            .child(mode_selector)
            .child(autoeq_form);

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
            .map(|(idx, config)| render_channel_config_row(idx, config, &theme, &view))
            .collect();

        VStack::new()
            .spacing(StackSpacing::Sm)
            .children(rows)
            .into_any_element()
    }
}
