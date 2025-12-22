use crate::app::types::RoomEqAlgorithm;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui_ui_kit::{
    AutoEqConfig, AutoEqForm, AutoEqFormUiState, Card, StackSpacing, Text, TextSize, TextWeight,
    VStack,
};

use super::render::render_channel_config_row;

impl PlayerView {

    pub(crate) fn render_room_eq_configure(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let room_eq = &state.app.room_eq_state;

        // Build AutoEqConfig from our RoomEqOptimizerConfig
        let config = &room_eq.optimizer_config;
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
                RoomEqAlgorithm::Cobyla => "nlopt:cobyla",
                RoomEqAlgorithm::DifferentialEvolution => "autoeq:de",
                RoomEqAlgorithm::NelderMead => "nlopt:neldermead",
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
            algo_open: room_eq.dropdowns.algorithm_open,
            peq_model_open: room_eq.dropdowns.peq_model_open,
            strategy_open: false,
            local_algo_open: false,
            ..Default::default()
        };

        // Build the AutoEQ form with handlers
        let autoeq_form = AutoEqForm::new("room-eq-optimizer-form")
            .config(autoeq_config)
            .ui_state(autoeq_ui_state)
            .on_algo_change({
                let state = self.state.clone();
                move |algo, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.room_eq_state.optimizer_config.algorithm = match algo {
                            "nlopt:cobyla" => RoomEqAlgorithm::Cobyla,
                            "autoeq:de" => RoomEqAlgorithm::DifferentialEvolution,
                            "nlopt:neldermead" => RoomEqAlgorithm::NelderMead,
                            _ => RoomEqAlgorithm::Cobyla,
                        };
                        state.app.room_eq_state.dropdowns.algorithm_open = false;
                    });
                }
            })
            .on_algo_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.room_eq_state.dropdowns.algorithm_open = open;
                    });
                }
            })
            .on_peq_model_change({
                let state = self.state.clone();
                move |_model, _window, cx| {
                    state.update(cx, |state, _cx| {
                        // PEQ model is stored in autoeq_config.peq_model which is read-only display
                        // The actual model selection doesn't need to be stored separately
                        state.app.room_eq_state.dropdowns.peq_model_open = false;
                    });
                }
            })
            .on_peq_model_toggle({
                let state = self.state.clone();
                move |open, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.room_eq_state.dropdowns.peq_model_open = open;
                    });
                }
            })
            .on_num_filters_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.room_eq_state.optimizer_config.num_filters = value;
                    });
                }
            })
            .on_min_q_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.room_eq_state.optimizer_config.min_q = value;
                    });
                }
            })
            .on_max_q_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.room_eq_state.optimizer_config.max_q = value;
                    });
                }
            })
            .on_min_db_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.room_eq_state.optimizer_config.min_db = value;
                    });
                }
            })
            .on_max_db_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.room_eq_state.optimizer_config.max_db = value;
                    });
                }
            })
            .on_min_freq_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.room_eq_state.optimizer_config.min_freq = value;
                    });
                }
            })
            .on_max_freq_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.room_eq_state.optimizer_config.max_freq = value;
                    });
                }
            })
            .on_maxeval_change({
                let state = self.state.clone();
                move |value, _window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.room_eq_state.optimizer_config.max_iter = value;
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
            .child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(
                        Text::new("Optimizer Settings")
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
                    .content(autoeq_form),
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
        let theme = state.app.theme.clone();
        let speaker_configs = state.app.room_eq_state.speaker_configs.clone();

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
