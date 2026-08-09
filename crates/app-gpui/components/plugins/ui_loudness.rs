//! Loudness Monitor Plugin UI Component

use super::level_meters::render_lufs_with_true_peak;
use crate::app::i18n::LevelMeterTranslations;
use crate::components::design::Ds;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;

/// Render the Loudness Monitor plugin (analyzer)
pub fn render_loudness_monitor_plugin(
    d: &Ds,
    loudness: Option<std::sync::Arc<sotf_audio_player::LoudnessData>>,
    _plugin_idx: usize,
    _is_editing: bool,
    text: LevelMeterTranslations,
    theme: &Theme,
) -> impl IntoElement {
    div().child(render_lufs_with_true_peak(
        d,
        loudness.as_deref(),
        text,
        theme,
    ))
}
