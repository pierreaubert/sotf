use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Card, HStack, StackAlign, StackSpacing, Text, TextSize,
    TextWeight, VStack,
};

// === Step Content Renderers ===

impl PlayerView {
    pub(crate) fn render_room_eq_load_data(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let error_message = state.app.room_eq_state.error_message.clone();
        let status_message = state.app.room_eq_state.status_message.clone();
        let has_measurements = state.app.room_eq_state.has_measurements();

        // Check if there are valid recordings in the recording state
        let has_recording_session_data = state
            .app
            .recording_state
            .channel_recordings
            .iter()
            .any(|r| r.state == crate::app::types::ChannelRecordingState::Done);

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Text::new("Load Measurement Data")
                    .weight(TextWeight::Bold)
                    .size(TextSize::Lg),
            )
            .child(
                Text::new(
                    "Load measurement data from a previous recording session or import from a JSON file.",
                )
                .size(TextSize::Sm)
                .color(theme.text_secondary),
            )
            // Success message display (simple inline)
            .when(has_measurements && !status_message.is_empty(), |div| {
                div.child(
                    HStack::new()
                        .spacing(StackSpacing::Sm)
                        .align(StackAlign::Center)
                        .child(
                            Text::new("✓")
                                .weight(TextWeight::Bold)
                                .size(TextSize::Sm)
                                .color(theme.success),
                        )
                        .child(
                            Text::new(status_message.clone())
                                .size(TextSize::Sm)
                                .color(theme.text_primary),
                        ),
                )
            })
            // Error message display
            .when(error_message.is_some(), |div| {
                div.child(
                    Card::new()
                        .background(theme.surface)
                        .header_background(theme.background_secondary)
                        .border(theme.border)
                        .content(
                            VStack::new()
                                .spacing(StackSpacing::Sm)
                                .child(
                                    HStack::new()
                                        .spacing(StackSpacing::Sm)
                                        .align(StackAlign::Center)
                                        .child(
                                            Text::new("Error")
                                                .weight(TextWeight::Bold)
                                                .size(TextSize::Sm)
                                                .color(theme.error),
                                        )
                                        .child(
                                            Text::new(error_message.unwrap_or_default())
                                                .size(TextSize::Sm)
                                                .color(theme.text_primary),
                                        ),
                                )
                                .child(
                                    Button::new("dismiss_error", "Dismiss")
                                        .variant(ButtonVariant::Secondary)
                                        .size(ButtonSize::Sm)
                                        .build()
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(|view, _, _, cx| {
                                                view.state.update(cx, |state, _| {
                                                    state.app.room_eq_state.error_message = None;
                                                });
                                                cx.notify();
                                            }),
                                        ),
                                ),
                        )
                        .into_any_element()
                        .into_any(),
                )
            })
            .child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(
                        Text::new("From Recording Session")
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Text::new(if has_recording_session_data {
                                    "Use measurements from the Recording screen."
                                } else {
                                    "No recordings found. Go to the Recording screen to measure your speakers."
                                })
                                .size(TextSize::Sm)
                                .color(theme.text_secondary),
                            )
                            .child(if has_recording_session_data {
                                Button::new("load_from_recording", "Load from Recording")
                                    .variant(ButtonVariant::Primary)
                                    .size(ButtonSize::Lg)
                                    .theme(theme.to_button_theme())
                                    .build()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _, _, cx| {
                                            view.load_room_eq_from_recording(cx);
                                        }),
                                    )
                            } else {
                                Button::new("go_to_recording", "Go to Recording")
                                    .variant(ButtonVariant::Primary)
                                    .size(ButtonSize::Lg)
                                    .theme(theme.to_button_theme())
                                    .build()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _, _, cx| {
                                            view.switch_screen(crate::app::Screen::Recording, cx);
                                        }),
                                    )
                            }),
                    ),
            )
            .child(
                Card::new()
                    .background(theme.surface)
                    .header_background(theme.background_secondary)
                    .border(theme.border)
                    .header(
                        Text::new("From JSON File")
                            .color(theme.text_primary)
                            .weight(TextWeight::Semibold),
                    )
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Md)
                            .child(
                                Text::new("Import measurements from a previously saved JSON file.")
                                    .size(TextSize::Sm)
                                    .color(theme.text_secondary),
                            )
                            .child(
                                Button::new("load_from_file", "Import from File")
                                    .variant(ButtonVariant::Primary)
                                    .size(ButtonSize::Lg)
                                    .theme(theme.to_button_theme())
                                    .build()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _, _, cx| {
                                            view.load_room_eq_from_file(cx);
                                        }),
                                    ),
                            ),
                    ),
            )
    }

}
