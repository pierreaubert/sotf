use super::super::data::ChannelMetric;
use super::super::{SpatialSpiderSnapshot, SpiderMode, SpiderViewMode};
#[cfg(feature = "gpu-3d")]
use super::misc::attach_orbit_handlers;
use super::spider_colors::SpiderColors;
use super::spider_disc2_d::SpiderDisc2D;
#[cfg(feature = "gpu-3d")]
use super::spider_view3_d::SpiderView3D;
use crate::app::AppState;
use crate::app::i18n::PluginCommonTranslations;
use crate::components::design::Ds;
use gpui::prelude::*;
use gpui::*;
#[cfg(feature = "gpu-3d")]
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant};
use gpui_ui_kit::{Select, SelectOption, SelectSize, Toggle, ToggleStyle};
use sotf_plugins::speaker_config::SpeakerConfig;

#[allow(clippy::too_many_arguments)]
pub(super) fn build_header(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    spider_mode: SpiderMode,
    view_mode: SpiderViewMode,
    ref_channel: usize,
    ref_channel_select_open: bool,
    cfg_opt: Option<&'static SpeakerConfig>,
    text: PluginCommonTranslations,
    theme: &crate::theme::Theme,
) -> AnyElement {
    let e_2d = entity.clone();
    #[cfg(feature = "gpu-3d")]
    let e_3d = entity.clone();
    #[cfg(feature = "gpu-3d")]
    let e_reset = entity.clone();
    let e_spl = entity.clone();
    let e_corr = entity.clone();

    let header = div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(d.gap_md)
        .child(
            div()
                .flex_none()
                .text_size(d.text_sm)
                .font_weight(FontWeight::BOLD)
                .text_color(theme.text_muted)
                .child(text.spatial_view_label),
        )
        .child(
            // TODO: this widget today reflects the *chain output* (the last
            // permanent LoudnessMonitor), not the host plugin's own output.
            // When per-plugin analyzer hooks land, replace this label with
            // the host plugin's name so the source is unambiguous.
            div()
                .flex_none()
                .text_size(d.text_sm)
                .text_color(theme.text_muted)
                .whitespace_nowrap()
                .child("(chain out)".to_string()),
        )
        .child(
            Toggle::new(("spider-view-2d", plugin_idx))
                .checked(view_mode == SpiderViewMode::Disc2D)
                .style(ToggleStyle::Segmented)
                .theme(theme.to_toggle_theme())
                .aria_label(text.spatial_view_2d)
                .on_change(move |checked, _, cx| {
                    if checked {
                        e_2d.update(cx, |st, cx| {
                            st.app.plugin_ui.spatial_spider.view_mode = SpiderViewMode::Disc2D;
                            cx.notify();
                        });
                    }
                }),
        )
        .child(
            div()
                .flex_none()
                .text_size(d.text_sm)
                .text_color(theme.text_secondary)
                .child("2D".to_string()),
        );

    #[cfg(feature = "gpu-3d")]
    let header = header
        .child(
            Toggle::new(("spider-view-3d", plugin_idx))
                .checked(view_mode == SpiderViewMode::View3D)
                .style(ToggleStyle::Segmented)
                .theme(theme.to_toggle_theme())
                .aria_label(text.spatial_view_3d)
                .on_change(move |checked, _, cx| {
                    if checked {
                        e_3d.update(cx, |st, cx| {
                            st.app.plugin_ui.spatial_spider.view_mode = SpiderViewMode::View3D;
                            cx.notify();
                        });
                    }
                }),
        )
        .child(
            div()
                .flex_none()
                .text_size(d.text_sm)
                .text_color(theme.text_secondary)
                .child("3D".to_string()),
        );

    #[cfg(feature = "gpu-3d")]
    let header = header.when(view_mode == SpiderViewMode::View3D, |header| {
        header
            .child(
                div()
                    .text_size(d.text_xs)
                    .text_color(theme.text_muted)
                    .child(text.spatial_orbit_hint),
            )
            .child(
                Button::new(
                    ("spider-reset-camera", plugin_idx),
                    text.spatial_reset_camera,
                )
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Xs)
                .theme(theme.to_button_theme())
                .on_click_event(move |_, _, cx| {
                    e_reset.update(cx, |state, cx| {
                        state.app.plugin_ui.spatial_spider.reset_camera();
                        cx.notify();
                    });
                }),
            )
    });

    header
        .child(
            div()
                .flex_none()
                .w(rems(0.0625))
                .h(rems(0.875))
                .bg(theme.border),
        )
        .child(
            Toggle::new(("spider-mode-spl", plugin_idx))
                .checked(matches!(spider_mode, SpiderMode::Spl))
                .style(ToggleStyle::Segmented)
                .theme(theme.to_toggle_theme())
                .aria_label(text.spatial_spl_mode)
                .on_change(move |checked, _, cx| {
                    if checked {
                        e_spl.update(cx, |st, cx| {
                            st.app.plugin_ui.spatial_spider.spider_mode = SpiderMode::Spl;
                            cx.notify();
                        });
                    }
                }),
        )
        .child(
            div()
                .flex_none()
                .text_size(d.text_sm)
                .text_color(theme.text_secondary)
                .child("SPL".to_string()),
        )
        .child(
            Toggle::new(("spider-mode-corr", plugin_idx))
                .checked(matches!(spider_mode, SpiderMode::CorrelationFromRef { .. }))
                .style(ToggleStyle::Segmented)
                .theme(theme.to_toggle_theme())
                .aria_label(text.spatial_correlation_mode)
                .on_change({
                    let ref_ch = ref_channel;
                    move |checked, _, cx| {
                        if checked {
                            e_corr.update(cx, |st, cx| {
                                st.app.plugin_ui.spatial_spider.spider_mode =
                                    SpiderMode::CorrelationFromRef {
                                        ref_channel: ref_ch,
                                    };
                                cx.notify();
                            });
                        }
                    }
                }),
        )
        .child(
            div()
                .flex_none()
                .text_size(d.text_sm)
                .text_color(theme.text_secondary)
                .child(text.spatial_correlation_label),
        )
        .child(build_ref_channel_select(
            d,
            entity,
            plugin_idx,
            spider_mode,
            ref_channel,
            ref_channel_select_open,
            cfg_opt,
            text,
            theme,
        ))
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_ref_channel_select(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    spider_mode: SpiderMode,
    ref_channel: usize,
    is_open: bool,
    cfg_opt: Option<&'static SpeakerConfig>,
    text: PluginCommonTranslations,
    theme: &crate::theme::Theme,
) -> AnyElement {
    let active = matches!(spider_mode, SpiderMode::CorrelationFromRef { .. });
    let cfg = match cfg_opt {
        Some(c) => c,
        None => return div().w(rems(0.0)).into_any_element(),
    };
    let options: Vec<SelectOption> = cfg
        .speakers
        .iter()
        .filter(|s| !s.is_lfe)
        .map(|s| SelectOption::new(s.label.to_string(), s.label.to_string()))
        .collect();
    let selected_label = cfg
        .speakers
        .iter()
        .find(|s| s.channel == ref_channel && !s.is_lfe)
        .map(|s| s.label.to_string())
        .unwrap_or_else(|| {
            cfg.speakers
                .iter()
                .find(|s| !s.is_lfe)
                .map(|s| s.label.to_string())
                .unwrap_or_default()
        });

    div()
        .flex()
        .items_center()
        .gap(d.gap)
        .when(!active, |el| el.opacity(0.4))
        .child(
            div()
                .text_size(d.text_sm)
                .text_color(theme.text_secondary)
                .child(text.spatial_reference_channel),
        )
        .child(
            Select::new(("spider-ref-channel", plugin_idx))
                .options(options)
                .selected(selected_label)
                .disabled(!active)
                .is_open(is_open)
                .size(SelectSize::Xs)
                .theme(theme.to_select_theme())
                .aria_label(text.spatial_reference_channel)
                .on_toggle({
                    let entity = entity.downgrade();
                    move |open, _window, cx| {
                        let Some(entity) = entity.upgrade() else {
                            return;
                        };
                        entity.update(cx, |st, cx| {
                            st.app.plugin_ui.spatial_spider.ref_channel_select_open = open;
                            cx.notify();
                        });
                    }
                })
                .on_change({
                    let entity = entity.clone();
                    move |value, _window, cx| {
                        let picked = cfg
                            .speakers
                            .iter()
                            .find(|s| s.label == value.as_ref() && !s.is_lfe)
                            .map(|s| s.channel)
                            .unwrap_or(0);
                        entity.update(cx, |st, cx| {
                            st.app.plugin_ui.spatial_spider.correlation_ref_channel = picked;
                            if let SpiderMode::CorrelationFromRef { .. } =
                                st.app.plugin_ui.spatial_spider.spider_mode
                            {
                                st.app.plugin_ui.spatial_spider.spider_mode =
                                    SpiderMode::CorrelationFromRef {
                                        ref_channel: picked,
                                    };
                            }
                            // Close the dropdown after a selection so the
                            // next click reopens it cleanly.
                            st.app.plugin_ui.spatial_spider.ref_channel_select_open = false;
                            cx.notify();
                        });
                    }
                }),
        )
        .into_any_element()
}

pub(super) fn build_body(
    d: &Ds,
    snapshot: &SpatialSpiderSnapshot,
    cfg_opt: Option<&'static SpeakerConfig>,
    view_mode: SpiderViewMode,
    spider_mode: SpiderMode,
    ref_channel: usize,
    text: PluginCommonTranslations,
    theme: &crate::theme::Theme,
) -> AnyElement {
    let cfg = match cfg_opt {
        None => {
            return div()
                .h(rems(17.5))
                .w_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_size(d.text_xs)
                        .text_color(theme.text_muted)
                        .child(text.spatial_no_layout),
                )
                .into_any_element();
        }
        Some(c) => c,
    };
    let loudness = snapshot.loudness.as_deref();
    let n = cfg.total_channels;
    if loudness.is_none() {
        return div()
            .h(rems(17.5))
            .w_full()
            .flex()
            .items_center()
            .justify_center()
            .text_size(d.text_xs)
            .text_color(theme.text_muted)
            .child(text.spatial_waiting_data)
            .into_any_element();
    }
    if matches!(spider_mode, SpiderMode::CorrelationFromRef { .. })
        && loudness.is_some_and(|data| data.correlation_samples_seen == 0)
    {
        return div()
            .h(rems(17.5))
            .w_full()
            .flex()
            .items_center()
            .justify_center()
            .text_size(d.text_xs)
            .text_color(theme.text_muted)
            .child(text.spatial_waiting_correlation)
            .into_any_element();
    }

    // SPL buffer.
    let metric_buf: Vec<f64> = match spider_mode {
        SpiderMode::Spl => loudness
            .map(|li| li.true_peaks_dbtp.iter().copied().collect())
            .unwrap_or_else(|| vec![f64::NEG_INFINITY; n]),
        SpiderMode::CorrelationFromRef { .. } => Vec::new(),
    };
    // Correlation row.
    let corr_row: Vec<f32> = match (spider_mode, loudness) {
        (SpiderMode::CorrelationFromRef { ref_channel: rc }, Some(li))
            if !li.correlation_matrix.is_empty() && li.correlation_samples_seen > 0 =>
        {
            let mc = li.correlation_matrix.len();
            let n_ch = (mc as f64).sqrt() as usize;
            if n_ch * n_ch == mc && rc < n_ch {
                let row_start = rc * n_ch;
                let row_end = row_start + n_ch;
                li.correlation_matrix
                    .get(row_start..row_end)
                    .map(|s| s.to_vec())
                    .unwrap_or_else(|| vec![0.0; n])
            } else {
                vec![0.0; n]
            }
        }
        (SpiderMode::CorrelationFromRef { .. }, _) => vec![0.0; n],
        _ => Vec::new(),
    };
    let metric = match spider_mode {
        SpiderMode::Spl => ChannelMetric::Spl(&metric_buf),
        SpiderMode::CorrelationFromRef { .. } => ChannelMetric::Correlation(&corr_row),
    };

    let palette = SpiderColors::from_theme(theme);
    let highlight =
        matches!(spider_mode, SpiderMode::CorrelationFromRef { .. }).then_some(ref_channel);
    // Container fixes both dimensions explicitly so the child's
    // `relative(1.0)` request resolves to a non-zero rect. Going through a
    // flex parent has bitten us before — `relative(1.0)` width inside a
    // flex row without an explicit flex_basis collapses to 0.
    let container = || div().h(rems(20.0)).w_full();
    match view_mode {
        SpiderViewMode::Disc2D => container()
            .child(
                SpiderDisc2D::new(cfg, metric)
                    .colors(palette)
                    .highlight_channel(highlight),
            )
            .into_any_element(),
        #[cfg(feature = "gpu-3d")]
        SpiderViewMode::View3D => {
            // Wrap the 3D element in an interactive container so mouse
            // events drive the OrbitControls. State is shared via Rc so
            // every event handler mutates the same camera.
            let camera_state = snapshot.ui.camera_3d.clone();
            attach_orbit_handlers(container().id("spider-3d-viewport"), camera_state.clone())
                .child(
                    SpiderView3D::new(cfg, metric, camera_state)
                        .colors(palette)
                        .vertical_color(theme.warning),
                )
                .into_any_element()
        }
        #[cfg(not(feature = "gpu-3d"))]
        SpiderViewMode::View3D => container()
            .child(
                SpiderDisc2D::new(cfg, metric)
                    .colors(palette)
                    .highlight_channel(highlight),
            )
            .into_any_element(),
    }
}
