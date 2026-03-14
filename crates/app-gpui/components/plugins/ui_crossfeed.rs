//! Crossfeed Plugin UI Component
//!
//! Headphone crossfeed for speaker-like listening:
//! - Mode selector row: Bauer | Meier | Multiband | Disable
//! - Per-mode parameters shown conditionally
//! - Output column: target gain, auto gain toggle + max gain, smoothing, mix

use super::common::{render_knob, render_section_title, render_toggle};
use crate::app::AppState;
use crate::components::plugins::editing::PluginEditingManager;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{ButtonSet, ButtonSetOption, ButtonSetSize};
use sotf_plugins::param_specs::{crossfeed::PARAMS as CF, find_by_key as pk};
use sotf_plugins::CrossfeedMode;

/// State for rendering the Crossfeed plugin
pub struct CrossfeedRenderState {
    pub mode: CrossfeedMode,
    pub mix: f64,
    // Bauer
    pub bauer_fcut_hz: f64,
    pub bauer_feed_db: f64,
    // Meier
    pub meier_level: f64,
    // Multiband
    pub mb_low_freq_hz: f64,
    pub mb_mid_high_freq_hz: f64,
    pub mb_low_feed_db: f64,
    pub mb_mid_feed_db: f64,
    pub mb_high_feed_db: f64,
    // Auto gain
    pub autogain_enabled: bool,
    pub autogain_target_lufs: f64,
    pub autogain_max_gain_db: f64,
    pub autogain_smoothing_ms: f64,
    // UI state
    pub is_editing: bool,
    pub selected_param: usize,
}

/// Render the Crossfeed plugin
pub fn render_crossfeed_plugin(
    entity: Entity<AppState>,
    plugin_idx: usize,
    state: CrossfeedRenderState,
    theme: &Theme,
) -> impl IntoElement {
    let current_mode = state.mode;

    // Map CrossfeedMode to the ButtonSet label
    let mode_selected = match current_mode {
        CrossfeedMode::Bauer => "Bauer",
        CrossfeedMode::Meier => "Meier",
        CrossfeedMode::Mb => "Multiband",
        CrossfeedMode::Off => "Disable",
    };

    div()
        .flex()
        .flex_col()
        .gap_4()
        // Row 1: Mode selector (Bauer | Meier | Multiband | Disable)
        .child({
            let entity_mode = entity.clone();
            ButtonSet::new(("crossfeed-mode", plugin_idx))
                .options(vec![
                    ButtonSetOption::new("Bauer", "Bauer"),
                    ButtonSetOption::new("Meier", "Meier"),
                    ButtonSetOption::new("Multiband", "Multiband"),
                    ButtonSetOption::new("Disable", "Disable"),
                ])
                .selected(mode_selected)
                .size(ButtonSetSize::Xs)
                .theme(theme.to_button_set_theme())
                .on_change(move |value, _, cx| {
                    let mode_idx: f64 = match value.as_ref() {
                        "Bauer" => 1.0,
                        "Meier" => 2.0,
                        "Multiband" => 3.0,
                        "Disable" => 0.0,
                        _ => 0.0,
                    };
                    entity_mode.update(cx, |state, cx| {
                        state.app.set_plugin_param(plugin_idx, 0, mode_idx);
                        cx.notify();
                    });
                })
        })
        // Row 2: Columns - Mode params | Output
        .child(
            div()
                .flex()
                .gap_6()
                .items_start()
                // Column 1: Mode-specific parameters
                .when(current_mode == CrossfeedMode::Bauer, |d| {
                    d.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(render_section_title("BAUER", theme))
                            .child(render_knob(
                                entity.clone(),
                                plugin_idx,
                                "Fcut",
                                state.bauer_fcut_hz,
                                pk(CF, "bauer_fcut_hz").min_f64(),
                                pk(CF, "bauer_fcut_hz").max_f64(),
                                "Hz",
                                4,
                                state.selected_param,
                                state.is_editing,
                                Some('f'),
                                theme,
                            ))
                            .child(render_knob(
                                entity.clone(),
                                plugin_idx,
                                "Feed",
                                state.bauer_feed_db,
                                pk(CF, "bauer_feed_db").min_f64(),
                                pk(CF, "bauer_feed_db").max_f64(),
                                "dB",
                                5,
                                state.selected_param,
                                state.is_editing,
                                None,
                                theme,
                            )),
                    )
                })
                .when(current_mode == CrossfeedMode::Meier, |d| {
                    d.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(render_section_title("MEIER", theme))
                            .child(render_knob(
                                entity.clone(),
                                plugin_idx,
                                "Level",
                                state.meier_level,
                                pk(CF, "meier_level").min_f64(),
                                pk(CF, "meier_level").max_f64(),
                                "%",
                                6,
                                state.selected_param,
                                state.is_editing,
                                Some('l'),
                                theme,
                            )),
                    )
                })
                .when(current_mode == CrossfeedMode::Mb, |d| {
                    d.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(render_section_title("MULTIBAND", theme))
                            .child(render_knob(
                                entity.clone(),
                                plugin_idx,
                                "Low Freq",
                                state.mb_low_freq_hz,
                                pk(CF, "mb_low_freq_hz").min_f64(),
                                pk(CF, "mb_low_freq_hz").max_f64(),
                                "Hz",
                                7,
                                state.selected_param,
                                state.is_editing,
                                None,
                                theme,
                            ))
                            .child(render_knob(
                                entity.clone(),
                                plugin_idx,
                                "Mid-Hi Freq",
                                state.mb_mid_high_freq_hz,
                                pk(CF, "mb_mid_high_freq_hz").min_f64(),
                                pk(CF, "mb_mid_high_freq_hz").max_f64(),
                                "Hz",
                                8,
                                state.selected_param,
                                state.is_editing,
                                None,
                                theme,
                            ))
                            .child(render_knob(
                                entity.clone(),
                                plugin_idx,
                                "Low Feed",
                                state.mb_low_feed_db,
                                pk(CF, "mb_low_feed_db").min_f64(),
                                pk(CF, "mb_low_feed_db").max_f64(),
                                "dB",
                                9,
                                state.selected_param,
                                state.is_editing,
                                None,
                                theme,
                            ))
                            .child(render_knob(
                                entity.clone(),
                                plugin_idx,
                                "Mid Feed",
                                state.mb_mid_feed_db,
                                pk(CF, "mb_mid_feed_db").min_f64(),
                                pk(CF, "mb_mid_feed_db").max_f64(),
                                "dB",
                                10,
                                state.selected_param,
                                state.is_editing,
                                None,
                                theme,
                            ))
                            .child(render_knob(
                                entity.clone(),
                                plugin_idx,
                                "High Feed",
                                state.mb_high_feed_db,
                                pk(CF, "mb_high_feed_db").min_f64(),
                                pk(CF, "mb_high_feed_db").max_f64(),
                                "dB",
                                11,
                                state.selected_param,
                                state.is_editing,
                                None,
                                theme,
                            )),
                    )
                })
                // Column 3: Output (target gain, auto gain on/off + max gain, smoothing, mix)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("OUTPUT", theme))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Target",
                            state.autogain_target_lufs,
                            pk(CF, "autogain_target_lufs").min_f64(),
                            pk(CF, "autogain_target_lufs").max_f64(),
                            "LUFS",
                            13,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        .child(render_toggle(
                            entity.clone(),
                            plugin_idx,
                            "Auto Gain",
                            state.autogain_enabled,
                            12,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Max Gain",
                            state.autogain_max_gain_db,
                            pk(CF, "autogain_max_gain_db").min_f64(),
                            pk(CF, "autogain_max_gain_db").max_f64(),
                            "dB",
                            14,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Smoothing",
                            state.autogain_smoothing_ms,
                            pk(CF, "autogain_smoothing_ms").min_f64(),
                            pk(CF, "autogain_smoothing_ms").max_f64(),
                            "ms",
                            15,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Mix",
                            state.mix * 100.0,
                            pk(CF, "mix").min_f64() * 100.0,
                            pk(CF, "mix").max_f64() * 100.0,
                            "%",
                            3,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        )),
                ),
        )
}

