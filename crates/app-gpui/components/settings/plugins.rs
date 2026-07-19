//! Misc settings content (CPU cores, etc.)

use crate::app::AppState;
use crate::app::i18n::{ExternalPluginSettingsTranslations, SettingsSurfaceTranslations};
use crate::app::state::{
    EXTERNAL_PLUGIN_SCAN_PAGE_SIZE, ExternalPluginRuntimeSummary, ExternalPluginScanCounts,
    ExternalPluginWorkerHealth, external_plugin_error_key, external_plugin_worker_health,
};
use crate::app::types::PluginUpdateType;
use crate::components::design::Ds;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, HStack, NumberInput, NumberInputSize, StackSpacing, Text,
    TextSize, VStack,
};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use sotf_audio_player::PluginSettings;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use sotf_plugins::{
    ExternalPluginSandboxMode, ExternalPluginState, PluginDescriptor, PluginFormat,
    PluginScanStatus,
};

impl PlayerView {
    /// Render misc settings content
    pub(crate) fn render_plugins_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let text = SettingsSurfaceTranslations::for_language(state.app.ui_state.language);
        let max_cores = state.app.ui_state.max_cpu_cores;
        let plugin_sandbox_status_section: Option<AnyElement> = {
            #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
            {
                let external = text.external;
                let ui = state.app.plugin_state.external_plugin_ui.clone();
                let plugins = state.app.plugin_state.scanned_external_plugins.clone();
                let counts = ExternalPluginScanCounts::from_plugins(&plugins);

                let runtime_summary = ui
                    .runtime_summary
                    .map(|summary| render_external_runtime_summary(external, summary, &theme));
                let runtime_error = ui
                    .runtime_error
                    .as_deref()
                    .map(|error| render_external_error(&d, external.runtime_error, error, &theme));
                let build_diagnostics = (!ui.build_diagnostics.is_empty()).then(|| {
                    VStack::new()
                        .spacing(StackSpacing::Xs)
                        .children(ui.build_diagnostics.iter().map(|diagnostic| {
                            render_external_error(
                                &d,
                                external.host_diagnostic,
                                &diagnostic.message,
                                &theme,
                            )
                        }))
                        .build()
                        .into_any_element()
                });
                let worker_statuses = (!ui.worker_statuses.is_empty()).then(|| {
                    render_external_worker_statuses(&d, external, &ui.worker_statuses, &theme)
                });
                let scan_error = ui
                    .scan_error
                    .as_deref()
                    .map(|error| render_external_error(&d, external.scan_error, error, &theme));
                let scan_results = ui.scan_completed.then(|| {
                    render_external_plugin_results(
                        &d,
                        external,
                        &plugins,
                        counts,
                        &ui.load_errors,
                        ui.visible_scan_result_count(plugins.len()),
                        self.state.clone(),
                        &theme,
                    )
                });

                Some(
                    VStack::new()
                        .spacing(StackSpacing::Sm)
                        .child(
                            Text::new(external.title)
                                .size(TextSize::Sm)
                                .weight(gpui_ui_kit::TextWeight::Bold)
                                .color(theme.text_primary),
                        )
                        .when_some(runtime_summary, |stack, summary| stack.child(summary))
                        .when_some(runtime_error, |stack, error| stack.child(error))
                        .when_some(build_diagnostics, |stack, diagnostics| {
                            stack.child(diagnostics)
                        })
                        .when_some(worker_statuses, |stack, statuses| stack.child(statuses))
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap(d.grid)
                                .child(
                                    Button::new(
                                        "activate-external-plugins",
                                        if ui.runtime_update_in_progress {
                                            external.activating
                                        } else {
                                            external.activate
                                        },
                                    )
                                    .variant(ButtonVariant::Primary)
                                    .size(ButtonSize::Xs)
                                    .disabled(ui.runtime_update_in_progress)
                                    .theme(theme.to_button_theme())
                                    .on_click_event(
                                        cx.listener(move |view, _: &ClickEvent, _window, cx| {
                                            start_external_runtime_update(view, external, cx);
                                        }),
                                    ),
                                )
                                .child(
                                    Button::new(
                                        "scan-external-plugins",
                                        if ui.scan_in_progress {
                                            external.scanning
                                        } else {
                                            external.scan
                                        },
                                    )
                                    .variant(ButtonVariant::Secondary)
                                    .size(ButtonSize::Xs)
                                    .disabled(ui.scan_in_progress)
                                    .theme(theme.to_button_theme())
                                    .on_click_event(
                                        cx.listener(move |view, _: &ClickEvent, _window, cx| {
                                            start_external_plugin_scan(view, external, cx);
                                        }),
                                    ),
                                ),
                        )
                        .when_some(scan_error, |stack, error| stack.child(error))
                        .when_some(scan_results, |stack, results| stack.child(results))
                        .build()
                        .into_any_element(),
                )
            }

            #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
            {
                None
            }
        };

        let total_cores = std::thread::available_parallelism()
            .map(|n| n.get() as u8)
            .unwrap_or(4);

        let current_value = max_cores.unwrap_or(total_cores) as f64;

        div()
            .flex()
            .flex_col()
            .gap(d.section_lg)
            .child(
                div()
                    .text_size(d.text_sm)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text_primary)
                    .child(text.miscellaneous),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(d.section)
                    .p(d.card)
                    .bg(theme.background_secondary)
                    .rounded(d.r_md)
                    .border_1()
                    .border_color(theme.border)
                    .when_some(plugin_sandbox_status_section, |this, section| {
                        this.child(section)
                    })
                    // CPU cores row
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                VStack::new()
                                    .spacing(StackSpacing::Xs)
                                    .child(
                                        Text::new(text.max_cpu_cores)
                                            .size(TextSize::Sm)
                                            .weight(gpui_ui_kit::TextWeight::Bold)
                                            .color(theme.text_primary),
                                    )
                                    .child(
                                        Text::new(format!(
                                            "{} ({}: {})",
                                            text.max_cpu_cores_description,
                                            text.max_cpu_cores,
                                            total_cores,
                                        ))
                                        .size(TextSize::Xs)
                                        .color(theme.text_secondary),
                                    )
                                    .build()
                                    .flex_1(),
                            )
                            .child({
                                let state_entity = self.state.clone();
                                NumberInput::new("max-cpu-cores")
                                    .value(current_value)
                                    .range(1.0, total_cores as f64)
                                    .step(1.0)
                                    .decimals(0)
                                    .unit(text.cpu_cores_unit)
                                    .aria_label(text.max_cpu_cores)
                                    .size(NumberInputSize::Sm)
                                    .width(120.0)
                                    .on_change(move |val, _window, cx| {
                                        let cores = (val as u8).clamp(1, total_cores);
                                        state_entity.update(cx, |state, _cx| {
                                            if cores == total_cores {
                                                state.app.ui_state.max_cpu_cores = None;
                                            } else {
                                                state.app.ui_state.max_cpu_cores = Some(cores);
                                            }
                                        });
                                    })
                            }),
                    ),
            )
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn render_external_runtime_summary(
    text: ExternalPluginSettingsTranslations,
    summary: ExternalPluginRuntimeSummary,
    theme: &Theme,
) -> AnyElement {
    VStack::new()
        .spacing(StackSpacing::Xs)
        .child(
            Text::new(format!(
                "{}: {}",
                text.runtime_access,
                if summary.runtime_external_access_disabled {
                    text.disabled
                } else {
                    text.enabled
                }
            ))
            .size(TextSize::Xs)
            .color(theme.text_secondary),
        )
        .child(
            HStack::new()
                .spacing(StackSpacing::Sm)
                .child(
                    Text::new(format!(
                        "{}: {}",
                        text.import_grants, summary.persistent_grant_count
                    ))
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
                )
                .child(
                    Text::new(format!(
                        "{}: {}",
                        text.media_roots, summary.media_read_path_count
                    ))
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
                )
                .child(
                    Text::new(format!(
                        "{}: {}",
                        text.protected_import_roots, summary.protected_import_path_count
                    ))
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
                )
                .build(),
        )
        .build()
        .into_any_element()
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn render_external_error(d: &Ds, label: &'static str, error: &str, theme: &Theme) -> AnyElement {
    div()
        .p(d.pad_y)
        .rounded(d.r_sm)
        .bg(theme.error.opacity(0.12))
        .text_size(d.text_xs)
        .text_color(theme.error)
        .child(format!("{label}: {error}"))
        .into_any_element()
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn render_external_worker_statuses(
    d: &Ds,
    text: ExternalPluginSettingsTranslations,
    statuses: &[sotf_audio::engine::IsolatedExternalPluginWorkerStatus],
    theme: &Theme,
) -> AnyElement {
    VStack::new()
        .spacing(StackSpacing::Xs)
        .children(statuses.iter().map(|status| {
            let health = external_plugin_worker_health(status);
            let health_color = match health {
                ExternalPluginWorkerHealth::Healthy => theme.success,
                ExternalPluginWorkerHealth::Degraded => theme.warning,
                ExternalPluginWorkerHealth::Failed => theme.error,
            };
            let event = text.worker.event_label(status.event.as_ref());
            let sandbox = text.worker.sandbox_label(status);
            let counters = text.worker.counters(status);
            div()
                .flex()
                .flex_col()
                .gap(d.grid)
                .p(d.pad_y)
                .rounded(d.r_sm)
                .border_1()
                .border_color(match health {
                    ExternalPluginWorkerHealth::Healthy => theme.border,
                    ExternalPluginWorkerHealth::Degraded => theme.warning,
                    ExternalPluginWorkerHealth::Failed => theme.error,
                })
                .bg(theme.surface)
                .child(
                    Text::label(format!(
                        "{} #{}: {}",
                        text.worker.status,
                        status.plugin_index + 1,
                        event,
                    ))
                    .color(health_color),
                )
                .child(Text::caption(counters).color(theme.text_secondary))
                .child(Text::caption(sandbox).color(theme.text_muted))
                .when_some(status.error.as_deref(), |row, error| {
                    row.child(render_external_error(d, text.runtime_error, error, theme))
                })
                .when_some(status.sandbox_reason.as_deref(), |row, reason| {
                    row.child(Text::caption(reason.to_string()).color(theme.warning))
                })
                .into_any_element()
        }))
        .build()
        .into_any_element()
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn render_external_plugin_results(
    d: &Ds,
    text: ExternalPluginSettingsTranslations,
    plugins: &[PluginDescriptor],
    counts: ExternalPluginScanCounts,
    load_errors: &std::collections::HashMap<String, String>,
    visible_count: usize,
    state_entity: Entity<AppState>,
    theme: &Theme,
) -> AnyElement {
    let mut results = VStack::new().spacing(StackSpacing::Xs).child(
        Text::new(format!("{}: {}", text.results, counts.total))
            .size(TextSize::Sm)
            .weight(gpui_ui_kit::TextWeight::Bold)
            .color(theme.text_primary),
    );

    if plugins.is_empty() {
        return results
            .child(
                Text::new(text.none_found)
                    .size(TextSize::Xs)
                    .color(theme.text_secondary),
            )
            .build()
            .into_any_element();
    }

    results = results
        .child(
            HStack::new()
                .spacing(StackSpacing::Sm)
                .child(external_count_chip(
                    d,
                    text.loadable,
                    counts.loadable,
                    theme,
                ))
                .child(external_count_chip(
                    d,
                    text.discovered,
                    counts.discovered,
                    theme,
                ))
                .child(external_count_chip(
                    d,
                    text.unsupported,
                    counts.unsupported,
                    theme,
                ))
                .build(),
        )
        .child(
            div().flex().flex_col().gap(d.grid).children(
                plugins
                    .iter()
                    .take(visible_count)
                    .enumerate()
                    .map(|(index, plugin)| {
                        let error_key = external_plugin_error_key(plugin);
                        render_external_plugin_row(
                            d,
                            text,
                            index,
                            plugin,
                            load_errors.get(&error_key).map(String::as_str),
                            state_entity.clone(),
                            theme,
                        )
                    }),
            ),
        );

    let hidden = plugins.len().saturating_sub(visible_count);
    if hidden > 0 {
        let state_for_more = state_entity.clone();
        let next_count = hidden.min(EXTERNAL_PLUGIN_SCAN_PAGE_SIZE);
        results = results.child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(d.grid)
                .child(
                    Text::caption(format!("{}: {}", text.more_results, hidden))
                        .color(theme.text_muted),
                )
                .child(
                    Button::new(
                        "show-more-external-plugins",
                        format!("{} (+{next_count})", text.more_results),
                    )
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Xs)
                    .aria_label(text.more_results)
                    .theme(theme.to_button_theme())
                    .on_click_event(move |_: &ClickEvent, _window, cx| {
                        state_for_more.update(cx, |state, cx| {
                            let total = state.app.plugin_state.scanned_external_plugins.len();
                            state
                                .app
                                .plugin_state
                                .external_plugin_ui
                                .show_more_scan_results(total);
                            cx.notify();
                        });
                    }),
                ),
        );
    }

    results.build().into_any_element()
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn external_count_chip(d: &Ds, label: &'static str, count: usize, theme: &Theme) -> AnyElement {
    div()
        .px(d.pad_y)
        .py(d.grid)
        .rounded(d.r_sm)
        .bg(theme.surface)
        .text_size(d.text_xs)
        .text_color(theme.text_secondary)
        .child(format!("{label}: {count}"))
        .into_any_element()
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn render_external_plugin_row(
    d: &Ds,
    text: ExternalPluginSettingsTranslations,
    index: usize,
    plugin: &PluginDescriptor,
    load_error: Option<&str>,
    state_entity: Entity<AppState>,
    theme: &Theme,
) -> AnyElement {
    let (status, status_color) = match plugin.scan_status {
        PluginScanStatus::Loadable => (text.loadable, theme.success),
        PluginScanStatus::Discovered => (text.discovered, theme.warning),
        PluginScanStatus::UnsupportedByBuild => (text.unsupported, theme.error),
    };
    let format = match plugin.format {
        PluginFormat::Clap => "CLAP",
        PluginFormat::Vst3 => "VST3",
        PluginFormat::AudioUnit => "AU",
    };
    let can_add = plugin.scan_status == PluginScanStatus::Loadable
        && !plugin.is_instrument
        && plugin.audio_inputs > 0;
    let descriptor = plugin.clone();
    let error_key = external_plugin_error_key(plugin);
    let add_button = Button::new(format!("add-external-plugin-{index}"), text.add_to_rack)
        .variant(ButtonVariant::Primary)
        .size(ButtonSize::Xs)
        .aria_label(format!("{}: {}", text.add_to_rack, plugin.name))
        .disabled(!can_add)
        .theme(theme.to_button_theme())
        .on_click_event(move |_: &ClickEvent, _window, cx| {
            let settings = PluginSettings::External {
                state: ExternalPluginState::new(
                    descriptor.clone(),
                    ExternalPluginSandboxMode::Isolated,
                    Vec::new(),
                ),
            };
            state_entity.update(cx, |state, cx| {
                match state.app.plugin_state.add_plugin_settings(settings) {
                    Ok(_) => {
                        state.app.plugin_state.update_state.plugin_graph_modified = true;
                        state.app.plugin_state.update_state.pending_plugin_update =
                            Some(PluginUpdateType::Structural);
                        state
                            .app
                            .plugin_state
                            .external_plugin_ui
                            .load_errors
                            .remove(&error_key);
                        state.app.ui_state.toast_message = Some(crate::app::ToastMessage::success(
                            text.added_to_rack_message(&descriptor.name),
                        ));
                    }
                    Err(error) => {
                        state
                            .app
                            .plugin_state
                            .external_plugin_ui
                            .load_errors
                            .insert(error_key.clone(), error.clone());
                        state.app.ui_state.toast_message = Some(crate::app::ToastMessage::error(
                            text.add_error_message(&error),
                        ));
                    }
                }
                cx.notify();
            });
        });

    div()
        .flex()
        .flex_col()
        .gap(d.grid)
        .p(d.pad_y)
        .rounded(d.r_sm)
        .bg(theme.surface)
        .border_1()
        .border_color(theme.border)
        .child(
            div()
                .flex()
                .items_start()
                .gap(d.grid)
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap(d.grid)
                        .child(
                            div().min_w_0().overflow_hidden().text_ellipsis().child(
                                Text::new(plugin.name.clone())
                                    .size(TextSize::Sm)
                                    .weight(gpui_ui_kit::TextWeight::Bold)
                                    .color(theme.text_primary),
                            ),
                        )
                        .child(
                            Text::new(format!("{format} · {status}"))
                                .size(TextSize::Xs)
                                .color(status_color),
                        ),
                )
                .child(div().flex_none().child(add_button)),
        )
        .child(
            Text::new(format!(
                "{} · {} · {}: {} → {}",
                plugin.vendor,
                plugin.version,
                text.channels,
                plugin.audio_inputs,
                plugin.audio_outputs,
            ))
            .size(TextSize::Xs)
            .color(theme.text_secondary),
        )
        .child(
            Text::caption(if plugin.is_instrument {
                text.instruments_unsupported
            } else {
                text.isolated
            })
            .color(if plugin.is_instrument {
                theme.warning
            } else {
                theme.text_muted
            }),
        )
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .text_ellipsis()
                .text_size(d.text_xs)
                .text_color(theme.text_muted)
                .child(format!("{}: {}", text.path, plugin.path.display())),
        )
        .when_some(load_error, |row, error| {
            row.child(render_external_error(d, text.add_error, error, theme))
        })
        .into_any_element()
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn start_external_plugin_scan(
    view: &mut PlayerView,
    text: ExternalPluginSettingsTranslations,
    cx: &mut Context<PlayerView>,
) {
    let state_entity = view.state.clone();
    state_entity.update(cx, |state, _| {
        let ui = &mut state.app.plugin_state.external_plugin_ui;
        ui.scan_in_progress = true;
        ui.scan_completed = false;
        ui.scan_error = None;
        ui.reset_scan_result_pagination();
    });

    cx.spawn(async move |_, cx| {
        let mut plugins = cx
            .background_executor()
            .spawn(async move {
                let mut scanner = sotf_plugins::PluginScanner::new();
                scanner.scan_all();
                scanner.plugins
            })
            .await;
        plugins.sort_by(|left, right| {
            left.name
                .to_lowercase()
                .cmp(&right.name.to_lowercase())
                .then_with(|| left.path.cmp(&right.path))
        });
        let count = plugins.len();

        state_entity.update(&mut cx.clone(), |state, cx| {
            state.app.plugin_state.scanned_external_plugins = plugins;
            let ui = &mut state.app.plugin_state.external_plugin_ui;
            ui.scan_in_progress = false;
            ui.scan_completed = true;
            ui.scan_error = None;
            state.app.ui_state.toast_message = Some(crate::app::ToastMessage::success(
                text.scan_results_message(count),
            ));
            cx.notify();
        });
    })
    .detach();
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn start_external_runtime_update(
    view: &mut PlayerView,
    text: ExternalPluginSettingsTranslations,
    cx: &mut Context<PlayerView>,
) {
    let state_entity = view.state.clone();
    let directories = state_entity.update(cx, |state, _| {
        let ui = &mut state.app.plugin_state.external_plugin_ui;
        ui.runtime_update_in_progress = true;
        ui.runtime_error = None;
        state.app.external_plugin_media_directories()
    });

    cx.spawn(async move |_, cx| {
        let result: Result<ExternalPluginRuntimeSummary, String> = cx
            .background_executor()
            .spawn(async move {
                sotf_audio_player::config::install_authorized_runtime_plugin_sandbox(
                    directories.clone(),
                )
                .map_err(|error| error.to_string())?;
                sotf_audio_player::config::plugin_sandbox_runtime_status(directories)
                    .map(ExternalPluginRuntimeSummary::from)
                    .map_err(|error| error.to_string())
            })
            .await;

        state_entity.update(&mut cx.clone(), |state, cx| {
            let ui = &mut state.app.plugin_state.external_plugin_ui;
            ui.runtime_update_in_progress = false;
            match result {
                Ok(summary) => {
                    ui.runtime_summary = Some(summary);
                    ui.runtime_error = None;
                    state.app.ui_state.toast_message = Some(crate::app::ToastMessage::success(
                        text.runtime_enabled_message(),
                    ));
                }
                Err(error) => {
                    ui.runtime_error = Some(error.clone());
                    state.app.ui_state.toast_message = Some(crate::app::ToastMessage::error(
                        text.runtime_error_message(&error),
                    ));
                }
            }
            cx.notify();
        });
    })
    .detach();
}
