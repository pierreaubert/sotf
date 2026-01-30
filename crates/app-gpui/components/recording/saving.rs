//! Recording Saving Step (Step 4)
//!
//! Save recordings and configuration to disk.

use crate::app::types::ChannelRecordingState;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Card, HStack, Input, StackAlign, StackSpacing, Text,
    TextSize, TextWeight, VStack,
};

impl PlayerView {
    /// Render the saving step UI
    pub(crate) fn render_recording_saving_step(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let recording_state = &state.app.measurement_state.recording_state;

        let has_recordings = recording_state
            .channel_recordings
            .iter()
            .any(|r| r.state == ChannelRecordingState::Done);

        let recording_dir = recording_state.recording_directory.clone();

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                // Header
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(
                        Text::new("Save Recordings")
                            .size(TextSize::Lg)
                            .weight(TextWeight::Bold)
                            .color(theme.text_primary),
                    )
                    .child(
                        Text::new("Save your recordings and configuration to disk. Files will be saved to the directory you selected during setup.")
                            .size(TextSize::Sm)
                            .color(theme.text_secondary),
                    ),
            )
            .child(self.render_save_name_card(cx))
            .child(self.render_save_location_card(cx))
            .child(self.render_save_contents_card(cx))
            .child(self.render_save_actions(has_recordings, recording_dir, cx))
    }

    /// Render the save name input card
    fn render_save_name_card(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let save_name = state
            .app
            .measurement_state
            .recording_state
            .save_name
            .clone();
        let view = cx.entity().clone();

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Md)
                .child(
                    Text::new("RECORDING NAME")
                        .size(TextSize::Sm)
                        .weight(TextWeight::Bold)
                        .color(theme.accent),
                )
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Md)
                        .align(StackAlign::Center)
                        .child(
                            Text::new("Name:")
                                .size(TextSize::Sm)
                                .color(theme.text_secondary),
                        )
                        .child(
                            div()
                                .w(px(300.0))
                                // Stop keyboard/mouse events from propagating to global handlers
                                .on_key_down(|_event, _window, cx| {
                                    cx.stop_propagation();
                                })
                                .on_mouse_down(MouseButton::Left, |_event, _window, cx| {
                                    cx.stop_propagation();
                                })
                                .child(
                                    Input::new("save_name_input")
                                        .value(save_name.clone())
                                        .placeholder("Enter recording name")
                                        .on_text_change({
                                            let view = view.clone();
                                            move |value, _window, cx| {
                                                view.update(cx, |this, cx| {
                                                    this.state.update(cx, |state, _| {
                                                        state
                                                            .app
                                                            .measurement_state
                                                            .recording_state
                                                            .save_name = value;
                                                    });
                                                    cx.notify();
                                                });
                                            }
                                        }),
                                ),
                        ),
                )
                .child(
                    Text::new(
                        "This name will be used for the subdirectory containing your recordings.",
                    )
                    .size(TextSize::Xs)
                    .color(theme.text_muted),
                ),
        )
    }

    /// Render the save location card
    fn render_save_location_card(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let recording_state = &state.app.measurement_state.recording_state;
        let view = cx.entity().clone();

        let base_dir = recording_state.recording_base_directory.clone();
        let base_dir_display = base_dir.clone().unwrap_or_else(|| "Not set".to_string());

        let save_name = &recording_state.save_name;
        let full_path = if base_dir.is_some() {
            format!("{}/{}/", base_dir_display, save_name)
        } else {
            "Not set".to_string()
        };

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Md)
                .child(
                    Text::new("SAVE LOCATION")
                        .size(TextSize::Sm)
                        .weight(TextWeight::Bold)
                        .color(theme.accent),
                )
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Md)
                        .align(StackAlign::Center)
                        .child(
                            Text::new("Base Directory:")
                                .size(TextSize::Sm)
                                .color(theme.text_secondary),
                        )
                        .child(
                            Text::new(base_dir_display.clone())
                                .size(TextSize::Sm)
                                .color(theme.text_primary),
                        )
                        .child(
                            Button::new("browse_save_dir", "Browse...")
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Sm)
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
                                Button::new("clear_save_dir", "Clear")
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
                                                    state
                                                        .app
                                                        .measurement_state
                                                        .recording_state
                                                        .recording_directory = None;
                                                });
                                                cx.notify();
                                            });
                                        }
                                    }),
                            )
                        }),
                )
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Sm)
                        .align(StackAlign::Center)
                        .child(
                            Text::new("Full Path:")
                                .size(TextSize::Sm)
                                .color(theme.text_secondary),
                        )
                        .child(
                            Text::new(full_path)
                                .size(TextSize::Sm)
                                .weight(TextWeight::Semibold)
                                .color(theme.text_primary),
                        ),
                ),
        )
    }

    /// Render the save contents card showing what will be saved
    fn render_save_contents_card(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let recording_state = &state.app.measurement_state.recording_state;
        let save_name = &recording_state.save_name;

        let recorded_channels: Vec<_> = recording_state
            .channel_recordings
            .iter()
            .filter(|r| r.state == ChannelRecordingState::Done)
            .collect();

        // Create safe version of save_name for filenames
        let safe_save_name: String = save_name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Md)
                .child(
                    Text::new("FILES TO SAVE")
                        .size(TextSize::Sm)
                        .weight(TextWeight::Bold)
                        .color(theme.accent),
                )
                .child(
                    VStack::new()
                        .spacing(StackSpacing::Sm)
                        // recordings.json
                        .child(
                            HStack::new()
                                .spacing(StackSpacing::Sm)
                                .align(StackAlign::Center)
                                .child(Text::new("+").size(TextSize::Sm).color(theme.success))
                                .child(
                                    Text::new(format!("{}.json", safe_save_name))
                                        .size(TextSize::Sm)
                                        .weight(TextWeight::Semibold)
                                        .color(theme.text_primary),
                                )
                                .child(
                                    Text::new("- Configuration and measurement data")
                                        .size(TextSize::Sm)
                                        .color(theme.text_muted),
                                ),
                        )
                        // Per-channel files
                        .children(
                            recorded_channels
                                .iter()
                                .flat_map(|rec| {
                                    let safe_channel_name: String = rec
                                        .channel_name
                                        .chars()
                                        .map(|c| {
                                            if c.is_alphanumeric() || c == '_' || c == '-' {
                                                c
                                            } else {
                                                '_'
                                            }
                                        })
                                        .collect();

                                    vec![
                                        HStack::new()
                                            .spacing(StackSpacing::Sm)
                                            .align(StackAlign::Center)
                                            .child(
                                                Text::new("+")
                                                    .size(TextSize::Sm)
                                                    .color(theme.success),
                                            )
                                            .child(
                                                Text::new(format!(
                                                    "{}_{}.wav",
                                                    safe_save_name, safe_channel_name
                                                ))
                                                .size(TextSize::Sm)
                                                .color(theme.text_primary),
                                            )
                                            .child(
                                                Text::new(format!(
                                                    "- {} recording",
                                                    rec.channel_name
                                                ))
                                                .size(TextSize::Sm)
                                                .color(theme.text_muted),
                                            )
                                            .into_any_element(),
                                        HStack::new()
                                            .spacing(StackSpacing::Sm)
                                            .align(StackAlign::Center)
                                            .child(
                                                Text::new("+")
                                                    .size(TextSize::Sm)
                                                    .color(theme.success),
                                            )
                                            .child(
                                                Text::new(format!(
                                                    "{}_{}.csv",
                                                    safe_save_name, safe_channel_name
                                                ))
                                                .size(TextSize::Sm)
                                                .color(theme.text_primary),
                                            )
                                            .child(
                                                Text::new(format!(
                                                    "- {} frequency response",
                                                    rec.channel_name
                                                ))
                                                .size(TextSize::Sm)
                                                .color(theme.text_muted),
                                            )
                                            .into_any_element(),
                                    ]
                                })
                                .collect::<Vec<_>>(),
                        ),
                ),
        )
    }

    /// Render save action buttons
    fn render_save_actions(
        &self,
        has_recordings: bool,
        recording_dir: Option<String>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let status_message = state
            .app
            .measurement_state
            .recording_state
            .status_message
            .clone();
        let view = cx.entity().clone();

        let can_save = has_recordings && recording_dir.is_some();

        Card::new().content(
            VStack::new()
                .spacing(StackSpacing::Md)
                .child(
                    Text::new("ACTIONS")
                        .size(TextSize::Sm)
                        .weight(TextWeight::Bold)
                        .color(theme.accent),
                )
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Md)
                        .child(
                            Button::new("save_recordings", "Save All")
                                .variant(ButtonVariant::Primary)
                                .size(ButtonSize::Lg)
                                .disabled(!can_save)
                                .theme(theme.to_button_theme())
                                .on_click({
                                    let view = view.clone();
                                    move |_, cx| {
                                        view.update(cx, |this, cx| {
                                            this.save_recordings(cx);
                                        });
                                    }
                                }),
                        )
                        .child(
                            Button::new("load_recordings", "Load Previous")
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Md)
                                .theme(theme.to_button_theme())
                                .on_click({
                                    let view = view.clone();
                                    move |_, cx| {
                                        view.update(cx, |this, cx| {
                                            this.load_recordings_from_file(cx);
                                        });
                                    }
                                }),
                        ),
                )
                .when(!status_message.is_empty(), |stack| {
                    let theme = theme.clone();
                    let is_error = status_message.to_lowercase().contains("error")
                        || status_message.to_lowercase().contains("failed");
                    stack.child(
                        Text::new(status_message.clone())
                            .size(TextSize::Sm)
                            .color(if is_error { theme.error } else { theme.success }),
                    )
                })
                .when(!can_save, |stack| {
                    let reason = if !has_recordings {
                        "No recordings to save. Go back to capture some channels."
                    } else {
                        "No save directory selected. Go back to setup to select a directory."
                    };
                    stack.child(Text::new(reason).size(TextSize::Sm).color(theme.warning))
                }),
        )
    }
}
