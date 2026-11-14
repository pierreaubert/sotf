// intentional-file: fixed pixel values here are graph and plugin control geometry.
//! Compact EQ layouts for small windows.
//!
//! - `render_eq_bottom_strip`: medium workbench with a horizontal band strip, graph, and property strip.
//! - `render_eq_inspector`: narrow drawer with graph, selected-band editor, and bottom band chips.

use crate::app::AppState;
#[cfg(feature = "dev-api")]
use crate::app::dev_api::DevTrackExt;
use crate::app::i18n::EqViewTranslations;
use crate::components::PluginEditingManager;
use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName};
use crate::eq_layout::narrow_graph_width;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::{AnyElement, Entity, FocusHandle, FontWeight, MouseButton, div, px, rems};
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant};
use sotf_audio_player::EQFilter;

use super::render::{EqBandIndexing, render_eq_property_strip, render_eq_visualization_sized};
use super::types::{EqRenderState, EqViewMode};

const COMPACT_GRAPH_HEIGHT: f32 = 200.0;
const MEDIUM_GRAPH_HEIGHT: f32 = 160.0;

/// Medium layout: full-width graph with a horizontal band strip and property strip.
#[allow(
    clippy::too_many_arguments,
    reason = "GPUI rendering and callback interfaces accept the complete UI context"
)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
pub(crate) fn render_eq_bottom_strip(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &EqRenderState,
    display_filters: &[EQFilter],
    selected_band_idx: usize,
    indexing: EqBandIndexing,
    theme: &Theme,
    chart_focus_handle: FocusHandle,
    cx: &mut Context<PlayerView>,
) -> impl IntoElement {
    let d = Ds::from_cx(cx);
    let text = EqViewTranslations::for_language(entity.read(cx).app.ui_state.language);
    // Keep intrinsic height so the rack-owned parameter scroller can expose
    // controls below the graph instead of laying them out after a 100%-high
    // child surface.
    let mut root = div().flex().flex_col().gap(d.section).w_full();

    let selected_channel = entity.read(cx).app.plugin_state.selected_eq_channel;
    let is_lp_mode = matches!(state.mode, EqViewMode::LinearPhase { .. });

    if !is_lp_mode && state.per_channel_mode {
        root = root.child(super::render::render_eq_channel_segment(
            &d,
            entity.clone(),
            plugin_idx,
            state.channels,
            selected_channel,
            state.per_channel_mode,
            text,
            theme,
        ));
    }

    // Keep selected-band controls ahead of the graph in medium layouts. The
    // rack detail viewport is height-constrained; putting exact entry and band
    // actions after the chart made them unreachable behind the transport bar.
    root = root.child(render_eq_property_strip(
        &d,
        entity.clone(),
        plugin_idx,
        display_filters.get(selected_band_idx),
        selected_band_idx,
        indexing,
        state,
        is_lp_mode,
        text,
        theme,
    ));

    root = root.child(render_narrow_band_strip(
        &d,
        entity.clone(),
        plugin_idx,
        display_filters,
        selected_band_idx,
        !is_lp_mode || display_filters.len() < 10,
        None,
        None,
        theme,
    ));
    let graph_height = MEDIUM_GRAPH_HEIGHT * state.layout_scale;
    root = root.child(
        div()
            .id("eq-medium-workbench")
            .w_full()
            .min_w_0()
            .h(px(graph_height))
            .child(render_eq_visualization_sized(
                entity.clone(),
                plugin_idx,
                display_filters,
                Some(selected_band_idx),
                indexing,
                theme,
                narrow_graph_width(state.available_width),
                graph_height,
                state.layout_scale,
                state.sample_rate,
                chart_focus_handle,
            )),
    );

    root
}

/// Narrow layout: selected-band drawer plus a compact chip carousel.
#[allow(
    clippy::too_many_arguments,
    reason = "GPUI rendering and callback interfaces accept the complete UI context"
)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
pub(crate) fn render_eq_inspector(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &EqRenderState,
    display_filters: &[EQFilter],
    selected_band_idx: usize,
    indexing: EqBandIndexing,
    theme: &Theme,
    chart_focus_handle: FocusHandle,
    cx: &mut Context<PlayerView>,
) -> impl IntoElement {
    let d = Ds::from_cx(cx);
    let text = EqViewTranslations::for_language(entity.read(cx).app.ui_state.language);
    let selected_channel = entity.read(cx).app.plugin_state.selected_eq_channel;
    let is_lp_mode = matches!(state.mode, EqViewMode::LinearPhase { .. });

    let mut root = div().flex().flex_col().gap(d.grid).w_full();

    if !is_lp_mode && state.per_channel_mode {
        root = root.child(super::render::render_eq_channel_segment(
            &d,
            entity.clone(),
            plugin_idx,
            state.channels,
            selected_channel,
            state.per_channel_mode,
            text,
            theme,
        ));
    }

    let graph_height = COMPACT_GRAPH_HEIGHT * state.layout_scale;
    root = root
        .child(render_narrow_band_strip(
            &d,
            entity.clone(),
            plugin_idx,
            display_filters,
            selected_band_idx,
            !is_lp_mode || display_filters.len() < 10,
            None,
            None,
            theme,
        ))
        .child(render_eq_property_strip(
            &d,
            entity.clone(),
            plugin_idx,
            display_filters.get(selected_band_idx),
            selected_band_idx,
            indexing,
            state,
            is_lp_mode,
            text,
            theme,
        ))
        .child(
            div()
                .id("eq-narrow-graph")
                .w_full()
                .min_w_0()
                .h(px(graph_height))
                .child(render_eq_visualization_sized(
                    entity.clone(),
                    plugin_idx,
                    display_filters,
                    Some(selected_band_idx),
                    indexing,
                    theme,
                    narrow_graph_width(state.available_width),
                    graph_height,
                    state.layout_scale,
                    state.sample_rate,
                    chart_focus_handle,
                )),
        );

    root
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
pub(crate) fn render_narrow_band_strip(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    display_filters: &[EQFilter],
    selected_band_idx: usize,
    can_add: bool,
    leading: Option<AnyElement>,
    popup_focus: Option<FocusHandle>,
    theme: &Theme,
) -> impl IntoElement {
    let mut strip = div()
        .id("eq-bottom-strip")
        .flex()
        .items_center()
        .gap(d.grid)
        .overflow_x_scroll()
        // The strip handles wheel scrolling before this bubble listener.
        // Keep its wheel event from scrolling the surrounding rack as well.
        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
        .px(d.pad_x)
        .py(d.pad_y_half)
        .bg(theme.surface)
        .rounded(d.r_md);

    if let Some(leading) = leading {
        strip = strip.child(leading).child(
            div()
                .w(px(1.0))
                .h(rems(1.5))
                .flex_shrink_0()
                .bg(theme.border),
        );
    }

    for (i, filter) in display_filters.iter().enumerate() {
        strip = strip.child(render_narrow_band_chip(
            d,
            entity.clone(),
            plugin_idx,
            i,
            filter,
            i == selected_band_idx,
            popup_focus.clone(),
            theme,
        ));
    }

    strip.child(render_add_band_button(
        d, entity, plugin_idx, can_add, theme,
    ))
}

fn render_narrow_band_chip(
    _d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    band_idx: usize,
    filter: &EQFilter,
    selected: bool,
    _popup_focus: Option<FocusHandle>,
    theme: &Theme,
) -> impl IntoElement {
    let button = Button::new(
        ("eq-band-chip", band_idx),
        format!(
            "#{} {} {}",
            band_idx + 1,
            filter.filter_type.short_name(),
            compact_freq(filter.frequency)
        ),
    )
    .size(ButtonSize::Sm)
    .variant(if selected {
        ButtonVariant::Primary
    } else {
        ButtonVariant::Ghost
    })
    .selected(selected)
    .theme(theme.to_button_theme())
    .aria_label(format!("Edit EQ band {}", band_idx + 1))
    .on_click(move |_, cx| {
        entity.update(cx, |state, cx| {
            state.app.plugin_state.selected_eq_band = band_idx;
            state.app.plugin_state.plugin_ui_state.eq_band_popup_open = true;
            state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
            cx.notify();
        });
    });
    #[cfg(feature = "dev-api")]
    let button = button.dev_track(format!("eq.band.{band_idx}.select"));
    div().key_context("PluginChoice").child(button)
}

fn compact_freq(freq: f64) -> String {
    if freq >= 1000.0 {
        format!("{:.1}k", freq / 1000.0)
    } else {
        format!("{freq:.0}")
    }
}

fn render_add_band_button(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    can_add: bool,
    theme: &Theme,
) -> impl IntoElement {
    let key_entity = entity.clone();
    let focus_color = theme.border_focused;
    let button = div()
        .id("eq-add-band")
        .px(d.pad_x)
        .py_1p5()
        .text_size(d.text_sm)
        .font_weight(FontWeight::BOLD)
        .rounded(d.r_sm)
        .cursor_pointer()
        .when(!can_add, |button| button.opacity(0.5))
        .bg(theme.success)
        .text_color(theme.text_on_accent)
        .hover(|s| s.opacity(0.8))
        .focusable()
        .focus_visible(move |s| s.border_1().border_color(focus_color))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            if !can_add {
                return;
            }
            entity.update(cx, |state, cx| {
                state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                if let Err(e) = state.app.add_eq_band() {
                    log::warn!("Failed to add EQ band: {e}");
                }
                cx.notify();
            });
        })
        .on_key_down(move |event, _window, cx| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                if !can_add {
                    cx.stop_propagation();
                    return;
                }
                key_entity.update(cx, |state, cx| {
                    state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                    if let Err(error) = state.app.add_eq_band() {
                        log::warn!("Failed to add EQ band: {error}");
                    }
                    cx.notify();
                });
                cx.stop_propagation();
            }
        })
        .child(
            Icon::new(IconName::Plus)
                .small()
                .color(theme.text_on_accent),
        );
    #[cfg(feature = "dev-api")]
    let button = button.dev_track("eq.band.add-rail");
    button
}
