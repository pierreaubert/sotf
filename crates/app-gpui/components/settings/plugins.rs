//! Misc settings content (CPU cores, etc.)

use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, HStack, NumberInput, NumberInputSize, StackSpacing, Text,
    TextSize, VStack,
};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use sotf_audio_player::config::plugin_sandbox_runtime_status;

impl PlayerView {
    /// Render misc settings content
    pub(crate) fn render_plugins_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let max_cores = state.app.ui_state.max_cpu_cores;
        let plugin_sandbox_status_section: Option<AnyElement> = {
            #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
            {
                plugin_sandbox_runtime_status(state.app.external_plugin_media_directories())
                    .ok()
                    .map(|status| {
                        VStack::new()
                            .spacing(StackSpacing::Xs)
                            .child(
                                Text::new("External Plugins")
                                    .size(TextSize::Sm)
                                    .weight(gpui_ui_kit::TextWeight::Bold)
                                    .color(theme.text_primary),
                            )
                            .child(
                                Text::new(format!(
                                    "Runtime external access: {}",
                                    if status.runtime_external_access_disabled {
                                        "disabled"
                                    } else {
                                        "enabled"
                                    }
                                ))
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                            )
                            .child(
                                Text::new(format!(
                                    "{} import grants, {} media roots, {} protected import roots",
                                    status.persistent_grant_count,
                                    status.media_read_paths.len(),
                                    status.protected_import_paths.len(),
                                ))
                                .size(TextSize::Xs)
                                .color(theme.text_secondary),
                            )
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Xs)
                                    .child(
                                        Button::new(
                                            "activate-external-plugins",
                                            "Activate External Plugins",
                                        )
                                        .variant(ButtonVariant::Primary)
                                        .size(ButtonSize::Xs)
                                        .theme(theme.to_button_theme())
                                        .on_click_event(cx.listener(|view, _, _, cx| {
                                            view.state.update(cx, |state, _cx| {
                                                state
                                                    .app
                                                    .install_external_plugin_runtime_sandbox();
                                            });
                                            cx.notify();
                                        })),
                                    )
                                    .child(
                                        Button::new(
                                            "scan-external-plugins",
                                            "Scan External Plugins",
                                        )
                                        .variant(ButtonVariant::Secondary)
                                        .size(ButtonSize::Xs)
                                        .theme(theme.to_button_theme())
                                        .on_click_event(cx.listener(|view, _, _, cx| {
                                            view.state.update(cx, |state, _cx| {
                                                let mut scanner = sotf_plugins::PluginScanner::new();
                                                scanner.scan_all();
                                                state.app.plugin_state.scanned_external_plugins =
                                                    scanner.plugins.clone();
                                                state
                                                    .app
                                                    .ui_state
                                                    .toast_message = Some(crate::app::ToastMessage::success(
                                                    format!(
                                                        "Found {} external plugins",
                                                        state.app.plugin_state.scanned_external_plugins.len()
                                                    ),
                                                ));
                                            });
                                            cx.notify();
                                        })),
                                    )
                                    .build(),
                            )
                            .build()
                            .into_any_element()
                    })
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
                    .child("Miscellaneous"),
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
                                        Text::new("Max CPU Cores")
                                            .size(TextSize::Sm)
                                            .weight(gpui_ui_kit::TextWeight::Bold)
                                            .color(theme.text_primary),
                                    )
                                    .child(
                                        Text::new(format!(
                                            "Limit the number of CPU cores SotF can use ({} available).",
                                            total_cores
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
                                    .unit("cores")
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
