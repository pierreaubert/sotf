//! Compact EQ layouts for small windows.
//!
//! - `render_eq_bottom_strip`: graph on top, horizontal band strip + inline editor below.
//! - `render_eq_inspector`: scrollable band list; graph optional.

use crate::app::AppState;
use crate::components::PluginEditingManager;
use crate::components::design::Ds;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::EQFilter;
use sotf_plugins::param_specs::{eq::BAND_TEMPLATE as EQ, find_by_key as pk};

use super::render::{
    EqBandIndexing, EqGlobalControl, render_eq_active_toggle, render_eq_global_stepper,
    render_eq_global_toggle, render_eq_knob_with_midi, render_eq_visualization,
    render_filter_type_selector,
};
use super::types::{EqRenderState, EqViewMode};

const COMPACT_GRAPH_HEIGHT: f32 = 200.0;

/// Bottom-strip layout: graph on top, band cards below, selected band expands inline.
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

    root = root.child(
        div()
            .h(px(COMPACT_GRAPH_HEIGHT))
            .child(render_eq_visualization(
                entity.clone(),
                plugin_idx,
                display_filters,
                Some(selected_band_idx),
                indexing,
                theme,
                state.available_width,
            )),
    );

    let mut strip = div()
        .id("eq-bottom-strip")
        .flex()
        .gap(d.gap)
        .overflow_x_scroll()
        .px(d.pad_x);

    for (i, filter) in display_filters.iter().enumerate() {
        strip = strip.child(render_compact_band_card(
            &d,
            entity.clone(),
            plugin_idx,
            i,
            filter,
            i == selected_band_idx,
            theme,
        ));
    }
    strip = strip.child(render_add_band_button(
        &d,
        entity.clone(),
        plugin_idx,
        theme,
    ));
    root = root.child(strip);

    if let Some(filter) = display_filters.get(selected_band_idx) {
        root = root.child(render_compact_band_editor(
            &d,
            entity.clone(),
            plugin_idx,
            selected_band_idx,
            filter,
            indexing,
            state,
            theme,
        ));
    }

    root
}

/// Inspector layout: vertical band list; graph toggled on/off.
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
    let graph_visible = entity
        .read(cx)
        .app
        .plugin_state
        .plugin_ui_state
        .eq_compact_graph_visible;
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
        true,
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

    if graph_visible {
        root = root.child(
            div()
                .h(px(COMPACT_GRAPH_HEIGHT))
                .child(render_eq_visualization(
                    entity.clone(),
                    plugin_idx,
                    display_filters,
                    Some(selected_band_idx),
                    indexing,
                    theme,
                    state.available_width,
                )),
        );
        if let Some(filter) = display_filters.get(selected_band_idx) {
            root = root.child(render_compact_band_editor(
                &d,
                entity.clone(),
                plugin_idx,
                selected_band_idx,
                filter,
                indexing,
                state,
                theme,
            ));
        }
    } else {
        let mut list = div()
            .id("eq-inspector-list")
            .flex()
            .flex_col()
            .gap(d.gap)
            .overflow_y_scroll()
            .px(d.pad_x);
        for (i, filter) in display_filters.iter().enumerate() {
            list = list.child(render_compact_inspector_row(
                &d,
                entity.clone(),
                plugin_idx,
                i,
                filter,
                indexing,
                state,
                theme,
            ));
        }
        list = list.child(render_add_band_button(
            &d,
            entity.clone(),
            plugin_idx,
            theme,
        ));
        root = root.child(list);
    }

    root
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

/// Compact clickable card for one band in the bottom strip.
fn render_compact_band_card(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    band_idx: usize,
    filter: &EQFilter,
    selected: bool,
    theme: &Theme,
) -> impl IntoElement {
    let entity_clone = entity.clone();
    let is_muted = filter.muted;
    div()
        .id(("eq-band-card", band_idx))
        .flex()
        .flex_col()
        .items_center()
        .gap(d.grid)
        .px(d.pad_x)
        .py(d.pad_y)
        .min_w(px(80.0))
        .rounded(d.r_md)
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
        .when(is_muted, |div| div.opacity(0.5))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            entity_clone.update(cx, |state, cx| {
                state.app.plugin_state.selected_eq_band = band_idx;
                state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                cx.notify();
            });
        })
        .child(div().child(format!(
            "#{} {}",
            band_idx + 1,
            filter.filter_type.short_name()
        )))
        .child(
            div()
                .text_size(d.text_xs)
                .child(format!("{:.0}Hz", filter.frequency)),
        )
        .child(
            div()
                .text_size(d.text_xs)
                .child(format!("{:+.1}dB", filter.gain_db)),
        )
}

/// Full inline editor for a single band (used by bottom strip and graph overlay).
#[allow(clippy::too_many_arguments)]
fn render_compact_band_editor(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    band_idx: usize,
    filter: &EQFilter,
    indexing: EqBandIndexing,
    state: &EqRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let base_param_idx = band_idx * indexing.stride;
    let midi_overlay = state.midi_overlay;

    let mut editor = div()
        .flex()
        .flex_col()
        .gap(d.gap)
        .p(d.pad_x)
        .bg(theme.background_secondary)
        .rounded(d.r_md)
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(d.text_xs)
                        .text_color(theme.text_muted)
                        .child(format!("Band {}", band_idx + 1)),
                )
                .child(render_filter_type_selector(
                    d,
                    entity.clone(),
                    plugin_idx,
                    &filter.filter_type,
                    band_idx,
                    base_param_idx + indexing.filter_type,
                    None,
                    theme,
                )),
        )
        .child(
            div()
                .flex()
                .gap(d.gap)
                .justify_center()
                .child(render_eq_knob_with_midi(
                    d,
                    entity.clone(),
                    plugin_idx,
                    "Freq",
                    filter.frequency,
                    pk(EQ, "freq").min_f64(),
                    pk(EQ, "freq").max_f64(),
                    "Hz",
                    base_param_idx + indexing.frequency,
                    state.selected_param,
                    state.is_editing,
                    midi_overlay,
                    theme,
                ))
                .child(render_eq_knob_with_midi(
                    d,
                    entity.clone(),
                    plugin_idx,
                    "Q",
                    filter.q,
                    pk(EQ, "q").min_f64(),
                    pk(EQ, "q").max_f64(),
                    "",
                    base_param_idx + indexing.q,
                    state.selected_param,
                    state.is_editing,
                    midi_overlay,
                    theme,
                ))
                .child(render_eq_knob_with_midi(
                    d,
                    entity.clone(),
                    plugin_idx,
                    "Gain",
                    filter.gain_db,
                    pk(EQ, "gain").min_f64(),
                    pk(EQ, "gain").max_f64(),
                    "dB",
                    base_param_idx + indexing.gain,
                    state.selected_param,
                    state.is_editing,
                    midi_overlay,
                    theme,
                )),
        );

    // Mute / Solo buttons
    let mute_entity = entity.clone();
    let solo_entity = entity.clone();
    editor = editor.child(
        div()
            .flex()
            .gap(d.gap)
            .justify_center()
            .child(small_action_button(
                d,
                "M",
                filter.muted,
                theme.error,
                theme,
                move |_, _, cx| {
                    mute_entity.update(cx, |state, cx| {
                        state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                        if let Err(e) = state.app.toggle_eq_band_mute(band_idx) {
                            log::warn!("Failed to toggle EQ band mute: {}", e);
                        }
                        cx.notify();
                    });
                },
            ))
            .child(small_action_button(
                d,
                "S",
                filter.solo,
                theme.success,
                theme,
                move |_, _, cx| {
                    solo_entity.update(cx, |state, cx| {
                        state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                        if let Err(e) = state.app.toggle_eq_band_solo(band_idx) {
                            log::warn!("Failed to toggle EQ band solo: {}", e);
                        }
                        cx.notify();
                    });
                },
            )),
    );

    if let Some(active_local_idx) = indexing.active {
        editor = editor.child(render_eq_active_toggle(
            d,
            entity,
            plugin_idx,
            filter,
            base_param_idx + active_local_idx,
            state.selected_param,
            state.is_editing,
            theme,
        ));
    }

    editor
}

/// One self-contained row in the inspector list (card + inline editor).
#[allow(clippy::too_many_arguments)]
fn render_compact_inspector_row(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    band_idx: usize,
    filter: &EQFilter,
    indexing: EqBandIndexing,
    state: &EqRenderState,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(d.gap)
        .p(d.pad_x)
        .bg(theme.background_secondary)
        .rounded(d.r_md)
        .child(render_compact_band_card(
            d,
            entity.clone(),
            plugin_idx,
            band_idx,
            filter,
            false,
            theme,
        ))
        .child(render_compact_band_editor(
            d, entity, plugin_idx, band_idx, filter, indexing, state, theme,
        ))
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
        .child("Config ⚙")
}

/// Small circular M/S action button.
fn small_action_button<F>(
    d: &Ds,
    label: &'static str,
    active: bool,
    active_color: Rgba,
    theme: &Theme,
    on_click: F,
) -> impl IntoElement
where
    F: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
{
    div()
        .w(px(28.0))
        .h(px(24.0))
        .rounded(d.r_sm)
        .flex()
        .items_center()
        .justify_center()
        .bg(if active {
            active_color
        } else {
            theme.background_secondary
        })
        .border(px(1.0))
        .border_color(if active { active_color } else { theme.border })
        .text_size(d.text_xs)
        .font_weight(FontWeight::BOLD)
        .cursor_pointer()
        .text_color(if active {
            theme.text_on_accent
        } else {
            theme.text_muted
        })
        .hover(|s| {
            s.bg(if active {
                active_color
            } else {
                theme.surface_hover
            })
        })
        .on_mouse_down(MouseButton::Left, on_click)
        .child(label)
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
