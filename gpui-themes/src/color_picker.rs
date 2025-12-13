//! Color picker component for theme editing
//!
//! Provides an interactive color picker with:
//! - RGB/HSL sliders (clickable bars)
//! - Color preview
//! - RGBA/HSL display

use crate::theme::Color;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, HStack, StackSpacing, Text, TextSize, TextWeight, VStack,
};

/// Color picker mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::upper_case_acronyms)]
pub enum ColorPickerMode {
    #[default]
    RGB,
    HSL,
}

/// Standalone color picker view for use in dialogs
pub struct ColorPickerView {
    color: Color,
    original_color: Color,
    mode: ColorPickerMode,
    label: SharedString,
}

impl ColorPickerView {
    pub fn new(label: impl Into<SharedString>, color: Color) -> Self {
        Self {
            color,
            original_color: color,
            mode: ColorPickerMode::RGB,
            label: label.into(),
        }
    }

    /// Get current color
    pub fn color(&self) -> Color {
        self.color
    }

    /// Set color
    pub fn set_color(&mut self, color: Color) {
        self.color = color;
        self.original_color = color;
    }

    fn update_red(&mut self, value: u8, cx: &mut Context<Self>) {
        self.color.r = value;
        cx.notify();
    }

    fn update_green(&mut self, value: u8, cx: &mut Context<Self>) {
        self.color.g = value;
        cx.notify();
    }

    fn update_blue(&mut self, value: u8, cx: &mut Context<Self>) {
        self.color.b = value;
        cx.notify();
    }

    fn update_alpha(&mut self, value: u8, cx: &mut Context<Self>) {
        self.color.a = value;
        cx.notify();
    }

    fn update_hue(&mut self, value: f32, cx: &mut Context<Self>) {
        let (_, s, l) = self.color.to_hsl();
        self.color = Color::from_hsl(value, s, l).with_alpha(self.color.a as f32 / 255.0);
        cx.notify();
    }

    fn update_saturation(&mut self, value: f32, cx: &mut Context<Self>) {
        let (h, _, l) = self.color.to_hsl();
        self.color = Color::from_hsl(h, value, l).with_alpha(self.color.a as f32 / 255.0);
        cx.notify();
    }

    fn update_lightness(&mut self, value: f32, cx: &mut Context<Self>) {
        let (h, s, _) = self.color.to_hsl();
        self.color = Color::from_hsl(h, s, value).with_alpha(self.color.a as f32 / 255.0);
        cx.notify();
    }

    fn toggle_mode(&mut self, cx: &mut Context<Self>) {
        self.mode = match self.mode {
            ColorPickerMode::RGB => ColorPickerMode::HSL,
            ColorPickerMode::HSL => ColorPickerMode::RGB,
        };
        cx.notify();
    }

    fn reset_color(&mut self, cx: &mut Context<Self>) {
        self.color = self.original_color;
        cx.notify();
    }

    /// Render a simple draggable slider bar
    fn render_slider(
        &self,
        label: &'static str,
        value: f32,
        max: f32,
        _color_gradient: Option<(Rgba, Rgba)>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let mode = self.mode;
        let ratio = value / max;
        let bar_width = 200.0;

        HStack::new()
            .spacing(StackSpacing::Sm)
            .child(
                div()
                    .w(px(24.0))
                    .child(Text::new(label).size(TextSize::Sm).weight(TextWeight::Bold)),
            )
            .child(
                div()
                    .id(SharedString::from(format!("slider-{}", label)))
                    .w(px(bar_width))
                    .h(px(20.0))
                    .bg(Rgba { r: 0.2, g: 0.2, b: 0.2, a: 1.0 })
                    .rounded(px(4.0))
                    .relative()
                    .cursor_pointer()
                    // Fill indicator
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .h_full()
                            .w(px(bar_width * ratio))
                            .bg(Rgba { r: 0.0, g: 0.48, b: 0.8, a: 1.0 })
                            .rounded(px(4.0)),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
                            let click_x: f32 = event.position.x.into();
                            let new_ratio = (click_x / bar_width).clamp(0.0, 1.0);
                            let new_val = new_ratio * max;

                            match (mode, label) {
                                (ColorPickerMode::RGB, "R") => this.update_red(new_val as u8, cx),
                                (ColorPickerMode::RGB, "G") => this.update_green(new_val as u8, cx),
                                (ColorPickerMode::RGB, "B") => this.update_blue(new_val as u8, cx),
                                (ColorPickerMode::HSL, "H") => this.update_hue(new_val / 360.0, cx),
                                (ColorPickerMode::HSL, "S") => this.update_saturation(new_val / 100.0, cx),
                                (ColorPickerMode::HSL, "L") => this.update_lightness(new_val / 100.0, cx),
                                (_, "A") => this.update_alpha(new_val as u8, cx),
                                _ => {}
                            }
                        }),
                    ),
            )
            .child(
                div()
                    .w(px(50.0))
                    .child(Text::new(SharedString::from(format!("{:.0}", value))).size(TextSize::Sm)),
            )
            .build()
    }
}

impl Render for ColorPickerView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let color = self.color;
        let original = self.original_color;
        let hex_string = color.to_hex_string();
        let mode = self.mode;
        let (h, s, l) = color.to_hsl();

        div()
            .flex()
            .flex_col()
            .gap_4()
            .p_4()
            .min_w(px(400.0))
            // Header
            .child(
                HStack::new()
                    .spacing(StackSpacing::Md)
                    .child(
                        Text::new(self.label.clone())
                            .size(TextSize::Lg)
                            .weight(TextWeight::Bold),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new(
                            "mode-toggle",
                            if mode == ColorPickerMode::RGB {
                                "Switch to HSL"
                            } else {
                                "Switch to RGB"
                            },
                        )
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .build()
                        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                            this.toggle_mode(cx);
                        })),
                    )
                    .build(),
            )
            // Color comparison
            .child(
                HStack::new()
                    .spacing(StackSpacing::Lg)
                    .child(
                        VStack::new()
                            .spacing(StackSpacing::Xs)
                            .child(Text::new("Original").size(TextSize::Xs))
                            .child(
                                div()
                                    .w(px(80.0))
                                    .h(px(60.0))
                                    .rounded_lg()
                                    .bg(original.to_rgba())
                                    .border_1()
                                    .border_color(Rgba {
                                        r: 0.4,
                                        g: 0.4,
                                        b: 0.4,
                                        a: 1.0,
                                    }),
                            )
                            .build(),
                    )
                    .child(
                        VStack::new()
                            .spacing(StackSpacing::Xs)
                            .child(Text::new("New").size(TextSize::Xs))
                            .child(
                                div()
                                    .w(px(80.0))
                                    .h(px(60.0))
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
                            .build(),
                    )
                    .child(
                        VStack::new()
                            .spacing(StackSpacing::Xs)
                            .child(
                                Text::new(SharedString::from(format!("Hex: {}", hex_string)))
                                    .size(TextSize::Sm)
                                    .weight(TextWeight::Bold),
                            )
                            .child(
                                Text::new(SharedString::from(format!(
                                    "RGBA: {}, {}, {}, {}",
                                    color.r, color.g, color.b, color.a
                                )))
                                .size(TextSize::Sm),
                            )
                            .child(
                                Text::new(SharedString::from(format!(
                                    "HSL: {:.0}°, {:.0}%, {:.0}%",
                                    h * 360.0,
                                    s * 100.0,
                                    l * 100.0
                                )))
                                .size(TextSize::Sm),
                            )
                            .build(),
                    )
                    .build(),
            )
            // Sliders
            .child(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .when(mode == ColorPickerMode::RGB, |el| {
                        el.child(self.render_slider(
                            "R",
                            color.r as f32,
                            255.0,
                            Some((
                                Rgba {
                                    r: 0.0,
                                    g: 0.0,
                                    b: 0.0,
                                    a: 1.0,
                                },
                                Rgba {
                                    r: 1.0,
                                    g: 0.0,
                                    b: 0.0,
                                    a: 1.0,
                                },
                            )),
                            cx,
                        ))
                        .child(self.render_slider(
                            "G",
                            color.g as f32,
                            255.0,
                            Some((
                                Rgba {
                                    r: 0.0,
                                    g: 0.0,
                                    b: 0.0,
                                    a: 1.0,
                                },
                                Rgba {
                                    r: 0.0,
                                    g: 1.0,
                                    b: 0.0,
                                    a: 1.0,
                                },
                            )),
                            cx,
                        ))
                        .child(self.render_slider(
                            "B",
                            color.b as f32,
                            255.0,
                            Some((
                                Rgba {
                                    r: 0.0,
                                    g: 0.0,
                                    b: 0.0,
                                    a: 1.0,
                                },
                                Rgba {
                                    r: 0.0,
                                    g: 0.0,
                                    b: 1.0,
                                    a: 1.0,
                                },
                            )),
                            cx,
                        ))
                    })
                    .when(mode == ColorPickerMode::HSL, |el| {
                        el.child(self.render_slider(
                            "H",
                            h * 360.0,
                            360.0,
                            Some((
                                Rgba {
                                    r: 1.0,
                                    g: 0.0,
                                    b: 0.0,
                                    a: 1.0,
                                },
                                Rgba {
                                    r: 1.0,
                                    g: 0.0,
                                    b: 1.0,
                                    a: 1.0,
                                },
                            )),
                            cx,
                        ))
                        .child(self.render_slider(
                            "S",
                            s * 100.0,
                            100.0,
                            Some((
                                Rgba {
                                    r: 0.5,
                                    g: 0.5,
                                    b: 0.5,
                                    a: 1.0,
                                },
                                Rgba {
                                    r: 0.0,
                                    g: 0.7,
                                    b: 1.0,
                                    a: 1.0,
                                },
                            )),
                            cx,
                        ))
                        .child(self.render_slider(
                            "L",
                            l * 100.0,
                            100.0,
                            Some((
                                Rgba {
                                    r: 0.0,
                                    g: 0.0,
                                    b: 0.0,
                                    a: 1.0,
                                },
                                Rgba {
                                    r: 1.0,
                                    g: 1.0,
                                    b: 1.0,
                                    a: 1.0,
                                },
                            )),
                            cx,
                        ))
                    })
                    .child(self.render_slider(
                        "A",
                        color.a as f32,
                        255.0,
                        Some((
                            Rgba {
                                r: 0.2,
                                g: 0.2,
                                b: 0.2,
                                a: 1.0,
                            },
                            Rgba {
                                r: 1.0,
                                g: 1.0,
                                b: 1.0,
                                a: 1.0,
                            },
                        )),
                        cx,
                    ))
                    .build(),
            )
            // Reset button
            .child(
                HStack::new()
                    .spacing(StackSpacing::Md)
                    .child(
                        Button::new("reset-color", "Reset to Original")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .build()
                            .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                this.reset_color(cx);
                            })),
                    )
                    .build(),
            )
    }
}
