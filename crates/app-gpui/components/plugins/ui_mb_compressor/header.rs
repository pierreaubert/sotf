//! Compact multiband compressor structural and channel controls.

// Rust guideline compliant 2026-02-21
use crate::app::{AppState, i18n::PluginCommonTranslations};
use crate::components::design::Ds;
use crate::components::plugins::ui_multiband_common::render_dense_multiband_header;
use crate::theme::Theme;
use gpui::{AnyElement, App, Entity};
use sotf_audio_player::PluginSettings;

#[expect(
    clippy::large_types_passed_by_value,
    reason = "Copy translation snapshots remain owned by returned elements and deferred callbacks"
)]
pub(crate) fn render_header(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    settings: &PluginSettings,
    text: PluginCommonTranslations,
    theme: &Theme,
    cx: &App,
) -> AnyElement {
    render_dense_multiband_header(d, entity, plugin_idx, settings, text, theme, cx, &[12, 14])
}
