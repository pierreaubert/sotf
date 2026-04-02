//! Multiband Compressor Plugin UI Component
//!
//! Layout (3-column):
//! +------------------+--------------------------------------------+------------------+
//! | GLOBAL           | BAND VIEW                                  | OUTPUT           |
//! |                  |                                            |                  |
//! | [Bands]    knob  | [Global] [1] [2] [3] ... tabs              | [Mix]      knob  |
//! | [XOver 1]  knob  | Per band:                                  | [Link Ch]  tog   |
//! | [XOver 2]  knob  | [Thresh] [Ratio] [Attack] [Release]        |                  |
//! | [XOver 3]  knob  | [Knee] [Makeup] [Solo] [Bypass]            |                  |
//! | [XOver 4]  knob  |                                            |                  |
//! +------------------+--------------------------------------------+------------------+

use super::common::{
    render_knob, render_section_title, render_toggle, render_transfer_curve_with_level,
    render_vertical_slider_with_ticks,
};
use crate::app::AppState;
use crate::app::types::PluginUpdateType;
use crate::components::design::Ds;
use crate::components::plugins::editing::PluginEditingManager;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{NumberInput, NumberInputSize};
use sotf_plugins::param_specs::{find_by_key as pk, multiband_compressor::GLOBAL_PARAMS as MC};

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
    pub is_editing: bool,
    pub selected_param: usize,
    pub selected_band_idx: usize,
}

// Layout constants
const SLIDER_HEIGHT: f32 = 180.0;
const TRANSFER_CURVE_SIZE: f32 = 140.0;

/// Render the Multiband Compressor plugin
pub fn render_mb_compressor_plugin(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: MbCompressorRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let get_param_idx = |base_idx: usize| -> usize {
        if state.selected_band_idx > 0 {
            state.selected_band_idx * 100 + base_idx
        } else {
            base_idx
        }
    };

    // === LEFT COLUMN: Global ===
    let bands_entity = entity.clone();
    let mut global_col = div()
        .flex()
        .flex_col()
        .flex_shrink_0()
        .gap(d.gap_md)
        .child(render_section_title(d, "GLOBAL", theme))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(d.grid)
                .child(
                    div()
                        .text_size(d.text_xs)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_secondary)
                        .child("Bands"),
                )
                .child(
                    NumberInput::new("mb-bands")
                        .value(state.num_bands as f64)
                        .min(pk(MC, "num_bands").min_f64())
                        .max(pk(MC, "num_bands").max_f64())
                        .step(1.0)
                        .decimals(0)
                        .size(NumberInputSize::Xs)
                        .width(80.0)
                        .on_change(move |val, _window, cx| {
                            bands_entity.update(cx, |state, _| {
                                state.app.set_plugin_param(plugin_idx, 0, val);
                                state.app.plugin_state.pending_plugin_update =
                                    Some(PluginUpdateType::Structural);
                            });
                        }),
                ),
        )
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            "XOver 1",
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
            "XOver 2",
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
            "XOver 3",
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
            "XOver 4",
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

    // === CENTER COLUMN: Band view ===
    // Band tabs
    let band_tabs = div()
        .flex()
        .justify_center()
        .border_b_1()
        .border_color(theme.border)
        .children((0..=state.num_bands).map(|i| {
            let is_selected = state.selected_band_idx == i;
            let label = if i == 0 {
                "Global".to_string()
            } else {
                format!("{}", i)
            };
            div()
                .px(d.card)
                .pb(px(6.0))
                .pt(px(4.0))
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
                    gpui::Rgba { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }
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
        TRANSFER_CURVE_SIZE,
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
                .child(render_section_title(d, "DYNAMICS", theme))
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
                            SLIDER_HEIGHT,
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
                            SLIDER_HEIGHT,
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
                            SLIDER_HEIGHT,
                            theme,
                        )),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(d.grid)
                .child(render_section_title(d, "TIMING", theme))
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
                            SLIDER_HEIGHT,
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
                            SLIDER_HEIGHT,
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
                                SLIDER_HEIGHT,
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
        .gap(d.gap_md)
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
                    "Active",
                    state.active,
                    get_param_idx(17),
                    state.selected_param,
                    state.is_editing,
                    theme,
                ))
                .child(render_toggle(
                    entity.clone(),
                    plugin_idx,
                    "Solo",
                    state.solo,
                    get_param_idx(15),
                    state.selected_param,
                    state.is_editing,
                    theme,
                ))
                .child(render_toggle(
                    entity.clone(),
                    plugin_idx,
                    "Bypass",
                    state.bypass,
                    get_param_idx(14),
                    state.selected_param,
                    state.is_editing,
                    theme,
                ))
                .child(render_toggle(
                    entity.clone(),
                    plugin_idx,
                    "AutoGain",
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
        .child(render_section_title(d, "OUTPUT", theme))
        .child(render_toggle(
            entity.clone(),
            plugin_idx,
            "Link Ch",
            state.link_channels,
            12,
            state.selected_param,
            state.is_editing,
            theme,
        ))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            "Mix",
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

    // === Main layout: 3 columns, centered ===
    div().w_full().flex().justify_center().p(d.pad_x).child(
        div()
            .flex()
            .gap(d.section)
            .child(global_col)
            .child(center_col)
            .child(right_col),
    )
}
