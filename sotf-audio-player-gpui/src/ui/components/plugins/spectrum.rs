//! Spectrum Analyzer Plugin UI Component
//!
//! Contains both the plugin parameter editing UI and the full-screen spectrum display.

use super::common::{render_edit_hints, render_param_row, render_section_header};
use crate::theme::Theme;
use crate::ui::PlayerView;
use crate::ui::elements::SpectrumElement;
use gpui::prelude::*;
use gpui::*;
use std::sync::Arc;

/// Render the Spectrum Analyzer plugin
pub fn render_spectrum_analyzer_plugin(
    num_bins: usize,
    min_freq: f32,
    max_freq: f32,
    smoothing: f32,
    is_editing: bool,
    selected_param: usize,
    theme: &Theme,
) -> impl IntoElement {
    // Generate simulated spectrum bars
    let bar_count = 32;
    let bars: Vec<f32> = (0..bar_count)
        .map(|i| {
            // Simulated frequency response curve (peaked around midrange)
            let t = i as f32 / bar_count as f32;
            let peak = 0.5;
            let spread = 0.3;
            let value = (-(t - peak).powi(2) / (2.0 * spread * spread)).exp();
            value * 0.8 + 0.1 // Scale to 0.1-0.9 range
        })
        .collect();

    div()
        .flex()
        .flex_col()
        .gap_4()
        // Spectrum display section
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .rounded_xl()
                .bg(theme.background_secondary)
                .border_1()
                .border_color(theme.border)
                .p_4()
                .child(render_section_header("SPECTRUM ANALYZER", theme))
                // Spectrum visualization
                .child(
                    div()
                        .h(px(120.0))
                        .w_full()
                        .bg(theme.surface)
                        .rounded_lg()
                        .border_1()
                        .border_color(theme.border)
                        .flex()
                        .items_end()
                        .gap_px()
                        .p_2()
                        .children(bars.into_iter().enumerate().map(|(i, height)| {
                            // Color gradient from green to red based on frequency
                            let t = i as f32 / bar_count as f32;
                            let color = if t < 0.3 {
                                rgb(0x22c55e) // Green for bass
                            } else if t < 0.7 {
                                rgb(0xeab308) // Yellow for mids
                            } else {
                                rgb(0xef4444) // Red for highs
                            };

                            div().flex_1().h(relative(height)).bg(color).rounded_t_sm()
                        })),
                )
                // Frequency labels
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .text_xs()
                        .text_color(theme.text_muted)
                        .mt_1()
                        .child(format!("{:.0} Hz", min_freq))
                        .child("1k")
                        .child("10k")
                        .child(format!("{:.0} Hz", max_freq)),
                ),
        )
        // Analyzer info
        .child(
            div().flex().gap_4().children([
                // Resolution
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .p_3()
                    .rounded_xl()
                    .bg(theme.background_secondary)
                    .border_1()
                    .border_color(theme.border)
                    .child(div().text_xs().text_color(theme.text_muted).child("Bins"))
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.text_primary)
                            .child(format!("{}", num_bins)),
                    ),
                // Frequency range
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .p_3()
                    .rounded_xl()
                    .bg(theme.background_secondary)
                    .border_1()
                    .border_color(theme.border)
                    .child(div().text_xs().text_color(theme.text_muted).child("Range"))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.text_primary)
                            .child(format!("{:.0}-{:.0}k", min_freq, max_freq / 1000.0)),
                    ),
                // Smoothing
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .p_3()
                    .rounded_xl()
                    .bg(theme.background_secondary)
                    .border_1()
                    .border_color(theme.border)
                    .child(div().text_xs().text_color(theme.text_muted).child("Smooth"))
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.text_primary)
                            .child(format!("{:.0}%", smoothing * 100.0)),
                    ),
            ]),
        )
        // Parameters section
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .rounded_xl()
                .bg(theme.background_secondary)
                .border_1()
                .border_color(theme.border)
                .p_3()
                .child(render_section_header("PARAMETERS", theme))
                .child(render_param_row(
                    "Bins",
                    &format!("{}", num_bins),
                    0,
                    selected_param,
                    is_editing,
                    theme,
                ))
                .child(render_param_row(
                    "Min Freq",
                    &format!("{:.0} Hz", min_freq),
                    1,
                    selected_param,
                    is_editing,
                    theme,
                ))
                .child(render_param_row(
                    "Max Freq",
                    &format!("{:.0} Hz", max_freq),
                    2,
                    selected_param,
                    is_editing,
                    theme,
                ))
                .child(render_param_row(
                    "Smoothing",
                    &format!("{:.2}", smoothing),
                    3,
                    selected_param,
                    is_editing,
                    theme,
                )),
        )
        .when(is_editing, |d| d.child(render_edit_hints(theme)))
}

impl PlayerView {
    /// Render the full-screen spectrum analyzer display
    /// Uses GPU-accelerated SpectrumElement for high-performance rendering
    pub(crate) fn render_spectrum_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        let content = if let Some(info) = &state.app.spectrum_info {
            // Convert magnitudes to Arc for the GPU element
            let magnitudes: Arc<[f32]> = info.magnitudes.clone().into();

            div()
                .flex()
                .flex_col()
                .size_full()
                // GPU-accelerated spectrum visualization
                .child(
                    SpectrumElement::new(magnitudes)
                        .height(px(256.0))
                        .frequency_range(20.0, 20000.0)
                        .smoothing(0.3),
                )
                // Frequency labels
                .child(
                    div()
                        .mt_2()
                        .flex()
                        .justify_between()
                        .text_xs()
                        .text_color(rgb(0x999999))
                        .child("20 Hz")
                        .child("1 kHz")
                        .child("20 kHz"),
                )
        } else {
            div()
                .flex()
                .items_center()
                .justify_center()
                .size_full()
                .text_color(rgb(0x666666))
                .child("No spectrum data available. Play audio to see visualization.")
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .mb_4()
                    .child("Spectrum Analyzer"),
            )
            .child(content)
    }
}
