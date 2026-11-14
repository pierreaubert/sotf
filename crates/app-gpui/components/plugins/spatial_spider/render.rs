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

use super::{SpatialSpiderSnapshot, SpiderMode, SpiderViewMode};
use crate::app::AppState;
use crate::app::i18n::PluginCommonTranslations;
use crate::components::design::Ds;
use gpui::prelude::*;
use gpui::{AnyElement, Entity};
use gpui_ui_kit::{StackSpacing, VStack};
use sotf_plugins::speaker_config::SpeakerConfig;

mod build;
mod header_menu;
pub use header_menu::render_spatial_header_menu;
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
#[must_use]
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
#[must_use]
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
        snapshot.ui.ref_channel_highlight,
        cfg_opt,
        text,
        theme,
    )
}

/// Compact SPL/Correlation selector for hosts that expose view mode in their
/// own header. Keep the metric selector visible even before analyzer data arrives.
#[must_use]
pub fn render_spatial_spider_metric_controls(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    snapshot: &SpatialSpiderSnapshot,
    cfg_opt: Option<&'static SpeakerConfig>,
    text: PluginCommonTranslations,
    theme: &crate::theme::Theme,
) -> AnyElement {
    let mode = snapshot.ui.spider_mode;
    let reference = snapshot.ui.correlation_ref_channel;
    let mode_entity = entity.clone();
    let buttons = gpui_ui_kit::ButtonSet::new(("upmixer-metric", plugin_idx))
        .options(vec![
            gpui_ui_kit::ButtonSetOption::new("spl", "SPL"),
            gpui_ui_kit::ButtonSetOption::new("correlation", text.spatial.correlation_label),
        ])
        .selected(if matches!(mode, SpiderMode::Spl) {
            "spl"
        } else {
            "correlation"
        })
        .size(gpui_ui_kit::ButtonSetSize::Xs)
        .theme(theme.to_button_set_theme())
        .on_change(move |value, _, cx| {
            mode_entity.update(cx, |state, cx| {
                state.app.plugin_ui.spatial_spider.spider_mode = if value.as_ref() == "spl" {
                    SpiderMode::Spl
                } else {
                    SpiderMode::CorrelationFromRef {
                        ref_channel: reference,
                    }
                };
                cx.notify();
            });
        });
    #[cfg(feature = "dev-api")]
    let buttons = {
        use crate::app::dev_api::{DevElementState, DevTrackExt};
        buttons
            .option_wrapper(move |index, option| {
                let spl = index == 0;
                option
                    .dev_track_with_state(
                        format!("upmixer.metric.{}", if spl { "spl" } else { "correlation" }),
                        DevElementState::default().selected(spl == matches!(mode, SpiderMode::Spl)),
                    )
                    .into_any_element()
            })
            .dev_track("upmixer.metric.mode")
    };
    gpui::div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(d.gap)
        .child(buttons)
        .when(
            matches!(mode, SpiderMode::CorrelationFromRef { .. }),
            |row| {
                row.child(build::build_ref_channel_select(
                    d,
                    entity,
                    plugin_idx,
                    mode,
                    reference,
                    snapshot.ui.ref_channel_select_open,
                    snapshot.ui.ref_channel_highlight,
                    cfg_opt,
                    text,
                    theme,
                ))
            },
        )
        .into_any_element()
}

/// Render only the graph (no controls). Use when the controls live elsewhere
/// (e.g. embedded in a plugin's tab content) and you want the visualization
/// to occupy its own row.
#[must_use]
pub fn render_spatial_spider_graph(
    d: &Ds,
    snapshot: &SpatialSpiderSnapshot,
    cfg_opt: Option<&'static SpeakerConfig>,
    text: PluginCommonTranslations,
    theme: &crate::theme::Theme,
) -> AnyElement {
    if snapshot.ui.view_mode == SpiderViewMode::Off {
        return gpui::div().into_any_element();
    }
    let reference = cfg_opt.map_or(snapshot.ui.correlation_ref_channel, |config| {
        config
            .speakers
            .iter()
            .find(|s| !s.is_lfe && s.channel == snapshot.ui.correlation_ref_channel)
            .or_else(|| config.speakers.iter().find(|s| !s.is_lfe))
            .map_or(0, |s| s.channel)
    });
    let mode = match snapshot.ui.spider_mode {
        super::SpiderMode::Spl => super::SpiderMode::Spl,
        super::SpiderMode::CorrelationFromRef { .. } => super::SpiderMode::CorrelationFromRef {
            ref_channel: reference,
        },
    };
    build_body(
        d,
        snapshot,
        cfg_opt,
        snapshot.ui.view_mode,
        mode,
        reference,
        text,
        theme,
    )
}
