use crate::app::types::{RoomEqAlgorithm, UiSubwooferStrategy, UiSystemModel};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    AutoEqConfig, AutoEqForm, AutoEqFormUiState, Button, ButtonSize, ButtonVariant, Card, HStack,
    NumberInput, NumberInputSize, Select, SelectOption, StackAlign, StackSpacing, Text, TextSize,
    TextWeight,
    VStack,
};

use super::render::render_channel_config_row;

impl PlayerView {
    pub(crate) fn render_room_eq_configure(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let room_eq = &state.app.measurement_state.room_eq_state;

        // Build AutoEqConfig from our RoomEqOptimizerConfig
        let config = &room_eq.optimizer_config;
        let autoeq_config = AutoEqConfig {
            opt_mode: config.mode.to_code().to_string(),
            fir_taps: config.fir.taps,
            fir_phase: config.fir.phase.clone(),
            num_filters: config.num_filters,
            sample_rate: config.sample_rate,
            min_db: config.min_db,
            max_db: config.max_db,
            min_q: config.min_q,
            max_q: config.max_q,
            min_freq: config.min_freq,
            max_freq: config.max_freq,
            peq_model: config.peq_model.clone(),
            algo: match config.algorithm {
                RoomEqAlgorithm::Cobyla => "nlopt:cobyla",
                RoomEqAlgorithm::DifferentialEvolution => "autoeq:de",
                RoomEqAlgorithm::NelderMead => "nlopt:neldermead",
            }
            .to_string(),
            population: config.population,
            maxeval: config.max_iter,
            de_f: config.de_f,
            de_cr: config.de_cr,
            strategy: config.strategy.clone(),
            refine: config.refine,
            local_algo: config.local_algo.clone(),
            smooth: config.smooth,
            smooth_n: config.smooth_n,
            spacing_weight: config.spacing_weight,
            min_spacing_oct: config.min_spacing_oct,
            tolerance: config.tolerance,
            atolerance: config.atolerance,
            psychoacoustic: config.psychoacoustic,
            asymmetric_loss: config.asymmetric_loss,
            loss_type: config.loss_type.clone(),
            target_curve: config.target_curve.clone(),
            system_type: config.system_type.clone(),

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
        };

        // Build the AutoEQ form with handlers
        let autoeq_form = AutoEqForm::new("room-eq-optimizer-form")
            .config(autoeq_config)
            .ui_state(autoeq_ui_state)
            .show_optimization_tuning(true) // Show Optimization Fine Tuning section
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
                            .fir.taps = value;
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
                            .fir.phase = phase.to_string();
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
                            .algorithm = match algo {
                            "nlopt:cobyla" => RoomEqAlgorithm::Cobyla,
                            "autoeq:de" => RoomEqAlgorithm::DifferentialEvolution,
                            "nlopt:neldermead" => RoomEqAlgorithm::NelderMead,
                            _ => RoomEqAlgorithm::Cobyla,
                        };
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
            .on_sample_rate_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .sample_rate = value as u32;
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
            .on_de_f_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .de_f = value;
                    });
                }
            })
            .on_de_cr_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .de_cr = value;
                    });
                }
            })
            .on_strategy_change({
                let state = self.state.clone();
                move |strategy, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .strategy = strategy.to_string();
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .strategy_open = false;
                    });
                }
            })
            .on_strategy_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .dropdowns
                            .strategy_open = open;
                        cx.notify();
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
            .on_smooth_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .smooth = value;
                    });
                }
            })
            .on_smooth_n_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .smooth_n = value;
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
            .on_spacing_weight_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .spacing_weight = value;
                    });
                }
            })
            .on_min_spacing_oct_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .min_spacing_oct = value;
                    });
                }
            })
            .on_tolerance_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .tolerance = value;
                    });
                }
            })
            .on_atolerance_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state
                            .app
                            .measurement_state
                            .room_eq_state
                            .optimizer_config
                            .atolerance = value;
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
                        state.app.measurement_state.room_eq_state.optimizer_config.target_tilt.enabled = v;
                    });
                }
            })
            .on_tilt_type_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.room_eq_state.optimizer_config.target_tilt.tilt_type = v.to_string();
                        state.app.measurement_state.room_eq_state.dropdowns.tilt_type_open = false;
                    });
                }
            })
            .on_tilt_type_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, cx| {
                        state.app.measurement_state.room_eq_state.dropdowns.tilt_type_open = open;
                        cx.notify();
                    });
                }
            })
            .on_tilt_slope_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.room_eq_state.optimizer_config.target_tilt.slope = v;
                    });
                }
            })
            .on_tilt_reference_freq_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.room_eq_state.optimizer_config.target_tilt.reference_freq = v;
                    });
                }
            })
            .on_tilt_bass_shelf_db_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.room_eq_state.optimizer_config.target_tilt.bass_shelf_db = v;
                    });
                }
            })
            .on_tilt_bass_shelf_freq_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.room_eq_state.optimizer_config.target_tilt.bass_shelf_freq = v;
                    });
                }
            })
            // Excursion Protection
            .on_use_excursion_protection_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.room_eq_state.optimizer_config.excursion_protection.enabled = v;
                    });
                }
            })
            .on_excursion_auto_detect_f3_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.room_eq_state.optimizer_config.excursion_protection.auto_detect_f3 = v;
                    });
                }
            })
            .on_excursion_manual_f3_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.room_eq_state.optimizer_config.excursion_protection.manual_f3_hz = v;
                    });
                }
            })
            .on_excursion_filter_order_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.room_eq_state.optimizer_config.excursion_protection.filter_order = v;
                    });
                }
            })
            .on_excursion_filter_type_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.room_eq_state.optimizer_config.excursion_protection.filter_type = v.to_string();
                        state.app.measurement_state.room_eq_state.dropdowns.excursion_filter_type_open = false;
                    });
                }
            })
            .on_excursion_filter_type_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, cx| {
                        state.app.measurement_state.room_eq_state.dropdowns.excursion_filter_type_open = open;
                        cx.notify();
                    });
                }
            })
            .on_excursion_margin_octaves_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.room_eq_state.optimizer_config.excursion_protection.margin_octaves = v;
                    });
                }
            })
            // Schroeder Split
            .on_use_schroeder_split_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.room_eq_state.optimizer_config.schroeder_split.enabled = v;
                    });
                }
            })
            .on_schroeder_freq_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.room_eq_state.optimizer_config.schroeder_split.schroeder_freq = v;
                    });
                }
            })
            .on_schroeder_low_max_q_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.room_eq_state.optimizer_config.schroeder_split.low_freq_max_q = v;
                    });
                }
            })
            .on_schroeder_low_allow_boost_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.room_eq_state.optimizer_config.schroeder_split.low_freq_allow_boost = v;
                    });
                }
            })
            .on_schroeder_high_max_q_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.room_eq_state.optimizer_config.schroeder_split.high_freq_max_q = v;
                    });
                }
            })
            .on_schroeder_high_shelving_only_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.room_eq_state.optimizer_config.schroeder_split.high_freq_shelving_only = v;
                    });
                }
            })
            // Phase Alignment
            .on_use_phase_alignment_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.room_eq_state.optimizer_config.phase_alignment.enabled = v;
                    });
                }
            })
            .on_phase_min_freq_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.room_eq_state.optimizer_config.phase_alignment.min_freq = v;
                    });
                }
            })
            .on_phase_max_freq_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.room_eq_state.optimizer_config.phase_alignment.max_freq = v;
                    });
                }
            })
            .on_phase_optimize_polarity_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.room_eq_state.optimizer_config.phase_alignment.optimize_polarity = v;
                    });
                }
            })
            .on_phase_max_delay_ms_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.room_eq_state.optimizer_config.phase_alignment.max_delay_ms = v;
                    });
                }
            })
            // Multi-Seat
            .on_use_multi_seat_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.room_eq_state.optimizer_config.multi_seat.enabled = v;
                    });
                }
            })
            .on_multi_seat_strategy_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.room_eq_state.optimizer_config.multi_seat.strategy = v.to_string();
                        state.app.measurement_state.room_eq_state.dropdowns.multi_seat_strategy_open = false;
                    });
                }
            })
            .on_multi_seat_strategy_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, cx| {
                        state.app.measurement_state.room_eq_state.dropdowns.multi_seat_strategy_open = open;
                        cx.notify();
                    });
                }
            })
            .on_multi_seat_primary_seat_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.room_eq_state.optimizer_config.multi_seat.primary_seat = v;
                    });
                }
            })
            .on_multi_seat_max_deviation_db_change({
                let state = self.state.clone();
                move |v, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.measurement_state.room_eq_state.optimizer_config.multi_seat.max_deviation_db = v;
                    });
                }
            });

        // Build the system configuration card
        let system_config_card = self.render_system_config_card(cx);

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Text::new("Configure Optimization")
                    .weight(TextWeight::Bold)
                    .size(TextSize::Lg),
            )
            .child(
                Text::new("Configure per-channel settings and optimizer parameters.")
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
            // System Configuration card (topology, sub, GD-Opt, VoG, delay)
            .child(system_config_card)
            // Wrap in div to capture key events and prevent global shortcuts
            // from firing while typing in input fields
            .child(
                div()
                    .on_key_down(|_event, _window, cx| {
                        cx.stop_propagation();
                    })
                    .child(autoeq_form),
            )
            .child(
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
            )
            .child(self.render_room_eq_validation_summary(cx))
    }

    /// Render the System Configuration card with topology, sub strategy, GD-Opt, VoG, allow_delay
    fn render_system_config_card(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let config = &state.app.measurement_state.room_eq_state.optimizer_config;
        let dropdowns = &state.app.measurement_state.room_eq_state.dropdowns;
        let channel_names: Vec<String> = state
            .app
            .measurement_state
            .room_eq_state
            .channel_measurements
            .iter()
            .map(|m| m.channel_name.clone())
            .collect();

        let system_model = config.system_model;
        let has_sub = config.has_subwoofer;
        let sub_strategy = config.subwoofer_strategy;
        let gd_opt_enabled = config.gd_opt.enabled;
        let gd_opt_target_ms = config.gd_opt.target_ms;
        let vog_enabled = config.vog.enabled;
        let vog_ref = config.vog.reference_channel.clone();
        let allow_delay = config.allow_delay;

        let system_model_open = dropdowns.system_model_open;
        let sub_strategy_open = dropdowns.subwoofer_strategy_open;
        let vog_ref_open = dropdowns.vog_reference_channel_open;

        let mut content = VStack::new().spacing(StackSpacing::Md);

        // --- System Model dropdown ---
        let model_options: Vec<SelectOption> = UiSystemModel::all()
            .iter()
            .map(|m| SelectOption::new(m.as_str(), m.as_str()))
            .collect();

        content = content.child(
            Select::new("system-model-select")
                .label("System Model")
                .options(model_options)
                .selected(system_model.as_str())
                .is_open(system_model_open)
                .on_toggle({
                    let state = self.state.clone();
                    move |open, _window, cx| {
                        state.update(cx, |state, _| {
                            state.app.measurement_state.room_eq_state.dropdowns.system_model_open = open;
                        });
                    }
                })
                .on_change({
                    let state = self.state.clone();
                    move |value, _window, cx| {
                        state.update(cx, |state, _| {
                            let cfg = &mut state.app.measurement_state.room_eq_state.optimizer_config;
                            cfg.system_model = match value.as_ref() {
                                "Home Cinema" => UiSystemModel::HomeCinema,
                                "Custom" => UiSystemModel::Custom,
                                _ => UiSystemModel::Stereo,
                            };
                            state.app.measurement_state.room_eq_state.dropdowns.system_model_open = false;
                        });
                    }
                })
                .theme(theme.to_select_theme()),
        );

        // --- Subwoofer toggle ---
        content = content.child(
            HStack::new()
                .spacing(StackSpacing::Sm)
                .align(StackAlign::Center)
                .child(
                    Text::new("Has Subwoofer:")
                        .size(TextSize::Sm)
                        .color(theme.text_secondary),
                )
                .child(
                    Button::new(
                        "has-sub-toggle",
                        if has_sub { "Yes" } else { "No" },
                    )
                    .variant(if has_sub {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::Secondary
                    })
                    .size(ButtonSize::Sm)
                    .theme(theme.to_button_theme())
                    .on_click({
                        let state = self.state.clone();
                        move |_window, cx| {
                            state.update(cx, |state, cx| {
                                let cfg = &mut state.app.measurement_state.room_eq_state.optimizer_config;
                                cfg.has_subwoofer = !cfg.has_subwoofer;
                                cx.notify();
                            });
                        }
                    }),
                ),
        );

        // --- Subwoofer Strategy (shown when has_sub) ---
        if has_sub {
            let sub_options: Vec<SelectOption> = UiSubwooferStrategy::all()
                .iter()
                .map(|s| SelectOption::new(s.as_str(), s.as_str()))
                .collect();

            content = content.child(
                Select::new("sub-strategy-select")
                    .label("Subwoofer Strategy")
                    .options(sub_options)
                    .selected(sub_strategy.as_str())
                    .is_open(sub_strategy_open)
                    .on_toggle({
                        let state = self.state.clone();
                        move |open, _window, cx| {
                            state.update(cx, |state, _| {
                                state.app.measurement_state.room_eq_state.dropdowns.subwoofer_strategy_open = open;
                            });
                        }
                    })
                    .on_change({
                        let state = self.state.clone();
                        move |value, _window, cx| {
                            state.update(cx, |state, _| {
                                let cfg = &mut state.app.measurement_state.room_eq_state.optimizer_config;
                                cfg.subwoofer_strategy = match value.as_ref() {
                                    "MSO (Multi-Sub)" => UiSubwooferStrategy::Mso,
                                    "DBA (Double Bass Array)" => UiSubwooferStrategy::Dba,
                                    _ => UiSubwooferStrategy::Single,
                                };
                                state.app.measurement_state.room_eq_state.dropdowns.subwoofer_strategy_open = false;
                            });
                        }
                    })
                    .theme(theme.to_select_theme()),
            );

            // --- Group Delay Optimization ---
            content = content.child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Sm)
                            .align(StackAlign::Center)
                            .child(
                                Text::new("Group Delay Optimization:")
                                    .size(TextSize::Sm)
                                    .color(theme.text_secondary),
                            )
                            .child(
                                Button::new(
                                    "gd-opt-toggle",
                                    if gd_opt_enabled { "Enabled" } else { "Disabled" },
                                )
                                .variant(if gd_opt_enabled {
                                    ButtonVariant::Primary
                                } else {
                                    ButtonVariant::Secondary
                                })
                                .size(ButtonSize::Sm)
                                .theme(theme.to_button_theme())
                                .on_click({
                                    let state = self.state.clone();
                                    move |_window, cx| {
                                        state.update(cx, |state, cx| {
                                            let cfg = &mut state.app.measurement_state.room_eq_state.optimizer_config;
                                            cfg.gd_opt.enabled = !cfg.gd_opt.enabled;
                                            cx.notify();
                                        });
                                    }
                                }),
                            ),
                    )
                    .when(gd_opt_enabled, |el| {
                        el.child(
                            HStack::new()
                                .spacing(StackSpacing::Sm)
                                .align(StackAlign::Center)
                                .child(
                                    Text::new("Target delay (ms):")
                                        .size(TextSize::Sm)
                                        .color(theme.text_secondary),
                                )
                                .child(
                                    NumberInput::new("gd-opt-target-ms")
                                        .value(gd_opt_target_ms)
                                        .min(0.0)
                                        .max(50.0)
                                        .step(0.5)
                                        .size(NumberInputSize::Sm)
                                        .on_change({
                                            let state = self.state.clone();
                                            move |v, _window, cx| {
                                                state.update(cx, |state, _| {
                                                    state.app.measurement_state.room_eq_state.optimizer_config.gd_opt.target_ms = v;
                                                });
                                            }
                                        }),
                                ),
                        )
                    }),
            );
        }

        // --- Allow Inter-Speaker Delay ---
        content = content.child(
            HStack::new()
                .spacing(StackSpacing::Sm)
                .align(StackAlign::Center)
                .child(
                    Text::new("Allow Inter-Speaker Delay:")
                        .size(TextSize::Sm)
                        .color(theme.text_secondary),
                )
                .child(
                    Button::new(
                        "allow-delay-toggle",
                        if allow_delay { "Yes" } else { "No" },
                    )
                    .variant(if allow_delay {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::Secondary
                    })
                    .size(ButtonSize::Sm)
                    .theme(theme.to_button_theme())
                    .on_click({
                        let state = self.state.clone();
                        move |_window, cx| {
                            state.update(cx, |state, cx| {
                                let cfg = &mut state.app.measurement_state.room_eq_state.optimizer_config;
                                cfg.allow_delay = !cfg.allow_delay;
                                cx.notify();
                            });
                        }
                    }),
                ),
        );

        // --- Voice of God ---
        content = content.child(
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Sm)
                        .align(StackAlign::Center)
                        .child(
                            Text::new("Voice of God (Timbre Matching):")
                                .size(TextSize::Sm)
                                .color(theme.text_secondary),
                        )
                        .child(
                            Button::new(
                                "vog-toggle",
                                if vog_enabled { "Enabled" } else { "Disabled" },
                            )
                            .variant(if vog_enabled {
                                ButtonVariant::Primary
                            } else {
                                ButtonVariant::Secondary
                            })
                            .size(ButtonSize::Sm)
                            .theme(theme.to_button_theme())
                            .on_click({
                                let state = self.state.clone();
                                move |_window, cx| {
                                    state.update(cx, |state, cx| {
                                        let cfg = &mut state.app.measurement_state.room_eq_state.optimizer_config;
                                        cfg.vog.enabled = !cfg.vog.enabled;
                                        cx.notify();
                                    });
                                }
                            }),
                        ),
                )
                .when(vog_enabled, {
                    let theme = theme.clone();
                    let vog_ref_options: Vec<SelectOption> = channel_names
                        .iter()
                        .map(|name| SelectOption::new(name.clone(), name.clone()))
                        .collect();

                    move |el| {
                        el.child(
                            Select::new("vog-ref-channel-select")
                                .label("Reference Channel")
                                .options(vog_ref_options)
                                .selected(&vog_ref)
                                .is_open(vog_ref_open)
                                .on_toggle({
                                    let state = self.state.clone();
                                    move |open, _window, cx| {
                                        state.update(cx, |state, _| {
                                            state.app.measurement_state.room_eq_state.dropdowns.vog_reference_channel_open = open;
                                        });
                                    }
                                })
                                .on_change({
                                    let state = self.state.clone();
                                    move |value, _window, cx| {
                                        state.update(cx, |state, _| {
                                            state.app.measurement_state.room_eq_state.optimizer_config.vog.reference_channel = value.to_string();
                                            state.app.measurement_state.room_eq_state.dropdowns.vog_reference_channel_open = false;
                                        });
                                    }
                                })
                                .theme(theme.to_select_theme()),
                        )
                    }
                }),
        );

        Card::new()
            .background(theme.surface)
            .header_background(theme.background_secondary)
            .border(theme.border)
            .header(
                Text::new("System Configuration")
                    .color(theme.text_primary)
                    .weight(TextWeight::Semibold),
            )
            .content(content)
    }

    /// Render validation summary based on current config
    fn render_room_eq_validation_summary(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let validation = state.app.measurement_state.room_eq_state.validate();

        if validation.is_valid && validation.warnings.is_empty() {
            return div().into_any_element();
        }

        let mut content = VStack::new().spacing(StackSpacing::Sm);

        for error in &validation.errors {
            content = content.child(
                HStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::new("!").color(theme.error).weight(TextWeight::Bold))
                    .child(Text::new(error.clone()).size(TextSize::Sm).color(theme.error)),
            );
        }

        for warning in &validation.warnings {
            content = content.child(
                HStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::new("?").color(theme.warning).weight(TextWeight::Bold))
                    .child(Text::new(warning.clone()).size(TextSize::Sm).color(theme.warning)),
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
                .spacing(StackSpacing::Md)
                .child(
                    Text::new("No channels configured. Load measurement data first.")
                        .size(TextSize::Sm)
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
            .spacing(StackSpacing::Md)
            .children(rows)
            .into_any_element()
    }
}