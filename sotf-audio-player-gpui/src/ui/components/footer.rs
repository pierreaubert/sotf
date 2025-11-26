//! Footer component rendering

use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;

impl PlayerView {
    pub(crate) fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        // Format time as MM:SS
        let format_time = |secs: f64| -> String {
            let mins = (secs / 60.0) as u32;
            let secs = (secs % 60.0) as u32;
            format!("{:02}:{:02}", mins, secs)
        };

        let position_str = format_time(state.app.position_secs);
        let duration_str = format_time(state.app.duration_secs);
        let time_display = format!("{} / {}", position_str, duration_str);

        // Calculate progress percentage for visual bar
        let progress = if state.app.duration_secs > 0.0 {
            (state.app.position_secs / state.app.duration_secs).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };

        // Get LUFS display if available
        let lufs_display = state
            .app
            .loudness_info
            .as_ref()
            .map(|info| format!("{:.1} LUFS", info.integrated_lufs))
            .unwrap_or_default();

        div()
            .flex()
            .flex_col()
            .bg(rgb(0x2d2d2d))
            .border_t_1()
            .border_color(rgb(0x3e3e3e))
            // Progress bar
            .child(
                div()
                    .w_full()
                    .h(px(4.0))
                    .bg(rgb(0x1e1e1e))
                    .child(
                        div()
                            .h_full()
                            .w(gpui::Length::Definite(gpui::DefiniteLength::Fraction(progress)))
                            .bg(rgb(0x007acc)),
                    ),
            )
            // Main footer content
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .p_3()
                    .child(
                        div()
                            .flex()
                            .gap_4()
                            .items_center()
                            .child(div().text_sm().child(if state.app.is_playing {
                                "▶ Playing"
                            } else {
                                "⏹ Stopped"
                            }))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0xcccccc))
                                    .child(time_display),
                            )
                            .when(!lufs_display.is_empty(), |d| {
                                d.child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(0x4ec9b0))
                                        .child(lufs_display),
                                )
                            })
                            .child(
                                div()
                                    .text_sm()
                                    .child(format!("Vol: {:.0}%", state.app.volume * 100.0)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x999999))
                                    .child("Space: Play/Pause"),
                            )
                            .child(div().text_xs().text_color(rgb(0x999999)).child("N: Next"))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x999999))
                                    .child("+/-: Volume"),
                            ),
                    ),
            )
    }
}
