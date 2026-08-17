//! Channel Mute/Solo Plugin UI Component
//!
//! Layout (3-column):
//! +------------------+--------------------------------------------+------------------+
//! | SETUP            | CHANNELS (dynamic per channel count)       | (no output)      |
//! |                  |                                            |                  |
//! | [Enabled] toggle | [Ch1: M S] [Ch2: M S] [Ch3: M S] ...      |                  |
//! +------------------+--------------------------------------------+------------------+

// intentional-file: channel strip with embedded level meter geometry

use super::common::{render_knob, render_section_title, render_toggle};
use crate::app::AppState;
use crate::app::i18n::PluginCommonTranslations;
use crate::app::types::PluginUpdateType;
use crate::components::design::Ds;
use crate::components::themed_tooltip;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::PluginSettings;
use sotf_plugins::ChannelState;
use sotf_plugins::param_specs::{channel_mute_solo::PARAMS as CMS, find_by_key as pk};

/// State for rendering the Channel Mute/Solo plugin
pub struct ChannelMuteSoloRenderState<'a> {
    pub enabled: bool,
    pub dim_gain_db: f64,
    pub fade_ms: f64,
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
    text: PluginCommonTranslations,
    theme: &Theme,
) -> impl IntoElement {
    let channel_count = state.channel_states.len();

    let channel_names = (0..channel_count)
        .map(|index| sotf_audio_player::get_channel_label(index, channel_count))
        .collect::<Vec<_>>();

    // === LEFT COLUMN: Setup ===
    let setup_col = div()
        .flex()
        .flex_col()
        .flex_shrink_0()
        .gap(d.gap_md)
        .child(render_section_title(d, text.label("SETUP"), theme))
        .child(render_toggle(
            entity.clone(),
            plugin_idx,
            text.label("Enabled"),
            state.enabled,
            0,
            state.selected_param,
            state.is_editing,
            theme,
        ))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            text.label("Dim Gain"),
            state.dim_gain_db,
            pk(CMS, "dim_gain_db").min_f64(),
            pk(CMS, "dim_gain_db").max_f64(),
            "dB",
            1,
            state.selected_param,
            state.is_editing,
            None,
            theme,
        ))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            text.label("Fade Time"),
            state.fade_ms,
            pk(CMS, "fade_ms").min_f64(),
            pk(CMS, "fade_ms").max_f64(),
            "ms",
            2,
            state.selected_param,
            state.is_editing,
            None,
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
        .child(render_section_title(d, text.label("CHANNELS"), theme))
        .child(div().flex().gap(d.gap_md).flex_wrap().children(
            state.channel_states.iter().enumerate().map(|(i, s)| {
                let name = channel_names.get(i).map(String::as_str).unwrap_or("Ch");
                let is_muted = s.muted;
                let is_soloed = s.soloed;
                let is_dimmed = s.dimmed;
                let mute_hint = format!("Mute {name}");
                let solo_hint = format!("Solo {name}");
                let dim_hint = format!("Dim {name}");
                let mute_theme = theme.clone();
                let solo_theme = theme.clone();
                let dim_theme = theme.clone();
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
                    // Mute button
                    .child(
                        div()
                            .id(ElementId::Name(
                                format!("mute-channel-{plugin_idx}-{i}").into(),
                            ))
                            .w(rems(1.75))
                            .h(rems(1.5))
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
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.85))
                            .tooltip(move |_window, cx| {
                                themed_tooltip(mute_hint.clone(), &mute_theme, cx)
                            })
                            .on_mouse_down(MouseButton::Left, {
                                let entity = entity.clone();
                                move |_, _, cx| {
                                    toggle_msd_state(&entity, plugin_idx, i, MsdAction::Mute, cx);
                                }
                            })
                            .child("M"),
                    )
                    // Solo button
                    .child(
                        div()
                            .id(ElementId::Name(
                                format!("solo-channel-{plugin_idx}-{i}").into(),
                            ))
                            .w(rems(1.75))
                            .h(rems(1.5))
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
                                theme.text_on_accent
                            } else {
                                theme.text_muted
                            })
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.85))
                            .tooltip(move |_window, cx| {
                                themed_tooltip(solo_hint.clone(), &solo_theme, cx)
                            })
                            .on_mouse_down(MouseButton::Left, {
                                let entity = entity.clone();
                                move |_, _, cx| {
                                    toggle_msd_state(&entity, plugin_idx, i, MsdAction::Solo, cx);
                                }
                            })
                            .child("S"),
                    )
                    // Dim button
                    .child(
                        div()
                            .id(ElementId::Name(
                                format!("dim-channel-{plugin_idx}-{i}").into(),
                            ))
                            .w(rems(1.75))
                            .h(rems(1.5))
                            .rounded(d.r_sm)
                            .bg(if is_dimmed {
                                theme.accent
                            } else {
                                theme.background
                            })
                            .border_1()
                            .border_color(if is_dimmed {
                                theme.accent
                            } else {
                                theme.border
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(d.text_xs)
                            .font_weight(FontWeight::BOLD)
                            .text_color(if is_dimmed {
                                theme.text_on_accent
                            } else {
                                theme.text_muted
                            })
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.85))
                            .tooltip(move |_window, cx| {
                                themed_tooltip(dim_hint.clone(), &dim_theme, cx)
                            })
                            .on_mouse_down(MouseButton::Left, {
                                let entity = entity.clone();
                                move |_, _, cx| {
                                    toggle_msd_state(&entity, plugin_idx, i, MsdAction::Dim, cx);
                                }
                            })
                            .child("D"),
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

#[derive(Clone, Copy)]
enum MsdAction {
    Mute,
    Solo,
    Dim,
}

fn toggle_msd_state(
    entity: &Entity<AppState>,
    plugin_idx: usize,
    channel_idx: usize,
    action: MsdAction,
    cx: &mut App,
) {
    entity.update(cx, |state, cx| {
        let Some(plugin) = state.app.plugin_state.graph.get_plugin_mut(plugin_idx) else {
            return;
        };
        let PluginSettings::ChannelMuteSolo { channel_states, .. } = &mut plugin.settings else {
            return;
        };
        if channel_idx >= channel_states.len() {
            channel_states.resize(channel_idx + 1, ChannelState::default());
        }
        let channel = &mut channel_states[channel_idx];
        match action {
            MsdAction::Mute => channel.muted = !channel.muted,
            MsdAction::Solo => channel.soloed = !channel.soloed,
            MsdAction::Dim => channel.dimmed = !channel.dimmed,
        }
        state.app.plugin_state.update_state.pending_plugin_update =
            Some(PluginUpdateType::Structural);
        cx.notify();
    });
}
