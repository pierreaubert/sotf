//! Recording Configuration Step (Step 1)
//!
//! Device selection, channel routing, and microphone calibration.

use crate::app::types::{ChannelMapping, RecordingState};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, Card, HStack, Input, InputSize,
    NumberInput, NumberInputSize, Select, SelectOption, StackAlign, StackSpacing, Text, TextSize,
    TextWeight, VStack,
};

/// Standard channel group definitions
const CHANNEL_GROUPS: &[(&str, &str)] = &[
    ("L", "Left (L)"),
    ("R", "Right (R)"),
    ("C", "Center (C)"),
    ("LFE", "Subwoofer (LFE)"),
    ("SL", "Surround Left (SL)"),
    ("SR", "Surround Right (SR)"),
    ("TFL", "Top Front Left"),
    ("TFR", "Top Front Right"),
    ("TBL", "Top Back Left"),
    ("TBR", "Top Back Right"),
];

impl PlayerView {
    /// Render the config step UI
    pub(crate) fn render_recording_config_step(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let recording_state = &state.app.recording_state;

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                // Header
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(
                        Text::new("Audio Device Configuration")
                            .size(TextSize::Lg)
                            .weight(TextWeight::Bold)
                            .color(theme.text_primary),
                    )
                    .child(
                        Text::new("Configure your playback and recording devices, set up channel routing, and load microphone calibration.")
                            .size(TextSize::Sm)
                            .color(theme.text_secondary),
                    ),
            )
            .child(
                // Two-column layout for devices
                div()
                    .flex()
                    .flex_row()
                    .gap_4()
                    .child(self.render_playback_device_section(cx))
                    .child(self.render_recording_device_section(cx)),
            )
            .child(self.render_mic_calibration_section(cx))
    }

    /// Render playback device configuration section
    fn render_playback_device_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (theme, num_channels, sample_rate) = {
            let state = self.state.read(cx);
            (
                state.app.theme.clone(),
                state.app.recording_state.playback_config.num_channels,
                state.app.recording_state.playback_config.sample_rate,
            )
        };
        let view = cx.entity().clone();

        // Build the card content, calling child renderers inline
        let header = HStack::new()
            .spacing(StackSpacing::Sm)
            .align(StackAlign::Center)
            .child(
                Text::new("PLAYBACK")
                    .size(TextSize::Sm)
                    .weight(TextWeight::Bold)
                    .color(theme.accent),
            );

        let device_label = VStack::new().spacing(StackSpacing::Sm).child(
            Text::new("Output Device")
                .size(TextSize::Sm)
                .weight(TextWeight::Semibold)
                .color(theme.text_secondary),
        );

        let badges = HStack::new()
            .spacing(StackSpacing::Sm)
            .child(Badge::new(format!("{} ch", num_channels)).variant(BadgeVariant::Info))
            .child(Badge::new(format!("{} kHz", sample_rate / 1000)).variant(BadgeVariant::Info));

        let channel_count_row = HStack::new()
            .spacing(StackSpacing::Md)
            .align(StackAlign::Center)
            .child(
                Text::new("Number of channels:")
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
            .child({
                let view = view.clone();
                NumberInput::new("playback_channel_count")
                    .value(num_channels as f64)
                    .min(1.0)
                    .max(16.0)
                    .step(1.0)
                    .size(NumberInputSize::Sm)
                    .on_change({
                        let view = view.clone();
                        move |value, _window, cx| {
                            view.update(cx, |this, cx| {
                                this.state.update(cx, |state, _| {
                                    state.app.recording_state.playback_config.num_channels =
                                        value as usize;
                                    update_playback_channel_mappings(
                                        &mut state.app.recording_state,
                                    );
                                });
                                cx.notify();
                            });
                        }
                    })
            });

        // Render device dropdown first, converting to AnyElement to release borrow
        let device_dropdown = self.render_playback_device_dropdown(cx).into_any_element();
        // Render channel mapping second (after first borrow is released)
        let channel_mapping = self.render_playback_channel_mapping(cx).into_any_element();

        Card::new()
            .content(
                VStack::new()
                    .spacing(StackSpacing::Md)
                    .child(header)
                    .child(device_label.child(device_dropdown))
                    .child(badges)
                    .child(channel_count_row)
                    .child(channel_mapping),
            )
            .into_any_element()
    }

    /// Render playback device dropdown
    fn render_playback_device_dropdown(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let recording_state = &state.app.recording_state;
        let view = cx.entity().clone();

        let options: Vec<SelectOption> = state
            .app
            .output_devices
            .iter()
            .map(|d| SelectOption::new(d.name.clone(), d.name.clone()))
            .collect();

        let selected_value = if recording_state.playback_config.device_name.is_empty() {
            state
                .app
                .output_devices
                .first()
                .map(|d| d.name.clone())
                .unwrap_or_default()
        } else {
            recording_state.playback_config.device_name.clone()
        };

        Select::new("playback_device")
            .options(options)
            .selected(selected_value)
            .placeholder("Select playback device...")
            .is_open(recording_state.playback_device_dropdown_open)
            .on_toggle({
                let view = view.clone();
                move |is_open, _window, cx| {
                    view.update(cx, |this, cx| {
                        this.state.update(cx, |state, _| {
                            state.app.recording_state.playback_device_dropdown_open = is_open;
                        });
                        cx.notify();
                    });
                }
            })
            .on_change({
                let view = view.clone();
                move |value, _window, cx| {
                    view.update(cx, |this, cx| {
                        this.state.update(cx, |state, _| {
                            state.app.recording_state.playback_config.device_name =
                                value.to_string();
                            state.app.recording_state.playback_config.device_id = value.to_string();
                            // Update device info from selected device
                            if let Some(device) = state
                                .app
                                .output_devices
                                .iter()
                                .find(|d| d.name == value.as_ref())
                            {
                                if let Some(config) = &device.default_config {
                                    state.app.recording_state.playback_config.sample_rate =
                                        config.sample_rate;
                                }
                            }
                            state.app.recording_state.playback_device_dropdown_open = false;
                        });
                        cx.notify();
                    });
                }
            })
    }

    /// Render playback channel mapping table
    fn render_playback_channel_mapping(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Extract all needed data upfront, then release the borrow
        let (theme, channel_data) = {
            let state = self.state.read(cx);
            let mappings: Vec<_> = state
                .app
                .recording_state
                .playback_config
                .channel_mappings
                .iter()
                .map(|m| (m.interface_channel, m.group_name.clone()))
                .collect();
            (state.app.theme.clone(), mappings)
        };
        let view = cx.entity().clone();

        VStack::new()
            .spacing(StackSpacing::Sm)
            .children(channel_data.iter().enumerate().map(
                |(idx, (interface_channel, group_name))| {
                    let view = view.clone();
                    let theme = theme.clone();
                    let interface_ch = *interface_channel;

                    // Render the dropdown for this channel
                    let group_dropdown =
                        self.render_channel_group_dropdown(cx, idx, group_name, true);

                    HStack::new()
                        .spacing(StackSpacing::Md)
                        .align(StackAlign::Center)
                        .child(
                            Text::new(format!("Channel {}:", idx + 1))
                                .size(TextSize::Sm)
                                .color(theme.text_secondary),
                        )
                        .child(
                            HStack::new()
                                .spacing(StackSpacing::Sm)
                                .align(StackAlign::Center)
                                .child(
                                    Text::new("Interface")
                                        .size(TextSize::Xs)
                                        .color(theme.text_muted),
                                )
                                .child({
                                    let view = view.clone();
                                    NumberInput::new(SharedString::from(format!(
                                        "playback_interface_{}",
                                        idx
                                    )))
                                    .value((interface_ch + 1) as f64)
                                    .min(1.0)
                                    .max(16.0)
                                    .step(1.0)
                                    .size(NumberInputSize::Sm)
                                    .on_change({
                                        let view = view.clone();
                                        move |value, _window, cx| {
                                            view.update(cx, |this, cx| {
                                                this.state.update(cx, |state, _| {
                                                    if let Some(m) = state
                                                        .app
                                                        .recording_state
                                                        .playback_config
                                                        .channel_mappings
                                                        .get_mut(idx)
                                                    {
                                                        m.interface_channel =
                                                            (value as usize).saturating_sub(1);
                                                    }
                                                });
                                                cx.notify();
                                            });
                                        }
                                    })
                                }),
                        )
                        .child(group_dropdown)
                        .into_any_element()
                },
            ))
            .into_any_element()
    }

    /// Render channel group dropdown for a specific channel
    fn render_channel_group_dropdown(
        &self,
        cx: &mut Context<Self>,
        channel_idx: usize,
        current_group: &str,
        is_playback: bool,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let view = cx.entity().clone();

        let options: Vec<SelectOption> = std::iter::once(SelectOption::new("", "No group"))
            .chain(
                CHANNEL_GROUPS
                    .iter()
                    .map(|(id, name)| SelectOption::new(*id, *name)),
            )
            .collect();

        // Simple dropdown using Button + list for now since Select requires Entity state
        let current_label = CHANNEL_GROUPS
            .iter()
            .find(|(id, _)| *id == current_group)
            .map(|(_, name)| *name)
            .unwrap_or("No group");

        Button::new(
            SharedString::from(format!("group_{}_{}", is_playback, channel_idx)),
            current_label,
        )
        .variant(ButtonVariant::Secondary)
        .size(ButtonSize::Sm)
    }

    /// Render recording device configuration section
    fn render_recording_device_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (theme, num_channels, sample_rate) = {
            let state = self.state.read(cx);
            (
                state.app.theme.clone(),
                state.app.recording_state.recording_config.num_channels,
                state.app.recording_state.recording_config.sample_rate,
            )
        };
        let view = cx.entity().clone();

        // Build the card content, calling child renderers inline
        let header = HStack::new()
            .spacing(StackSpacing::Sm)
            .align(StackAlign::Center)
            .child(
                Text::new("RECORDING")
                    .size(TextSize::Sm)
                    .weight(TextWeight::Bold)
                    .color(theme.success),
            );

        let device_label = VStack::new().spacing(StackSpacing::Sm).child(
            Text::new("Input Device")
                .size(TextSize::Sm)
                .weight(TextWeight::Semibold)
                .color(theme.text_secondary),
        );

        let badges = HStack::new()
            .spacing(StackSpacing::Sm)
            .child(Badge::new(format!("{} ch", num_channels)).variant(BadgeVariant::Info))
            .child(Badge::new(format!("{} kHz", sample_rate / 1000)).variant(BadgeVariant::Info));

        let channel_count_row = HStack::new()
            .spacing(StackSpacing::Md)
            .align(StackAlign::Center)
            .child(
                Text::new("Number of channels:")
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
            .child({
                let view = view.clone();
                NumberInput::new("recording_channel_count")
                    .value(num_channels as f64)
                    .min(1.0)
                    .max(16.0)
                    .step(1.0)
                    .size(NumberInputSize::Sm)
                    .on_change({
                        let view = view.clone();
                        move |value, _window, cx| {
                            view.update(cx, |this, cx| {
                                this.state.update(cx, |state, _| {
                                    state.app.recording_state.recording_config.num_channels =
                                        value as usize;
                                    update_recording_channel_mappings(
                                        &mut state.app.recording_state,
                                    );
                                });
                                cx.notify();
                            });
                        }
                    })
            });

        // Render device dropdown first, converting to AnyElement to release borrow
        let device_dropdown = self.render_recording_device_dropdown(cx).into_any_element();
        // Render channel mapping second (after first borrow is released)
        let channel_mapping = self.render_recording_channel_mapping(cx).into_any_element();

        Card::new()
            .content(
                VStack::new()
                    .spacing(StackSpacing::Md)
                    .child(header)
                    .child(device_label.child(device_dropdown))
                    .child(badges)
                    .child(channel_count_row)
                    .child(channel_mapping),
            )
            .into_any_element()
    }

    /// Render recording device dropdown
    fn render_recording_device_dropdown(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let recording_state = &state.app.recording_state;
        let view = cx.entity().clone();

        let options: Vec<SelectOption> = state
            .app
            .input_devices
            .iter()
            .map(|d| SelectOption::new(d.name.clone(), d.name.clone()))
            .collect();

        let selected_value = if recording_state.recording_config.device_name.is_empty() {
            state
                .app
                .input_devices
                .first()
                .map(|d| d.name.clone())
                .unwrap_or_default()
        } else {
            recording_state.recording_config.device_name.clone()
        };

        Select::new("recording_device")
            .options(options)
            .selected(selected_value)
            .placeholder("Select recording device...")
            .is_open(recording_state.recording_device_dropdown_open)
            .on_toggle({
                let view = view.clone();
                move |is_open, _window, cx| {
                    view.update(cx, |this, cx| {
                        this.state.update(cx, |state, _| {
                            state.app.recording_state.recording_device_dropdown_open = is_open;
                        });
                        cx.notify();
                    });
                }
            })
            .on_change({
                let view = view.clone();
                move |value, _window, cx| {
                    view.update(cx, |this, cx| {
                        this.state.update(cx, |state, _| {
                            state.app.recording_state.recording_config.device_name =
                                value.to_string();
                            state.app.recording_state.recording_config.device_id =
                                value.to_string();
                            // Update device info from selected device
                            if let Some(device) = state
                                .app
                                .input_devices
                                .iter()
                                .find(|d| d.name == value.as_ref())
                            {
                                if let Some(config) = &device.default_config {
                                    state.app.recording_state.recording_config.sample_rate =
                                        config.sample_rate;
                                }
                            }
                            state.app.recording_state.recording_device_dropdown_open = false;
                        });
                        cx.notify();
                    });
                }
            })
    }

    /// Render recording channel mapping table
    fn render_recording_channel_mapping(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let recording_state = &state.app.recording_state;
        let view = cx.entity().clone();

        VStack::new()
            .spacing(StackSpacing::Sm)
            .children(
                recording_state
                    .recording_config
                    .channel_mappings
                    .iter()
                    .enumerate()
                    .map(|(idx, &interface_ch)| {
                        let view = view.clone();
                        let theme = theme.clone();

                        HStack::new()
                            .spacing(StackSpacing::Md)
                            .align(StackAlign::Center)
                            .child(
                                Text::new(format!("Channel {}:", idx + 1))
                                    .size(TextSize::Sm)
                                    .color(theme.text_secondary),
                            )
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .align(StackAlign::Center)
                                    .child(
                                        Text::new("Interface")
                                            .size(TextSize::Xs)
                                            .color(theme.text_muted),
                                    )
                                    .child({
                                        let view = view.clone();
                                        NumberInput::new(SharedString::from(format!(
                                            "recording_interface_{}",
                                            idx
                                        )))
                                        .value((interface_ch + 1) as f64)
                                        .min(1.0)
                                        .max(16.0)
                                        .step(1.0)
                                        .size(NumberInputSize::Sm)
                                        .on_change({
                                            let view = view.clone();
                                            move |value, _window, cx| {
                                                view.update(cx, |this, cx| {
                                                    this.state.update(cx, |state, _| {
                                                        if let Some(m) = state
                                                            .app
                                                            .recording_state
                                                            .recording_config
                                                            .channel_mappings
                                                            .get_mut(idx)
                                                        {
                                                            *m = (value as usize).saturating_sub(1);
                                                        }
                                                    });
                                                    cx.notify();
                                                });
                                            }
                                        })
                                    }),
                            )
                            .into_any_element()
                    }),
            )
            .into_any_element()
    }

    /// Render microphone calibration section
    fn render_mic_calibration_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let recording_state = &state.app.recording_state;
        let view = cx.entity().clone();

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Md)
                .child(
                    Text::new("MICROPHONE CALIBRATION")
                        .size(TextSize::Sm)
                        .weight(TextWeight::Bold)
                        .color(theme.warning),
                )
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Md)
                        .align(StackAlign::Center)
                        .child(
                            Input::new("calibration_file")
                                .placeholder("No calibration file loaded")
                                .value(
                                    recording_state
                                        .mic_calibration_path
                                        .clone()
                                        .unwrap_or_default(),
                                )
                                .size(InputSize::Md)
                                .disabled(true),
                        )
                        .child(
                            Button::new("browse_calibration", "Browse...")
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Md)
                                .on_click({
                                    let view = view.clone();
                                    move |_, cx| {
                                        view.update(cx, |this, cx| {
                                            this.browse_calibration_file(cx);
                                        });
                                    }
                                }),
                        )
                        .when(recording_state.mic_calibration_path.is_some(), |stack| {
                            let view = view.clone();
                            stack.child(
                                Button::new("clear_calibration", "Clear")
                                    .variant(ButtonVariant::Secondary)
                                    .size(ButtonSize::Md)
                                    .on_click({
                                        move |_, cx| {
                                            view.update(cx, |this, cx| {
                                                this.state.update(cx, |state, _| {
                                                    state
                                                        .app
                                                        .recording_state
                                                        .mic_calibration_path = None;
                                                });
                                                cx.notify();
                                            });
                                        }
                                    }),
                            )
                        }),
                )
                .child(
                    Text::new("Load a microphone calibration file (CSV) to compensate for microphone frequency response")
                        .size(TextSize::Xs)
                        .color(theme.text_muted),
                ),
        )
    }

    /// Open file dialog to browse for calibration file
    fn browse_calibration_file(&mut self, cx: &mut Context<Self>) {
        let state_entity = self.state.clone();

        cx.spawn(async move |_, cx| {
            // Open file dialog
            let file = rfd::AsyncFileDialog::new()
                .add_filter("CSV", &["csv", "txt"])
                .add_filter("All files", &["*"])
                .set_title("Select Microphone Calibration File")
                .pick_file()
                .await;

            if let Some(file) = file {
                let path = file.path().to_string_lossy().to_string();
                log::info!("Selected calibration file: {}", path);
                let _ = state_entity.update(&mut cx.clone(), |state, _| {
                    state.app.recording_state.mic_calibration_path = Some(path);
                });
            }
        })
        .detach();
    }
}

/// Update playback channel mappings when channel count changes
fn update_playback_channel_mappings(state: &mut RecordingState) {
    let target_count = state.playback_config.num_channels;
    let current_count = state.playback_config.channel_mappings.len();

    if target_count > current_count {
        // Add new mappings
        for i in current_count..target_count {
            let group = CHANNEL_GROUPS
                .get(i)
                .map(|(id, _)| id.to_string())
                .unwrap_or_default();
            state.playback_config.channel_mappings.push(ChannelMapping {
                interface_channel: i,
                group_name: group,
            });
        }
    } else if target_count < current_count {
        // Remove extra mappings
        state
            .playback_config
            .channel_mappings
            .truncate(target_count);
    }
}

/// Update recording channel mappings when channel count changes
fn update_recording_channel_mappings(state: &mut RecordingState) {
    let target_count = state.recording_config.num_channels;
    let current_count = state.recording_config.channel_mappings.len();

    if target_count > current_count {
        // Add new mappings
        for i in current_count..target_count {
            state.recording_config.channel_mappings.push(i);
        }
    } else if target_count < current_count {
        // Remove extra mappings
        state
            .recording_config
            .channel_mappings
            .truncate(target_count);
    }
}
