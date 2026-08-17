use super::super::common::{
    render_knob, render_section_title, render_toggle, render_transfer_curve_with_level,
    render_vertical_slider_with_ticks,
};
use super::misc::SLIDER_HEIGHT;
use super::misc::TRANSFER_CURVE_SIZE;
use crate::app::AppState;
use crate::components::design::Ds;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_plugins::param_specs::{find_by_key as pk, multiband_compressor::GLOBAL_PARAMS as MC};

use super::super::ui_multiband_common::{
    band_tab_label, render_band_count_editor, render_crossover_preset_editor,
};

/// State for rendering the Multiband Compressor plugin
pub struct MbCompressorRenderState {
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
    pub knee_db: f64,
    pub makeup_gain_db: f64,
    pub auto_makeup: bool,
    pub active: bool,
    pub solo: bool,
    pub bypass: bool,
    pub mix: f64,
    pub link_channels: bool,
    pub per_band_lookahead_ms: f64,
    pub ms_mode: bool,
    pub sidechain_tilt_db: f64,
    pub link_amount: f64,
    pub is_editing: bool,
    pub selected_param: usize,
    pub selected_band_idx: usize,
}

/// Render the Multiband Compressor plugin
pub fn render_mb_compressor_plugin(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: MbCompressorRenderState,
    available_width: f32,
    layout_scale: f32,
    text: PluginCommonTranslations,
    theme: &Theme,
) -> impl IntoElement {
    let compact = available_width / layout_scale.max(0.01) < 720.0;
    let slider_height = (SLIDER_HEIGHT * layout_scale).clamp(120.0, 240.0);
    let transfer_curve_size = (TRANSFER_CURVE_SIZE * layout_scale).clamp(120.0, 260.0);
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
        .child(render_crossover_preset_editor(
            d,
            "mb-crossover-preset",
            "mb-crossover-preset-detail",
            text.label("Preset"),
            text.multiband_presets.labels(),
            compact,
            entity.clone(),
            plugin_idx,
            [
                state.crossover_freq_1,
                state.crossover_freq_2,
                state.crossover_freq_3,
                state.crossover_freq_4,
            ],
            theme,
        ))
        .child(render_band_count_editor(
            d,
            "mb-bands",
            entity.clone(),
            plugin_idx,
            state.num_bands,
            pk(MC, "num_bands").min_f64(),
            pk(MC, "num_bands").max_f64(),
            text.bands,
            theme,
        ))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            text.label("XOver 1"),
            state.crossover_freq_1,
            pk(MC, "crossover_freq_1").min_f64(),
            pk(MC, "crossover_freq_1").max_f64(),
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
            pk(MC, "crossover_freq_2").min_f64(),
            pk(MC, "crossover_freq_2").max_f64(),
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
            pk(MC, "crossover_freq_3").min_f64(),
            pk(MC, "crossover_freq_3").max_f64(),
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
            pk(MC, "crossover_freq_4").min_f64(),
            pk(MC, "crossover_freq_4").max_f64(),
            "Hz",
            5,
            state.selected_param,
            state.is_editing,
            Some('4'),
            theme,
        ));
    }

    // Global params
    global_col = global_col
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            text.label("Lookahead"),
            state.per_band_lookahead_ms,
            pk(MC, "per_band_lookahead_ms").min_f64(),
            pk(MC, "per_band_lookahead_ms").max_f64(),
            "ms",
            13,
            state.selected_param,
            state.is_editing,
            Some('l'),
            theme,
        ))
        .child(render_toggle(
            entity.clone(),
            plugin_idx,
            text.label("M/S Mode"),
            state.ms_mode,
            14,
            state.selected_param,
            state.is_editing,
            theme,
        ))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            text.label("SC Tilt"),
            state.sidechain_tilt_db,
            pk(MC, "sidechain_tilt_db").min_f64(),
            pk(MC, "sidechain_tilt_db").max_f64(),
            "dB",
            15,
            state.selected_param,
            state.is_editing,
            Some('t'),
            theme,
        ))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            text.label("Link Amt"),
            state.link_amount * 100.0,
            pk(MC, "link_amount").min_f64() * 100.0,
            pk(MC, "link_amount").max_f64() * 100.0,
            "%",
            16,
            state.selected_param,
            state.is_editing,
            Some('a'),
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
                    crate::theme::Theme::with_opacity(theme.border, 0.0)
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
                .id(("mb-band", i))
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

    // Transfer curve for current band
    let transfer_curve = render_transfer_curve_with_level(
        d,
        state.threshold_db,
        state.ratio,
        state.knee_db,
        false,
        transfer_curve_size,
        None,
        theme,
    );

    // Band sliders (DYNAMICS + TIMING side by side, transfer curve below both)
    let slider_row = div()
        .flex()
        .gap(d.section)
        .child(
            div()
                .flex()
                .flex_col()
                .gap(d.grid)
                .child(render_section_title(d, text.label("DYNAMICS"), theme))
                .child(
                    div()
                        .flex()
                        .gap(d.gap)
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(),
                            plugin_idx,
                            "Threshold",
                            state.threshold_db,
                            pk(MC, "threshold").min_f64(),
                            pk(MC, "threshold").max_f64(),
                            "dB",
                            get_param_idx(6),
                            state.selected_param,
                            state.is_editing,
                            Some('t'),
                            slider_height,
                            theme,
                        ))
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(),
                            plugin_idx,
                            "Ratio",
                            state.ratio,
                            pk(MC, "ratio").min_f64(),
                            pk(MC, "ratio").max_f64(),
                            ":1",
                            get_param_idx(7),
                            state.selected_param,
                            state.is_editing,
                            Some('r'),
                            slider_height,
                            theme,
                        ))
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(),
                            plugin_idx,
                            "Knee",
                            state.knee_db,
                            pk(MC, "knee").min_f64(),
                            pk(MC, "knee").max_f64(),
                            "dB",
                            get_param_idx(10),
                            state.selected_param,
                            state.is_editing,
                            Some('k'),
                            slider_height,
                            theme,
                        )),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(d.grid)
                .child(render_section_title(d, text.label("TIMING"), theme))
                .child(
                    div()
                        .flex()
                        .gap(d.gap)
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(),
                            plugin_idx,
                            "Attack",
                            state.attack_ms,
                            pk(MC, "attack").min_f64(),
                            pk(MC, "attack").max_f64(),
                            "ms",
                            get_param_idx(8),
                            state.selected_param,
                            state.is_editing,
                            Some('a'),
                            slider_height,
                            theme,
                        ))
                        .child(render_vertical_slider_with_ticks(
                            entity.clone(),
                            plugin_idx,
                            "Release",
                            state.release_ms,
                            pk(MC, "release").min_f64(),
                            pk(MC, "release").max_f64(),
                            "ms",
                            get_param_idx(9),
                            state.selected_param,
                            state.is_editing,
                            Some('e'),
                            slider_height,
                            theme,
                        ))
                        .when(state.selected_band_idx > 0, |d| {
                            d.child(render_vertical_slider_with_ticks(
                                entity.clone(),
                                plugin_idx,
                                "Makeup",
                                state.makeup_gain_db,
                                -24.0,
                                24.0,
                                "dB",
                                get_param_idx(13),
                                state.selected_param,
                                state.is_editing,
                                Some('g'),
                                slider_height,
                                theme,
                            ))
                        }),
                ),
        );

    // Stack sliders and transfer curve vertically
    let sliders = div()
        .flex()
        .flex_col()
        .gap(d.gap)
        .child(slider_row)
        .child(transfer_curve);

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
            12,
            state.selected_param,
            state.is_editing,
            theme,
        ))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            text.label("Mix"),
            state.mix * 100.0,
            pk(MC, "mix").min_f64() * 100.0,
            pk(MC, "mix").max_f64() * 100.0,
            "%",
            11,
            state.selected_param,
            state.is_editing,
            Some('m'),
            theme,
        ));

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
use crate::app::i18n::PluginCommonTranslations;
