//! Vertical Slider component for audio plugin parameters
//!
//! A vertical slider with:
//! - Selection highlighting for plugin parameter editing
//! - Drag support with vertical mouse movement
//! - Scroll wheel adjustment
//! - Double-click to reset to default
//! - Keyboard navigation when selected:
//!   - Arrow Up/Right: increase value by 5%
//!   - Arrow Down/Left: decrease value by 5%
//!   - Home: set to minimum
//!   - End: set to maximum
//!   - Escape: reset to default
//! - Value display with units
//! - Keyboard shortcut hints

use crate::theme::{Theme, ThemeExt};
use gpui::prelude::*;
use gpui::*;

/// Theme colors for vertical slider styling
#[derive(Debug, Clone)]
pub struct VerticalSliderTheme {
    /// Background color of the slider container
    pub surface: Rgba,
    /// Surface hover color
    pub surface_hover: Rgba,
    /// Track background color
    pub track_bg: Rgba,
    /// Fill color (accent)
    pub accent: Rgba,
    /// Accent muted (for selection background)
    pub accent_muted: Rgba,
    /// Border color
    pub border: Rgba,
    /// Label text color
    pub text_secondary: Rgba,
    /// Value text color
    pub text_primary: Rgba,
    /// Muted text color (for scale markers)
    pub text_muted: Rgba,
    /// Text on accent background
    pub text_on_accent: Rgba,
    /// Background secondary (for value badge)
    pub background_secondary: Rgba,
}

impl Default for VerticalSliderTheme {
    fn default() -> Self {
        Self {
            surface: rgba(0x2a2a2aff),
            surface_hover: rgba(0x3a3a3aff),
            track_bg: rgba(0x1a1a1aff),
            accent: rgba(0x007accff),
            accent_muted: rgba(0x007acc33),
            border: rgba(0x3a3a3aff),
            text_secondary: rgba(0xaaaaaaff),
            text_primary: rgba(0xffffffff),
            text_muted: rgba(0x888888ff),
            text_on_accent: rgba(0xffffffff),
            background_secondary: rgba(0x2a2a2aff),
        }
    }
}

impl From<&Theme> for VerticalSliderTheme {
    fn from(theme: &Theme) -> Self {
        Self {
            surface: theme.surface,
            surface_hover: theme.surface_hover,
            track_bg: theme.muted,
            accent: theme.accent,
            accent_muted: Rgba {
                r: theme.accent.r,
                g: theme.accent.g,
                b: theme.accent.b,
                a: 0.2,
            },
            border: theme.border,
            text_secondary: theme.text_secondary,
            text_primary: theme.text_primary,
            text_muted: theme.text_muted,
            text_on_accent: theme.text_primary,
            background_secondary: theme.surface,
        }
    }
}

/// Vertical slider size variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VerticalSliderSize {
    /// Compact size
    Sm,
    /// Default size
    #[default]
    Md,
    /// Large size
    Lg,
}

impl VerticalSliderSize {
    fn track_width(&self) -> f32 {
        match self {
            Self::Sm => 14.0,
            Self::Md => 18.0,
            Self::Lg => 24.0,
        }
    }

    fn track_height(&self) -> f32 {
        match self {
            Self::Sm => 80.0,
            Self::Md => 120.0,
            Self::Lg => 160.0,
        }
    }

    fn min_width(&self) -> f32 {
        match self {
            Self::Sm => 50.0,
            Self::Md => 70.0,
            Self::Lg => 90.0,
        }
    }
}

/// A vertical slider component for audio plugin parameters
#[derive(IntoElement)]
pub struct VerticalSlider {
    id: ElementId,
    value: f64,
    min: f64,
    max: f64,
    unit: SharedString,
    label: Option<SharedString>,
    shortcut_key: Option<char>,
    size: VerticalSliderSize,
    selected: bool,
    disabled: bool,
    theme: Option<VerticalSliderTheme>,
    on_change: Option<Box<dyn Fn(f64, &mut Window, &mut App) + 'static>>,
    on_drag_start: Option<Box<dyn Fn(f32, f64, &mut Window, &mut App) + 'static>>,
    on_select: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_reset: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl VerticalSlider {
    /// Create a new vertical slider with the given ID
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            value: 0.0,
            min: 0.0,
            max: 100.0,
            unit: "".into(),
            label: None,
            shortcut_key: None,
            size: VerticalSliderSize::default(),
            selected: false,
            disabled: false,
            theme: None,
            on_change: None,
            on_drag_start: None,
            on_select: None,
            on_reset: None,
        }
    }

    /// Set the current value
    pub fn value(mut self, value: f64) -> Self {
        self.value = value.clamp(self.min, self.max);
        self
    }

    /// Set the minimum value
    pub fn min(mut self, min: f64) -> Self {
        self.min = min;
        self
    }

    /// Set the maximum value
    pub fn max(mut self, max: f64) -> Self {
        self.max = max;
        self
    }

    /// Set the unit label (e.g., "dB", "Hz", "%", ":1")
    pub fn unit(mut self, unit: impl Into<SharedString>) -> Self {
        self.unit = unit.into();
        self
    }

    /// Set the display label
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set the keyboard shortcut key for the label
    pub fn shortcut_key(mut self, key: char) -> Self {
        self.shortcut_key = Some(key);
        self
    }

    /// Set the slider size
    pub fn size(mut self, size: VerticalSliderSize) -> Self {
        self.size = size;
        self
    }

    /// Set selected state (for plugin parameter editing)
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Set disabled state
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set theme colors
    pub fn theme(mut self, theme: VerticalSliderTheme) -> Self {
        self.theme = Some(theme);
        self
    }

    /// Set value change handler (called on scroll wheel)
    pub fn on_change(mut self, handler: impl Fn(f64, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// Set drag start handler (called on mouse down with y position and current value)
    pub fn on_drag_start(
        mut self,
        handler: impl Fn(f32, f64, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_drag_start = Some(Box::new(handler));
        self
    }

    /// Set select handler (called on click to select this parameter)
    pub fn on_select(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Box::new(handler));
        self
    }

    /// Set reset handler (called on double-click)
    pub fn on_reset(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_reset = Some(Box::new(handler));
        self
    }

    /// Format the label with keyboard shortcut indicator
    fn format_label(&self) -> String {
        let label = self
            .label
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_default();
        match self.shortcut_key {
            Some(key) => {
                let key_lower = key.to_ascii_lowercase();
                let label_lower = label.to_lowercase();
                if let Some(pos) = label_lower.find(key_lower) {
                    format!(
                        "{}[{}]{}",
                        &label[..pos],
                        label.chars().nth(pos).unwrap().to_ascii_uppercase(),
                        &label[pos + 1..]
                    )
                } else {
                    format!("[{}] {}", key.to_ascii_uppercase(), label)
                }
            }
            None => label,
        }
    }

    /// Format the value display
    fn format_value(&self) -> String {
        let unit = self.unit.to_string();
        if unit == ":1" {
            format!("{:.1}{}", self.value, unit)
        } else if unit == "%" {
            format!("{:.0}{}", self.value * 100.0, unit)
        } else if unit.is_empty() {
            format!("{:.1}", self.value)
        } else {
            format!("{:.1} {}", self.value, unit)
        }
    }
}

impl RenderOnce for VerticalSlider {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let global_theme = cx.theme();
        let theme = self
            .theme
            .clone()
            .unwrap_or_else(|| VerticalSliderTheme::from(&global_theme));
        let selected = self.selected;
        let disabled = self.disabled;

        let normalized = if self.max > self.min {
            ((self.value - self.min) / (self.max - self.min)).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };

        let formatted_label = self.format_label();
        let value_str = self.format_value();

        let track_width = self.size.track_width();
        let track_height = self.size.track_height();
        let min_width = self.size.min_width();

        // Colors based on selection state
        let bg_color = if selected {
            theme.accent_muted
        } else {
            theme.surface
        };
        let border_color = if selected { theme.accent } else { theme.border };
        let track_border = if selected { theme.accent } else { theme.border };
        let label_color = if selected {
            theme.accent
        } else {
            theme.text_secondary
        };
        let value_bg = if selected {
            theme.accent
        } else {
            theme.background_secondary
        };
        let value_color = if selected {
            theme.text_on_accent
        } else {
            theme.text_primary
        };
        let track_bg = if selected {
            theme.surface_hover
        } else {
            theme.track_bg
        };
        let thumb_color = if selected {
            theme.text_on_accent
        } else {
            theme.accent
        };
        let thumb_height = if selected { 6.0 } else { 4.0 };
        let scale_color = if selected {
            theme.text_secondary
        } else {
            theme.text_muted
        };

        // Capture values for closures
        let value = self.value;
        let min = self.min;
        let max = self.max;

        let mut container = div()
            .id(self.id)
            .flex()
            .flex_col()
            .items_center()
            .gap_2()
            .p_2()
            .rounded_lg()
            .bg(bg_color)
            .border_2()
            .border_color(border_color)
            .min_w(px(min_width));

        // Add shadow when selected
        if selected {
            container = container.shadow_md();
        }

        // Hover effect
        let hover_border = theme.accent;
        let hover_bg = theme.surface_hover;
        container = container.hover(|s| s.border_color(hover_border).bg(hover_bg));

        // Cursor
        if disabled {
            container = container.cursor_not_allowed().opacity(0.5);
        } else {
            container = container.cursor_ns_resize();
        }

        // Event handlers
        if !disabled {
            // Wrap on_change in Rc for sharing between handlers
            let on_change_rc = self.on_change.map(|h| std::rc::Rc::new(h));
            let on_reset_rc = self.on_reset.map(|h| std::rc::Rc::new(h));

            // Mouse down - start drag and select
            if let Some(on_select) = self.on_select {
                let on_drag_start = self.on_drag_start;
                container = container.on_mouse_down(MouseButton::Left, move |event, window, cx| {
                    on_select(window, cx);
                    if let Some(ref handler) = on_drag_start {
                        handler(event.position.y.into(), value, window, cx);
                    }
                });
            } else if let Some(on_drag_start) = self.on_drag_start {
                container = container.on_mouse_down(MouseButton::Left, move |event, window, cx| {
                    on_drag_start(event.position.y.into(), value, window, cx);
                });
            } else if let Some(ref handler_rc) = on_change_rc {
                // If no drag handler, use click to step value
                let handler_click = handler_rc.clone();
                container =
                    container.on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                        let step = (max - min) * 0.1;
                        let new_value = (value + step).clamp(min, max);
                        handler_click(new_value, window, cx);
                    });
            }

            // Double-click - reset
            if let Some(ref reset_rc) = on_reset_rc {
                let reset_handler = reset_rc.clone();
                container = container.on_click(move |event, window, cx| {
                    if event.click_count() == 2 {
                        reset_handler(window, cx);
                    }
                });
            }

            // Scroll wheel - adjust value
            if let Some(ref handler_rc) = on_change_rc {
                let handler_scroll = handler_rc.clone();
                container = container.on_scroll_wheel(move |event, window, cx| {
                    let delta = event.delta.pixel_delta(px(20.0)).y;
                    let should_negate = delta > px(0.0);
                    let step = (max - min) * 0.05;
                    let change = if should_negate { -step } else { step };
                    let new_value = (value + change).clamp(min, max);
                    handler_scroll(new_value, window, cx);
                });
            }

            // Keyboard navigation when selected
            if selected {
                if let Some(ref handler_rc) = on_change_rc {
                    let handler_key = handler_rc.clone();
                    let reset_key = on_reset_rc.clone();
                    container = container.on_key_down(move |event, window, cx| {
                        let step = (max - min) * 0.05;

                        match event.keystroke.key.as_str() {
                            // Arrow Up or Right - increase value
                            "up" | "right" => {
                                let new_value = (value + step).clamp(min, max);
                                handler_key(new_value, window, cx);
                            }
                            // Arrow Down or Left - decrease value
                            "down" | "left" => {
                                let new_value = (value - step).clamp(min, max);
                                handler_key(new_value, window, cx);
                            }
                            // Home - set to minimum
                            "home" => {
                                handler_key(min, window, cx);
                            }
                            // End - set to maximum
                            "end" => {
                                handler_key(max, window, cx);
                            }
                            // Escape - reset to default
                            "escape" => {
                                if let Some(ref reset_handler) = reset_key {
                                    reset_handler(window, cx);
                                }
                            }
                            _ => {}
                        }
                    });
                }
            }
        }

        // Label with keyboard shortcut
        container = container.child(
            div()
                .text_xs()
                .font_weight(if selected {
                    FontWeight::BOLD
                } else {
                    FontWeight::SEMIBOLD
                })
                .text_color(label_color)
                .text_center()
                .child(formatted_label),
        );

        // Value badge
        container = container.child(
            div()
                .px_2()
                .py_1()
                .rounded_md()
                .bg(value_bg)
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(value_color)
                .child(value_str),
        );

        // Track with fill and thumb
        let mut track = div()
            .w(px(track_width))
            .h(px(track_height))
            .bg(track_bg)
            .rounded_lg()
            .border_2()
            .border_color(track_border)
            .relative()
            .overflow_hidden();

        if selected {
            track = track.shadow_sm();
        }

        // Filled portion (from bottom)
        track = track.child(
            div()
                .absolute()
                .bottom_0()
                .left_0()
                .right_0()
                .h(relative(normalized))
                .bg(theme.accent)
                .rounded_b_md(),
        );

        // Thumb indicator
        track = track.child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom(relative(normalized))
                .h(px(thumb_height))
                .bg(thumb_color)
                .rounded_sm()
                .when(selected, |d| d.shadow_sm()),
        );

        container = container.child(track);

        // Scale markers
        container = container.child(
            div()
                .flex()
                .justify_between()
                .w_full()
                .text_xs()
                .text_color(scale_color)
                .child(format!("{:.0}", min))
                .child(format!("{:.0}", max)),
        );

        container
    }
}
