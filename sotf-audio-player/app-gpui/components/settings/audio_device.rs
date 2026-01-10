//! Audio device settings content

#[cfg(all(target_os = "macos", feature = "hal"))]
use crate::app::types::PlaybackSource;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Badge, BadgeVariant, HStack, StackAlign, StackSpacing, Text, TextSize, TextWeight, VStack,
};

impl PlayerView {
    pub(crate) fn render_audio_device_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let _playback_source = state.app.playback_source;

        let mut content = VStack::new().spacing(StackSpacing::Md);

        // HAL Input Source section (macOS only with hal feature)
        #[cfg(all(target_os = "macos", feature = "hal"))]
        {
            let is_hal_mode = matches!(_playback_source, PlaybackSource::HalDevice);
            let state_entity = self.state.clone();
            let theme_for_source = theme.clone();

            content = content
                .child(
                    Text::new("Audio Source")
                        .size(TextSize::Sm)
                        .weight(TextWeight::Semibold),
                )
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Md)
                        .child({
                            // File Player option
                            let is_selected = !is_hal_mode;
                            let theme = theme_for_source.clone();
                            let state_clone = state_entity.clone();
                            div()
                                .id("source-file")
                                .px_4()
                                .py_2()
                                .rounded_md()
                                .cursor_pointer()
                                .border_1()
                                .when(is_selected, |d| {
                                    d.bg(theme.accent).border_color(theme.accent)
                                })
                                .when(!is_selected, |d| {
                                    d.bg(theme.surface)
                                        .border_color(theme.border)
                                        .hover(|s| s.bg(theme.surface_hover))
                                })
                                .child(
                                    VStack::new()
                                        .spacing(StackSpacing::Xs)
                                        .child(
                                            Text::new("File Player")
                                                .size(TextSize::Sm)
                                                .weight(TextWeight::Semibold)
                                                .color(if is_selected {
                                                    theme.text_on_accent
                                                } else {
                                                    theme.text_primary
                                                }),
                                        )
                                        .child(
                                            Text::new("Play audio files from library")
                                                .size(TextSize::Xs)
                                                .color(if is_selected {
                                                    theme.text_on_accent
                                                } else {
                                                    theme.text_muted
                                                }),
                                        ),
                                )
                                .on_mouse_up(
                                    MouseButton::Left,
                                    move |_: &MouseUpEvent, _window, cx| {
                                        state_clone.update(cx, |state, _cx| {
                                            state.app.playback_source = PlaybackSource::File;
                                            // Stop HAL playback if running
                                            if let Err(e) = state.player.lock().stop() {
                                                log::error!("Failed to stop HAL playback: {}", e);
                                            }
                                            state.app.playback.is_playing = false;
                                        });
                                    },
                                )
                        })
                        .child({
                            // HAL Device option
                            let is_selected = is_hal_mode;
                            let theme = theme_for_source.clone();
                            let state_clone = state_entity.clone();
                            div()
                                .id("source-hal")
                                .px_4()
                                .py_2()
                                .rounded_md()
                                .cursor_pointer()
                                .border_1()
                                .when(is_selected, |d| {
                                    d.bg(theme.accent).border_color(theme.accent)
                                })
                                .when(!is_selected, |d| {
                                    d.bg(theme.surface)
                                        .border_color(theme.border)
                                        .hover(|s| s.bg(theme.surface_hover))
                                })
                                .child(
                                    VStack::new()
                                        .spacing(StackSpacing::Xs)
                                        .child(
                                            Text::new("HAL Device")
                                                .size(TextSize::Sm)
                                                .weight(TextWeight::Semibold)
                                                .color(if is_selected {
                                                    theme.text_on_accent
                                                } else {
                                                    theme.text_primary
                                                }),
                                        )
                                        .child(
                                            Text::new("Process system audio through plugins")
                                                .size(TextSize::Xs)
                                                .color(if is_selected {
                                                    theme.text_on_accent
                                                } else {
                                                    theme.text_muted
                                                }),
                                        ),
                                )
                                .on_mouse_up(
                                    MouseButton::Left,
                                    move |_: &MouseUpEvent, _window, cx| {
                                        Self::start_hal_playback_from_ui(&state_clone, cx);
                                    },
                                )
                        }),
                )
                .child(div().h(px(8.0))); // Spacer between sections
        }

        content = content.child(
            Text::new(translations.devices_title)
                .size(TextSize::Sm)
                .weight(TextWeight::Semibold),
        );

        content.child(
            // Grid layout with 2 equal-width columns
            div().grid().grid_cols(2).gap_3().w_full().children(
                state
                    .app
                    .output_devices
                    .iter()
                    .enumerate()
                    .map(|(idx, device)| {
                        let is_selected = state.app.selected_output_device_index == idx;
                        let sample_rate = device
                            .default_config
                            .as_ref()
                            .map(|c| c.sample_rate)
                            .unwrap_or(0);
                        let channels = device
                            .default_config
                            .as_ref()
                            .map(|c| c.channels)
                            .unwrap_or(0);
                        let theme = theme.clone();
                        let device_name = device.name.clone();
                        let is_default = device.is_default;

                        // Try to find a brand image
                        let brand_image = get_brand_image_path(&device_name);

                        div()
                            .w_full()
                            .p_3()
                            .rounded_md()
                            .cursor_pointer()
                            .border_1()
                            .when(is_selected, |d| {
                                d.bg(theme.accent).border_color(theme.accent)
                            })
                            .when(!is_selected, |d| {
                                d.bg(theme.surface)
                                    .border_color(theme.border)
                                    .hover(|s| s.bg(theme.surface_hover))
                            })
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Md)
                                    .align(StackAlign::Center)
                                    .when_some(brand_image, |stack, image_path| {
                                        stack.child(
                                            div()
                                                .w(px(60.0))
                                                .h(px(60.0))
                                                .rounded_md()
                                                .bg(theme.background)
                                                .overflow_hidden()
                                                .child(
                                                    img(image_path)
                                                        .w_full()
                                                        .h_full()
                                                        .object_fit(ObjectFit::Contain), // Contain to show full brand
                                                ),
                                        )
                                    })
                                    .child(
                                        VStack::new()
                                            .spacing(StackSpacing::Sm)
                                            .child(
                                                Text::new(device_name)
                                                    .size(TextSize::Sm)
                                                    .weight(TextWeight::Semibold)
                                                    .color(if is_selected {
                                                        theme.text_on_accent
                                                    } else {
                                                        theme.text_primary
                                                    }),
                                            )
                                            .child(
                                                HStack::new()
                                                    .spacing(StackSpacing::Md)
                                                    .child(
                                                        Badge::new(format!("{} ch", channels))
                                                            .variant(BadgeVariant::Info),
                                                    )
                                                    .child(
                                                        Badge::new(if sample_rate >= 1000 {
                                                            format!("{} kHz", sample_rate / 1000)
                                                        } else {
                                                            format!("{} Hz", sample_rate)
                                                        })
                                                        .variant(BadgeVariant::Info),
                                                    ),
                                            )
                                            .when(is_default, |stack| {
                                                stack.child(
                                                    Badge::new(format!(
                                                        "✓ {}",
                                                        translations.settings_default_badge
                                                    ))
                                                    .variant(BadgeVariant::Success),
                                                )
                                            }),
                                    ),
                            )
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                    view.state.update(cx, |state, _cx| {
                                        state.app.selected_output_device_index = idx;
                                        if let Some(device) = state.app.output_devices.get(idx) {
                                            state.app.current_output_device_name =
                                                Some(device.name.clone());

                                            // If playing, restart track with new device
                                            if state.app.playback.is_playing {
                                                if let Some(queue_idx) =
                                                    state.app.playback.current_queue_index
                                                {
                                                    if let Some(item) =
                                                        state.app.queue.get(queue_idx)
                                                    {
                                                        if let Some(track) = item.current_track() {
                                                            let path = track.path.clone();
                                                            Self::play_track(state, path);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    });
                                    cx.notify();
                                }),
                            )
                    }),
            ),
        )
    }

    /// Start HAL playback from the UI settings toggle
    #[cfg(all(target_os = "macos", feature = "hal"))]
    fn start_hal_playback_from_ui(state_entity: &Entity<crate::app::AppState>, cx: &mut App) {
        use sotf_audio::engine::PluginConfig;

        state_entity.update(cx, |state, _cx| {
            // Build plugin chain with hal_input as first plugin
            let mut plugins: Vec<PluginConfig> = Vec::new();

            // Add hal_input plugin as the source
            plugins.push(PluginConfig {
                plugin_type: "hal_input".to_string(),
                parameters: serde_json::json!({
                    "channels": 2,
                    "sample_rate": 48000
                }),
            });

            // Add plugins from the current plugin chain (48kHz is the HAL default rate)
            for plugin_config in state.app.plugin_state.plugin_chain.to_plugin_configs(48000.0) {
                plugins.push(plugin_config);
            }

            // Get output device
            let output_device = state.app.current_output_device_name.clone();

            // Determine output channels from plugin chain
            let output_channels = state.app.plugin_state.plugin_chain.output_channels();

            // Start HAL playback
            match state
                .player
                .lock()
                .start_hal_playback(plugins, output_channels, output_device)
            {
                Ok(()) => {
                    state.app.playback_source = PlaybackSource::HalDevice;
                    state.app.playback.is_playing = true;
                    log::info!("HAL playback started successfully");
                }
                Err(e) => {
                    log::error!("Failed to start HAL playback: {}", e);
                    state.app.ui_state.toast_message = Some(crate::app::types::ToastMessage::error(
                        format!("Failed to start HAL: {}", e),
                    ));
                }
            }
        });
    }
}

/// Helper to get brand image path from device name
fn get_brand_image_path(device_name: &str) -> Option<&'static str> {
    let lower_name = device_name.to_lowercase();
    if lower_name.contains("mac") || lower_name.contains("apple") {
        return Some("brands/apple-mac-mini.png");
    }
    if lower_name.contains("phonum") || lower_name.contains("beyerdynamic") {
        return Some("brands/bayerdynamic-phonum.jpg");
    }
    if lower_name.contains("blackhole") {
        return Some("brands/blackhole.jpeg");
    }
    if lower_name.contains("dolby") {
        return Some("brands/dolby-audio.png");
    }
    if lower_name.contains("focusrite") || lower_name.contains("scarlett") {
        return Some("brands/focusrite.png");
    }
    if lower_name.contains("kef") || lower_name.contains("ls60") {
        return Some("brands/kef-ls60.jpg");
    }
    if lower_name.contains("lg") || lower_name.contains("ultrafine") {
        return Some("brands/lg.png");
    }
    if lower_name.contains("rme") || lower_name.contains("fireface") {
        return Some("brands/rme.jpg");
    }
    if lower_name.contains("adam") {
        return Some("brands/adam.png");
    }
    if lower_name.contains("samsung") {
        return Some("brands/samsung-q9.png");
    }
    if lower_name.contains("usb") {
        return Some("brands/usb.png");
    }
    None
}
