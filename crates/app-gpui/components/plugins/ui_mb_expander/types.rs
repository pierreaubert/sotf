use super::super::common::{
    render_knob, render_section_title, render_toggle, render_transfer_curve_with_level,
    render_vertical_slider_with_ticks,
};
use super::misc::SLIDER_HEIGHT;
use super::misc::TRANSFER_CURVE_SIZE;
use crate::app::AppState;
use crate::components::design::Ds;
use crate::components::plugins::editing::PluginEditingManager;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_plugins::param_specs::{find_by_key as pk, multiband_expander::GLOBAL_PARAMS as ME};

use super::super::ui_multiband_common::{band_tab_label, render_band_count_editor};

/// State for rendering the Multiband Expander plugin
pub struct MbExpanderRenderState {
    pub num_bands: usize,
    pub crossover_preset: i32,
    pub crossover_freq_1: f64,
    pub crossover_freq_2: f64,
    pub crossover_freq_3: f64,
    pub crossover_freq_4: f64,
    pub threshold_db: f64,
    pub ratio: f64,
    pub attack_ms: f64,
    pub release_ms: f64,
    pub range_db: f64,
    pub knee_db: f64,
    pub hysteresis_db: f64,
    pub hold_ms: f64,
    pub auto_makeup: bool,
    pub active: bool,
    pub solo: bool,
    pub bypass: bool,
    pub mix: f64,
    pub link_channels: bool,
    pub detection_mode: i32,
    pub lookahead_ms: f64,
    pub is_editing: bool,
    pub selected_param: usize,
    pub selected_band_idx: usize,
}

/// Render the Multiband Expander plugin
pub fn render_mb_expander_plugin(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: MbExpanderRenderState,
    available_width: f32,
    layout_scale: f32,
    text: PluginCommonTranslations,
    theme: &Theme,
) -> impl IntoElement {
    let compact = available_width / layout_scale.max(0.01) < 720.0;
    let get_param_idx = |base_idx: usize| -> usize {
        if state.selected_band_idx > 0 {
            state.selected_band_idx * 100 + base_idx
        } else {
            base_idx
        }
    };

    // === LEFT COLUMN: Global ===
    let mut global_col = div()
        .flex()
        .flex_col()
        .flex_shrink_0()
        .gap(d.gap_md)
        .child(render_section_title(d, text.label("GLOBAL"), theme))
        .child(render_band_count_editor(
            d,
            "mb-expander-bands",
            entity.clone(),
            plugin_idx,
            state.num_bands,
            pk(ME, "num_bands").min_f64(),
            pk(ME, "num_bands").max_f64(),
            text.bands,
            theme,
        ))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            text.label("XOver 1"),
            state.crossover_freq_1,
            pk(ME, "crossover_freq_1").min_f64(),
            pk(ME, "crossover_freq_1").max_f64(),
            "Hz",
            2,
            state.selected_param,
            state.is_editing,
            Some('1'),
            theme,
        ));

    if state.num_bands > 2 {
        global_col = global_col.child(render_knob(
            entity.clone(),
            plugin_idx,
            text.label("XOver 2"),
            state.crossover_freq_2,
            pk(ME, "crossover_freq_2").min_f64(),
            pk(ME, "crossover_freq_2").max_f64(),
            "Hz",
            3,
            state.selected_param,
            state.is_editing,
            Some('2'),
            theme,
        ));
    }
    if state.num_bands >= 4 {
        global_col = global_col.child(render_knob(
            entity.clone(),
            plugin_idx,
            text.label("XOver 3"),
            state.crossover_freq_3,
            pk(ME, "crossover_freq_3").min_f64(),
            pk(ME, "crossover_freq_3").max_f64(),
            "Hz",
            4,
            state.selected_param,
            state.is_editing,
            Some('3'),
            theme,
        ));
    }
    if state.num_bands >= 5 {
        global_col = global_col.child(render_knob(
            entity.clone(),
            plugin_idx,
            text.label("XOver 4"),
            state.crossover_freq_4,
            pk(ME, "crossover_freq_4").min_f64(),
            pk(ME, "crossover_freq_4").max_f64(),
            "Hz",
            5,
            state.selected_param,
            state.is_editing,
            Some('4'),
            theme,
        ));
    }

    // Global params
    // crossover_preset is not rendered: the DSP currently stores the value
    // without applying a frequency profile, so exposing it would be misleading.
    global_col = global_col
        .child(render_detection_mode_selector(
            d,
            entity.clone(),
            plugin_idx,
            state.detection_mode,
            state.selected_param,
            state.is_editing,
            text,
            theme,
        ))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            text.label("Lookahead"),
            state.lookahead_ms,
            pk(ME, "lookahead_ms").min_f64(),
            pk(ME, "lookahead_ms").max_f64(),
            "ms",
            17,
            state.selected_param,
            state.is_editing,
            Some('l'),
            theme,
        ));

    // === CENTER COLUMN: Band view ===
    // Band tabs
    let band_tabs = div()
        .flex()
        .flex_wrap()
        .justify_center()
        .border_b_1()
        .border_color(theme.border)
        .children((0..=state.num_bands).map(|i| {
            let is_selected = state.selected_band_idx == i;
            let label = band_tab_label(
                i,
                state.num_bands,
                [
                    state.crossover_freq_1,
                    state.crossover_freq_2,
                    state.crossover_freq_3,
                    state.crossover_freq_4,
                ],
            );
            div()
                .px(d.card)
                // intentional: asymmetric underline-tab padding — 4/6 pair is visually tuned
                .pb(rems(0.375))
                .pt(rems(0.25))
                .text_size(d.text_xs)
                .font_weight(if is_selected {
                    FontWeight::BOLD
                } else {
                    FontWeight::NORMAL
                })
                .text_color(if is_selected {
                    theme.accent
                } else {
                    theme.text_muted
                })
                .border_b_2()
                .border_color(if is_selected {
                    theme.accent
                } else {
                    Theme::with_opacity(theme.border, 0.0)
                })
                .cursor_pointer()
                .hover(|s| {
                    s.text_color(theme.text_primary)
                        .border_color(if is_selected {
                            theme.accent
                        } else {
                            theme.text_muted
                        })
                })
                .id(("mb-ex-band", i))
                .on_mouse_down(MouseButton::Left, {
                    let entity = entity.clone();
                    move |_, _window, cx| {
                        entity.update(cx, |state, cx| {
                            state.app.plugin_state.selected_eq_band = i;
                            cx.notify();
                        });
                    }
                })
                .child(label)
        }));

    // Transfer curve for current band (uses expander ratio visualization)
    let transfer_curve = render_transfer_curve_with_level(
        d,
        state.threshold_db,
        state.ratio,
        state.knee_db,
        false,
        TRANSFER_CURVE_SIZE,
        None,
        theme,
    );

    // Band sliders
    let slider_layout = if compact {
        div().flex().flex_col().items_center().w_full()
    } else {
        div().flex().flex_wrap().justify_center()
    };
    let sliders = slider_layout
        .gap(d.section)
        .child(
            div()
                .flex()
                .flex_col()
                .gap(d.grid)
                .when(compact, |section| section.w_full())
                .child(render_section_title(d, text.label("DYNAMICS"), theme))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .justify_center()
                        .gap(d.gap)
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(),
                            plugin_idx,
                            "Threshold",
                            state.threshold_db,
                            pk(ME, "threshold").min_f64(),
                            pk(ME, "threshold").max_f64(),
                            "dB",
                            get_param_idx(6),
                            state.selected_param,
                            state.is_editing,
                            Some('t'),
                            SLIDER_HEIGHT,
                            theme,
                        ))
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(),
                            plugin_idx,
                            "Ratio",
                            state.ratio,
                            pk(ME, "ratio").min_f64(),
                            pk(ME, "ratio").max_f64(),
                            ":1",
                            get_param_idx(7),
                            state.selected_param,
                            state.is_editing,
                            Some('r'),
                            SLIDER_HEIGHT,
                            theme,
                        ))
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(),
                            plugin_idx,
                            "Knee",
                            state.knee_db,
                            pk(ME, "knee").min_f64(),
                            pk(ME, "knee").max_f64(),
                            "dB",
                            get_param_idx(11),
                            state.selected_param,
                            state.is_editing,
                            Some('k'),
                            SLIDER_HEIGHT,
                            theme,
                        ))
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(),
                            plugin_idx,
                            "Range",
                            state.range_db,
                            pk(ME, "range").min_f64(),
                            pk(ME, "range").max_f64(),
                            "dB",
                            get_param_idx(10),
                            state.selected_param,
                            state.is_editing,
                            Some('g'),
                            SLIDER_HEIGHT,
                            theme,
                        ))
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(),
                            plugin_idx,
                            "Hyst",
                            state.hysteresis_db,
                            pk(ME, "hysteresis").min_f64(),
                            pk(ME, "hysteresis").max_f64(),
                            "dB",
                            get_param_idx(12),
                            state.selected_param,
                            state.is_editing,
                            Some('y'),
                            SLIDER_HEIGHT,
                            theme,
                        )),
                )
                .child(transfer_curve),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(d.grid)
                .when(compact, |section| section.w_full())
                .child(render_section_title(d, text.label("TIMING"), theme))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .justify_center()
                        .gap(d.gap)
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(),
                            plugin_idx,
                            "Attack",
                            state.attack_ms,
                            pk(ME, "attack").min_f64(),
                            pk(ME, "attack").max_f64(),
                            "ms",
                            get_param_idx(8),
                            state.selected_param,
                            state.is_editing,
                            Some('a'),
                            SLIDER_HEIGHT,
                            theme,
                        ))
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(),
                            plugin_idx,
                            "Release",
                            state.release_ms,
                            pk(ME, "release").min_f64(),
                            pk(ME, "release").max_f64(),
                            "ms",
                            get_param_idx(9),
                            state.selected_param,
                            state.is_editing,
                            Some('e'),
                            SLIDER_HEIGHT,
                            theme,
                        ))
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(),
                            plugin_idx,
                            "Hold",
                            state.hold_ms,
                            pk(ME, "hold").min_f64(),
                            pk(ME, "hold").max_f64(),
                            "ms",
                            get_param_idx(13),
                            state.selected_param,
                            state.is_editing,
                            Some('h'),
                            SLIDER_HEIGHT,
                            theme,
                        )),
                ),
        );

    let mut center_col = div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .gap(d.gap_md)
        .when(compact, |column| column.w_full())
        .child(band_tabs)
        .child(sliders);

    // Band Solo/Bypass/Active/AutoGain (only for band-level)
    if state.selected_band_idx > 0 {
        center_col = center_col.child(
            div()
                .flex()
                .gap(d.section)
                .justify_center()
                .child(render_toggle(
                    entity.clone(),
                    plugin_idx,
                    text.label("Active"),
                    state.active,
                    get_param_idx(17),
                    state.selected_param,
                    state.is_editing,
                    theme,
                ))
                .child(render_toggle(
                    entity.clone(),
                    plugin_idx,
                    text.label("Solo"),
                    state.solo,
                    get_param_idx(15),
                    state.selected_param,
                    state.is_editing,
                    theme,
                ))
                .child(render_toggle(
                    entity.clone(),
                    plugin_idx,
                    text.label("Bypass"),
                    state.bypass,
                    get_param_idx(14),
                    state.selected_param,
                    state.is_editing,
                    theme,
                ))
                .child(render_toggle(
                    entity.clone(),
                    plugin_idx,
                    text.label("AutoGain"),
                    state.auto_makeup,
                    get_param_idx(16),
                    state.selected_param,
                    state.is_editing,
                    theme,
                )),
        );
    }

    // === RIGHT COLUMN: Output ===
    let right_col = div()
        .flex()
        .flex_col()
        .flex_shrink_0()
        .gap(d.gap_md)
        .child(render_section_title(d, text.label("OUTPUT"), theme))
        .child(render_toggle(
            entity.clone(),
            plugin_idx,
            text.label("Link Ch"),
            state.link_channels,
            15,
            state.selected_param,
            state.is_editing,
            theme,
        ))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            text.label("Mix"),
            state.mix * 100.0,
            pk(ME, "mix").min_f64() * 100.0,
            pk(ME, "mix").max_f64() * 100.0,
            "%",
            14,
            state.selected_param,
            state.is_editing,
            Some('m'),
            theme,
        ));

    // Keep the dense band editor inside the available rack width. At compact
    // desktop widths it leads the flow and the global/output columns wrap
    // below it; the enclosing rack supplies vertical scrolling.
    let content = div()
        .w_full()
        .min_w_0()
        .flex()
        .items_start()
        .justify_center()
        .gap(d.section)
        .when(compact, |layout| layout.flex_col().items_center());

    let content = if compact {
        content.child(center_col).child(
            div()
                .w_full()
                .flex()
                .flex_wrap()
                .items_start()
                .justify_center()
                .gap(d.section)
                .child(global_col)
                .child(right_col),
        )
    } else {
        content.child(global_col).child(center_col).child(right_col)
    };

    div().w_full().min_w_0().p(d.pad_x).child(content)
}

fn render_detection_mode_selector(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    current: i32,
    selected_param: usize,
    is_editing: bool,
    text: PluginCommonTranslations,
    theme: &Theme,
) -> impl IntoElement {
    let param_idx = 16;
    div()
        .flex()
        .flex_col()
        .items_stretch()
        .gap(d.grid)
        .w(rems(8.125))
        .rounded(d.r_md)
        .when(selected_param == param_idx && is_editing, |el| {
            el.border_1().border_color(theme.accent)
        })
        .child(
            div()
                .text_size(d.text_xs)
                .text_color(theme.text_muted)
                .child(text.detection),
        )
        .child(
            div()
                .flex()
                .gap(d.grid)
                .children(
                    ["Peak", "RMS"]
                        .into_iter()
                        .enumerate()
                        .map(move |(idx, label)| {
                            let is_active = current as usize == idx;
                            let entity = entity.clone();
                            div()
                                .text_size(d.text_xs)
                                .px(d.pad_y)
                                .py(d.pad_y_half)
                                .rounded(d.r_sm)
                                .cursor_pointer()
                                .when(is_active, |el| {
                                    el.bg(theme.accent).text_color(theme.text_on_accent)
                                })
                                .when(!is_active, |el| {
                                    el.bg(theme.background_secondary)
                                        .text_color(theme.text_secondary)
                                        .hover(|s| s.bg(theme.surface_hover))
                                })
                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                    entity.update(cx, |state, _| {
                                        state
                                            .app
                                            .set_plugin_param(plugin_idx, param_idx, idx as f64);
                                    });
                                })
                                .child(label)
                        }),
                ),
        )
}
use crate::app::i18n::PluginCommonTranslations;
