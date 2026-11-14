//! Compact channel mute, solo and dim controls.
//!
//! Master enable belongs to the plugin header. Schema-backed gain and fade
//! controls remain visible above independent per-channel M/S/D buttons.

use super::ui_layout_renderer::{render_compact_choice_param, render_horizontal_param};
use crate::app::AppState;
use crate::app::i18n::PluginCommonTranslations;
use crate::components::design::Ds;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::{AnyElement, App, Entity, ScrollAnchor, ScrollHandle, SharedString, div, rems};
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant, Text};
use sotf_audio_player::PluginSettings;
use sotf_plugins::ChannelState;
use sotf_plugins::param_specs::channel_mute_solo::PARAMS as CMS;

/// State for rendering the Channel Mute/Solo plugin.
#[derive(Debug)]
pub struct ChannelMuteSoloRenderState<'a> {
    pub dim_gain_db: f64,
    pub fade_ms: f64,
    pub channel_states: &'a [ChannelState],
    pub channel_count: usize,
    pub is_editing: bool,
    pub selected_param: usize,
}

/// Render master enable beside the plugin title.
#[expect(
    clippy::large_types_passed_by_value,
    reason = "Copy translation snapshots remain owned by returned elements and deferred callbacks"
)]
pub(crate) fn render_header(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    settings: &PluginSettings,
    text: PluginCommonTranslations,
    theme: &Theme,
) -> AnyElement {
    let mut param = CMS[0];
    param.name = text.label(param.name);
    div()
        .flex_1()
        .min_w_0()
        .child(render_compact_choice_param(
            d,
            entity,
            plugin_idx,
            0,
            &param,
            settings.param_value(0).unwrap_or(param.default_f64()),
            theme,
        ))
        .into_any_element()
}

/// Render primary settings and independent channel actions.
#[must_use]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
pub fn render_mute_solo_plugin(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: ChannelMuteSoloRenderState,
    text: PluginCommonTranslations,
    theme: &Theme,
    scroll_handle: Option<ScrollHandle>,
) -> impl IntoElement {
    let mut primary = div().flex().flex_wrap().gap(d.grid).w_full().min_w_0();
    for (index, value) in [(1, state.dim_gain_db), (2, state.fade_ms)] {
        let mut param = CMS[index];
        param.name = text.label(param.name);
        primary = primary.child(
            div()
                .flex_1()
                .min_w(rems(23.0))
                .child(render_horizontal_param(
                    d,
                    entity.clone(),
                    plugin_idx,
                    index,
                    &param,
                    value,
                    state.is_editing,
                    state.selected_param,
                    true,
                    theme,
                )),
        );
    }
    let channels = div()
        .flex()
        .flex_wrap()
        .gap(d.grid)
        .children((0..state.channel_count).map(|index| {
            let channel = state.channel_states.get(index).cloned().unwrap_or_default();
            let name = sotf_audio_player::get_channel_label(index, state.channel_count);
            div()
                .flex()
                .items_center()
                .gap(d.gap)
                .child(Text::label(name.clone()))
                .child(render_channel_button(
                    entity.clone(),
                    plugin_idx,
                    index,
                    &name,
                    MsdAction::Mute,
                    channel.muted,
                    theme,
                    scroll_handle.clone(),
                ))
                .child(render_channel_button(
                    entity.clone(),
                    plugin_idx,
                    index,
                    &name,
                    MsdAction::Solo,
                    channel.soloed,
                    theme,
                    scroll_handle.clone(),
                ))
                .child(render_channel_button(
                    entity.clone(),
                    plugin_idx,
                    index,
                    &name,
                    MsdAction::Dim,
                    channel.dimmed,
                    theme,
                    scroll_handle.clone(),
                ))
        }));
    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(d.grid)
        .child(primary)
        .child(Text::caption(format!(
            "{} channels · M: Mute · S: Solo · D: Dim",
            state.channel_count
        )))
        .child(channels)
}

fn render_channel_button(
    entity: Entity<AppState>,
    plugin_idx: usize,
    channel_idx: usize,
    channel_name: &str,
    action: MsdAction,
    active: bool,
    theme: &Theme,
    scroll_handle: Option<ScrollHandle>,
) -> impl IntoElement {
    let (id, label, action_name) = match action {
        MsdAction::Mute => ("mute", "M", "Mute"),
        MsdAction::Solo => ("solo", "S", "Solo"),
        MsdAction::Dim => ("dim", "D", "Dim"),
    };
    let button = Button::new(
        SharedString::from(format!("{id}-channel-{plugin_idx}-{channel_idx}")),
        label,
    )
    .aria_label(format!("{action_name} {channel_name}"))
    .variant(if active {
        ButtonVariant::Primary
    } else {
        ButtonVariant::Secondary
    })
    .size(ButtonSize::Xs)
    .theme(theme.to_button_theme())
    .on_click_event(move |_, _, cx| {
        toggle_msd_state(&entity, plugin_idx, channel_idx, action, cx);
    });
    let button = if let Some(handle) = scroll_handle {
        button.scroll_anchor(ScrollAnchor::for_handle(handle))
    } else {
        button
    };
    #[cfg(feature = "dev-api")]
    let button = {
        use crate::app::dev_api::DevTrackExt;
        button.dev_track(format!("{id}-channel-{plugin_idx}-{channel_idx}"))
    };
    button
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
        let Some(plugin) = state.app.plugin_state.editor_plugin_mut(plugin_idx) else {
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
        state
            .app
            .plugin_state
            .record_editor_effect(sotf_audio_player::PluginUpdateEffect::Structural);
        cx.notify();
    });
}
