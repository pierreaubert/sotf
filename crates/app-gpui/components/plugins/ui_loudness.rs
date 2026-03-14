//! Loudness Monitor Plugin UI Component

use super::common::ParamSectionStyle;
use super::level_meters::render_lufs_with_true_peak;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;

/// Render the Loudness Monitor plugin (analyzer)
pub fn render_loudness_monitor_plugin(
    loudness: Option<sotf_audio_player::LoudnessData>,
    _plugin_idx: usize,
    _is_editing: bool,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_4()
        .child(render_lufs_with_true_peak(loudness.as_ref(), theme))
        .child(div().flex().flex_col().gap_2().param_section_base(theme))
}
