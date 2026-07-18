// intentional-file: fixed pixel values here are graph and plugin control geometry.
//! Compact EQ layouts for small windows.
//!
//! - `render_eq_bottom_strip`: medium workbench with a vertical band rail, graph, and property strip.
//! - `render_eq_inspector`: narrow drawer with graph, selected-band editor, and bottom band chips.

use crate::app::AppState;
use crate::app::i18n::EqViewTranslations;
use crate::components::PluginEditingManager;
use crate::components::design::Ds;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::EQFilter;

use super::render::{
    EqBandIndexing, EqGlobalControl, render_eq_global_stepper, render_eq_global_toggle,
    render_eq_property_strip, render_eq_visualization_sized,
};
use super::types::{EqRenderState, EqViewMode};

const COMPACT_GRAPH_HEIGHT: f32 = 200.0;

/// Medium layout: graph workbench with a vertical band rail and one property strip.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_eq_bottom_strip(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &EqRenderState,
    display_filters: &[EQFilter],
    selected_band_idx: usize,
    indexing: EqBandIndexing,
    theme: &Theme,
    cx: &mut Context<PlayerView>,
) -> impl IntoElement {
    let d = Ds::from_cx(cx);
    let text = EqViewTranslations::for_language(entity.read(cx).app.ui_state.language);
    let config_open = entity
        .read(cx)
        .app
        .plugin_state
        .plugin_ui_state
        .eq_compact_config_open;

    let mut root = div().flex().flex_col().gap(d.section).size_full();

    root = root.child(render_compact_global_bar(
        &d,
        entity.clone(),
        plugin_idx,
        state,
        false,
        theme,
        cx,
    ));

    if config_open {
        root = root.child(render_compact_config_panel(
            &d,
            entity.clone(),
            plugin_idx,
            state,
            theme,
            cx,
        ));
    }

    let selected_channel = entity.read(cx).app.plugin_state.selected_eq_channel;
    let is_lp_mode = matches!(
        state.mode,
        EqViewMode::LinearPhase { .. } | EqViewMode::FirDesigner { .. }
    );

    if !is_lp_mode {
        root = root.child(super::render::render_eq_channel_toolbar(
            &d,
            entity.clone(),
            plugin_idx,
            state.channels,
            selected_channel,
            state.per_channel_mode,
            theme,
        ));
    }

    let graph_width = (state.available_width - 104.0).max(360.0);
    root = root.child(
        div()
            .id("eq-medium-workbench")
            .flex()
            .items_stretch()
            .gap(d.gap)
            .min_h(px(COMPACT_GRAPH_HEIGHT + 24.0))
            .child(render_medium_band_rail(
                &d,
                entity.clone(),
                plugin_idx,
                display_filters,
                selected_band_idx,
                theme,
            ))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h(px(COMPACT_GRAPH_HEIGHT + 24.0))
                    .child(render_eq_visualization_sized(
                        entity.clone(),
                        plugin_idx,
                        display_filters,
                        Some(selected_band_idx),
                        indexing,
                        theme,
                        graph_width,
                        COMPACT_GRAPH_HEIGHT + 24.0,
                    )),
            ),
    );

    root = root.child(render_eq_property_strip(
        &d,
        entity,
        plugin_idx,
        display_filters.get(selected_band_idx),
        selected_band_idx,
        indexing,
        state,
        is_lp_mode,
        text,
        theme,
    ));

    root
}

/// Narrow layout: selected-band drawer plus a compact chip carousel.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_eq_inspector(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &EqRenderState,
    display_filters: &[EQFilter],
    selected_band_idx: usize,
    indexing: EqBandIndexing,
    theme: &Theme,
    cx: &mut Context<PlayerView>,
) -> impl IntoElement {
    let d = Ds::from_cx(cx);
    let text = EqViewTranslations::for_language(entity.read(cx).app.ui_state.language);
    let config_open = entity
        .read(cx)
        .app
        .plugin_state
        .plugin_ui_state
        .eq_compact_config_open;
    let selected_channel = entity.read(cx).app.plugin_state.selected_eq_channel;
    let is_lp_mode = matches!(
        state.mode,
        EqViewMode::LinearPhase { .. } | EqViewMode::FirDesigner { .. }
    );

    let mut root = div().flex().flex_col().gap(d.section).size_full();

    root = root.child(render_compact_global_bar(
        &d,
        entity.clone(),
        plugin_idx,
        state,
        false,
        theme,
        cx,
    ));

    if config_open {
        root = root.child(render_compact_config_panel(
            &d,
            entity.clone(),
            plugin_idx,
            state,
            theme,
            cx,
        ));
    }

    if !is_lp_mode {
        root = root.child(super::render::render_eq_channel_toolbar(
            &d,
            entity.clone(),
            plugin_idx,
            state.channels,
            selected_channel,
            state.per_channel_mode,
            theme,
        ));
    }

    root = root
        .child(
            div()
                .id("eq-narrow-graph")
                .h(px(COMPACT_GRAPH_HEIGHT))
                .child(render_eq_visualization_sized(
                    entity.clone(),
                    plugin_idx,
                    display_filters,
                    Some(selected_band_idx),
                    indexing,
                    theme,
                    state.available_width.max(320.0),
                    COMPACT_GRAPH_HEIGHT,
                )),
        )
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
        .child(render_narrow_band_strip(
            &d,
            entity,
            plugin_idx,
            display_filters,
            selected_band_idx,
            theme,
        ));

    root
}

fn render_medium_band_rail(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    display_filters: &[EQFilter],
    selected_band_idx: usize,
    theme: &Theme,
) -> impl IntoElement {
    let mut rail = div()
        .id("eq-band-rail")
        .flex()
        .flex_col()
        .gap(d.grid)
        .min_w(rems(5.25))
        .max_w(rems(5.25))
        .p(d.grid)
        .overflow_y_scroll()
        .bg(theme.surface)
        .rounded(d.r_md);

    for (i, filter) in display_filters.iter().enumerate() {
        rail = rail.child(render_rail_band_button(
            d,
            entity.clone(),
            plugin_idx,
            i,
            filter,
            i == selected_band_idx,
            theme,
        ));
    }

    rail.child(render_add_band_button(d, entity, plugin_idx, theme))
}

fn render_narrow_band_strip(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    display_filters: &[EQFilter],
    selected_band_idx: usize,
    theme: &Theme,
) -> impl IntoElement {
    let mut strip = div()
        .id("eq-bottom-strip")
        .flex()
        .items_center()
        .gap(d.grid)
        .overflow_x_scroll()
        .px(d.pad_x)
        .py(d.pad_y_half)
        .bg(theme.surface)
        .rounded(d.r_md);

    for (i, filter) in display_filters.iter().enumerate() {
        strip = strip.child(render_narrow_band_chip(
            d,
            entity.clone(),
            plugin_idx,
            i,
            filter,
            i == selected_band_idx,
            theme,
        ));
    }

    strip.child(render_add_band_button(d, entity, plugin_idx, theme))
}

fn render_rail_band_button(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    band_idx: usize,
    filter: &EQFilter,
    selected: bool,
    theme: &Theme,
) -> impl IntoElement {
    let entity_clone = entity.clone();
    div()
        .id(("eq-band-rail-button", band_idx))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(2.0))
        .px(d.grid)
        .py(d.pad_y_half)
        .rounded(d.r_sm)
        .cursor_pointer()
        .when(selected, |div| {
            div.bg(theme.accent)
                .text_color(theme.text_on_accent)
                .font_weight(FontWeight::SEMIBOLD)
        })
        .when(!selected, |div| {
            div.bg(theme.background_secondary)
                .text_color(theme.text_secondary)
                .hover(|s| s.bg(theme.surface_hover))
        })
        .when(filter.muted, |div| div.opacity(0.5))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            entity_clone.update(cx, |state, cx| {
                state.app.plugin_state.selected_eq_band = band_idx;
                state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                cx.notify();
            });
        })
        .child(
            div()
                .text_size(d.text_xs)
                .child(format!("#{}", band_idx + 1)),
        )
        .child(
            div()
                .text_size(d.text_xs)
                .child(filter.filter_type.short_name()),
        )
        .child(
            div()
                .text_size(d.text_xs)
                .text_color(if selected {
                    theme.text_on_accent
                } else {
                    theme.text_muted
                })
                .child(compact_freq(filter.frequency)),
        )
}

fn render_narrow_band_chip(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    band_idx: usize,
    filter: &EQFilter,
    selected: bool,
    theme: &Theme,
) -> impl IntoElement {
    let entity_clone = entity.clone();
    div()
        .id(("eq-band-chip", band_idx))
        .flex()
        .items_center()
        .gap(d.grid)
        .px(d.pad_y)
        .py(d.pad_y_half)
        .min_w(rems(5.5))
        .rounded(d.r_sm)
        .cursor_pointer()
        .when(selected, |div| {
            div.bg(theme.accent)
                .text_color(theme.text_on_accent)
                .font_weight(FontWeight::SEMIBOLD)
        })
        .when(!selected, |div| {
            div.bg(theme.background_secondary)
                .text_color(theme.text_secondary)
                .hover(|s| s.bg(theme.surface_hover))
        })
        .when(filter.muted, |div| div.opacity(0.5))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            entity_clone.update(cx, |state, cx| {
                state.app.plugin_state.selected_eq_band = band_idx;
                state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                cx.notify();
            });
        })
        .child(format!(
            "#{} {} {}",
            band_idx + 1,
            filter.filter_type.short_name(),
            compact_freq(filter.frequency)
        ))
}

fn compact_freq(freq: f64) -> String {
    if freq >= 1000.0 {
        format!("{:.1}k", freq / 1000.0)
    } else {
        format!("{freq:.0}")
    }
}

/// Slim top bar shared by both compact layouts.
fn render_compact_global_bar(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &EqRenderState,
    show_graph_toggle: bool,
    theme: &Theme,
    cx: &mut Context<PlayerView>,
) -> impl IntoElement {
    let config_open = entity
        .read(cx)
        .app
        .plugin_state
        .plugin_ui_state
        .eq_compact_config_open;
    let graph_visible = entity
        .read(cx)
        .app
        .plugin_state
        .plugin_ui_state
        .eq_compact_graph_visible;

    let mode_label = match state.mode {
        EqViewMode::Standard => "EQ",
        EqViewMode::LinearPhase { .. } => "Linear-Phase EQ",
        EqViewMode::FirDesigner { .. } => "FIR Designer",
    };

    let mut bar = div()
        .flex()
        .items_center()
        .justify_between()
        .gap(d.gap)
        .px(d.pad_x)
        .py(d.pad_y_half)
        .bg(theme.surface)
        .rounded(d.r_md)
        .child(
            div()
                .flex()
                .items_center()
                .gap(d.gap)
                .child(
                    div()
                        .text_size(d.text_sm)
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.text_primary)
                        .child(mode_label),
                )
                .child(config_toggle_button(
                    d,
                    entity.clone(),
                    plugin_idx,
                    config_open,
                    theme,
                )),
        );

    if show_graph_toggle {
        let graph_entity = entity.clone();
        let label = if graph_visible {
            "Graph ■"
        } else {
            "Graph □"
        };
        bar = bar.child(
            div()
                .px(d.pad_y)
                .py(d.pad_y_half)
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .rounded(d.r_sm)
                .cursor_pointer()
                .when(graph_visible, |d| {
                    d.bg(theme.accent).text_color(theme.text_on_accent)
                })
                .when(!graph_visible, |d| {
                    d.bg(theme.background_secondary)
                        .text_color(theme.text_secondary)
                        .hover(|s| s.bg(theme.surface_hover))
                })
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    graph_entity.update(cx, |state, cx| {
                        let visible = &mut state
                            .app
                            .plugin_state
                            .plugin_ui_state
                            .eq_compact_graph_visible;
                        *visible = !*visible;
                        cx.notify();
                    });
                })
                .child(label),
        );
    }

    bar
}

/// Expandable panel containing global controls and channel mode.
fn render_compact_config_panel(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: &EqRenderState,
    theme: &Theme,
    cx: &mut Context<PlayerView>,
) -> impl IntoElement {
    let is_lp_mode = matches!(
        state.mode,
        EqViewMode::LinearPhase { .. } | EqViewMode::FirDesigner { .. }
    );

    let mut col = div()
        .flex()
        .flex_col()
        .gap(d.gap)
        .p(d.pad_x)
        .bg(theme.background_secondary)
        .rounded(d.r_md);

    // Channel mode (standard EQ only)
    if !is_lp_mode {
        let all_entity = entity.clone();
        let per_entity = entity.clone();
        let per_channel = state.per_channel_mode;
        col = col.child(
            div()
                .flex()
                .items_center()
                .gap(d.grid)
                .child(mode_pill(
                    d,
                    "All Channels",
                    !per_channel,
                    theme,
                    move |_, _, cx| {
                        all_entity.update(cx, |state, cx| {
                            state.app.set_eq_per_channel_mode(plugin_idx, false);
                            cx.notify();
                        });
                    },
                ))
                .child(mode_pill(
                    d,
                    "Per Channel",
                    per_channel,
                    theme,
                    move |_, _, cx| {
                        per_entity.update(cx, |state, cx| {
                            state.app.set_eq_per_channel_mode(plugin_idx, true);
                            cx.notify();
                        });
                    },
                )),
        );

        if state.per_channel_mode {
            let selected_channel = entity.read(cx).app.plugin_state.selected_eq_channel;
            let channel_entity = entity.clone();
            col = col.child(div().flex().items_center().gap(d.grid).children(
                (0..state.channels).map(|ch| {
                    let entity = channel_entity.clone();
                    let is_selected = ch == selected_channel;
                    mode_pill(
                        d,
                        channel_label(ch, state.channels),
                        is_selected,
                        theme,
                        move |_, _, cx| {
                            entity.update(cx, |state, cx| {
                                state.app.plugin_state.selected_eq_channel = ch;
                                cx.notify();
                            });
                        },
                    )
                }),
            ));
        }
    }

    // Global controls based on EQ variant
    match &state.mode {
        EqViewMode::Standard => {
            col = col.child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(d.gap)
                    .child(render_eq_global_stepper(
                        d,
                        entity.clone(),
                        plugin_idx,
                        EqGlobalControl::StandardMaxFilters,
                        "Filters",
                        state.num_filters.to_string(),
                        theme,
                    ))
                    .child(render_eq_global_toggle(
                        d,
                        entity.clone(),
                        plugin_idx,
                        EqGlobalControl::StandardTopology,
                        "Topology",
                        state.topology > 0.5,
                        "SVF",
                        "Biquad",
                        theme,
                    ))
                    .child(render_eq_global_toggle(
                        d,
                        entity.clone(),
                        plugin_idx,
                        EqGlobalControl::StandardTdf2,
                        "TDF-II",
                        state.tdf2,
                        "On",
                        "Off",
                        theme,
                    )),
            );
        }
        EqViewMode::LinearPhase {
            latency_samples,
            latency_ms,
            fir_length,
            auto_gain,
            mix,
            ..
        } => {
            col = col
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(d.gap)
                        .child(render_eq_global_stepper(
                            d,
                            entity.clone(),
                            plugin_idx,
                            EqGlobalControl::LpNumFilters,
                            "Filters",
                            state.num_filters.to_string(),
                            theme,
                        ))
                        .child(render_eq_global_stepper(
                            d,
                            entity.clone(),
                            plugin_idx,
                            EqGlobalControl::LpFirLength,
                            "FIR length",
                            fir_length.to_string(),
                            theme,
                        ))
                        .child(render_eq_global_toggle(
                            d,
                            entity.clone(),
                            plugin_idx,
                            EqGlobalControl::LpAutoGain,
                            "Auto-gain",
                            *auto_gain,
                            "On",
                            "Off",
                            theme,
                        ))
                        .child(render_eq_global_stepper(
                            d,
                            entity.clone(),
                            plugin_idx,
                            EqGlobalControl::LpMix,
                            "Mix",
                            format!("{:.0}%", mix * 100.0),
                            theme,
                        )),
                )
                .child(
                    div()
                        .text_size(d.text_xs)
                        .text_color(theme.text_muted)
                        .child(format!(
                            "Latency: {latency_samples} samples ({latency_ms:.2} ms)"
                        )),
                );
        }
        EqViewMode::FirDesigner {
            latency_samples,
            latency_ms,
            fir_length,
            phase_mode,
            auto_gain,
            mix,
            ..
        } => {
            col = col
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(d.gap)
                        .child(render_eq_global_stepper(
                            d,
                            entity.clone(),
                            plugin_idx,
                            EqGlobalControl::FirNumFilters,
                            "Filters",
                            state.num_filters.to_string(),
                            theme,
                        ))
                        .child(render_eq_global_stepper(
                            d,
                            entity.clone(),
                            plugin_idx,
                            EqGlobalControl::FirLength,
                            "FIR length",
                            fir_length.to_string(),
                            theme,
                        ))
                        .child(render_eq_global_toggle(
                            d,
                            entity.clone(),
                            plugin_idx,
                            EqGlobalControl::FirPhaseMode,
                            "Phase",
                            *phase_mode == "Minimum",
                            "Minimum",
                            "Linear",
                            theme,
                        ))
                        .child(render_eq_global_toggle(
                            d,
                            entity.clone(),
                            plugin_idx,
                            EqGlobalControl::FirAutoGain,
                            "Auto-gain",
                            *auto_gain,
                            "On",
                            "Off",
                            theme,
                        ))
                        .child(render_eq_global_stepper(
                            d,
                            entity.clone(),
                            plugin_idx,
                            EqGlobalControl::FirMix,
                            "Mix",
                            format!("{:.0}%", mix * 100.0),
                            theme,
                        )),
                )
                .child(
                    div()
                        .text_size(d.text_xs)
                        .text_color(theme.text_muted)
                        .child(format!(
                            "Latency: {latency_samples} samples ({latency_ms:.2} ms)"
                        )),
                );
        }
    }

    col
}

/// "+" button to add a band.
fn render_add_band_button(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .id("eq-add-band")
        .px(d.pad_x)
        .py_1p5()
        .text_size(d.text_sm)
        .font_weight(FontWeight::BOLD)
        .rounded(d.r_sm)
        .cursor_pointer()
        .bg(theme.success)
        .text_color(theme.text_on_accent)
        .hover(|s| s.opacity(0.8))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            entity.update(cx, |state, cx| {
                state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                if let Err(e) = state.app.add_eq_band() {
                    log::warn!("Failed to add EQ band: {}", e);
                }
                cx.notify();
            });
        })
        .child("+")
}

/// Small toggle pill used for channel mode and config toggles.
fn mode_pill<F>(
    d: &Ds,
    label: impl Into<SharedString>,
    selected: bool,
    theme: &Theme,
    on_click: F,
) -> impl IntoElement
where
    F: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
{
    let label = label.into();
    div()
        .px(d.pad_y)
        .py(d.pad_y_half)
        .text_size(d.text_xs)
        .font_weight(FontWeight::SEMIBOLD)
        .rounded(d.r_sm)
        .cursor_pointer()
        .when(selected, |div| {
            div.bg(theme.accent).text_color(theme.text_on_accent)
        })
        .when(!selected, |div| {
            div.bg(theme.background_secondary)
                .text_color(theme.text_secondary)
                .hover(|s| s.bg(theme.surface_hover))
        })
        .on_mouse_down(MouseButton::Left, on_click)
        .child(label)
}

/// Config toggle button in the global bar.
fn config_toggle_button(
    d: &Ds,
    entity: Entity<AppState>,
    _plugin_idx: usize,
    open: bool,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .px(d.pad_y)
        .py(d.pad_y_half)
        .text_size(d.text_xs)
        .font_weight(FontWeight::SEMIBOLD)
        .rounded(d.r_sm)
        .cursor_pointer()
        .when(open, |div| {
            div.bg(theme.accent).text_color(theme.text_on_accent)
        })
        .when(!open, |div| {
            div.bg(theme.background_secondary)
                .text_color(theme.text_secondary)
                .hover(|s| s.bg(theme.surface_hover))
        })
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            entity.update(cx, |state, cx| {
                let open = &mut state
                    .app
                    .plugin_state
                    .plugin_ui_state
                    .eq_compact_config_open;
                *open = !*open;
                cx.notify();
            });
        })
        .child("⚙")
}

/// Helper: readable label for a channel index.
fn channel_label(ch: usize, channels: usize) -> String {
    match channels {
        1 => "Mono".to_string(),
        2 => match ch {
            0 => "L".to_string(),
            1 => "R".to_string(),
            _ => format!("Ch{}", ch + 1),
        },
        _ => format!("Ch{}", ch + 1),
    }
}
