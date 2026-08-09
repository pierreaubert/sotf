//! GPUI elements for the spatial spider visualizer.
//!
//! Two flavours:
//!
//! - [`SpiderDisc2D`]: top-down horizontal disc (`gpui::PathBuilder` paths).
//! - [`SpiderView3D`]: two intersecting reference planes drawn through
//!   [`d3rs::gpu3d::Lines3DElement`] so we inherit orbit / pan / zoom.
//!
//! Both consume the polygon geometry built by [`crate::components::plugins::spatial_spider::data`].
//! The renderer is decoupled from the underlying audio plumbing — the
//! plugin UI is responsible for materialising the [`ChannelMetric`] and
//! re-painting the element every refresh.

use super::SpatialSpiderSnapshot;
use crate::app::AppState;
use crate::app::i18n::PluginCommonTranslations;
use crate::components::design::Ds;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{StackSpacing, VStack};
use sotf_plugins::speaker_config::SpeakerConfig;

mod build;
mod consts;
mod misc;
mod paint;
mod spider_colors;
mod spider_disc2_d;
mod spider_disc2_dinner;
mod spider_view3_d;
#[cfg(test)]
mod tests;

pub use misc::*;
pub use spider_colors::*;
pub use spider_disc2_d::*;
#[cfg(feature = "gpu-3d")]
pub use spider_view3_d::SpiderView3D;

use build::build_body;
use build::build_header;

/// Render the complete spider panel (header + body). Both the upmixer
/// custom view and the layout-renderer custom-viz hook delegate here so
/// they share toggles, ref-channel selector, and palette.
///
/// - `speaker_config_id`: optional explicit speaker config id (e.g. "5.1.4")
///   when the host knows it. When `None`, we fall back to deriving the
///   layout from the loudness data's channel count.
pub fn render_spatial_spider_panel(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    snapshot: &SpatialSpiderSnapshot,
    speaker_config_id: Option<&str>,
    text: PluginCommonTranslations,
    theme: &crate::theme::Theme,
) -> AnyElement {
    let cfg_opt = resolve_speaker_config(snapshot, speaker_config_id);
    let header =
        render_spatial_spider_controls(d, entity, plugin_idx, snapshot, cfg_opt, text, theme);
    let body = render_spatial_spider_graph(d, snapshot, cfg_opt, text, theme);

    VStack::new()
        .spacing(StackSpacing::Sm)
        .child(header)
        .child(body)
        .build()
        .into_any_element()
}

/// Render only the controls row (mode toggles + ref-channel selector). Use
/// when you want to host the graph separately from its controls (e.g. a
/// permanent graph row below a tab bar).
pub fn render_spatial_spider_controls(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    snapshot: &SpatialSpiderSnapshot,
    cfg_opt: Option<&'static SpeakerConfig>,
    text: PluginCommonTranslations,
    theme: &crate::theme::Theme,
) -> AnyElement {
    build_header(
        d,
        entity,
        plugin_idx,
        snapshot.ui.spider_mode,
        snapshot.ui.view_mode,
        snapshot.ui.correlation_ref_channel,
        snapshot.ui.ref_channel_select_open,
        cfg_opt,
        text,
        theme,
    )
}

/// Render only the graph (no controls). Use when the controls live elsewhere
/// (e.g. embedded in a plugin's tab content) and you want the visualization
/// to occupy its own row.
pub fn render_spatial_spider_graph(
    d: &Ds,
    snapshot: &SpatialSpiderSnapshot,
    cfg_opt: Option<&'static SpeakerConfig>,
    text: PluginCommonTranslations,
    theme: &crate::theme::Theme,
) -> AnyElement {
    build_body(
        d,
        snapshot,
        cfg_opt,
        snapshot.ui.view_mode,
        snapshot.ui.spider_mode,
        snapshot.ui.correlation_ref_channel,
        text,
        theme,
    )
}
