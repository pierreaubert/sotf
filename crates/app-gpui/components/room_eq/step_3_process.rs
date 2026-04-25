//! Room EQ wizard — Step 3: Process (wizard mode selector).
//!
//! Presents two cards — Simple Wizard and Full Wizard — so the user
//! chooses their Configure experience before advancing. The choice is
//! stored in `RoomEqState.wizard_mode` and read by the Configure step
//! to decide which layout to render.

use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Card, HStack, StackAlign, StackSpacing, Text, TextSize,
    TextWeight, VStack,
};
use sotf_audio_player::room_eq_types::RoomEqWizardMode;

impl PlayerView {
    pub(crate) fn render_room_eq_process(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let current_mode = state.app.measurement_state.room_eq_state.wizard_mode;

        let simple_selected = current_mode == RoomEqWizardMode::Simple;
        let full_selected = current_mode == RoomEqWizardMode::Full;

        VStack::new()
            .spacing(StackSpacing::Md)
            .child(
                Text::new("Choose Your Workflow")
                    .weight(TextWeight::Bold)
                    .size(TextSize::Md),
            )
            .child(
                Text::new(
                    "Pick how you want to configure the optimization. \
                     You can switch back here at any time.",
                )
                .size(TextSize::Xs)
                .color(theme.text_secondary),
            )
            .child(
                HStack::new()
                    .spacing(StackSpacing::Lg)
                    .align(StackAlign::Start)
                    .child(
                        Card::new()
                            .background(if simple_selected {
                                theme.surface_selected
                            } else {
                                theme.surface
                            })
                            .border(if simple_selected {
                                theme.accent
                            } else {
                                theme.border
                            })
                            .content(
                                VStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .child(
                                        Text::new("Simple Wizard")
                                            .weight(TextWeight::Bold)
                                            .size(TextSize::Md)
                                            .color(if simple_selected {
                                                theme.accent
                                            } else {
                                                theme.text_primary
                                            }),
                                    )
                                    .child(
                                        Text::new(
                                            "Guided presets for common setups. \
                                             Pick target distance, loss function, \
                                             and processing mode — we handle the rest.",
                                        )
                                        .size(TextSize::Xs)
                                        .color(theme.text_secondary),
                                    )
                                    .child(
                                        Button::new("select-simple", "Select")
                                            .variant(if simple_selected {
                                                ButtonVariant::Primary
                                            } else {
                                                ButtonVariant::Secondary
                                            })
                                            .size(ButtonSize::Sm)
                                            .theme(theme.to_button_theme())
                                            .on_click_event(cx.listener(|view, _, _, cx| {
                                                    view.state.update(cx, |state, _| {
                                                        state
                                                            .app
                                                            .measurement_state
                                                            .room_eq_state
                                                            .wizard_mode = RoomEqWizardMode::Simple;
                                                        if let Some(next) = state
                                                            .app
                                                            .measurement_state
                                                            .room_eq_state
                                                            .step
                                                            .next()
                                                        {
                                                            state
                                                                .app
                                                                .measurement_state
                                                                .room_eq_state
                                                                .step = next;
                                                        }
                                                    });
                                                    cx.notify();
                                                })),
                                    ),
                            ),
                    )
                    .child(
                        Card::new()
                            .background(if full_selected {
                                theme.surface_selected
                            } else {
                                theme.surface
                            })
                            .border(if full_selected {
                                theme.accent
                            } else {
                                theme.border
                            })
                            .content(
                                VStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .child(
                                        Text::new("Full Wizard")
                                            .weight(TextWeight::Bold)
                                            .size(TextSize::Md)
                                            .color(if full_selected {
                                                theme.accent
                                            } else {
                                                theme.text_primary
                                            }),
                                    )
                                    .child(
                                        Text::new(
                                            "All parameters exposed in two organized blocks: \
                                             Acoustic (what to fix) and Optimizer (how to fix). \
                                             Full control over every parameter.",
                                        )
                                        .size(TextSize::Xs)
                                        .color(theme.text_secondary),
                                    )
                                    .child(
                                        Button::new("select-full", "Select")
                                            .variant(if full_selected {
                                                ButtonVariant::Primary
                                            } else {
                                                ButtonVariant::Secondary
                                            })
                                            .size(ButtonSize::Sm)
                                            .theme(theme.to_button_theme())
                                            .on_click_event(cx.listener(|view, _, _, cx| {
                                                    view.state.update(cx, |state, _| {
                                                        state
                                                            .app
                                                            .measurement_state
                                                            .room_eq_state
                                                            .wizard_mode = RoomEqWizardMode::Full;
                                                        if let Some(next) = state
                                                            .app
                                                            .measurement_state
                                                            .room_eq_state
                                                            .step
                                                            .next()
                                                        {
                                                            state
                                                                .app
                                                                .measurement_state
                                                                .room_eq_state
                                                                .step = next;
                                                        }
                                                    });
                                                    cx.notify();
                                                })),
                                    ),
                            ),
                    ),
            )
    }
}
