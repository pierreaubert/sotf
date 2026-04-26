//! Misc settings content (CPU cores, etc.)

use crate::components::design::Ds;
use crate::ui::PlayerView;
use crate::ui::{DEFAULT_MAX_FONT_SIZE_PX, DEFAULT_MIN_FONT_SIZE_PX};
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{HStack, NumberInput, NumberInputSize, StackSpacing, Text, TextSize, VStack};

impl PlayerView {
    /// Render misc settings content
    pub(crate) fn render_plugins_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let max_cores = state.app.ui_state.max_cpu_cores;
        let min_font_px = state.app.ui_state.min_font_size_px;
        let max_font_px = state.app.ui_state.max_font_size_px;

        let total_cores = std::thread::available_parallelism()
            .map(|n| n.get() as u8)
            .unwrap_or(4);

        let current_value = max_cores.unwrap_or(total_cores) as f64;

        div()
            .flex()
            .flex_col()
            .gap(d.section_lg)
            .child(
                div()
                    .text_size(d.text_sm)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text_primary)
                    .child("Miscellaneous"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(d.section)
                    .p(d.card)
                    .bg(theme.background_secondary)
                    .rounded(d.r_md)
                    .border_1()
                    .border_color(theme.border)
                    // CPU cores row
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                VStack::new()
                                    .spacing(StackSpacing::Xs)
                                    .child(
                                        Text::new("Max CPU Cores")
                                            .size(TextSize::Sm)
                                            .weight(gpui_ui_kit::TextWeight::Bold)
                                            .color(theme.text_primary),
                                    )
                                    .child(
                                        Text::new(format!(
                                            "Limit the number of CPU cores SotF can use ({} available).",
                                            total_cores
                                        ))
                                        .size(TextSize::Xs)
                                        .color(theme.text_secondary),
                                    )
                                    .build()
                                    .flex_1(),
                            )
                            .child({
                                let state_entity = self.state.clone();
                                NumberInput::new("max-cpu-cores")
                                    .value(current_value)
                                    .range(1.0, total_cores as f64)
                                    .step(1.0)
                                    .decimals(0)
                                    .unit("cores")
                                    .size(NumberInputSize::Sm)
                                    .width(120.0)
                                    .on_change(move |val, _window, cx| {
                                        let cores = (val as u8).clamp(1, total_cores);
                                        state_entity.update(cx, |state, _cx| {
                                            if cores == total_cores {
                                                state.app.ui_state.max_cpu_cores = None;
                                            } else {
                                                state.app.ui_state.max_cpu_cores = Some(cores);
                                            }
                                        });
                                    })
                            }),
                    )
                    // Min font size row
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                VStack::new()
                                    .spacing(StackSpacing::Xs)
                                    .child(
                                        Text::new("Min Font Size")
                                            .size(TextSize::Sm)
                                            .weight(gpui_ui_kit::TextWeight::Bold)
                                            .color(theme.text_primary),
                                    )
                                    .child(
                                        Text::new("Smallest font size when the window is resized down.")
                                            .size(TextSize::Xs)
                                            .color(theme.text_secondary),
                                    )
                                    .build()
                                    .flex_1(),
                            )
                            .child({
                                let state_entity = self.state.clone();
                                let current_max = max_font_px.unwrap_or(DEFAULT_MAX_FONT_SIZE_PX);
                                NumberInput::new("min-font-size")
                                    .value(min_font_px.unwrap_or(DEFAULT_MIN_FONT_SIZE_PX) as f64)
                                    .range(4.0, (current_max - 1.0) as f64)
                                    .step(1.0)
                                    .decimals(0)
                                    .unit("px")
                                    .size(NumberInputSize::Sm)
                                    .width(120.0)
                                    .on_change(move |val, _window, cx| {
                                        let px = (val as f32).clamp(4.0, current_max - 1.0);
                                        state_entity.update(cx, |state, _cx| {
                                            state.app.ui_state.min_font_size_px = Some(px);
                                        });
                                    })
                            }),
                    )
                    // Max font size row
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                VStack::new()
                                    .spacing(StackSpacing::Xs)
                                    .child(
                                        Text::new("Max Font Size")
                                            .size(TextSize::Sm)
                                            .weight(gpui_ui_kit::TextWeight::Bold)
                                            .color(theme.text_primary),
                                    )
                                    .child(
                                        Text::new("Largest font size when the window is resized up.")
                                            .size(TextSize::Xs)
                                            .color(theme.text_secondary),
                                    )
                                    .build()
                                    .flex_1(),
                            )
                            .child({
                                let state_entity = self.state.clone();
                                let current_min = min_font_px.unwrap_or(DEFAULT_MIN_FONT_SIZE_PX);
                                NumberInput::new("max-font-size")
                                    .value(max_font_px.unwrap_or(DEFAULT_MAX_FONT_SIZE_PX) as f64)
                                    .range((current_min + 1.0) as f64, 48.0)
                                    .step(1.0)
                                    .decimals(0)
                                    .unit("px")
                                    .size(NumberInputSize::Sm)
                                    .width(120.0)
                                    .on_change(move |val, _window, cx| {
                                        let px = (val as f32).clamp(current_min + 1.0, 48.0);
                                        state_entity.update(cx, |state, _cx| {
                                            state.app.ui_state.max_font_size_px = Some(px);
                                        });
                                    })
                            }),
                    ),
            )
    }
}
