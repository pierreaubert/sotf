use crate::components::icons::{Icon, IconName, IconSize};
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
        let translations = state.app.ui_state.translations.clone();
        let workflow_text =
            crate::app::i18n::RoomEqWorkflowTranslations::for_language(state.app.ui_state.language);
        let theme = state.app.ui_state.theme.clone();
        let runtime_text =
            crate::app::i18n::RuntimeMessageTranslations::for_language(state.app.ui_state.language);
        let error_message = state
            .app
            .measurement_state
            .room_eq_state
            .error_message
            .as_deref()
            .map(|message| runtime_text.translate(message).into_owned());
        let status_message = runtime_text
            .translate(&state.app.measurement_state.room_eq_state.status_message)
            .into_owned();
        let has_measurements = state.app.measurement_state.room_eq_state.has_measurements();

        // Check if there are valid recordings in the recording state
        let has_recording_session_data = state
            .app
            .measurement_state
            .recording_state
            .channel_recordings
            .iter()
            .any(|r| r.state == crate::app::types::ChannelRecordingState::Done);

        VStack::new()
            .spacing(StackSpacing::Md)
            .child(
                Text::new(translations.roomeq_load_measurement_data)
                    .weight(TextWeight::Bold)
                    .size(TextSize::Md),
            )
            .child(
                Text::new(workflow_text.load_measurement_description)
                .size(TextSize::Xs)
                .color(theme.text_secondary),
            )
            // Error message display
            .when(error_message.is_some(), |div| {
                div.child(
                    Card::new()
                        .background(theme.surface)
                        .header_background(theme.background_secondary)
                        .border(theme.border)
                        .content(
                            VStack::new()
                                .spacing(StackSpacing::Xs)
                                .child(
                                    HStack::new()
                                        .spacing(StackSpacing::Xs)
                                        .align(StackAlign::Center)
                                        .child(
                                            Text::new(translations.roomeq_error)
                                                .weight(TextWeight::Bold)
                                                .size(TextSize::Xs)
                                                .color(theme.error),
                                        )
                                        .child(
                                            Text::new(error_message.unwrap_or_default())
                                                .size(TextSize::Xs)
                                                .color(theme.text_primary),
                                        ),
                                )
                                .child(
                                    Button::new("dismiss_error", workflow_text.dismiss)
                                        .variant(ButtonVariant::Secondary)
                                        .size(ButtonSize::Xs)
                                        .theme(theme.to_button_theme())
                                        .on_click_event(cx.listener(|view, _, _, cx| {
                                                view.state.update(cx, |state, _| {
                                                    state.app.measurement_state.room_eq_state.error_message = None;
                                                });
                                                cx.notify();
                                            })),
                                ),
                        )
                        .into_any_element()
                        .into_any(),
                )
            })
            // Two source cards side by side
            .child(
                HStack::new()
                    .spacing(StackSpacing::Md)
                    .child(
                        div().flex_1().child(
                            Card::new()
                                .background(theme.surface)
                                .header_background(theme.background_secondary)
                                .border(theme.border)
                                .header(
                                    Text::new(translations.roomeq_from_recording_session)
                                        .color(theme.text_primary)
                                        .weight(TextWeight::Semibold),
                                )
                                .content(
                                    VStack::new()
                                        .spacing(StackSpacing::Sm)
                                        .child(
                                            Text::new(if has_recording_session_data {
                                                "Use measurements from the Recording screen."
                                            } else {
                                                "No recordings found. Go to the Recording screen to measure your speakers."
                                            })
                                            .size(TextSize::Xs)
                                            .color(theme.text_secondary),
                                        )
                                        .child(if has_recording_session_data {
                                            Button::new(
                                                "load_from_recording",
                                                workflow_text.load_from_recording,
                                            )
                                                .variant(ButtonVariant::Primary)
                                                .size(ButtonSize::Sm)
                                                .theme(theme.to_button_theme())
                                                .on_click_event(cx.listener(|view, _, _, cx| {
                                                        view.load_room_eq_from_recording(cx);
                                                    }))
                                        } else {
                                            Button::new(
                                                "go_to_recording",
                                                workflow_text.go_to_recording,
                                            )
                                                .variant(ButtonVariant::Primary)
                                                .size(ButtonSize::Sm)
                                                .theme(theme.to_button_theme())
                                                .on_click_event(cx.listener(|view, _, _, cx| {
                                                        view.switch_screen(crate::app::Screen::Recording, cx);
                                                    }))
                                        }),
                                ),
                        ),
                    )
                    .child(
                        div().flex_1().child(
                            Card::new()
                                .background(theme.surface)
                                .header_background(theme.background_secondary)
                                .border(theme.border)
                                .header(
                                    Text::new(translations.roomeq_from_json_file)
                                        .color(theme.text_primary)
                                        .weight(TextWeight::Semibold),
                                )
                                .content(
                                    VStack::new()
                                        .spacing(StackSpacing::Sm)
                                        .child(
                                            Text::new(translations.roomeq_import_from_json_desc)
                                                .size(TextSize::Xs)
                                                .color(theme.text_secondary),
                                        )
                                        .child(
                                            Button::new(
                                                "load_from_file",
                                                workflow_text.import_from_file,
                                            )
                                                .variant(ButtonVariant::Primary)
                                                .size(ButtonSize::Sm)
                                                .theme(theme.to_button_theme())
                                                .on_click_event(cx.listener(|view, _, _, cx| {
                                                        view.load_room_eq_from_file(cx);
                                                    })),
                                        ),
                                ),
                        ),
                    ),
            )
            // Success status below the cards. The shared wizard header owns
            // navigation so every step has one predictable primary action.
            .when(has_measurements && !status_message.is_empty(), |vstack| {
                vstack.child(
                        Card::new()
                            .background(theme.surface)
                            .header_background(theme.background_secondary)
                            .border(theme.border)
                            .content(
                                HStack::new()
                                    .spacing(StackSpacing::Xs)
                                    .align(StackAlign::Center)
                                    .child(
                                        Icon::new(IconName::Check)
                                            .size(IconSize::Xs)
                                            .color(theme.success),
                                    )
                                    .child(
                                        Text::new(status_message.clone())
                                            .size(TextSize::Xs)
                                            .color(theme.text_primary),
                                    ),
                            ),
                    )
            })
    }
}
