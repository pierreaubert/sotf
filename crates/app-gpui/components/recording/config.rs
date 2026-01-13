//! Recording Configuration Step (Step 1)
//!
//! Device selection, channel routing, and microphone calibration.

use crate::app::types::{CalibrationData, ChannelMapping, RecordingState, SpeakerConfiguration};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use d3rs::axis::{AxisConfig, DefaultAxisTheme, render_axis};
use d3rs::color::D3Color;
use d3rs::grid::{GridConfig, render_grid};
use d3rs::prelude::LogScale;
use d3rs::scale::LinearScale;
use d3rs::shape::{LineConfig, LinePoint, render_line};
use gpui_ui_kit::{
    Accordion, AccordionItem, AccordionMode, Badge, BadgeVariant, Button, ButtonSize,
    ButtonVariant, HStack, Input, InputSize, NumberInput, NumberInputSize, Select, SelectOption,
    StackAlign, StackSpacing, Text, TextSize, TextWeight, VStack,
};

/// Standard channel group definitions
const CHANNEL_GROUPS: &[(&str, &str)] = &[
    ("L", "Left (L)"),
    ("R", "Right (R)"),
    ("C", "Center (C)"),
    ("LFE", "Subwoofer (LFE)"),
    ("SL", "Surround Left (SL)"),
    ("SR", "Surround Right (SR)"),
    ("BL", "Back Left (BL)"),
    ("BR", "Back Right (BR)"),
    ("WL", "Wide Left (WL)"),
    ("WR", "Wide Right (WR)"),
    ("TFL", "Top Front Left (TFL)"),
    ("TFR", "Top Front Right (TFR)"),
    ("TML", "Top Middle Left (TML)"),
    ("TMR", "Top Middle Right (TMR)"),
    ("TBL", "Top Back Left (TBL)"),
    ("TBR", "Top Back Right (TBR)"),
];

impl PlayerView {
    /// Render the config step UI
    pub(crate) fn render_recording_config_step(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let expanded_sections = state.app.measurement_state.recording_state.config_accordion_expanded.clone();
        let view = cx.entity().clone();

        // Build accordion content for each section (convert to AnyElement to release borrows)
        let playback_content = self.render_playback_device_content(cx).into_any_element();
        let recording_content = self.render_recording_device_content(cx).into_any_element();
        let calibration_content = self.render_mic_calibration_content(cx).into_any_element();
        let output_dir_content = self.render_output_directory_content(cx).into_any_element();

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                // Header
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(
                        Text::new("Audio Device Configuration")
                            .size(TextSize::Lg)
                            .weight(TextWeight::Bold),
                    )
                    .child(
                        Text::new("Configure your playback and recording devices, set up channel routing, and load microphone calibration.")
                            .size(TextSize::Sm),
                    ),
            )
            .child(
                // Accordion with four sections
                Accordion::new()
                    .mode(AccordionMode::Multiple)
                    .expanded(expanded_sections)
                    .item(
                        AccordionItem::new("playback", "Playback Device")
                            .content(playback_content),
                    )
                    .item(
                        AccordionItem::new("recording", "Recording Device")
                            .content(recording_content),
                    )
                    .item(
                        AccordionItem::new("calibration", "Microphone Calibration")
                            .content(calibration_content),
                    )
                    .item(
                        AccordionItem::new("output_dir", "Output Directory")
                            .content(output_dir_content),
                    )
                    .on_change({
                        let view = view.clone();
                        move |item_id, is_expanded, _window, cx| {
                            view.update(cx, |this, cx| {
                                this.state.update(cx, |state, _| {
                                    let expanded =
                                        &mut state.app.measurement_state.recording_state.config_accordion_expanded;
                                    if is_expanded {
                                        if !expanded.contains(item_id) {
                                            expanded.push(item_id.clone());
                                        }
                                    } else {
                                        expanded.retain(|id| id != item_id);
                                    }
                                });
                                cx.notify();
                            });
                        }
                    }),
            )
    }

    /// Render playback device content for accordion
    fn render_playback_device_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (theme, num_channels, sample_rate, speaker_config) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state.app.measurement_state.recording_state.playback_config.num_channels,
                state.app.measurement_state.recording_state.playback_config.sample_rate,
                state
                    .app
                    .measurement_state
                    .recording_state
                    .playback_config
                    .speaker_configuration,
            )
        };
        let view = cx.entity().clone();

        let device_label = VStack::new().spacing(StackSpacing::Sm).child(
            Text::new("Output Device")
                .size(TextSize::Sm)
                .weight(TextWeight::Semibold),
        );

        // Sample rate dropdown row
        let sample_rate_row = self
            .render_playback_sample_rate_dropdown(cx)
            .into_any_element();

        // Speaker configuration dropdown row
        let speaker_config_row = self.render_speaker_config_dropdown(cx).into_any_element();

        // Channel count row - only show for Custom configuration
        let channel_count_row = if speaker_config == SpeakerConfiguration::Custom {
            HStack::new()
                .spacing(StackSpacing::Md)
                .align(StackAlign::Center)
                .child(Text::new("Number of channels:").size(TextSize::Sm))
                .child({
                    let view = view.clone();
                    NumberInput::new("playback_channel_count")
                        .value(num_channels as f64)
                        .min(1.0)
                        .max(128.0)
                        .step(1.0)
                        .size(NumberInputSize::Sm)
                        .on_change({
                            let view = view.clone();
                            move |value, _window, cx| {
                                view.update(cx, |this, cx| {
                                    this.state.update(cx, |state, _| {
                                        state.app.measurement_state.recording_state.playback_config.num_channels =
                                            value as usize;
                                        update_playback_channel_mappings(
                                            &mut state.app.measurement_state.recording_state,
                                        );
                                    });
                                    cx.notify();
                                });
                            }
                        })
                })
                .into_any_element()
        } else {
            // Show info badge for preset configurations
            HStack::new()
                .spacing(StackSpacing::Md)
                .align(StackAlign::Center)
                .child(
                    Text::new("Channels:")
                        .size(TextSize::Sm)
                        .color(theme.text_secondary),
                )
                .child(Badge::new(format!("{}", num_channels)).variant(BadgeVariant::Info))
                .child(
                    Badge::new(format!("{} kHz", sample_rate / 1000)).variant(BadgeVariant::Info),
                )
                .into_any_element()
        };

        // Render device dropdown first, converting to AnyElement to release borrow
        let device_dropdown = self.render_playback_device_dropdown(cx).into_any_element();
        // Render channel mapping second (after first borrow is released)
        let channel_mapping = self.render_playback_channel_mapping(cx).into_any_element();

        VStack::new()
            .spacing(StackSpacing::Md)
            .child(device_label.child(device_dropdown))
            .child(sample_rate_row)
            .child(speaker_config_row)
            .child(channel_count_row)
            .child(channel_mapping)
    }

    /// Render recording device content for accordion
    fn render_recording_device_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (theme, num_channels, sample_rate) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state.app.measurement_state.recording_state.recording_config.num_channels,
                state.app.measurement_state.recording_state.recording_config.sample_rate,
            )
        };
        let view = cx.entity().clone();

        let device_label = VStack::new().spacing(StackSpacing::Sm).child(
            Text::new("Input Device")
                .size(TextSize::Sm)
                .weight(TextWeight::Semibold)
                .color(theme.text_secondary),
        );

        // Sample rate dropdown row
        let sample_rate_row = self
            .render_recording_sample_rate_dropdown(cx)
            .into_any_element();

        // Info badges
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
                    .max(128.0)
                    .step(1.0)
                    .size(NumberInputSize::Sm)
                    .on_change({
                        let view = view.clone();
                        move |value, _window, cx| {
                            view.update(cx, |this, cx| {
                                this.state.update(cx, |state, _| {
                                    state.app.measurement_state.recording_state.recording_config.num_channels =
                                        value as usize;
                                    update_recording_channel_mappings(
                                        &mut state.app.measurement_state.recording_state,
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

        VStack::new()
            .spacing(StackSpacing::Md)
            .child(device_label.child(device_dropdown))
            .child(sample_rate_row)
            .child(badges)
            .child(channel_count_row)
            .child(channel_mapping)
    }

    /// Render microphone calibration content for accordion
    fn render_mic_calibration_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let recording_state = &state.app.measurement_state.recording_state;
        let view = cx.entity().clone();

        // Extract calibration data for the graph
        let calibration_data = recording_state.mic_calibration_data.clone();
        let calibration_path = recording_state.mic_calibration_path.clone();

        VStack::new()
            .spacing(StackSpacing::Md)
            .child(
                HStack::new()
                    .spacing(StackSpacing::Md)
                    .align(StackAlign::Center)
                    .child(
                        Input::new("calibration_file")
                            .placeholder("No calibration file loaded")
                            .value(calibration_path.clone().unwrap_or_default())
                            .size(InputSize::Md)
                            .disabled(true),
                    )
                    .child(
                        Button::new("browse_calibration", "Browse...")
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Md)
                            .theme(theme.to_button_theme())
                            .on_click({
                                let view = view.clone();
                                move |_, cx| {
                                    view.update(cx, |this, cx| {
                                        this.browse_calibration_file(cx);
                                    });
                                }
                            }),
                    )
                    .when(calibration_path.is_some(), |stack| {
                        let view = view.clone();
                        let theme = theme.clone();
                        stack.child(
                            Button::new("clear_calibration", "Clear")
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Md)
                                .theme(theme.to_button_theme())
                                .on_click({
                                    move |_, cx| {
                                        view.update(cx, |this, cx| {
                                            this.state.update(cx, |state, _| {
                                                state.app.measurement_state.recording_state.mic_calibration_path =
                                                    None;
                                                state.app.measurement_state.recording_state.mic_calibration_data =
                                                    None;
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
            )
            // Add calibration graph when data is available
            .when_some(calibration_data, |stack, data| {
                stack.child(Self::render_calibration_graph(&data, &theme))
            })
    }

    /// Render output directory content for accordion
    fn render_output_directory_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let recording_state = &state.app.measurement_state.recording_state;
        let view = cx.entity().clone();

        let base_dir = recording_state.recording_base_directory.clone();
        let recording_dir = recording_state.recording_directory.clone();
        let has_directory = recording_dir.is_some();

        let display_path = recording_dir
            .clone()
            .unwrap_or_else(|| "No directory selected".to_string());

        VStack::new()
            .spacing(StackSpacing::Md)
            .child(
                HStack::new()
                    .spacing(StackSpacing::Md)
                    .align(StackAlign::Center)
                    .child(
                        Text::new("Recording files will be saved to:")
                            .size(TextSize::Sm)
                            .color(theme.text_secondary),
                    )
                    .child(
                        Text::new(display_path)
                            .size(TextSize::Sm)
                            .weight(TextWeight::Semibold)
                            .color(if has_directory {
                                theme.text_primary
                            } else {
                                theme.warning
                            }),
                    ),
            )
            .child(
                HStack::new()
                    .spacing(StackSpacing::Md)
                    .child(
                        Button::new("browse_output_dir", "Browse...")
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Md)
                            .theme(theme.to_button_theme())
                            .on_click({
                                let view = view.clone();
                                move |_, cx| {
                                    view.update(cx, |this, cx| {
                                        this.browse_recording_directory(cx);
                                    });
                                }
                            }),
                    )
                    .when(base_dir.is_some(), |stack| {
                        let view = view.clone();
                        let theme = theme.clone();
                        stack.child(
                            Button::new("clear_output_dir", "Clear")
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Sm)
                                .theme(theme.to_button_theme())
                                .on_click({
                                    move |_, cx| {
                                        view.update(cx, |this, cx| {
                                            this.state.update(cx, |state, _| {
                                                state
                                                    .app
                                                    .measurement_state
                                                    .recording_state
                                                    .recording_base_directory = None;
                                                state.app.measurement_state.recording_state.recording_directory =
                                                    None;
                                            });
                                            cx.notify();
                                        });
                                    }
                                }),
                        )
                    }),
            )
            .child(
                Text::new("A timestamped subdirectory will be created for each recording session.")
                    .size(TextSize::Xs)
                    .color(theme.text_muted),
            )
            .when(!has_directory, |stack| {
                stack.child(
                    HStack::new()
                        .spacing(StackSpacing::Sm)
                        .align(StackAlign::Center)
                        .child(Text::new("⚠").size(TextSize::Sm).color(theme.warning))
                        .child(
                            Text::new("You must select an output directory before recording.")
                                .size(TextSize::Sm)
                                .color(theme.warning),
                        ),
                )
            })
    }

    /// Open directory dialog to select recording output directory
    pub(crate) fn browse_recording_directory(&mut self, cx: &mut Context<Self>) {
        let state_entity = self.state.clone();

        cx.spawn(async move |_, cx| {
            // Open directory dialog
            let folder = rfd::AsyncFileDialog::new()
                .set_title("Select Recording Output Directory")
                .pick_folder()
                .await;

            if let Some(folder) = folder {
                let base_path = folder.path().to_string_lossy().to_string();
                log::info!("Selected recording directory: {}", base_path);

                // Create timestamped subdirectory name
                let now = chrono::Local::now();
                let timestamp_dir = format!("recording-{}", now.format("%Y%m%d-%H%M%S"));
                let full_path = std::path::Path::new(&base_path)
                    .join(&timestamp_dir)
                    .to_string_lossy()
                    .to_string();

                // Create the directory
                if let Err(e) = std::fs::create_dir_all(&full_path) {
                    log::error!("Failed to create recording directory: {}", e);
                    return;
                }

                log::info!("Created recording directory: {}", full_path);

                let _ = state_entity.update(&mut cx.clone(), |state, _| {
                    state.app.measurement_state.recording_state.recording_base_directory = Some(base_path);
                    state.app.measurement_state.recording_state.recording_directory = Some(full_path);
                });
            }
        })
        .detach();
    }

    /// Render playback device dropdown
    fn render_playback_device_dropdown(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let recording_state = &state.app.measurement_state.recording_state;
        let view = cx.entity().clone();

        let options: Vec<SelectOption> = state
            .app
            .audio_device_state
            .output_devices
            .iter()
            .map(|d| SelectOption::new(d.name.clone(), d.name.clone()))
            .collect();

        let selected_value = if recording_state.playback_config.device_name.is_empty() {
            state
                .app
                .audio_device_state
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
            .theme(theme.to_select_theme())
            .on_toggle({
                let view = view.clone();
                move |is_open, _window, cx| {
                    view.update(cx, |this, cx| {
                        this.state.update(cx, |state, _| {
                            state.app.measurement_state.recording_state.playback_device_dropdown_open = is_open;
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
                            state.app.measurement_state.recording_state.playback_config.device_name =
                                value.to_string();
                            state.app.measurement_state.recording_state.playback_config.device_id = value.to_string();
                            // Update device info from selected device
                            if let Some(device) = state
                                .app
                                .audio_device_state
                                .output_devices
                                .iter()
                                .find(|d| d.name == value.as_ref())
                            {
                                // Set default sample rate (prefer 48k, then 44.1k, then default)
                                let rates = &device.available_sample_rates;
                                let default_rate = if rates.contains(&48000) {
                                    48000
                                } else if rates.contains(&44100) {
                                    44100
                                } else {
                                    device
                                        .default_config
                                        .as_ref()
                                        .map(|c| c.sample_rate)
                                        .unwrap_or(48000)
                                };

                                state.app.measurement_state.recording_state.playback_config.sample_rate =
                                    default_rate;
                                // Update available sample rates from device
                                state
                                    .app
                                    .measurement_state
                                    .recording_state
                                    .playback_config
                                    .available_sample_rates = device.available_sample_rates.clone();
                            }
                            state.app.measurement_state.recording_state.playback_device_dropdown_open = false;
                        });
                        cx.notify();
                    });
                }
            })
    }

    /// Render playback sample rate dropdown
    fn render_playback_sample_rate_dropdown(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let recording_state = &state.app.measurement_state.recording_state;
        let view = cx.entity().clone();

        let available_rates = &recording_state.playback_config.available_sample_rates;
        let options: Vec<SelectOption> = available_rates
            .iter()
            .map(|&rate| {
                let label = if rate >= 1000 {
                    format!("{} kHz", rate / 1000)
                } else {
                    format!("{} Hz", rate)
                };
                SelectOption::new(rate.to_string(), label)
            })
            .collect();

        let selected_value = recording_state.playback_config.sample_rate.to_string();

        HStack::new()
            .spacing(StackSpacing::Md)
            .align(StackAlign::Center)
            .child(
                Text::new("Sample Rate:")
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
            .child(
                div().w(px(120.0)).child(
                    Select::new("playback_sample_rate")
                        .options(options)
                        .selected(selected_value)
                        .placeholder("Select rate...")
                        .is_open(recording_state.playback_sample_rate_dropdown_open)
                        .theme(theme.to_select_theme())
                        .on_toggle({
                            let view = view.clone();
                            move |is_open, _window, cx| {
                                view.update(cx, |this, cx| {
                                    this.state.update(cx, |state, _| {
                                        state
                                            .app
                                            .measurement_state
                                            .recording_state
                                            .playback_sample_rate_dropdown_open = is_open;
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
                                        if let Ok(rate) = value.parse::<u32>() {
                                            state.app.measurement_state.recording_state.playback_config.sample_rate =
                                                rate;
                                        }
                                        state
                                            .app
                                            .measurement_state
                                            .recording_state
                                            .playback_sample_rate_dropdown_open = false;
                                    });
                                    cx.notify();
                                });
                            }
                        }),
                ),
            )
    }

    /// Render speaker configuration dropdown
    fn render_speaker_config_dropdown(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let recording_state = &state.app.measurement_state.recording_state;
        let view = cx.entity().clone();

        let options: Vec<SelectOption> = SpeakerConfiguration::all()
            .iter()
            .map(|config| SelectOption::new(config.as_str(), config.as_str()))
            .collect();

        let selected_value = recording_state
            .playback_config
            .speaker_configuration
            .as_str();

        HStack::new()
            .spacing(StackSpacing::Md)
            .align(StackAlign::Center)
            .child(
                Text::new("Speaker Configuration:")
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
            .child(
                div().w(px(120.0)).child(
                    Select::new("speaker_config")
                        .options(options)
                        .selected(selected_value)
                        .placeholder("Select config...")
                        .is_open(recording_state.speaker_config_dropdown_open)
                        .theme(theme.to_select_theme())
                        .on_toggle({
                            let view = view.clone();
                            move |is_open, _window, cx| {
                                view.update(cx, |this, cx| {
                                    this.state.update(cx, |state, _| {
                                        state.app.measurement_state.recording_state.speaker_config_dropdown_open =
                                            is_open;
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
                                        // Find the matching configuration
                                        let new_config = SpeakerConfiguration::all()
                                            .iter()
                                            .find(|c| c.as_str() == value.as_ref())
                                            .copied()
                                            .unwrap_or(SpeakerConfiguration::Custom);

                                        state
                                            .app
                                            .measurement_state
                                            .recording_state
                                            .playback_config
                                            .speaker_configuration = new_config;

                                        // Update channel count and mappings based on configuration
                                        if new_config != SpeakerConfiguration::Custom {
                                            state
                                                .app
                                                .measurement_state
                                                .recording_state
                                                .playback_config
                                                .num_channels = new_config.channel_count();
                                            // Set default channel names for the configuration
                                            let channel_names = new_config.default_channel_names();
                                            state
                                                .app
                                                .measurement_state
                                                .recording_state
                                                .playback_config
                                                .channel_mappings = channel_names
                                                .iter()
                                                .enumerate()
                                                .map(|(i, name)| ChannelMapping {
                                                    interface_channel: i, // 1-indexed for display
                                                    group_name: name.to_string(),
                                                })
                                                .collect();
                                        }

                                        state.app.measurement_state.recording_state.speaker_config_dropdown_open =
                                            false;
                                    });
                                    cx.notify();
                                });
                            }
                        }),
                ),
            )
    }

    /// Render playback channel mapping table
    fn render_playback_channel_mapping(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Extract all needed data upfront, then release the borrow
        let (theme, channel_data) = {
            let state = self.state.read(cx);
            let mappings: Vec<_> = state
                .app
                .measurement_state
                .recording_state
                .playback_config
                .channel_mappings
                .iter()
                .map(|m| (m.interface_channel, m.group_name.clone()))
                .collect();
            (state.app.ui_state.theme.clone(), mappings)
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
                    let group_dropdown = self.render_channel_group_dropdown(cx, idx, group_name);

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
                                    .value(interface_ch as f64)
                                    .min(1.0)
                                    .max(128.0)
                                    .step(1.0)
                                    .size(NumberInputSize::Sm)
                                    .on_change({
                                        let view = view.clone();
                                        move |value, _window, cx| {
                                            view.update(cx, |this, cx| {
                                                this.state.update(cx, |state, _| {
                                                    if let Some(m) = state
                                                        .app
                                                        .measurement_state
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

    /// Render channel group dropdown for a specific channel (playback only)
    fn render_channel_group_dropdown(
        &self,
        cx: &mut Context<Self>,
        channel_idx: usize,
        current_group: &str,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let recording_state = &state.app.measurement_state.recording_state;
        let view = cx.entity().clone();
        let current_group = current_group.to_string();

        // Build options from CHANNEL_GROUPS
        let options: Vec<SelectOption> = CHANNEL_GROUPS
            .iter()
            .map(|(id, name)| SelectOption::new(*id, *name))
            .collect();

        // Check if this channel's dropdown is open
        let is_open = recording_state.channel_name_dropdown_open == Some(channel_idx);

        // Find the display label for current selection
        let selected_label: SharedString = CHANNEL_GROUPS
            .iter()
            .find(|(id, _)| *id == current_group.as_str())
            .map(|(_, name)| SharedString::from(*name))
            .unwrap_or_else(|| SharedString::from(current_group.clone()));

        div().w(px(160.0)).child(
            Select::new(SharedString::from(format!("channel_name_{}", channel_idx)))
                .options(options)
                .selected(current_group.clone())
                .placeholder(selected_label)
                .is_open(is_open)
                .theme(theme.to_select_theme())
                .on_toggle({
                    let view = view.clone();
                    move |is_open, _window, cx| {
                        view.update(cx, |this, cx| {
                            this.state.update(cx, |state, _| {
                                state.app.measurement_state.recording_state.channel_name_dropdown_open =
                                    if is_open { Some(channel_idx) } else { None };
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
                                if let Some(mapping) = state
                                    .app
                                    .measurement_state
                                    .recording_state
                                    .playback_config
                                    .channel_mappings
                                    .get_mut(channel_idx)
                                {
                                    mapping.group_name = value.to_string();
                                }
                                state.app.measurement_state.recording_state.channel_name_dropdown_open = None;
                            });
                            cx.notify();
                        });
                    }
                }),
        )
    }

    /// Render recording device dropdown
    fn render_recording_device_dropdown(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let recording_state = &state.app.measurement_state.recording_state;
        let view = cx.entity().clone();

        let options: Vec<SelectOption> = state
            .app
            .audio_device_state
            .input_devices
            .iter()
            .map(|d| SelectOption::new(d.name.clone(), d.name.clone()))
            .collect();

        let selected_value = if recording_state.recording_config.device_name.is_empty() {
            state
                .app
                .audio_device_state
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
            .theme(theme.to_select_theme())
            .on_toggle({
                let view = view.clone();
                move |is_open, _window, cx| {
                    view.update(cx, |this, cx| {
                        this.state.update(cx, |state, _| {
                            state.app.measurement_state.recording_state.recording_device_dropdown_open = is_open;
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
                            state.app.measurement_state.recording_state.recording_config.device_name =
                                value.to_string();
                            state.app.measurement_state.recording_state.recording_config.device_id =
                                value.to_string();
                            // Update device info from selected device
                            if let Some(device) = state
                                .app
                                .audio_device_state
                                .input_devices
                                .iter()
                                .find(|d| d.name == value.as_ref())
                            {
                                // Set default sample rate (prefer 48k, then 44.1k, then default)
                                let rates = &device.available_sample_rates;
                                let default_rate = if rates.contains(&48000) {
                                    48000
                                } else if rates.contains(&44100) {
                                    44100
                                } else {
                                    device
                                        .default_config
                                        .as_ref()
                                        .map(|c| c.sample_rate)
                                        .unwrap_or(48000)
                                };

                                state.app.measurement_state.recording_state.recording_config.sample_rate =
                                    default_rate;
                                // Update available sample rates from device
                                state
                                    .app
                                    .measurement_state
                                    .recording_state
                                    .recording_config
                                    .available_sample_rates = device.available_sample_rates.clone();
                            }
                            state.app.measurement_state.recording_state.recording_device_dropdown_open = false;
                        });
                        cx.notify();
                    });
                }
            })
    }

    /// Render recording sample rate dropdown
    fn render_recording_sample_rate_dropdown(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let recording_state = &state.app.measurement_state.recording_state;
        let view = cx.entity().clone();

        let available_rates = &recording_state.recording_config.available_sample_rates;
        let options: Vec<SelectOption> = available_rates
            .iter()
            .map(|&rate| {
                let label = if rate >= 1000 {
                    format!("{} kHz", rate / 1000)
                } else {
                    format!("{} Hz", rate)
                };
                SelectOption::new(rate.to_string(), label)
            })
            .collect();

        let selected_value = recording_state.recording_config.sample_rate.to_string();

        HStack::new()
            .spacing(StackSpacing::Md)
            .align(StackAlign::Center)
            .child(
                Text::new("Sample Rate:")
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
            .child(
                div().w(px(120.0)).child(
                    Select::new("recording_sample_rate")
                        .options(options)
                        .selected(selected_value)
                        .placeholder("Select rate...")
                        .is_open(recording_state.recording_sample_rate_dropdown_open)
                        .theme(theme.to_select_theme())
                        .on_toggle({
                            let view = view.clone();
                            move |is_open, _window, cx| {
                                view.update(cx, |this, cx| {
                                    this.state.update(cx, |state, _| {
                                        state
                                            .app
                                            .measurement_state
                                            .recording_state
                                            .recording_sample_rate_dropdown_open = is_open;
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
                                        if let Ok(rate) = value.parse::<u32>() {
                                            state
                                                .app
                                                .measurement_state
                                                .recording_state
                                                .recording_config
                                                .sample_rate = rate;
                                        }
                                        state
                                            .app
                                            .measurement_state
                                            .recording_state
                                            .recording_sample_rate_dropdown_open = false;
                                    });
                                    cx.notify();
                                });
                            }
                        }),
                ),
            )
    }

    /// Render recording channel mapping table
    fn render_recording_channel_mapping(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let recording_state = &state.app.measurement_state.recording_state;
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
                                        .value(interface_ch as f64)
                                        .min(1.0)
                                        .max(128.0)
                                        .step(1.0)
                                        .size(NumberInputSize::Sm)
                                        .on_change({
                                            let view = view.clone();
                                            move |value, _window, cx| {
                                                view.update(cx, |this, cx| {
                                                    this.state.update(cx, |state, _| {
                                                        if let Some(m) = state
                                                            .app
                                                            .measurement_state
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

    /// Render calibration data as a frequency response graph
    fn render_calibration_graph(
        data: &CalibrationData,
        theme: &crate::theme::Theme,
    ) -> impl IntoElement {
        let chart_width: f32 = 500.0;
        let chart_height: f32 = 200.0;
        let margin_left: f32 = 60.0;
        let margin_right: f32 = 20.0;
        let margin_top: f32 = 20.0;
        let margin_bottom: f32 = 40.0;

        let plot_width = chart_width - margin_left - margin_right;
        let plot_height = chart_height - margin_top - margin_bottom;

        let axis_theme = DefaultAxisTheme;

        // Create log scale for frequency (20Hz - 20kHz)
        let x_scale = LogScale::new()
            .domain(20.0, 20000.0)
            .range(0.0, plot_width as f64);

        // Find y-axis range (spl_db is f64)
        let (min_db, max_db) = data
            .spl_db
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), &v| {
                (min.min(v), max.max(v))
            });

        // Add some padding to Y range
        let y_min = if min_db.is_finite() { min_db - 5.0 } else { -10.0 };
        let y_max = if max_db.is_finite() { max_db + 5.0 } else { 10.0 };

        let y_scale = LinearScale::new()
            .domain(y_min, y_max)
            .range(plot_height as f64, 0.0);

        let freq_ticks: Vec<f64> = vec![
            20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0,
        ];
        
        let mag_range = (y_max - y_min) as i32;
        let mag_step = if mag_range > 40 {
            10
        } else if mag_range > 20 {
            5
        } else {
            2
        };
        let mag_ticks: Vec<f64> = ((y_min as i32 / mag_step * mag_step)
            ..=(y_max as i32 / mag_step * mag_step + mag_step))
            .step_by(mag_step as usize)
            .map(|v| v as f64)
            .collect();

        // Build line points
        let points: Vec<LinePoint> = data.frequencies
            .iter()
            .zip(data.spl_db.iter())
            .filter(|&(&f, _)| f >= 20.0 && f <= 20000.0)
            .map(|(&f, &m)| LinePoint {
                x: f as f64,
                y: m as f64,
            })
            .collect();

        // Use theme primary color for visibility (high contrast)
        // D3Color::from_rgba expects gpui::Rgba. Theme colors are usually Hsla, which implements Into<Rgba>.
        let stroke_color = D3Color::from_rgba(theme.text_primary.into());
        let line_config = LineConfig::new().stroke_color(stroke_color).stroke_width(2.0);
        let line_element = render_line(&x_scale, &y_scale, &points, &line_config).into_any_element();

        div()
            .mt_4()
            .p_2()
            .bg(theme.surface)
            .rounded_md()
            .border_1()
            .border_color(theme.border)
            .child(
                div()
                    .w(px(chart_width))
                    .h(px(chart_height))
                    .relative()
                    .child(
                        div()
                            .absolute()
                            .top(px(0.0))
                            .left(px(margin_left))
                            .child(
                                Text::new("Microphone Calibration Curve")
                                    .size(TextSize::Sm)
                                    .weight(TextWeight::Bold)
                                    .color(theme.accent)
                            )
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(0.0))
                            .top(px(margin_top))
                            .w(px(margin_left))
                            .h(px(plot_height))
                            .child(render_axis(
                                &y_scale,
                                &AxisConfig::left()
                                    .with_tick_values(mag_ticks.clone())
                                    // LABEL: SPL (dB) - Exact casing as requested, ensuring correctness
                                    .with_formatter(|v| format!("{:.0} dB", v)),
                                plot_height,
                                &axis_theme,
                            )),
                    )
                    // Y-Axis Label
                    .child(
                         div()
                            .absolute()
                            .left(px(10.0))
                            .top(px(margin_top + plot_height / 2.0 - 40.0))
                            .w(px(20.0))
                            .child(
                                Text::new("SPL (dB)")
                                    .size(TextSize::Xs)
                                    .color(theme.text_secondary)
                            )
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(margin_left))
                            .top(px(margin_top))
                            .w(px(plot_width))
                            .h(px(plot_height))
                            .overflow_hidden()
                            .child(render_grid(
                                &x_scale,
                                &y_scale,
                                &GridConfig::with_lines()
                                    .with_vertical_values(freq_ticks.clone())
                                    .with_horizontal_values(mag_ticks),
                                plot_width,
                                plot_height,
                                &axis_theme,
                            ))
                            .child(line_element),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(margin_left))
                            .top(px(margin_top + plot_height))
                            .w(px(plot_width))
                            .h(px(margin_bottom))
                            .child(render_axis(
                                &x_scale,
                                &AxisConfig::bottom()
                                    .with_tick_values(freq_ticks)
                                    .with_formatter(|f| {
                                        if f >= 1000.0 {
                                            format!("{:.0}k", f / 1000.0)
                                        } else {
                                            format!("{:.0}", f)
                                        }
                                    }),
                                plot_width,
                                &axis_theme,
                            )),
                    )
                    // X-Axis Label
                    .child(
                        div()
                            .absolute()
                            .left(px(margin_left + plot_width / 2.0 - 40.0))
                            .bottom(px(5.0))
                            .child(
                                Text::new("Frequency (Hz)")
                                    .size(TextSize::Xs)
                                    .color(theme.text_secondary)
                            )
                    )
            )
            .into_any_element()
    }

    /// Open file dialog to browse for calibration file
    fn browse_calibration_file(&mut self, cx: &mut Context<Self>) {
        use crate::app::types::CalibrationData;

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

                // Read and parse the calibration file
                let calibration_data = match std::fs::read_to_string(&path) {
                    Ok(content) => CalibrationData::parse(&content),
                    Err(e) => {
                        log::error!("Failed to read calibration file: {}", e);
                        None
                    }
                };

                if let Some(ref data) = calibration_data {
                    log::info!(
                        "Parsed calibration data: {} points, freq range {:.0}-{:.0} Hz",
                        data.frequencies.len(),
                        data.frequencies.first().unwrap_or(&0.0),
                        data.frequencies.last().unwrap_or(&0.0)
                    );
                }

                let _ = state_entity.update(&mut cx.clone(), |state, _| {
                    state.app.measurement_state.recording_state.mic_calibration_path = Some(path);
                    state.app.measurement_state.recording_state.mic_calibration_data = calibration_data;
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
                interface_channel: i, // 0-indexed internally
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
        // Add new mappings (1-indexed for display)
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
