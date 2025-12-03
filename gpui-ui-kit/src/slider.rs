//! Slider component for selecting numeric values within a range

use gpui::*;

/// Theme colors for slider styling
#[derive(Debug, Clone)]
pub struct SliderTheme {
    /// Track background color (unfilled portion)
    pub track: Rgba,
    /// Fill color (active portion)
    pub fill: Rgba,
    /// Thumb/handle color
    pub thumb: Rgba,
    /// Label text color
    pub label: Rgba,
    /// Value text color
    pub value: Rgba,
}

impl Default for SliderTheme {
    fn default() -> Self {
        Self {
            track: rgba(0x3e3e3eff),
            fill: rgba(0x007accff),
            thumb: rgba(0xffffffff),
            label: rgba(0xccccccff),
            value: rgba(0x999999ff),
        }
    }
}

/// Slider size variants
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SliderSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl SliderSize {
    fn track_height(&self) -> f32 {
        match self {
            Self::Small => 4.0,
            Self::Medium => 6.0,
            Self::Large => 8.0,
        }
    }

    fn thumb_size(&self) -> f32 {
        match self {
            Self::Small => 14.0,
            Self::Medium => 18.0,
            Self::Large => 22.0,
        }
    }
}

/// A slider component for selecting numeric values
#[derive(IntoElement)]
pub struct Slider {
    id: ElementId,
    value: f32,
    min: f32,
    max: f32,
    step: Option<f32>,
    size: SliderSize,
    disabled: bool,
    show_value: bool,
    label: Option<SharedString>,
    width: f32,
    on_change: Option<Box<dyn Fn(f32, &mut Window, &mut App) + 'static>>,
    track_color: Option<Rgba>,
    fill_color: Option<Rgba>,
    thumb_color: Option<Rgba>,
    theme: Option<SliderTheme>,
}

impl Slider {
    /// Create a new slider with the given ID
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            value: 0.0,
            min: 0.0,
            max: 100.0,
            step: None,
            size: SliderSize::default(),
            disabled: false,
            show_value: false,
            label: None,
            width: 200.0,
            on_change: None,
            track_color: None,
            fill_color: None,
            thumb_color: None,
            theme: None,
        }
    }

    /// Set the current value
    pub fn value(mut self, value: f32) -> Self {
        self.value = value.clamp(self.min, self.max);
        self
    }

    /// Set the minimum value
    pub fn min(mut self, min: f32) -> Self {
        self.min = min;
        self
    }

    /// Set the maximum value
    pub fn max(mut self, max: f32) -> Self {
        self.max = max;
        self
    }

    /// Set the step size for snapping
    pub fn step(mut self, step: f32) -> Self {
        self.step = Some(step);
        self
    }

    /// Set the slider size
    pub fn size(mut self, size: SliderSize) -> Self {
        self.size = size;
        self
    }

    /// Set disabled state
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Show the current value as text
    pub fn show_value(mut self, show: bool) -> Self {
        self.show_value = show;
        self
    }

    /// Set a label for the slider
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set the width of the slider in pixels
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Set the change handler
    pub fn on_change(mut self, handler: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// Set the track color
    pub fn track_color(mut self, color: impl Into<Rgba>) -> Self {
        self.track_color = Some(color.into());
        self
    }

    /// Set the fill color
    pub fn fill_color(mut self, color: impl Into<Rgba>) -> Self {
        self.fill_color = Some(color.into());
        self
    }

    /// Set the thumb color
    pub fn thumb_color(mut self, color: impl Into<Rgba>) -> Self {
        self.thumb_color = Some(color.into());
        self
    }

    /// Set the slider theme (applies all colors at once)
    pub fn theme(mut self, theme: SliderTheme) -> Self {
        self.theme = Some(theme);
        self
    }
}

impl RenderOnce for Slider {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let track_height = self.size.track_height();
        let thumb_size = self.size.thumb_size();
        let width = self.width;

        // Use theme colors if available, then individual colors, then defaults
        let default_theme = SliderTheme::default();
        let theme = self.theme.as_ref().unwrap_or(&default_theme);
        let track_color = self.track_color.unwrap_or(theme.track);
        let fill_color = self.fill_color.unwrap_or(theme.fill);
        let thumb_color = self.thumb_color.unwrap_or(theme.thumb);
        let label_color = theme.label;
        let value_color = theme.value;

        let range = self.max - self.min;
        let progress = if range > 0.0 {
            (self.value - self.min) / range
        } else {
            0.0
        };

        let fill_width = (width * progress).max(0.0);
        let thumb_left = (width * progress) - (thumb_size / 2.0);

        let min = self.min;
        let max = self.max;
        let step = self.step;
        let disabled = self.disabled;

        let mut container = div().flex().flex_col().gap_1();

        // Label row
        if self.label.is_some() || self.show_value {
            let mut label_row = div().flex().justify_between().w(px(width)).text_sm();

            if let Some(label) = &self.label {
                label_row = label_row.child(
                    div()
                        .text_color(if disabled {
                            rgba(0x66666699)
                        } else {
                            label_color
                        })
                        .child(label.clone()),
                );
            }

            if self.show_value {
                label_row = label_row.child(div().text_color(value_color).child(format!("{:.1}", self.value)));
            }

            container = container.child(label_row);
        }

        // Slider track
        let on_change = self.on_change;
        let mut track = div()
            .id(self.id)
            .w(px(width))
            .h(px(thumb_size))
            .flex()
            .items_center()
            .relative()
            // Track background
            .child(
                div()
                    .absolute()
                    .left_0()
                    .w_full()
                    .h(px(track_height))
                    .rounded(px(track_height / 2.0))
                    .bg(track_color),
            )
            // Fill
            .child(
                div()
                    .absolute()
                    .left_0()
                    .w(px(fill_width))
                    .h(px(track_height))
                    .rounded(px(track_height / 2.0))
                    .bg(if disabled {
                        rgba(0xccccccff)
                    } else {
                        fill_color
                    }),
            )
            // Thumb
            .child(
                div()
                    .absolute()
                    .left(px(thumb_left.max(0.0)))
                    .w(px(thumb_size))
                    .h(px(thumb_size))
                    .rounded_full()
                    .bg(thumb_color)
                    .border_2()
                    .border_color(if disabled {
                        rgba(0xccccccff)
                    } else {
                        fill_color
                    })
                    .shadow_sm(),
            );

        // Apply cursor style
        if disabled {
            track = track.cursor_not_allowed();
        } else {
            track = track.cursor_pointer();
        }

        // Add click handling if not disabled and has callback
        // Use Rc to share handler between potential multiple calls
        if !disabled {
            if let Some(handler) = on_change {
                let handler = std::rc::Rc::new(handler);

                // Store current value in closure
                let current_value = self.value;

                track = track.on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                    // Since we can't reliably get element bounds in GPUI's on_mouse_down,
                    // we implement a simple step-based behavior:
                    // - Each click steps through values in the step direction
                    // - The step size is determined by the step parameter or a default
                    let step_amount = step.unwrap_or((max - min) / 10.0);

                    // Cycle through: increment by step, wrap at max
                    let new_value = current_value + step_amount;
                    let snapped = if new_value > max {
                        min // Wrap around to min when exceeding max
                    } else if let Some(step) = step {
                        let steps = ((new_value - min) / step).round();
                        (min + steps * step).clamp(min, max)
                    } else {
                        new_value.clamp(min, max)
                    };
                    handler(snapped, window, cx);
                });
            }
        }

        container.child(track)
    }
}
