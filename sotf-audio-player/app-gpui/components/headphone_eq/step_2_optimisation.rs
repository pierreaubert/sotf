use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    AutoEqConfig, AutoEqForm, AutoEqFormUiState, Button, ButtonSize, ButtonVariant, Card, HStack,
    Progress, ProgressSize, StackSpacing, Text, TextSize, TextWeight, VStack,
};

impl PlayerView {
    // ========================================================================
    // Step 2: Optimization (EQ Design, Fine Tuning, Generate)
    // ========================================================================

    pub(crate) fn render_headphone_eq_optimization(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let headphone_eq = &state.app.headphone_eq_state;

        // Build AutoEqConfig from our HeadphoneEqOptimizerConfig
        let config = &headphone_eq.optimizer_config;
        let autoeq_config = AutoEqConfig {
            num_filters: config.num_filters,
            sample_rate: 48000,
            min_db: config.min_db,
            max_db: config.max_db,
            min_q: config.min_q,
            max_q: config.max_q,
            min_freq: config.min_freq,
            max_freq: config.max_freq,
            peq_model: "pk".to_string(),
            algo: match config.algorithm {
                crate::app::types::RoomEqAlgorithm::Cobyla => "nlopt:cobyla",
                crate::app::types::RoomEqAlgorithm::DifferentialEvolution => "autoeq:de",
                crate::app::types::RoomEqAlgorithm::NelderMead => "nlopt:neldermead",
            }
            .to_string(),
            population: 100,
            maxeval: config.max_iter,
            de_f: 0.8,
            de_cr: 0.9,
            strategy: "currenttobest1bin".to_string(),
            refine: false,
            local_algo: "cobyla".to_string(),
            smooth: false,
            ..Default::default()
        };

        // Build AutoEqFormUiState from our dropdowns
        let autoeq_ui_state = AutoEqFormUiState {
            algo_open: headphone_eq.dropdowns.algorithm_open,
            peq_model_open: headphone_eq.dropdowns.peq_model_open,
            strategy_open: false,
            local_algo_open: false,
            ..Default::default()
        };

        // Build the AutoEQ form with handlers
        let autoeq_form = AutoEqForm::new("headphone-eq-optimizer-form")
            .config(autoeq_config)
            .ui_state(autoeq_ui_state)
            .show_optimization_tuning(false) // Only show EQ Design section
            .on_algo_change({
                let state = self.state.clone();
                move |algo, _window, cx| {
                    use crate::app::types::RoomEqAlgorithm;
                    state.update(cx, |state, _cx| {
                        state.app.headphone_eq_state.optimizer_config.algorithm = match algo {
                            "nlopt:cobyla" => RoomEqAlgorithm::Cobyla,
                            "autoeq:de" => RoomEqAlgorithm::DifferentialEvolution,
                            "nlopt:neldermead" => RoomEqAlgorithm::NelderMead,
                            _ => RoomEqAlgorithm::Cobyla,
                        };
                        state.app.headphone_eq_state.dropdowns.algorithm_open = false;
                    });
                }
            })
            .on_algo_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.headphone_eq_state.dropdowns.algorithm_open = open;
                    });
                }
            })
            .on_peq_model_change({
                let state = self.state.clone();
                move |_model, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.headphone_eq_state.dropdowns.peq_model_open = false;
                    });
                }
            })
            .on_peq_model_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.headphone_eq_state.dropdowns.peq_model_open = open;
                    });
                }
            })
            .on_num_filters_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.headphone_eq_state.optimizer_config.num_filters = value;
                    });
                }
            })
            .on_min_q_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.headphone_eq_state.optimizer_config.min_q = value;
                    });
                }
            })
            .on_max_q_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.headphone_eq_state.optimizer_config.max_q = value;
                    });
                }
            })
            .on_min_db_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.headphone_eq_state.optimizer_config.min_db = value;
                    });
                }
            })
            .on_max_db_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.headphone_eq_state.optimizer_config.max_db = value;
                    });
                }
            })
            .on_min_freq_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.headphone_eq_state.optimizer_config.min_freq = value;
                    });
                }
            })
            .on_max_freq_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.headphone_eq_state.optimizer_config.max_freq = value;
                    });
                }
            })
            .on_maxeval_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.headphone_eq_state.optimizer_config.max_iter = value;
                    });
                }
            });

        // Loss function selection
        let current_loss = headphone_eq.optimizer_config.loss.clone();

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Text::new("Configure Optimization")
                    .color(theme.text_primary)
                    .weight(TextWeight::Bold)
                    .size(TextSize::Lg),
            )
            .child(
                Text::new("Set the optimization parameters for your headphone EQ.")
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
            .child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(
                        Text::new("Optimization Goal")
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Text::new("Choose what the optimizer should optimize for.")
                                    .size(TextSize::Sm)
                                    .color(theme.text_secondary),
                            )
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .child({
                                        let is_selected = current_loss == "headphone-score";
                                        Button::new("loss-score", "Harman Score")
                                            .variant(if is_selected {
                                                ButtonVariant::Primary
                                            } else {
                                                ButtonVariant::Secondary
                                            })
                                            .size(ButtonSize::Sm)
                                            .build()
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(|view, _, _, cx| {
                                                    view.state.update(cx, |state, _cx| {
                                                        state
                                                            .app
                                                            .headphone_eq_state
                                                            .optimizer_config
                                                            .loss = "headphone-score".to_string();
                                                    });
                                                    cx.notify();
                                                }),
                                            )
                                    })
                                    .child({
                                        let is_selected = current_loss == "headphone-flat";
                                        Button::new("loss-flat", "Target Flat")
                                            .variant(if is_selected {
                                                ButtonVariant::Primary
                                            } else {
                                                ButtonVariant::Secondary
                                            })
                                            .size(ButtonSize::Sm)
                                            .build()
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(|view, _, _, cx| {
                                                    view.state.update(cx, |state, _cx| {
                                                        state
                                                            .app
                                                            .headphone_eq_state
                                                            .optimizer_config
                                                            .loss = "headphone-flat".to_string();
                                                    });
                                                    cx.notify();
                                                }),
                                            )
                                    }),
                            ),
                    ),
            )
            .child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(
                        Text::new("EQ Parameters")
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
                    .content(autoeq_form),
            )
            // Generate EQ section
            .child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(
                        Text::new("Generate Headphone EQ")
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
                    .content({
                        let progress = headphone_eq.progress;
                        let status_msg = headphone_eq.status_message.clone();
                        let is_optimizing = headphone_eq.is_optimizing();

                        VStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Button::new(
                                    "start_optimization",
                                    if is_optimizing {
                                        "Optimizing..."
                                    } else {
                                        "Generate Headphone EQ"
                                    },
                                )
                                .variant(ButtonVariant::Primary)
                                .size(ButtonSize::Lg)
                                .full_width(true)
                                .disabled(is_optimizing)
                                .build()
                                .when(!is_optimizing, |btn| {
                                    btn.on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _, _, cx| {
                                            view.start_headphone_eq_optimization(cx);
                                        }),
                                    )
                                }),
                            )
                            .when(is_optimizing || progress > 0.0, |vstack| {
                                vstack
                                    .child(
                                        Text::new(format!("Progress: {:.0}%", progress * 100.0))
                                            .size(TextSize::Sm)
                                            .color(theme.text_primary),
                                    )
                                    .child(Progress::new(progress * 100.0).size(ProgressSize::Md))
                                    .child(
                                        Text::new(status_msg)
                                            .size(TextSize::Sm)
                                            .color(theme.text_secondary),
                                    )
                            })
                    }),
            )
    }


}
