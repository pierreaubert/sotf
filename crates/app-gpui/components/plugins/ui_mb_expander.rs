//! Multiband Expander Plugin UI Component
//!
//! Layout (3-column):
//! +------------------+--------------------------------------------+------------------+
//! | GLOBAL           | BAND VIEW                                  | OUTPUT           |
//! |                  |                                            |                  |
//! | [Bands]    knob  | [Global] [1] [2] [3] ... tabs              | [Mix]      knob  |
//! | [XOver 1]  knob  | Per band:                                  | [Link Ch]  tog   |
//! | [XOver 2]  knob  | [Thresh] [Ratio] [Knee]                    |                  |
//! | [XOver 3]  knob  | [Attack] [Release] [Hold] [Range] [Hyst]   |                  |
//! | [XOver 4]  knob  | [Solo] [Bypass]                            |                  |
//! +------------------+--------------------------------------------+------------------+

use super::common::{
    render_knob, render_section_title, render_toggle, render_transfer_curve_with_level,
    render_vertical_slider_with_ticks,
};
use crate::app::AppState;
use crate::components::design::Ds;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_plugins::param_specs::{find_by_key as pk, multiband_expander::GLOBAL_PARAMS as ME};

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
    pub is_editing: bool,
    pub selected_param: usize,
    pub selected_band_idx: usize,
}

// Layout constants
const SLIDER_HEIGHT: f32 = 180.0;
const TRANSFER_CURVE_SIZE: f32 = 140.0;

/// Render the Multiband Expander plugin
pub fn render_mb_expander_plugin(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: MbExpanderRenderState,
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
    let mut global_col = div()
        .flex()
        .flex_col()
        .flex_shrink_0()
        .gap(d.gap_md)
        .child(render_section_title(d, "GLOBAL", theme))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            "Bands",
            state.num_bands as f64,
            pk(ME, "num_bands").min_f64(),
            pk(ME, "num_bands").max_f64(),
            "",
            0,
            state.selected_param,
            state.is_editing,
            Some('b'),
            theme,
        ))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            "XOver 1",
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
            "XOver 2",
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
            "XOver 3",
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
            "XOver 4",
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
                // intentional: asymmetric underline-tab padding — 4/6 pair is visually tuned
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
                    gpui::Rgba {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.0,
                    }
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
    let sliders = div()
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
            15,
            state.selected_param,
            state.is_editing,
            theme,
        ))
        .child(render_knob(
            entity.clone(),
            plugin_idx,
            "Mix",
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
