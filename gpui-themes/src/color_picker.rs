//! Color picker component for theme editing
//!
//! Provides an interactive color picker with:
//! - Hex input
//! - Color preview
//! - RGBA display

use crate::theme::Color;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, HStack, StackSpacing, Text, TextSize, TextWeight, VStack,
};

/// Color picker mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorPickerMode {
    #[default]
    RGB,
    HSL,
}

/// Standalone color picker view for use in dialogs
pub struct ColorPickerView {
    color: Color,
    mode: ColorPickerMode,
    label: SharedString,
    on_change: Option<Box<dyn Fn(Color) + Send + Sync + 'static>>,
}

impl ColorPickerView {
    pub fn new(label: impl Into<SharedString>, color: Color) -> Self {
        Self {
            color,
            mode: ColorPickerMode::RGB,
            label: label.into(),
            on_change: None,
        }
    }

    pub fn on_change(mut self, callback: impl Fn(Color) + Send + Sync + 'static) -> Self {
        self.on_change = Some(Box::new(callback));
        self
    }

    #[allow(dead_code)]
    fn update_color(&mut self, color: Color, cx: &mut Context<Self>) {
        self.color = color;
        if let Some(on_change) = &self.on_change {
            on_change(color);
        }
        cx.notify();
    }

    fn toggle_mode(&mut self, cx: &mut Context<Self>) {
        self.mode = match self.mode {
            ColorPickerMode::RGB => ColorPickerMode::HSL,
            ColorPickerMode::HSL => ColorPickerMode::RGB,
        };
        cx.notify();
    }
}

impl Render for ColorPickerView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let color = self.color;
        let hex_string = color.to_hex_string();
        let mode = self.mode;
        let (h, s, l) = color.to_hsl();

        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .bg(Rgba {
                r: 0.12,
                g: 0.12,
                b: 0.12,
                a: 1.0,
            })
            .rounded_lg()
            // Header
            .child(
                HStack::new()
                    .spacing(StackSpacing::Md)
                    .child(
                        Text::new(self.label.clone())
                            .size(TextSize::Md)
                            .weight(TextWeight::Bold),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new(
                            "mode-toggle",
                            if mode == ColorPickerMode::RGB {
                                "HSL"
                            } else {
                                "RGB"
                            },
                        )
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Xs)
                        .build()
                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                            this.toggle_mode(cx);
                        })),
                    )
                    .build(),
            )
            // Preview and hex input
            .child(
                HStack::new()
                    .spacing(StackSpacing::Md)
                    .child(
                        div()
                            .size(px(80.0))
                            .rounded_lg()
                            .bg(color.to_rgba())
                            .border_1()
                            .border_color(Rgba {
                                r: 0.4,
                                g: 0.4,
                                b: 0.4,
                                a: 1.0,
                            }),
                    )
                    .child(
                        VStack::new()
                            .spacing(StackSpacing::Sm)
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .child(Text::new("Hex:").size(TextSize::Sm))
                                    .child(
                                        Text::new(SharedString::from(hex_string))
                                            .size(TextSize::Sm)
                                            .weight(TextWeight::Bold),
                                    )
                                    .build(),
                            )
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .child(Text::new("RGBA:").size(TextSize::Sm))
                                    .child(
                                        Text::new(SharedString::from(format!(
                                            "{}, {}, {}, {}",
                                            color.r, color.g, color.b, color.a
                                        )))
                                        .size(TextSize::Sm),
                                    )
                                    .build(),
                            )
                            .when(mode == ColorPickerMode::HSL, |el| {
                                el.child(
                                    HStack::new()
                                        .spacing(StackSpacing::Sm)
                                        .child(Text::new("HSL:").size(TextSize::Sm))
                                        .child(
                                            Text::new(SharedString::from(format!(
                                                "{:.0}, {:.0}%, {:.0}%",
                                                h * 360.0,
                                                s * 100.0,
                                                l * 100.0
                                            )))
                                            .size(TextSize::Sm),
                                        )
                                        .build(),
                                )
                            })
                            .build(),
                    )
                    .build(),
            )
            // Hex value display
            .child(
                HStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::new("Hex:").size(TextSize::Sm))
                    .child(
                        Text::new(SharedString::from(color.to_hex_string()))
                            .size(TextSize::Sm)
                            .weight(TextWeight::Medium),
                    )
                    .build(),
            )
    }
}
