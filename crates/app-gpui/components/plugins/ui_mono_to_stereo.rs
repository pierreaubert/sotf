//! Mono to Stereo Plugin UI Component
//!
//! Controls for pseudo-stereo widening with:
//! - Stereo width control
//! - Haas delay (ms)
//! - Complementary EQ toggle and depth
//! - Decorrelation frequency range

use super::common::{render_knob, render_section_title, render_toggle};
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_plugins::param_specs::{find_by_key as pk, mono_to_stereo::PARAMS as MS};

/// State for rendering the MonoToStereo plugin
pub struct MonoToStereoRenderState {
    pub stereo_width: f64,
    pub haas_delay_ms: f64,
    pub enable_comp_eq: bool,
    pub comp_eq_depth_db: f64,
    pub decor_low_hz: f64,
    pub decor_high_hz: f64,
    pub is_editing: bool,
    pub selected_param: usize,
}

/// Render the MonoToStereo plugin
pub fn render_mono_to_stereo_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: MonoToStereoRenderState,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(render_section_title("STEREO WIDENING", theme))
                .child(
                    div()
                        .flex()
                        .gap_4()
                        .justify_around()
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Width",
                            state.stereo_width,
                            pk(MS, "stereo_width").min_f64(),
                            pk(MS, "stereo_width").max_f64(),
                            "",
                            0,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Haas Delay",
                            state.haas_delay_ms,
                            pk(MS, "haas_delay_ms").min_f64(),
                            pk(MS, "haas_delay_ms").max_f64(),
                            "ms",
                            1,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        )),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(render_section_title("COMPLEMENTARY EQ", theme))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_4()
                        .child(render_toggle(
                            entity.clone(),
                            plugin_idx,
                            "Enable Panning EQ",
                            state.enable_comp_eq,
                            2,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        .when(state.enable_comp_eq, |d| {
                            d.child(render_knob(
                                entity.clone(),
                                plugin_idx,
                                "EQ Depth",
                                state.comp_eq_depth_db,
                                pk(MS, "comp_eq_depth_db").min_f64(),
                                pk(MS, "comp_eq_depth_db").max_f64(),
                                "dB",
                                3,
                                state.selected_param,
                                state.is_editing,
                                None,
                                theme,
                            ))
                        }),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(render_section_title("DECORRELATION", theme))
                .child(
                    div()
                        .flex()
                        .gap_4()
                        .justify_around()
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Decor Low",
                            state.decor_low_hz,
                            pk(MS, "decor_low_hz").min_f64(),
                            pk(MS, "decor_low_hz").max_f64(),
                            "Hz",
                            4,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Decor High",
                            state.decor_high_hz,
                            pk(MS, "decor_high_hz").min_f64(),
                            pk(MS, "decor_high_hz").max_f64(),
                            "Hz",
                            5,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        )),
                ),
        )
}
