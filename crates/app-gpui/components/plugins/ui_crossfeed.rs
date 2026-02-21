//! Crossfeed Plugin UI Component
//!
//! Headphone crossfeed for speaker-like listening:
//! - Multiple modes: Off, Bauer, Meier, Multiband
//! - Presets for quick configuration
//! - Per-mode parameters (Bauer fcut/feed, Meier level, MB frequencies/feeds)
//! - Auto gain compensation

use super::common::{render_knob, render_section_title, render_toggle};
use crate::app::AppState;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use sotf_plugins::param_specs::crossfeed::*;
use sotf_plugins::{CrossfeedMode, CrossfeedPreset};

/// State for rendering the Crossfeed plugin
pub struct CrossfeedRenderState {
    pub mode: CrossfeedMode,
    pub preset: CrossfeedPreset,
    pub enabled: bool,
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
    div()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            div()
                .flex()
                .gap_6()
                .items_start()
                // Column 1: General
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("GENERAL", theme))
                        // Mode (param 0) - displayed as text, cycled via adjust
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Mode",
                            mode_to_display_value(&state.mode),
                            0.0,
                            3.0,
                            &format!("{:?}", state.mode),
                            0,
                            state.selected_param,
                            state.is_editing,
                            Some('m'),
                            theme,
                        ))
                        // Preset (param 1) - displayed as text, cycled via adjust
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Preset",
                            preset_to_display_value(&state.preset),
                            0.0,
                            4.0,
                            &format!("{:?}", state.preset),
                            1,
                            state.selected_param,
                            state.is_editing,
                            Some('p'),
                            theme,
                        ))
                        // Enabled (param 2)
                        .child(render_toggle(
                            entity.clone(),
                            plugin_idx,
                            "Enabled",
                            state.enabled,
                            2,
                            state.selected_param,
                            state.is_editing,
                            theme,
                        ))
                        // Mix (param 3)
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Mix",
                            state.mix * 100.0,
                            MIX_MIN as f64 * 100.0,
                            MIX_MAX as f64 * 100.0,
                            "%",
                            3,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        )),
                )
                // Column 2: Bauer
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("BAUER", theme))
                        // Fcut (param 4)
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Fcut",
                            state.bauer_fcut_hz,
                            BAUER_FCUT_MIN as f64,
                            BAUER_FCUT_MAX as f64,
                            "Hz",
                            4,
                            state.selected_param,
                            state.is_editing,
                            Some('f'),
                            theme,
                        ))
                        // Feed (param 5)
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Feed",
                            state.bauer_feed_db,
                            BAUER_FEED_MIN as f64,
                            BAUER_FEED_MAX as f64,
                            "dB",
                            5,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        )),
                )
                // Column 3: Meier
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("MEIER", theme))
                        // Level (param 6)
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Level",
                            state.meier_level,
                            MEIER_LEVEL_MIN as f64,
                            MEIER_LEVEL_MAX as f64,
                            "",
                            6,
                            state.selected_param,
                            state.is_editing,
                            Some('l'),
                            theme,
                        )),
                )
                // Column 4: Multiband
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("MULTIBAND", theme))
                        // Low Freq (param 7)
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Low Freq",
                            state.mb_low_freq_hz,
                            MB_LOW_FREQ_MIN as f64,
                            MB_LOW_FREQ_MAX as f64,
                            "Hz",
                            7,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        // Mid-High Freq (param 8)
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Mid-Hi Freq",
                            state.mb_mid_high_freq_hz,
                            MB_MID_HIGH_FREQ_MIN as f64,
                            MB_MID_HIGH_FREQ_MAX as f64,
                            "Hz",
                            8,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        // Low Feed (param 9)
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Low Feed",
                            state.mb_low_feed_db,
                            MB_LOW_FEED_MIN as f64,
                            MB_LOW_FEED_MAX as f64,
                            "dB",
                            9,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        // Mid Feed (param 10)
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Mid Feed",
                            state.mb_mid_feed_db,
                            MB_MID_FEED_MIN as f64,
                            MB_MID_FEED_MAX as f64,
                            "dB",
                            10,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        // High Feed (param 11)
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "High Feed",
                            state.mb_high_feed_db,
                            MB_HIGH_FEED_MIN as f64,
                            MB_HIGH_FEED_MAX as f64,
                            "dB",
                            11,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        )),
                )
                // Column 5: Auto Gain
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(render_section_title("AUTO GAIN", theme))
                        // Auto Gain Enabled (param 12)
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
                        // Target (param 13)
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Target",
                            state.autogain_target_lufs,
                            AUTOGAIN_TARGET_MIN as f64,
                            AUTOGAIN_TARGET_MAX as f64,
                            "LUFS",
                            13,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        // Max Gain (param 14)
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Max Gain",
                            state.autogain_max_gain_db,
                            AUTOGAIN_MAX_GAIN_MIN as f64,
                            AUTOGAIN_MAX_GAIN_MAX as f64,
                            "dB",
                            14,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        ))
                        // Smoothing (param 15)
                        .child(render_knob(
                            entity.clone(),
                            plugin_idx,
                            "Smoothing",
                            state.autogain_smoothing_ms,
                            AUTOGAIN_SMOOTHING_MIN as f64,
                            AUTOGAIN_SMOOTHING_MAX as f64,
                            "ms",
                            15,
                            state.selected_param,
                            state.is_editing,
                            None,
                            theme,
                        )),
                ),
        )
}

fn mode_to_display_value(mode: &CrossfeedMode) -> f64 {
    match mode {
        CrossfeedMode::Off => 0.0,
        CrossfeedMode::Bauer => 1.0,
        CrossfeedMode::Meier => 2.0,
        CrossfeedMode::Mb => 3.0,
    }
}

fn preset_to_display_value(preset: &CrossfeedPreset) -> f64 {
    match preset {
        CrossfeedPreset::Default => 0.0,
        CrossfeedPreset::Cmoy => 1.0,
        CrossfeedPreset::Meier => 2.0,
        CrossfeedPreset::Mb => 3.0,
        CrossfeedPreset::Off => 4.0,
    }
}
