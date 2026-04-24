//! Channel Mute/Solo Plugin UI Component
//!
//! Layout (3-column):
//! +------------------+--------------------------------------------+------------------+
//! | SETUP            | CHANNELS (dynamic per channel count)       | (no output)      |
//! |                  |                                            |                  |
//! | [Enabled] toggle | [Ch1: M S] [Ch2: M S] [Ch3: M S] ...      |                  |
//! +------------------+--------------------------------------------+------------------+

// intentional-file: channel strip with embedded level meter geometry

use super::common::{render_section_title, render_toggle};
use crate::app::AppState;
use crate::components::design::Ds;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_plugins::ChannelState;

/// State for rendering the Channel Mute/Solo plugin
pub struct ChannelMuteSoloRenderState<'a> {
    pub enabled: bool,
    pub channel_states: &'a [ChannelState],
    pub is_editing: bool,
    pub selected_param: usize,
}

// Layout constants

/// Render the Channel Mute/Solo plugin
pub fn render_mute_solo_plugin(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: ChannelMuteSoloRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let channel_count = state.channel_states.len();

    let channel_names: Vec<&str> = match channel_count {
        1 => vec!["Mono"],
        2 => vec!["Left", "Right"],
        6 => vec!["FL", "FR", "C", "LFE", "RL", "RR"],
        8 => vec!["FL", "FR", "C", "LFE", "RL", "RR", "SL", "SR"],
        _ => (0..channel_count).map(|_| "Ch").collect(),
    };

    // === LEFT COLUMN: Setup ===
    let setup_col = div()
        .flex()
        .flex_col()
        .flex_shrink_0()
        .gap(d.gap_md)
        .child(render_section_title(d, "SETUP", theme))
        .child(render_toggle(
            entity.clone(),
            plugin_idx,
            "Enabled",
            state.enabled,
            0,
            state.selected_param,
            state.is_editing,
            theme,
        ))
        .child(
            div()
                .text_size(d.text_xs)
                .text_color(theme.text_muted)
                .child(format!("{} channels", channel_count)),
        );

    // === CENTER COLUMN: Channel strips ===
    let center_col = div()
        .flex()
        .flex_col()
        .flex_1()
        .gap(d.gap_md)
        .child(render_section_title(d, "CHANNELS", theme))
        .child(div().flex().gap(d.gap_md).flex_wrap().children(
            state.channel_states.iter().enumerate().map(|(i, s)| {
                let name = channel_names.get(i).copied().unwrap_or("Ch");
                let is_muted = s.muted;
                let is_soloed = s.soloed;
                let is_active =
                    !is_muted && (!state.channel_states.iter().any(|st| st.soloed) || is_soloed);

                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(d.gap)
                    .p(d.pad_y)
                    .rounded(d.r_lg)
                    .bg(theme.surface)
                    .border_1()
                    .border_color(if is_soloed {
                        theme.warning
                    } else if is_muted {
                        theme.error
                    } else {
                        theme.border
                    })
                    // Channel label
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.text_primary)
                            .child(name.to_string()),
                    )
                    // Level meter
                    .child(
                        div()
                            .w(px(16.0))
                            .h(px(60.0))
                            .bg(theme.background)
                            .rounded(d.r_sm)
                            .flex()
                            .flex_col()
                            .justify_end()
                            .overflow_hidden()
                            .child(
                                div()
                                    .w_full()
                                    .h(relative(if is_active { 0.6 } else { 0.0 }))
                                    .bg(if is_active {
                                        theme.success
                                    } else {
                                        theme.surface
                                    }),
                            ),
                    )
                    // Mute button
                    .child(
                        div()
                            .w(px(24.0))
                            .h(px(20.0))
                            .rounded(d.r_sm)
                            .bg(if is_muted {
                                theme.error
                            } else {
                                theme.background
                            })
                            .border_1()
                            .border_color(if is_muted { theme.error } else { theme.border })
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(d.text_xs)
                            .font_weight(FontWeight::BOLD)
                            .text_color(if is_muted {
                                theme.text_on_accent
                            } else {
                                theme.text_muted
                            })
                            .child("M"),
                    )
                    // Solo button
                    .child(
                        div()
                            .w(px(24.0))
                            .h(px(20.0))
                            .rounded(d.r_sm)
                            .bg(if is_soloed {
                                theme.warning
                            } else {
                                theme.background
                            })
                            .border_1()
                            .border_color(if is_soloed {
                                theme.warning
                            } else {
                                theme.border
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(d.text_xs)
                            .font_weight(FontWeight::BOLD)
                            .text_color(if is_soloed {
                                theme.background
                            } else {
                                theme.text_muted
                            })
                            .child("S"),
                    )
            }),
        ));

    // === Main layout, centered ===
    div().w_full().flex().justify_center().p(d.pad_x).child(
        div()
            .flex()
            .gap(d.section)
            .child(setup_col)
            .child(center_col),
    )
}
