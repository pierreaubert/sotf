//! Channel Mute/Solo Plugin UI Component

use super::common::{render_edit_hints, render_section_header, render_toggle};
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_plugins::ChannelState;

/// Render the Channel Mute/Solo plugin
pub fn render_mute_solo_plugin(
    enabled: bool,
    channel_states: &[ChannelState],
    is_editing: bool,
    selected_param: usize,
    theme: &Theme,
) -> impl IntoElement {
    let channel_count = channel_states.len();

    // Channel layout names
    let channel_names: Vec<&str> = match channel_count {
        1 => vec!["Mono"],
        2 => vec!["Left", "Right"],
        6 => vec!["FL", "FR", "C", "LFE", "RL", "RR"],
        8 => vec!["FL", "FR", "C", "LFE", "RL", "RR", "SL", "SR"],
        _ => (0..channel_count).map(|_| "Ch").collect(),
    };

    div()
        .flex()
        .flex_col()
        .gap_4()
        // Channel mixer section
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
                .child(render_section_header("CHANNEL MIXER", theme))
                // Channel strips
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .justify_center()
                        .children(channel_states.iter().enumerate().map(|(i, state)| {
                            let name = channel_names.get(i).copied().unwrap_or("Ch");
                            let is_muted = state.muted;
                            let is_soloed = state.soloed;
                            let is_active = !is_muted && (!channel_states.iter().any(|s| s.soloed) || is_soloed);

                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap_2()
                                .p_2()
                                .rounded_lg()
                                .bg(theme.surface)
                                .border_1()
                                .border_color(if is_soloed {
                                    rgb(0xeab308)
                                } else if is_muted {
                                    rgb(0xef4444)
                                } else {
                                    theme.border
                                })
                                // Channel label
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme.text_primary)
                                        .child(name.to_string()),
                                )
                                // Level meter (simulated)
                                .child(
                                    div()
                                        .w(px(16.0))
                                        .h(px(60.0))
                                        .bg(theme.background)
                                        .rounded_sm()
                                        .flex()
                                        .flex_col()
                                        .justify_end()
                                        .overflow_hidden()
                                        .child(
                                            div()
                                                .w_full()
                                                .h(relative(if is_active { 0.6 } else { 0.0 }))
                                                .bg(if is_active { rgb(0x22c55e) } else { theme.surface }),
                                        ),
                                )
                                // Mute button
                                .child(
                                    div()
                                        .w(px(24.0))
                                        .h(px(20.0))
                                        .rounded_sm()
                                        .bg(if is_muted { rgb(0xef4444) } else { theme.background })
                                        .border_1()
                                        .border_color(if is_muted { rgb(0xef4444) } else { theme.border })
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(if is_muted { rgb(0xffffff) } else { theme.text_muted })
                                        .child("M"),
                                )
                                // Solo button
                                .child(
                                    div()
                                        .w(px(24.0))
                                        .h(px(20.0))
                                        .rounded_sm()
                                        .bg(if is_soloed { rgb(0xeab308) } else { theme.background })
                                        .border_1()
                                        .border_color(if is_soloed { rgb(0xeab308) } else { theme.border })
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(if is_soloed { rgb(0x000000) } else { theme.text_muted })
                                        .child("S"),
                                )
                        })),
                ),
        )
        // Status section
        .child(
            div()
                .flex()
                .gap_4()
                .children([
                    // Enabled status
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap_2()
                        .p_3()
                        .rounded_xl()
                        .bg(theme.background_secondary)
                        .border_1()
                        .border_color(theme.border)
                        .child(
                            div()
                                .w(px(12.0))
                                .h(px(12.0))
                                .rounded_full()
                                .bg(if enabled { rgb(0x22c55e) } else { rgb(0xef4444) }),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.text_primary)
                                .child(if enabled { "Active" } else { "Bypassed" }),
                        ),
                    // Mute count
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
                        .child(div().text_xs().text_color(theme.text_muted).child("Muted"))
                        .child(
                            div()
                                .text_lg()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(0xef4444))
                                .child(format!("{}", channel_states.iter().filter(|s| s.muted).count())),
                        ),
                    // Solo count
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
                        .child(div().text_xs().text_color(theme.text_muted).child("Soloed"))
                        .child(
                            div()
                                .text_lg()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(0xeab308))
                                .child(format!("{}", channel_states.iter().filter(|s| s.soloed).count())),
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
                .child(render_toggle("Enabled", enabled, 0, selected_param, is_editing, theme))
                .child(
                    div()
                        .px_3()
                        .py_2()
                        .text_sm()
                        .text_color(theme.text_muted)
                        .child(format!("Channels: {}", channel_count)),
                ),
        )
        // Keyboard hints
        .child(
            div()
                .p_3()
                .rounded_lg()
                .bg(theme.accent_muted)
                .border_1()
                .border_color(theme.accent)
                .flex()
                .gap_4()
                .text_xs()
                .text_color(theme.text_secondary)
                .child("m: Mute")
                .child("Shift+M: Solo")
                .child("x: Clear all"),
        )
        .when(is_editing, |d| d.child(render_edit_hints(theme)))
}
