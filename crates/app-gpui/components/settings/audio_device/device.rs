use super::misc::get_brand_image_path;
#[cfg(all(target_os = "macos", feature = "hal"))]
use crate::app::state::audio_device::{HalConfig, format_buffer_size, format_sample_rate};
#[cfg(all(target_os = "macos", feature = "hal"))]
use crate::app::types::PlaybackSource;
use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
#[cfg(target_os = "ios")]
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant};
#[cfg(all(target_os = "macos", feature = "hal"))]
use gpui_ui_kit::{ButtonSet, ButtonSetOption, Select, SelectOption};
use gpui_ui_kit::{HStack, StackAlign, StackSpacing, Text, VStack};

impl PlayerView {
    pub(crate) fn render_audio_device_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let _playback_source = state.app.audio_device_state.playback_source;

        let mut content = VStack::new().spacing(StackSpacing::Sm);

        // HAL Input Source section (macOS only with hal feature)
        #[cfg(all(target_os = "macos", feature = "hal"))]
        {
            let is_hal_mode = matches!(_playback_source, PlaybackSource::HalDevice);
            let state_entity = self.state.clone();
            let theme_for_source = theme.clone();

            content = content
                .child(Text::label("Audio Source"))
                .child({
                    let state_clone = state_entity.clone();
                    let selected = if is_hal_mode { "hal" } else { "file" };

                    ButtonSet::new("audio-source")
                        .options(vec![
                            ButtonSetOption::new("file", "File Player"),
                            ButtonSetOption::new("hal", "HAL Device"),
                        ])
                        .selected(selected)
                        .theme(theme_for_source.to_button_set_theme())
                        .on_change(move |value, _window, cx| {
                            let source = if value == "hal" {
                                PlaybackSource::HalDevice
                            } else {
                                PlaybackSource::File
                            };
                            state_clone.update(cx, |state, _cx| {
                                state.app.audio_device_state.playback_source = source;
                                // Stop HAL playback if switching to File mode
                                if matches!(source, PlaybackSource::File) {
                                    if let Err(e) = state.player.lock().stop() {
                                        log::error!("Failed to stop HAL playback: {}", e);
                                    }
                                    state.app.playback.is_playing = false;
                                }
                            });
                        })
                })
                .child(div().h(d.gap)); // Spacer between sections

            // HAL Configuration section (only show when in HAL mode)
            if is_hal_mode {
                let hal_config = state.app.audio_device_state.hal_config.clone();
                let hal_dropdowns = state.app.audio_device_state.hal_dropdowns.clone();

                // Sample rate options
                let sample_rate_options: Vec<SelectOption> = HalConfig::available_sample_rates()
                    .iter()
                    .map(|&rate| SelectOption::new(rate.to_string(), format_sample_rate(rate)))
                    .collect();

                // Channel count options
                let channel_options: Vec<SelectOption> = vec![
                    SelectOption::new("2", "2 ch (Stereo)"),
                    SelectOption::new("4", "4 ch (Quad)"),
                    SelectOption::new("6", "6 ch (5.1)"),
                    SelectOption::new("8", "8 ch (7.1)"),
                ];

                // Buffer size options
                let buffer_options: Vec<SelectOption> = HalConfig::available_buffer_sizes()
                    .iter()
                    .map(|&size| {
                        SelectOption::new(
                            size.to_string(),
                            format_buffer_size(size, hal_config.sample_rate),
                        )
                    })
                    .collect();

                content = content
                    .child(Text::label("HAL Configuration"))
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Md)
                            .child({
                                // Sample Rate selector
                                let state_for_change = state_entity.clone();
                                let state_for_toggle = state_entity.clone();
                                Select::new("hal-sample-rate")
                                    .label("Sample Rate")
                                    .options(sample_rate_options)
                                    .selected(hal_config.sample_rate.to_string())
                                    .is_open(hal_dropdowns.sample_rate_open)
                                    .theme(theme.to_select_theme())
                                    .on_change(move |value: &SharedString, _window, cx| {
                                        if let Ok(rate) = value.parse::<u32>() {
                                            Self::update_hal_sample_rate(
                                                &state_for_change,
                                                rate,
                                                cx,
                                            );
                                        }
                                    })
                                    .on_toggle(move |open, _window, cx| {
                                        state_for_toggle.update(cx, |state, cx| {
                                            // Close other dropdowns
                                            state
                                                .app
                                                .audio_device_state
                                                .hal_dropdowns
                                                .channel_count_open = false;
                                            state
                                                .app
                                                .audio_device_state
                                                .hal_dropdowns
                                                .buffer_size_open = false;
                                            state
                                                .app
                                                .audio_device_state
                                                .hal_dropdowns
                                                .sample_rate_open = open;
                                            cx.notify();
                                        });
                                    })
                            })
                            .child({
                                // Channel Count selector
                                let state_for_change = state_entity.clone();
                                let state_for_toggle = state_entity.clone();
                                Select::new("hal-channel-count")
                                    .label("Channels")
                                    .options(channel_options)
                                    .selected(hal_config.channel_count.to_string())
                                    .is_open(hal_dropdowns.channel_count_open)
                                    .theme(theme.to_select_theme())
                                    .on_change(move |value: &SharedString, _window, cx| {
                                        if let Ok(channels) = value.parse::<u32>() {
                                            Self::update_hal_channel_count(
                                                &state_for_change,
                                                channels,
                                                cx,
                                            );
                                        }
                                    })
                                    .on_toggle(move |open, _window, cx| {
                                        state_for_toggle.update(cx, |state, cx| {
                                            // Close other dropdowns
                                            state
                                                .app
                                                .audio_device_state
                                                .hal_dropdowns
                                                .sample_rate_open = false;
                                            state
                                                .app
                                                .audio_device_state
                                                .hal_dropdowns
                                                .buffer_size_open = false;
                                            state
                                                .app
                                                .audio_device_state
                                                .hal_dropdowns
                                                .channel_count_open = open;
                                            cx.notify();
                                        });
                                    })
                            })
                            .child({
                                // Buffer Size selector
                                let state_for_change = state_entity.clone();
                                let state_for_toggle = state_entity.clone();
                                Select::new("hal-buffer-size")
                                    .label("Buffer Size")
                                    .options(buffer_options)
                                    .selected(hal_config.buffer_frames.to_string())
                                    .is_open(hal_dropdowns.buffer_size_open)
                                    .theme(theme.to_select_theme())
                                    .on_change(move |value: &SharedString, _window, cx| {
                                        if let Ok(size) = value.parse::<u32>() {
                                            Self::update_hal_buffer_size(
                                                &state_for_change,
                                                size,
                                                cx,
                                            );
                                        }
                                    })
                                    .on_toggle(move |open, _window, cx| {
                                        state_for_toggle.update(cx, |state, cx| {
                                            // Close other dropdowns
                                            state
                                                .app
                                                .audio_device_state
                                                .hal_dropdowns
                                                .sample_rate_open = false;
                                            state
                                                .app
                                                .audio_device_state
                                                .hal_dropdowns
                                                .channel_count_open = false;
                                            state
                                                .app
                                                .audio_device_state
                                                .hal_dropdowns
                                                .buffer_size_open = open;
                                            cx.notify();
                                        });
                                    })
                            }),
                    )
                    .child(div().h(d.gap)); // Spacer
            }
        }

        #[cfg(target_os = "ios")]
        {
            content = content.child(
                HStack::new()
                    .spacing(StackSpacing::Sm)
                    .align(StackAlign::Center)
                    .child(Text::label("AirPlay and Bluetooth"))
                    .child(
                        Button::new("show-airplay-route-picker", "AirPlay")
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Sm)
                            .theme(theme.to_button_theme())
                            .on_click_event(|_, _window, _cx| {
                                unsafe extern "C" {
                                    fn sotf_ios_show_route_picker();
                                }
                                unsafe { sotf_ios_show_route_picker() };
                            }),
                    ),
            );
        }

        content = content.child(Text::label(translations.devices_title));

        content.child(
            // Grid layout with 2 equal-width columns
            div().grid().grid_cols(2).gap(d.gap_md).w_full().children(
                state
                    .app
                    .audio_device_state
                    .output_devices
                    .iter()
                    .enumerate()
                    .map(|(idx, device)| {
                        let is_selected =
                            state.app.audio_device_state.selected_output_device_index == idx;
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
                            .p(d.pad_x)
                            .rounded(d.r_md)
                            .cursor_pointer()
                            .border_1()
                            .when(is_selected, |el| {
                                el.bg(theme.surface_selected).border_color(theme.accent)
                            })
                            .when(!is_selected, |el| {
                                el.bg(theme.surface)
                                    .border_color(theme.border)
                                    .hover(|s| s.bg(theme.surface_hover))
                            })
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .align(StackAlign::Center)
                                    .when_some(brand_image, |stack, image_path| {
                                        stack.child(
                                            div()
                                                .w(rems(3.75))
                                                .h(rems(3.75))
                                                .rounded(d.r_md)
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
                                            .spacing(StackSpacing::Xs)
                                            .child(
                                                Text::label(device_name).color(theme.text_primary),
                                            )
                                            .child(
                                                HStack::new()
                                                    .spacing(StackSpacing::Sm)
                                                    .child(device_info_pill(
                                                        format!("{} ch", channels),
                                                        &theme,
                                                        d,
                                                    ))
                                                    .child(device_info_pill(
                                                        if sample_rate >= 1000 {
                                                            format!("{} kHz", sample_rate / 1000)
                                                        } else {
                                                            format!("{} Hz", sample_rate)
                                                        },
                                                        &theme,
                                                        d,
                                                    )),
                                            )
                                            .when(is_default, |stack| {
                                                stack.child(device_success_pill(
                                                    format!(
                                                        "✓ {}",
                                                        translations.settings_default_badge
                                                    ),
                                                    &theme,
                                                    d,
                                                ))
                                            }),
                                    ),
                            )
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                    view.state.update(cx, |state, _cx| {
                                        state.app.audio_device_state.selected_output_device_index =
                                            idx;
                                        if let Some(device) =
                                            state.app.audio_device_state.output_devices.get(idx)
                                        {
                                            state
                                                .app
                                                .audio_device_state
                                                .current_output_device_name =
                                                Some(device.name.clone());

                                            // If playing, restart track with new device
                                            if state.app.playback.is_playing
                                                && let Some(source) =
                                                    state.app.queue_state.current_track_source()
                                            {
                                                let position = state.app.playback.position_secs;
                                                Self::play_track_at(state, source, Some(position));
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
    pub(super) fn start_hal_playback_from_ui(
        state_entity: &Entity<crate::app::AppState>,
        cx: &mut App,
    ) {
        use sotf_audio::engine::PluginConfig;

        state_entity.update(cx, |state, _cx| {
            // Get HAL configuration from state
            let hal_config = &state.app.audio_device_state.hal_config;
            let sample_rate = hal_config.sample_rate;
            let channels = hal_config.channel_count;

            // Build plugin chain with hal_input as first plugin
            let mut plugins: Vec<PluginConfig> = Vec::new();

            // Add hal_input plugin as the source with configured settings
            plugins.push(PluginConfig {
                plugin_type: "hal_input".to_string(),
                parameters: serde_json::json!({
                    "channels": channels,
                }),
            });

            // Add plugins from the current plugin chain using configured sample rate
            for plugin_config in state
                .app
                .plugin_state
                .graph
                .to_plugin_configs(sample_rate as f64)
            {
                plugins.push(plugin_config);
            }

            // Get output device
            let output_device = state
                .app
                .audio_device_state
                .current_output_device_name
                .clone();

            // Determine output channels from plugin chain
            let output_channels = state.app.plugin_state.graph.output_channels();

            // Update driver-hal with the new configuration
            Self::apply_hal_config_to_driver(hal_config);

            // Start HAL playback with configured sample rate
            match state.player.lock().start_hal_playback_with_config(
                plugins,
                output_channels,
                output_device,
                sample_rate,
            ) {
                Ok(()) => {
                    state.app.audio_device_state.playback_source = PlaybackSource::HalDevice;
                    state.app.playback.is_playing = true;
                    log::info!(
                        "HAL playback started: {}Hz, {} channels",
                        sample_rate,
                        channels
                    );
                }
                Err(e) => {
                    log::error!("Failed to start HAL playback: {}", e);
                    state.app.ui_state.toast_message =
                        Some(crate::app::types::ToastMessage::error(format!(
                            "Failed to start HAL: {}",
                            e
                        )));
                }
            }
        });
    }

    /// Update HAL sample rate and restart playback if needed
    #[cfg(all(target_os = "macos", feature = "hal"))]
    pub(super) fn update_hal_sample_rate(
        state_entity: &Entity<crate::app::AppState>,
        sample_rate: u32,
        cx: &mut App,
    ) {
        let is_playing = state_entity.read(cx).app.playback.is_playing;
        let entity_id = state_entity.entity_id();

        state_entity.update(cx, |state, _cx| {
            state.app.audio_device_state.hal_config.sample_rate = sample_rate;
            state.app.audio_device_state.close_hal_dropdowns();
            log::info!("HAL sample rate changed to {}Hz", sample_rate);
        });

        // Restart HAL playback with new configuration
        if is_playing {
            Self::restart_hal_playback(state_entity, cx);
        }

        cx.notify(entity_id);
    }

    /// Update HAL channel count and restart playback if needed
    #[cfg(all(target_os = "macos", feature = "hal"))]
    pub(super) fn update_hal_channel_count(
        state_entity: &Entity<crate::app::AppState>,
        channel_count: u32,
        cx: &mut App,
    ) {
        let is_playing = state_entity.read(cx).app.playback.is_playing;
        let entity_id = state_entity.entity_id();

        state_entity.update(cx, |state, _cx| {
            state.app.audio_device_state.hal_config.channel_count = channel_count;
            state.app.audio_device_state.close_hal_dropdowns();
            log::info!("HAL channel count changed to {}", channel_count);
        });

        // Restart HAL playback with new configuration
        if is_playing {
            Self::restart_hal_playback(state_entity, cx);
        }

        cx.notify(entity_id);
    }

    /// Update HAL buffer size and restart playback if needed
    #[cfg(all(target_os = "macos", feature = "hal"))]
    pub(super) fn update_hal_buffer_size(
        state_entity: &Entity<crate::app::AppState>,
        buffer_frames: u32,
        cx: &mut App,
    ) {
        let is_playing = state_entity.read(cx).app.playback.is_playing;
        let entity_id = state_entity.entity_id();

        state_entity.update(cx, |state, _cx| {
            state.app.audio_device_state.hal_config.buffer_frames = buffer_frames;
            state.app.audio_device_state.close_hal_dropdowns();
            log::info!("HAL buffer size changed to {} frames", buffer_frames);
        });

        // Restart HAL playback with new configuration
        if is_playing {
            Self::restart_hal_playback(state_entity, cx);
        }

        cx.notify(entity_id);
    }

    /// Restart HAL playback with current configuration
    #[cfg(all(target_os = "macos", feature = "hal"))]
    pub(super) fn restart_hal_playback(state_entity: &Entity<crate::app::AppState>, cx: &mut App) {
        // Stop current playback
        state_entity.update(cx, |state, _cx| {
            if let Err(e) = state.player.lock().stop() {
                log::warn!("Failed to stop HAL playback for restart: {}", e);
            }
        });

        // Start with new configuration
        Self::start_hal_playback_from_ui(state_entity, cx);
    }

    /// Apply HAL configuration to the driver-hal shared memory
    #[cfg(all(target_os = "macos", feature = "hal"))]
    pub(super) fn apply_hal_config_to_driver(config: &HalConfig) {
        use driver_hal::HalOutputWriter;

        if let Some(mut writer) = HalOutputWriter::new() {
            writer.set_sample_rate(config.sample_rate);
            writer.set_channel_count(config.channel_count);
            writer.set_buffer_frames(config.buffer_frames);
            log::debug!(
                "Applied HAL config to driver: {}Hz, {} ch, {} frames",
                config.sample_rate,
                config.channel_count,
                config.buffer_frames
            );
        } else {
            log::warn!("Could not connect to HAL driver to apply configuration");
        }
    }
}

fn device_info_pill(
    label: impl Into<SharedString>,
    theme: &crate::theme::Theme,
    d: Ds,
) -> impl IntoElement {
    device_pill(
        label,
        crate::theme::Theme::with_opacity(theme.info, 0.16),
        theme.info,
        d,
    )
}

fn device_success_pill(
    label: impl Into<SharedString>,
    theme: &crate::theme::Theme,
    d: Ds,
) -> impl IntoElement {
    device_pill(
        label,
        crate::theme::Theme::with_opacity(theme.success, 0.16),
        theme.success,
        d,
    )
}

fn device_pill(
    label: impl Into<SharedString>,
    bg: Rgba,
    text_color: Rgba,
    d: Ds,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .px(d.pad_y)
        .py(d.grid)
        .rounded(d.r_sm)
        .bg(bg)
        .text_size(d.text_xs)
        .font_weight(FontWeight::MEDIUM)
        .text_color(text_color)
        .child(label.into())
}
