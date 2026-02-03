use crate::app::types::RoomEqAlgorithm;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    AutoEqConfig, AutoEqForm, AutoEqFormUiState, Card, HStack, StackJustify, StackSpacing, Text,
    TextSize, TextWeight, Toggle, ToggleTheme, VStack,
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
            smooth: config.smooth,
            smooth_n: config.smooth_n,
            psychoacoustic: config.psychoacoustic,
            asymmetric_loss: config.asymmetric_loss,
            loss_type: config.loss_type.clone(),
            target_curve: config.target_curve.clone(),
            system_type: config.system_type.clone(),
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
            });

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