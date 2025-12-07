//! Speaker settings content
//!
//! UI for Spinorama speaker optimization

use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonTheme, ButtonVariant, Card, HStack, Progress, ProgressSize, Text,
    TextSize, TextWeight, VStack,
};

/// Target curve options for speaker EQ
pub const SPEAKER_TARGET_OPTIONS: &[(&str, &str)] = &[
    ("flat", "Flat (Anechoic)"),
    ("slope", "Gentle Slope (In-Room)"), 
    ("custom", "Custom File..."),
];

impl PlayerView {
    pub(crate) fn render_speaker_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (
            theme,
            speaker_model,
            speaker_params,
            optimization_running,
            optimization_progress,
            speaker_optimization_result,
            speaker_export_format,
        ) = {
            let state = self.state.read(cx);
            (
                state.app.theme.clone(),
                state.app.speaker_model.clone(),
                state.app.speaker_params.clone(),
                state.app.speaker_optimization_running,
                state.app.speaker_optimization_progress.clone(),
                state.app.speaker_optimization_result.clone(),
                state.app.speaker_export_format.clone(),
            )
        };

        div()
            .id("speaker-settings-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .child(
                div()
                    .id("speaker-settings-content")
                    .flex()
                    .flex_col()
                    .gap_4()
                    .pb_4()
                    // Intro
                    .child(
                        Text::new("Optimize speakers using Spinorama.org measurements")
                            .size(TextSize::Xs)
                            .muted(true),
                    )
                    // Speaker Model Search
                    .child(self.render_speaker_selection(
                        &speaker_model,
                        &theme,
                        cx,
                    ))
                    // Optimization Goal (Loss)
                    .child(self.render_option_chips(
                        "Optimization Goal",
                        &speaker_params.loss,
                        crate::autoeq::params::SPEAKER_LOSS_OPTIONS,
                        "speaker-loss",
                        &theme,
                        cx,
                    ))
                    // Curve Selection (Listening Window, On Axis, etc.)
                    .child(self.render_option_chips(
                        "Curve to Optimize",
                        &speaker_params.curve_name,
                        crate::autoeq::params::CURVE_NAME_OPTIONS,
                        "speaker-curve",
                        &theme,
                        cx,
                    ))
                    // EQ Parameters
                    .child(self.render_eq_design_params(&speaker_params, "speaker", &theme, cx))
                    // Tuning Parameters
                    .child(self.render_optimization_tuning_params(&speaker_params, "speaker", &theme, cx))
                    // Generate Button
                    .child(
                        Button::new(
                            "generate-speaker-eq",
                            if optimization_running { "Optimizing..." } else { "Generate Speaker EQ" },
                        )
                        .variant(ButtonVariant::Primary)
                        .size(ButtonSize::Lg)
                        .full_width(true)
                        .disabled(optimization_running)
                        .build()
                        .when(!optimization_running, |d| {
                            d.on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                    view.run_speaker_optimization(cx);
                                }),
                            )
                        }),
                    )
                    // Progress
                    .when(optimization_running, |d| {
                        d.child(self.render_optimization_progress(
                            &optimization_progress,
                            speaker_params.maxeval,
                            &theme,
                        ))
                    })
                    // Results
                    .when_some(speaker_optimization_result.as_ref(), |d, result| {
                        d.child(
                            Card::new()
                                .header(Text::new("Optimization Results").weight(TextWeight::Semibold))
                                .content(self.render_speaker_results(result, &theme, 1000.0)),
                        )
                    }),
            )
    }

    /// Render speaker model selection (for now just a searchable input)
    // TODO: Implement actual autocomplete/search from spinorama DB
    fn render_speaker_selection(
        &self,
        current_model: &str,
        theme: &crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = theme.clone();
        let current_model = SharedString::from(current_model.to_string());
        
        Card::new()
            .header(Text::new("Speaker Selection").weight(TextWeight::Semibold))
            .content(
                VStack::new()
                    .spacing(gpui_ui_kit::StackSpacing::Sm)
                    .child(Text::new("Model Name (e.g. 'KEF LS50')").size(TextSize::Xs))
                    .child(
                        gpui_ui_kit::Input::new("speaker-model-input")
                            .value(current_model)
                            .placeholder("Search Spinorama.org...")
                            .on_input(cx.listener(move |view, text, _cx| {
                                view.state.update(_cx, |state, _| {
                                    state.app.speaker_model = text.to_string();
                                });
                            }))
                            .build()
                    )
                    .child(
                        Text::new("Use 'Dummy Speaker' to test UI without downloading.")
                            .size(TextSize::Xs)
                            .color(theme.text_secondary)
                            .italic(true)
                    )
            )
    }

    /// Render speaker optimization results
    fn render_speaker_results(
        &self,
        result: &crate::autoeq::speaker_eq::SpeakerOptimizationResult,
        theme: &crate::theme::Theme,
        available_width: f32,
    ) -> impl IntoElement {
        // Placeholder for now, will implement actual graphs in result_graphs.rs or here
        self.render_speaker_optimization_result_graphs(result, theme, available_width)
    }
}
