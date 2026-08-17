use super::types::CustomViewRenderContext;
use crate::app::i18n::PluginCommonTranslations;
use crate::app::state::{
    ExternalPluginWorkerHealth, external_plugin_error_key, external_plugin_worker_health,
};
use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_ui_kit::{HStack, StackSpacing, Text, VStack};
use sotf_audio_player::{
    PluginSettings, UpmixerAmbientAnalysisSettings, UpmixerBypassSettings,
    UpmixerDecorrelationSettings, UpmixerDialogueSettings, UpmixerGainSettings,
    UpmixerHeightSettings, UpmixerLfeSettings, UpmixerOutputSettings, UpmixerSubharmonicSettings,
};

pub(super) fn render_eq(ctx: &CustomViewRenderContext, cx: &mut Context<PlayerView>) -> AnyElement {
    use super::super::ui_eq;
    if let PluginSettings::EQ {
        channels,
        filters,
        channel_filters,
        per_channel_mode,
        max_filters,
        tdf2,
        ..
    } = ctx.settings
    {
        let selected_band_idx = ctx.selected_band_idx.min(filters.len().saturating_sub(1));
        super::super::render_eq_plugin(
            ctx.entity.clone(),
            ctx.plugin_idx,
            ui_eq::EqRenderState {
                channels: *channels,
                filters,
                channel_filters,
                per_channel_mode: *per_channel_mode,
                is_editing: ctx.is_editing,
                selected_param: ctx.selected_param,
                selected_band_idx,
                midi_overlay: ctx.midi_overlay,
                mode: ui_eq::EqViewMode::Standard,
                num_filters: *max_filters,
                tdf2: *tdf2,
                available_width: ctx.available_width,
                layout_scale: ctx.layout_scale,
            },
            ctx.theme,
            ctx.eq_chart_focus_handle.clone(),
            cx,
        )
        .into_any_element()
    } else {
        Empty.into_any_element()
    }
}

pub(super) fn render_external(
    ctx: &CustomViewRenderContext,
    cx: &mut Context<PlayerView>,
) -> AnyElement {
    let PluginSettings::External { state } = ctx.settings else {
        return Empty.into_any_element();
    };
    let d = Ds::from_cx(cx);
    let descriptor = &state.descriptor;
    let error_key = external_plugin_error_key(descriptor);
    let (language, build_error, load_error, worker_status) = {
        let app_state = ctx.entity.read(cx);
        let plugin_instance_id = ctx
            .plugin_graph
            .get_plugin(ctx.plugin_idx)
            .map(|plugin| plugin.id);
        let engine_index = ctx
            .plugin_graph
            .get_engine_index_by_linear_position(ctx.plugin_idx);
        let plugin_state = &app_state.app.plugin_state;
        let ui = &plugin_state.external_plugin_ui;
        (
            app_state.app.ui_state.language,
            plugin_state
                .external_plugin_build_diagnostic(plugin_instance_id, engine_index)
                .map(|diagnostic| diagnostic.message.clone()),
            plugin_instance_id
                .and_then(|instance_id| ui.worker_errors.get(&instance_id).cloned())
                .or_else(|| ui.load_errors.get(&error_key).cloned()),
            ui.worker_statuses
                .iter()
                .find(|status| {
                    (plugin_instance_id.is_some()
                        && status.plugin_instance_id == plugin_instance_id)
                        || (status.plugin_instance_id.is_none()
                            && Some(status.plugin_index) == engine_index)
                })
                .cloned(),
        )
    };
    let text = crate::app::i18n::SettingsSurfaceTranslations::for_language(language).external;
    let format = match descriptor.format {
        sotf_plugins::PluginFormat::Clap => "CLAP",
        sotf_plugins::PluginFormat::Vst3 => "VST3",
        sotf_plugins::PluginFormat::AudioUnit => "AU",
    };
    let (hosting, hosting_color) = match state.sandbox_mode {
        sotf_plugins::ExternalPluginSandboxMode::Isolated => (text.isolated, ctx.theme.success),
        sotf_plugins::ExternalPluginSandboxMode::InProcess
        | sotf_plugins::ExternalPluginSandboxMode::Disabled => (text.disabled, ctx.theme.warning),
    };

    let worker_status_view = worker_status.as_ref().map(|status| {
        let health_color = match external_plugin_worker_health(status) {
            ExternalPluginWorkerHealth::Healthy => ctx.theme.success,
            ExternalPluginWorkerHealth::Degraded => ctx.theme.warning,
            ExternalPluginWorkerHealth::Failed => ctx.theme.error,
        };
        VStack::new()
            .spacing(StackSpacing::Xs)
            .child(
                Text::label(format!(
                    "{}: {}",
                    text.worker.status,
                    text.worker.event_label(status.event.as_ref()),
                ))
                .color(health_color),
            )
            .child(Text::caption(text.worker.counters(status)).color(ctx.theme.text_secondary))
            .child(Text::caption(text.worker.sandbox_label(status)).color(ctx.theme.text_muted))
            .when_some(status.sandbox_reason.as_ref(), |stack, reason| {
                stack.child(Text::caption(reason.clone()).color(ctx.theme.warning))
            })
            .build()
            .into_any_element()
    });

    VStack::new()
        .spacing(StackSpacing::Sm)
        .child(Text::section_header(descriptor.name.clone()).color(ctx.theme.text_primary))
        .child(
            Text::body(format!(
                "{} · {} · {}",
                descriptor.vendor, descriptor.version, format
            ))
            .color(ctx.theme.text_secondary),
        )
        .child(
            HStack::new()
                .spacing(StackSpacing::Md)
                .child(
                    Text::label(format!(
                        "{}: {} → {}",
                        text.channels, descriptor.audio_inputs, descriptor.audio_outputs
                    ))
                    .color(ctx.theme.text_secondary),
                )
                .child(Text::label(hosting).color(hosting_color))
                .build(),
        )
        .child(
            Text::caption(format!("{}: {}", text.path, descriptor.path.display()))
                .color(ctx.theme.text_muted),
        )
        .child(
            Text::caption(format!(
                "{}: {}",
                text.saved_state,
                state.opaque_state.len()
            ))
            .color(ctx.theme.text_muted),
        )
        .when_some(worker_status_view, |stack, status| stack.child(status))
        .when_some(load_error, |stack, error| {
            stack.child(
                div()
                    .p(d.pad_y)
                    .rounded(d.r_sm)
                    .bg(ctx.theme.error.opacity(0.12))
                    .text_size(d.text_xs)
                    .text_color(ctx.theme.error)
                    .child(format!("{}: {error}", text.runtime_error)),
            )
        })
        .when_some(build_error, |stack, error| {
            stack.child(
                div()
                    .p(d.pad_y)
                    .rounded(d.r_sm)
                    .bg(ctx.theme.warning.opacity(0.12))
                    .text_size(d.text_xs)
                    .text_color(ctx.theme.warning)
                    .child(format!("{}: {error}", text.host_diagnostic)),
            )
        })
        .child(Text::body(text.parameters_unavailable).color(ctx.theme.warning))
        .build()
        .into_any_element()
}

pub(super) fn render_dynamic_eq(
    ctx: &CustomViewRenderContext,
    cx: &mut Context<PlayerView>,
) -> AnyElement {
    super::super::render_dynamic_eq_plugin(
        ctx.entity.clone(),
        ctx.plugin_idx,
        ctx.settings,
        ctx.is_editing,
        ctx.selected_param,
        ctx.selected_band_idx,
        ctx.plugin_data.clone(),
        ctx.available_width,
        ctx.layout_scale,
        ctx.theme,
        cx,
    )
    .into_any_element()
}

/// Render the denoiser's live noise-estimation surface.
///
/// The declarative layout still owns all parameter controls, including the
/// Learn/Clear profile actions. This view only adds the monitoring surface
/// that cannot be represented by `PluginLayout`: compact per-band noise-floor
/// and SNR strips plus profile/reduction status.
pub(super) fn render_denoiser(
    ctx: &CustomViewRenderContext,
    cx: &mut Context<PlayerView>,
) -> AnyElement {
    let d = Ds::from_cx(cx);
    let text = PluginCommonTranslations::for_language(ctx.entity.read(cx).app.ui_state.language);
    let data = ctx
        .plugin_data
        .as_ref()
        .and_then(|value| value.downcast_ref::<sotf_plugins::DenoiserData>());

    let mut root = div().flex().flex_col().gap(d.section).w_full().min_w_0();

    if let Some(data) = data {
        let profile_state = if data.is_learning_noise {
            format!(
                "{} · {} {:.0}%",
                text.denoiser_profile,
                text.denoiser_learning,
                data.learning_progress * 100.0
            )
        } else if data.has_captured_profile {
            format!(
                "{} · {}",
                text.denoiser_profile,
                if data.using_captured_profile {
                    text.denoiser_active
                } else {
                    text.denoiser_captured
                }
            )
        } else {
            format!("{} · {}", text.denoiser_profile, text.denoiser_none)
        };

        root = root.child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap(d.gap)
                .text_size(d.text_sm)
                .text_color(ctx.theme.text_secondary)
                .child(profile_state)
                .child(format!(
                    "{}: {:.1} dB",
                    text.denoiser_reduction, data.avg_reduction_db
                )),
        );
        root = root.child(render_denoiser_band_row(
            &d,
            text.denoiser_noise_floor,
            data.noise_floor_db.as_slice(),
            -80.0,
            0.0,
            ctx.theme.info,
            ctx.theme,
        ));
        root = root.child(render_denoiser_band_row(
            &d,
            text.denoiser_snr,
            data.snr_db.as_slice(),
            0.0,
            40.0,
            ctx.theme.success,
            ctx.theme,
        ));
    } else {
        root = root.child(
            div()
                .w_full()
                .py(d.card)
                .flex()
                .justify_center()
                .text_size(d.text_sm)
                .text_color(ctx.theme.text_muted)
                .child(text.spatial_waiting_data),
        );
    }

    root.child(
        super::super::ui_layout_renderer::render_main_controls_from_layout(
            &d,
            ctx.entity.clone(),
            ctx.plugin_idx,
            ctx.settings,
            ctx.is_editing,
            ctx.selected_param,
            ctx.plugin_data.as_ref(),
            ctx.available_width,
            ctx.layout_scale,
            ctx.theme,
        ),
    )
    .into_any_element()
}

fn render_denoiser_band_row(
    d: &Ds,
    label: &'static str,
    values: &[f32],
    min_value: f32,
    max_value: f32,
    color: gpui::Rgba,
    theme: &crate::theme::Theme,
) -> AnyElement {
    let mut bars = div()
        .flex()
        .items_end()
        .gap(d.half_grid)
        .h(rems(5.0))
        .w_full()
        .min_w_0();
    for value in values.iter().take(64) {
        let normalized = if value.is_finite() {
            ((*value - min_value) / (max_value - min_value)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        bars = bars.child(
            div().flex_1().min_w_0().h_full().flex().items_end().child(
                div()
                    .w_full()
                    .h(rems(0.25 + normalized * 4.25))
                    .rounded(d.r_sm)
                    .bg(color),
            ),
        );
    }

    div()
        .flex()
        .flex_col()
        .gap(d.grid)
        .child(
            div()
                .text_size(d.text_xs)
                .text_color(theme.text_muted)
                .child(label),
        )
        .child(bars)
        .into_any_element()
}

fn render_main_controls(ctx: &CustomViewRenderContext, d: &Ds) -> AnyElement {
    super::super::ui_layout_renderer::render_main_controls_from_layout(
        d,
        ctx.entity.clone(),
        ctx.plugin_idx,
        ctx.settings,
        ctx.is_editing,
        ctx.selected_param,
        ctx.plugin_data.as_ref(),
        ctx.available_width,
        ctx.layout_scale,
        ctx.theme,
    )
}

pub(super) fn render_convolution(
    ctx: &CustomViewRenderContext,
    cx: &mut Context<PlayerView>,
) -> AnyElement {
    let d = Ds::from_cx(cx);
    let text = PluginCommonTranslations::for_language(ctx.entity.read(cx).app.ui_state.language);
    let PluginSettings::Convolution {
        ir_file,
        zero_latency_head,
        head_taps,
        ..
    } = ctx.settings
    else {
        return Empty.into_any_element();
    };

    let has_ir = !ir_file.trim().is_empty();
    let status = if has_ir {
        text.label("Impulse response selected")
    } else {
        text.label("Choose or drop an impulse-response WAV file")
    };
    let latency = if *zero_latency_head {
        format!("Zero-latency head · {head_taps} taps")
    } else {
        format!("Partitioned FFT · {head_taps} head taps")
    };

    div()
        .flex()
        .flex_col()
        .gap(d.section)
        .w_full()
        .min_w_0()
        .child(
            div()
                .flex()
                .flex_col()
                .gap(d.gap)
                .p(d.card)
                .rounded(d.r_md)
                .border_1()
                .border_color(if has_ir {
                    ctx.theme.success
                } else {
                    ctx.theme.border
                })
                .bg(ctx.theme.surface)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(d.gap)
                        .text_size(d.text_sm)
                        .text_color(ctx.theme.text_primary)
                        .child(text.convolution_ir)
                        .child(
                            div()
                                .text_size(d.text_xs)
                                .text_color(if has_ir {
                                    ctx.theme.success
                                } else {
                                    ctx.theme.text_muted
                                })
                                .child(status),
                        ),
                )
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .py(d.pad_y)
                        .text_size(d.text_xs)
                        .text_color(ctx.theme.text_secondary)
                        .child(if has_ir {
                            ir_file.clone()
                        } else {
                            text.label("Use the file picker below to load an IR")
                                .to_string()
                        }),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(d.gap)
                        .text_size(d.text_xs)
                        .text_color(ctx.theme.text_muted)
                        .child(latency)
                        .child(format!("· {}", text.convolution_preview)),
                ),
        )
        .child(render_main_controls(ctx, &d))
        .into_any_element()
}

pub(super) fn render_xtc(
    ctx: &CustomViewRenderContext,
    cx: &mut Context<PlayerView>,
) -> AnyElement {
    let d = Ds::from_cx(cx);
    let text = PluginCommonTranslations::for_language(ctx.entity.read(cx).app.ui_state.language);
    let PluginSettings::XTC {
        distance_m,
        speaker_angle_deg,
        head_offset_x,
        head_offset_z,
        head_yaw_deg,
        ..
    } = ctx.settings
    else {
        return Empty.into_any_element();
    };

    let node = |label: &'static str, color: gpui::Rgba| {
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(d.half_grid)
            .text_size(d.text_xs)
            .text_color(ctx.theme.text_secondary)
            .child(div().size(rems(1.75)).rounded_full().bg(color))
            .child(label)
    };

    div()
        .flex()
        .flex_col()
        .gap(d.section)
        .w_full()
        .min_w_0()
        .child(
            div()
                .flex()
                .flex_col()
                .gap(d.gap)
                .p(d.card)
                .rounded(d.r_md)
                .border_1()
                .border_color(ctx.theme.border)
                .bg(ctx.theme.surface)
                .child(
                    div()
                        .text_size(d.text_sm)
                        .text_color(ctx.theme.text_primary)
                        .child(text.xtc_geometry),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(d.gap)
                        .w_full()
                        .py(d.card)
                        .child(node("Left speaker", ctx.theme.accent))
                        .child(div().flex_1().h(d.half_grid).bg(ctx.theme.border).min_w_0())
                        .child(node("Listener", ctx.theme.info))
                        .child(div().flex_1().h(d.half_grid).bg(ctx.theme.border).min_w_0())
                        .child(node("Right speaker", ctx.theme.accent)),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(d.gap)
                        .text_size(d.text_xs)
                        .text_color(ctx.theme.text_secondary)
                        .child(format!("{}: {distance_m:.2} m", text.label("Distance")))
                        .child(format!("{}: {speaker_angle_deg:.1}°", text.label("Angle")))
                        .child(format!(
                            "{}: {head_offset_x:.2} / {head_offset_z:.2} m",
                            text.label("Head offset")
                        ))
                        .child(format!("{}: {head_yaw_deg:.1}°", text.label("Yaw"))),
                ),
        )
        .child(render_main_controls(ctx, &d))
        .into_any_element()
}

pub(super) fn render_crossfeed(
    ctx: &CustomViewRenderContext,
    cx: &mut Context<PlayerView>,
) -> AnyElement {
    let d = Ds::from_cx(cx);
    let text = PluginCommonTranslations::for_language(ctx.entity.read(cx).app.ui_state.language);
    let PluginSettings::Crossfeed {
        enabled,
        mix,
        itd_delay_ms,
        ..
    } = ctx.settings
    else {
        return Empty.into_any_element();
    };

    let status = if *enabled { "Enabled" } else { "Bypassed" };
    div()
        .flex()
        .flex_col()
        .gap(d.section)
        .w_full()
        .min_w_0()
        .child(
            div()
                .flex()
                .flex_col()
                .gap(d.gap)
                .p(d.card)
                .rounded(d.r_md)
                .border_1()
                .border_color(if *enabled {
                    ctx.theme.accent
                } else {
                    ctx.theme.border
                })
                .bg(ctx.theme.surface)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .text_size(d.text_sm)
                        .text_color(ctx.theme.text_primary)
                        .child(text.crossfeed_signal)
                        .child(
                            div()
                                .text_size(d.text_xs)
                                .text_color(if *enabled {
                                    ctx.theme.success
                                } else {
                                    ctx.theme.text_muted
                                })
                                .child(status),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(d.gap)
                        .w_full()
                        .py(d.card)
                        .child(div().size(rems(1.5)).rounded_full().bg(ctx.theme.accent))
                        .child(
                            div()
                                .flex_1()
                                .h(d.half_grid)
                                .bg(ctx.theme.accent.opacity(0.55))
                                .min_w_0(),
                        )
                        .child(div().size(rems(1.5)).rounded_full().bg(ctx.theme.accent))
                        .child(
                            div()
                                .text_size(d.text_xs)
                                .text_color(ctx.theme.text_secondary)
                                .child(format!(
                                    "{:.0}% mix · {itd_delay_ms:.1} ms ITD",
                                    mix * 100.0
                                )),
                        ),
                ),
        )
        .child(render_main_controls(ctx, &d))
        .into_any_element()
}

pub(super) fn render_binaural(
    ctx: &CustomViewRenderContext,
    cx: &mut Context<PlayerView>,
) -> AnyElement {
    let d = Ds::from_cx(cx);
    let text = PluginCommonTranslations::for_language(ctx.entity.read(cx).app.ui_state.language);
    let PluginSettings::BinauralDecoder {
        sofa_file,
        input_channels,
        externalization,
        near_field_strength,
        ..
    } = ctx.settings
    else {
        return Empty.into_any_element();
    };

    let has_sofa = !sofa_file.trim().is_empty();
    div()
        .flex()
        .flex_col()
        .gap(d.section)
        .w_full()
        .min_w_0()
        .child(
            div()
                .flex()
                .flex_col()
                .gap(d.gap)
                .p(d.card)
                .rounded(d.r_md)
                .border_1()
                .border_color(if has_sofa {
                    ctx.theme.success
                } else {
                    ctx.theme.border
                })
                .bg(ctx.theme.surface)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(d.gap)
                        .text_size(d.text_sm)
                        .text_color(ctx.theme.text_primary)
                        .child(text.binaural_hrtf)
                        .child(
                            div()
                                .text_size(d.text_xs)
                                .text_color(if has_sofa {
                                    ctx.theme.success
                                } else {
                                    ctx.theme.warning
                                })
                                .child(if has_sofa {
                                    "SOFA loaded"
                                } else {
                                    "Select a SOFA file"
                                }),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(d.gap)
                        .text_size(d.text_xs)
                        .text_color(ctx.theme.text_secondary)
                        .child(format!("{input_channels} input channels"))
                        .child(format!("Externalization: {:.0}%", externalization * 100.0))
                        .child(format!("Near-field: {:.0}%", near_field_strength * 100.0)),
                )
                .when(has_sofa, |panel| {
                    panel.child(
                        div()
                            .w_full()
                            .min_w_0()
                            .text_size(d.text_xs)
                            .text_color(ctx.theme.text_muted)
                            .child(sofa_file.clone()),
                    )
                }),
        )
        .child(render_main_controls(ctx, &d))
        .into_any_element()
}

pub(super) fn render_linear_phase_eq(
    ctx: &CustomViewRenderContext,
    cx: &mut Context<PlayerView>,
) -> AnyElement {
    use super::super::ui_eq;
    if let PluginSettings::LinearPhaseEq {
        num_filters,
        fir_length,
        phase_mode,
        auto_gain,
        mix,
        filters,
        ..
    } = ctx.settings
    {
        let fir_len_samples: usize = match *fir_length as usize {
            0 => 1024,
            1 => 2048,
            2 => 4096,
            3 => 8192,
            _ => 2048,
        };
        let phase_mode_label = match *phase_mode as usize {
            1 => "Minimum",
            _ => "Linear",
        };
        let latency_samples = if phase_mode_label == "Linear" {
            fir_len_samples.saturating_sub(1) / 2
        } else {
            0
        };
        let sample_rate = ctx
            .entity
            .read(cx)
            .app
            .audio_device_state
            .hal_config
            .sample_rate
            .max(1);
        let latency_ms = (latency_samples as f32) * 1000.0 / (sample_rate as f32);

        let selected_band_idx = ctx.selected_band_idx.min(filters.len().saturating_sub(1));
        super::super::render_eq_plugin(
            ctx.entity.clone(),
            ctx.plugin_idx,
            ui_eq::EqRenderState {
                channels: 2,
                filters,
                channel_filters: &None,
                per_channel_mode: false,
                is_editing: ctx.is_editing,
                selected_param: ctx.selected_param,
                selected_band_idx,
                midi_overlay: ctx.midi_overlay,
                mode: ui_eq::EqViewMode::LinearPhase {
                    latency_samples,
                    latency_ms,
                    fir_length: fir_len_samples,
                    phase_mode: phase_mode_label,
                    auto_gain: *auto_gain,
                    mix: *mix,
                },
                num_filters: *num_filters as usize,
                tdf2: false,
                available_width: ctx.available_width,
                layout_scale: ctx.layout_scale,
            },
            ctx.theme,
            ctx.eq_chart_focus_handle.clone(),
            cx,
        )
        .into_any_element()
    } else {
        Empty.into_any_element()
    }
}

pub(super) fn render_spectrum(
    ctx: &CustomViewRenderContext,
    cx: &mut Context<PlayerView>,
) -> AnyElement {
    use super::super::ui_spectrum;
    let d = Ds::from_cx(cx);
    let text = crate::app::i18n::SpectrumTranslations::for_language(
        ctx.entity.read(cx).app.ui_state.language,
    );
    if let PluginSettings::SpectrumAnalyzer {
        num_bins,
        min_freq,
        max_freq,
        smoothing,
        tilt_correction,
        tilt_reference,
    } = ctx.settings
    {
        super::super::render_spectrum_analyzer_plugin(
            &d,
            ctx.entity.clone(),
            ctx.plugin_idx,
            ui_spectrum::SpectrumRenderState {
                num_bins: *num_bins,
                min_freq: *min_freq,
                max_freq: *max_freq,
                smoothing: *smoothing,
                tilt_correction: *tilt_correction,
                tilt_reference: *tilt_reference,
                tilt_select_open: ctx.spectrum_tilt_select_open,
                reference_select_open: ctx.spectrum_reference_select_open,
                is_editing: ctx.is_editing,
                selected_param: ctx.selected_param,
                data: ctx.plugin_data.as_ref().and_then(|d| d.downcast_ref()),
                chart_height: {
                    let state = ctx.entity.read(cx);
                    200.0
                        * crate::ui::compute_combined_scale(
                            state.app.ui_state.window_width,
                            state.app.ui_state.window_height,
                            state.app.ui_state.font_scale,
                            state.app.ui_state.min_font_size_px,
                            state.app.ui_state.max_font_size_px,
                        )
                },
                available_width: ctx.available_width,
            },
            text,
            ctx.theme,
        )
        .into_any_element()
    } else {
        Empty.into_any_element()
    }
}

pub(super) fn render_upmixer(
    ctx: &CustomViewRenderContext,
    cx: &mut Context<PlayerView>,
) -> AnyElement {
    use super::super::ui_upmixer;
    if let PluginSettings::Upmixer {
        speaker_config,
        gains:
            UpmixerGainSettings {
                gain_front_direct,
                gain_front_ambient,
                gain_rear_ambient,
                height_gain,
                stereo_width,
                center_spread,
                surround_direct_bleed,
                rear_late_reflection,
                ambient_boost,
                rear_ambient_boost,
            },
        lfe:
            UpmixerLfeSettings {
                lfe_cutoff_hz,
                lfe_gain,
                bandpass_hz,
            },
        subharmonic:
            UpmixerSubharmonicSettings {
                enable_subharmonic_synth,
                subharmonic_gain,
                subharmonic_freq_hz,
                subharmonic_attack_ms,
                subharmonic_release_ms,
            },
        decorrelation:
            UpmixerDecorrelationSettings {
                decorrelation_mode,
                decorrelation_lfo_rate_hz,
                velvet_noise_duration_ms,
                velvet_noise_density,
            },
        height:
            UpmixerHeightSettings {
                enable_hr_direct,
                hr_sharpen,
                height_hf_cap_hz,
                height_transient_reduction,
                height_direct_leak,
            },
        ambient_analysis:
            UpmixerAmbientAnalysisSettings {
                safety_cap_db,
                low_latency,
                frequency_resolution,
            },
        dialogue:
            UpmixerDialogueSettings {
                dialogue_weight,
                voice_freq_min_hz,
                voice_freq_max_hz,
                dialogue_centroid_weight,
                dialogue_variance_weight,
                dialogue_coherence_weight,
            },
        bypass:
            UpmixerBypassSettings {
                bypass_decorrelation,
                bypass_transient_detection,
                bypass_all_processing,
            },
        output:
            UpmixerOutputSettings {
                enable_ml_detection,
                multi_source_extraction,
                multi_source_threshold,
                binaural_preview,
                auto_gain_enabled,
                auto_gain_max_db,
                auto_gain_smoothing_ms,
            },
    } = ctx.settings
    {
        let (upmixer_tab, expanded_sections, loudness_info, spatial_spider) = {
            let app = &ctx.entity.read(cx).app;
            (
                app.plugin_ui.upmixer_tab,
                app.plugin_ui.upmixer_expanded_sections.clone(),
                app.playback.loudness_info.clone(),
                app.plugin_ui.spatial_spider.clone(),
            )
        };
        let d = Ds::from_cx(cx);
        let text = crate::app::i18n::PluginCommonTranslations::for_language(
            ctx.entity.read(cx).app.ui_state.language,
        );
        super::super::render_upmixer_plugin(
            &d,
            ctx.entity.clone(),
            ctx.plugin_idx,
            ui_upmixer::UpmixerRenderState {
                speaker_config,
                gain_front_direct: *gain_front_direct,
                gain_front_ambient: *gain_front_ambient,
                gain_rear_ambient: *gain_rear_ambient,
                height_gain: *height_gain,
                stereo_width: *stereo_width,
                center_spread: *center_spread,
                surround_direct_bleed: *surround_direct_bleed,
                rear_late_reflection: *rear_late_reflection,
                lfe_cutoff_hz: *lfe_cutoff_hz,
                lfe_gain: *lfe_gain,
                bandpass_hz: *bandpass_hz,
                enable_subharmonic_synth: *enable_subharmonic_synth,
                subharmonic_gain: *subharmonic_gain,
                subharmonic_freq_hz: *subharmonic_freq_hz,
                subharmonic_attack_ms: *subharmonic_attack_ms,
                subharmonic_release_ms: *subharmonic_release_ms,
                decorrelation_mode: *decorrelation_mode,
                decorrelation_lfo_rate_hz: *decorrelation_lfo_rate_hz,
                velvet_noise_duration_ms: *velvet_noise_duration_ms,
                velvet_noise_density: *velvet_noise_density,
                enable_hr_direct: *enable_hr_direct,
                hr_sharpen: *hr_sharpen,
                height_hf_cap_hz: *height_hf_cap_hz,
                height_transient_reduction: *height_transient_reduction,
                height_direct_leak: *height_direct_leak,
                ambient_boost: *ambient_boost,
                safety_cap_db: *safety_cap_db,
                rear_ambient_boost: *rear_ambient_boost,
                dialogue_weight: *dialogue_weight,
                voice_freq_min_hz: *voice_freq_min_hz,
                voice_freq_max_hz: *voice_freq_max_hz,
                dialogue_centroid_weight: *dialogue_centroid_weight,
                dialogue_variance_weight: *dialogue_variance_weight,
                dialogue_coherence_weight: *dialogue_coherence_weight,
                bypass_decorrelation: *bypass_decorrelation,
                bypass_transient_detection: *bypass_transient_detection,
                bypass_all_processing: *bypass_all_processing,
                enable_ml_detection: *enable_ml_detection,
                low_latency: *low_latency,
                frequency_resolution: *frequency_resolution,
                multi_source_extraction: *multi_source_extraction,
                multi_source_threshold: *multi_source_threshold,
                binaural_preview: *binaural_preview,
                auto_gain_enabled: *auto_gain_enabled,
                auto_gain_max_db: *auto_gain_max_db,
                auto_gain_smoothing_ms: *auto_gain_smoothing_ms,
                is_editing: ctx.is_editing,
                selected_param: ctx.selected_param,
                config_open: false,
                upmixer_tab,
                expanded_sections,
                loudness_info,
                spatial_spider,
            },
            ctx.available_width,
            ctx.layout_scale,
            text,
            ctx.theme,
            ctx.plugin_theme,
        )
        .into_any_element()
    } else {
        Empty.into_any_element()
    }
}

pub(super) fn render_mute_solo(
    ctx: &CustomViewRenderContext,
    cx: &mut Context<PlayerView>,
) -> AnyElement {
    let d = Ds::from_cx(cx);
    use super::super::ui_mute_solo;
    if let PluginSettings::ChannelMuteSolo {
        enabled,
        dim_gain_db,
        fade_ms,
        channel_states,
        ..
    } = ctx.settings
    {
        super::super::render_mute_solo_plugin(
            &d,
            ctx.entity.clone(),
            ctx.plugin_idx,
            ui_mute_solo::ChannelMuteSoloRenderState {
                enabled: *enabled,
                dim_gain_db: *dim_gain_db,
                fade_ms: *fade_ms,
                channel_states,
                is_editing: ctx.is_editing,
                selected_param: ctx.selected_param,
            },
            crate::app::i18n::PluginCommonTranslations::for_language(
                ctx.entity.read(cx).app.ui_state.language,
            ),
            ctx.theme,
        )
        .into_any_element()
    } else {
        Empty.into_any_element()
    }
}

pub(super) fn render_matrix(
    ctx: &CustomViewRenderContext,
    cx: &mut Context<PlayerView>,
) -> AnyElement {
    let d = Ds::from_cx(cx);
    use super::super::ui_matrix;
    if let PluginSettings::Matrix {
        input_channels,
        output_channels,
        matrix,
        channel_states,
    } = ctx.settings
        && let Some(plugin_instance_id) = ctx.plugin_instance_id
    {
        let speaker_config = ctx.plugin_graph.speaker_config_at_index(ctx.plugin_idx);
        super::super::render_matrix_plugin(
            &d,
            ctx.entity.clone(),
            ctx.plugin_idx,
            ui_matrix::MatrixRenderState {
                plugin_instance_id,
                input_channels: *input_channels,
                output_channels: *output_channels,
                available_width: ctx.available_width,
                layout_scale: ctx.layout_scale,
                matrix,
                channel_states,
                speaker_config,
                is_editing: ctx.is_editing,
                selected_param: ctx.selected_param,
                selected_cell: ctx
                    .entity
                    .read(cx)
                    .app
                    .plugin_state
                    .matrix_selected_cell
                    .and_then(|(selected_plugin_id, input, output)| {
                        (plugin_instance_id == selected_plugin_id).then_some((input, output))
                    }),
            },
            ctx.theme,
        )
        .into_any_element()
    } else {
        Empty.into_any_element()
    }
}

pub(super) fn render_loudness(
    ctx: &CustomViewRenderContext,
    cx: &mut Context<PlayerView>,
) -> AnyElement {
    let d = Ds::from_cx(cx);
    let text = crate::app::i18n::LevelMeterTranslations::for_language(
        ctx.entity.read(cx).app.ui_state.language,
    );
    super::super::render_loudness_monitor_plugin(
        &d,
        ctx.loudness.clone(),
        ctx.layout_scale,
        ctx.plugin_idx,
        ctx.is_editing,
        text,
        ctx.theme,
    )
    .into_any_element()
}

pub(super) fn render_mb_compressor(
    ctx: &CustomViewRenderContext,
    cx: &mut Context<PlayerView>,
) -> AnyElement {
    let d = Ds::from_cx(cx);
    let text = crate::app::i18n::PluginCommonTranslations::for_language(
        ctx.entity.read(cx).app.ui_state.language,
    );
    use super::super::ui_mb_compressor;
    if let PluginSettings::MultibandCompressor {
        num_bands,
        crossover_preset,
        crossover_freq_1,
        crossover_freq_2,
        crossover_freq_3,
        crossover_freq_4,
        threshold_db,
        ratio,
        attack_ms,
        release_ms,
        knee_db,
        mix,
        link_channels,
        per_band_lookahead_ms,
        ms_mode,
        sidechain_tilt_db,
        link_amount,
        bands,
    } = ctx.settings
    {
        let selected_band_idx = ctx.selected_band_idx.min(bands.len());
        let (dt, dr, da, drl, dk, dm, dam, dact, ds, db) = if selected_band_idx > 0 {
            let b = &bands[selected_band_idx - 1];
            (
                b.threshold_db.map(|v| v as f64).unwrap_or(*threshold_db),
                b.ratio.map(|v| v as f64).unwrap_or(*ratio),
                b.attack_ms.map(|v| v as f64).unwrap_or(*attack_ms),
                b.release_ms.map(|v| v as f64).unwrap_or(*release_ms),
                b.knee_db.map(|v| v as f64).unwrap_or(*knee_db),
                b.makeup_gain_db as f64,
                b.auto_makeup,
                b.active,
                b.solo,
                b.bypass,
            )
        } else {
            (
                *threshold_db,
                *ratio,
                *attack_ms,
                *release_ms,
                *knee_db,
                0.0,
                false,
                true,
                false,
                false,
            )
        };
        super::super::render_mb_compressor_plugin(
            &d,
            ctx.entity.clone(),
            ctx.plugin_idx,
            ui_mb_compressor::MbCompressorRenderState {
                num_bands: *num_bands,
                crossover_preset: *crossover_preset,
                crossover_freq_1: *crossover_freq_1,
                crossover_freq_2: *crossover_freq_2,
                crossover_freq_3: *crossover_freq_3,
                crossover_freq_4: *crossover_freq_4,
                threshold_db: dt,
                ratio: dr,
                attack_ms: da,
                release_ms: drl,
                knee_db: dk,
                makeup_gain_db: dm,
                auto_makeup: dam,
                active: dact,
                solo: ds,
                bypass: db,
                mix: *mix,
                link_channels: *link_channels,
                per_band_lookahead_ms: *per_band_lookahead_ms,
                ms_mode: *ms_mode,
                sidechain_tilt_db: *sidechain_tilt_db,
                link_amount: *link_amount,
                is_editing: ctx.is_editing,
                selected_param: ctx.selected_param,
                selected_band_idx,
            },
            ctx.available_width,
            ctx.layout_scale,
            text,
            ctx.theme,
        )
        .into_any_element()
    } else {
        Empty.into_any_element()
    }
}

pub(super) fn render_mb_expander(
    ctx: &CustomViewRenderContext,
    cx: &mut Context<PlayerView>,
) -> AnyElement {
    let d = Ds::from_cx(cx);
    let text = crate::app::i18n::PluginCommonTranslations::for_language(
        ctx.entity.read(cx).app.ui_state.language,
    );
    use super::super::ui_mb_expander;
    if let PluginSettings::MultibandExpander {
        num_bands,
        crossover_preset,
        crossover_freq_1,
        crossover_freq_2,
        crossover_freq_3,
        crossover_freq_4,
        threshold_db,
        ratio,
        attack_ms,
        release_ms,
        range_db,
        knee_db,
        hysteresis_db,
        hold_ms,
        mix,
        link_channels,
        detection_mode,
        lookahead_ms,
        bands,
    } = ctx.settings
    {
        let selected_band_idx = ctx.selected_band_idx.min(bands.len());
        let (dt, dr, da, drl, drng, dk, dh, dhold, dam, dact, ds, db) = if selected_band_idx > 0 {
            let b = &bands[selected_band_idx - 1];
            (
                b.threshold_db.map(|v| v as f64).unwrap_or(*threshold_db),
                b.ratio.map(|v| v as f64).unwrap_or(*ratio),
                b.attack_ms.map(|v| v as f64).unwrap_or(*attack_ms),
                b.release_ms.map(|v| v as f64).unwrap_or(*release_ms),
                b.range_db.map(|v| v as f64).unwrap_or(*range_db),
                b.knee_db.map(|v| v as f64).unwrap_or(*knee_db),
                b.hysteresis_db.map(|v| v as f64).unwrap_or(*hysteresis_db),
                b.hold_ms.map(|v| v as f64).unwrap_or(*hold_ms),
                b.auto_makeup,
                b.active,
                b.solo,
                b.bypass,
            )
        } else {
            (
                *threshold_db,
                *ratio,
                *attack_ms,
                *release_ms,
                *range_db,
                *knee_db,
                *hysteresis_db,
                *hold_ms,
                false,
                true,
                false,
                false,
            )
        };
        super::super::render_mb_expander_plugin(
            &d,
            ctx.entity.clone(),
            ctx.plugin_idx,
            ui_mb_expander::MbExpanderRenderState {
                num_bands: *num_bands,
                crossover_preset: *crossover_preset,
                crossover_freq_1: *crossover_freq_1,
                crossover_freq_2: *crossover_freq_2,
                crossover_freq_3: *crossover_freq_3,
                crossover_freq_4: *crossover_freq_4,
                threshold_db: dt,
                ratio: dr,
                attack_ms: da,
                release_ms: drl,
                range_db: drng,
                knee_db: dk,
                hysteresis_db: dh,
                hold_ms: dhold,
                auto_makeup: dam,
                active: dact,
                solo: ds,
                bypass: db,
                mix: *mix,
                link_channels: *link_channels,
                detection_mode: if detection_mode == "RMS" { 1 } else { 0 },
                lookahead_ms: *lookahead_ms,
                is_editing: ctx.is_editing,
                selected_param: ctx.selected_param,
                selected_band_idx,
            },
            ctx.available_width,
            ctx.layout_scale,
            text,
            ctx.theme,
        )
        .into_any_element()
    } else {
        Empty.into_any_element()
    }
}

pub(super) fn render_ab_compare(
    ctx: &CustomViewRenderContext,
    cx: &mut Context<PlayerView>,
) -> AnyElement {
    super::super::ui_ab_compare::render_ab_compare(ctx, cx)
}
