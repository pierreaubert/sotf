use gpui::*;

/// A circular potentiometer knob with fill indicator.
#[derive(IntoElement)]
pub struct Potentiometer {
    value: f32,
    label: SharedString,
    size: Pixels,
    muted: bool,
    accent_color: Hsla,
    muted_color: Hsla,
    bg_color: Hsla,
    text_color: Hsla,
    on_change: Option<Box<dyn Fn(f32, &mut Window, &mut App) + 'static>>,
}

impl Potentiometer {
    pub fn new() -> Self {
        Self {
            value: 0.0,
            label: "".into(),
            size: px(40.0),
            muted: false,
            accent_color: hsla(0.0, 0.0, 0.5, 1.0),
            muted_color: hsla(0.0, 0.0, 0.3, 1.0),
            bg_color: hsla(0.0, 0.0, 0.1, 1.0),
            text_color: hsla(0.0, 0.0, 0.9, 1.0),
            on_change: None,
        }
    }

    pub fn value(mut self, value: f32) -> Self {
        self.value = value;
        self
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = size.into();
        self
    }

    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = muted;
        self
    }

    pub fn accent_color(mut self, color: impl Into<Hsla>) -> Self {
        self.accent_color = color.into();
        self
    }

    pub fn muted_color(mut self, color: impl Into<Hsla>) -> Self {
        self.muted_color = color.into();
        self
    }

    pub fn bg_color(mut self, color: impl Into<Hsla>) -> Self {
        self.bg_color = color.into();
        self
    }

    pub fn text_color(mut self, color: impl Into<Hsla>) -> Self {
        self.text_color = color.into();
        self
    }

    pub fn on_change(mut self, handler: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for Potentiometer {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let display_value = if self.muted {
            0.0
        } else {
            self.value.clamp(0.0, 1.0)
        };
        let ring_color = if self.muted {
            self.muted_color
        } else {
            self.accent_color
        };
        let text_color_final = if self.muted {
            self.muted_color
        } else {
            self.text_color
        };

        // Make fill color slightly lighter than the background
        let fill_color = if self.muted {
            self.muted_color
        } else {
            // Lighten the background color by increasing lightness
            let mut lighter = self.bg_color;
            lighter.l = (lighter.l + 0.15).min(1.0);
            lighter
        };

        // Calculate the vertical offset for the fill circle
        // At 0%, the circle is completely below the visible area
        // At 100%, the circle is fully visible
        let fill_offset = self.size * (1.0 - display_value);

        div()
            .relative()
            .w(self.size)
            .h(self.size)
            .cursor_pointer()
            // Background circle
            .child(div().absolute().inset_0().rounded_full().bg(self.bg_color))
            // Filled portion (circular, slides up from bottom)
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded_full()
                    .overflow_hidden()
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .bottom(px(-1.0 * f32::from(fill_offset))) // Fix negation of Pixels
                            .w(self.size)
                            .h(self.size)
                            .rounded_full()
                            .bg(fill_color),
                    ),
            )
            // Border ring
            .child(
                div()
                    .absolute()
                    .inset(px(2.0))
                    .rounded_full()
                    .border_2()
                    .border_color(ring_color.opacity(0.3)),
            )
            // Label text in center
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_color_final)
                    .child(self.label),
            )
        // TODO: Add drag interaction logic if needed, currently this is just visual migration
        // The original component didn't seem to handle drag logic inside the render function
        // but rather relied on the parent or was just a display?
        // Looking at the original code, it returned a `div` which could have event handlers attached.
        // We will need to see how it's used.
    }
}
