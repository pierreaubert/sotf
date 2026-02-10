use crate::app::types::{RoomEqOptimizationMode, RoomEqStep};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Card, HStack, StackAlign, StackSpacing, Text, TextSize,
    TextWeight, VStack,
};

impl PlayerView {
    pub(crate) fn render_room_eq_select_mode(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let room_eq = &state.app.measurement_state.room_eq_state;
        let current_mode = room_eq.optimizer_config.mode;
        let release_channel = state.app.ui_state.release_channel;

        let available_modes = RoomEqOptimizationMode::available(release_channel);

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Text::new("Select Optimization Mode")
                    .weight(TextWeight::Bold)
                    .size(TextSize::Lg),
            )
            .child(
                Text::new("Choose the type of filters to generate for your room correction.")
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
            .child(
                HStack::new()
                    .spacing(StackSpacing::Md)
                    .align(StackAlign::Stretch)
                    .children(available_modes.iter().map(|mode| {
                        let is_selected = current_mode == *mode;
                        let mode_val = *mode;

                        // Card for each mode
                        div().flex_1().child(
                            Card::new()
                                .background(if is_selected {
                                    theme.surface_selected
                                } else {
                                    theme.surface
                                })
                                .border(if is_selected {
                                    theme.accent
                                } else {
                                    theme.border
                                })
                                .content(
                                    VStack::new()
                                        .spacing(StackSpacing::Sm)
                                        .child(
                                            Text::new(mode.as_str())
                                                .weight(TextWeight::Semibold)
                                                .color(if is_selected {
                                                    theme.accent
                                                } else {
                                                    theme.text_primary
                                                }),
                                        )
                                        .child(
                                            Text::new(mode.description())
                                                .size(TextSize::Sm)
                                                .color(theme.text_secondary),
                                        )
                                        .child(
                                            div().mt_4().child(
                                                Button::new(
                                                    SharedString::from(format!(
                                                        "select-mode-{:?}",
                                                        mode
                                                    )),
                                                    if is_selected { "Selected" } else { "Select" },
                                                )
                                                .variant(if is_selected {
                                                    ButtonVariant::Primary
                                                } else {
                                                    ButtonVariant::Secondary
                                                })
                                                .size(ButtonSize::Sm)
                                                .full_width(true)
                                                .theme(theme.to_button_theme())
                                                .build()
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(move |view, _, _, cx| {
                                                        view.state.update(cx, |state, _| {
                                                            state
                                                                .app
                                                                .measurement_state
                                                                .room_eq_state
                                                                .optimizer_config
                                                                .mode = mode_val;
                                                        });
                                                        cx.notify();
                                                    }),
                                                ),
                                            ),
                                        ),
                                ),
                        )
                    })),
            )
            .child(
                div().mt_4().flex().justify_end().child(
                    Button::new("next-step", "Next: Configure")
                        .variant(ButtonVariant::Primary)
                        .size(ButtonSize::Lg)
                        .theme(theme.to_button_theme())
                        .build()
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|view, _, _, cx| {
                                view.state.update(cx, |state, _| {
                                    state.app.measurement_state.room_eq_state.step =
                                        RoomEqStep::Configure;
                                });
                                cx.notify();
                            }),
                        ),
                ),
            )
    }
}