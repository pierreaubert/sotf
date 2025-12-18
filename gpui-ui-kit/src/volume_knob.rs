//! VolumeKnob - A simple circular volume knob with circular fill indicator
//!
//! A visual volume control with:
//! - Circular fill animation that slides up from bottom (stays circular, no square)
//! - Scroll wheel adjustment
//! - Double-click to toggle mute
//! - Keyboard support (requires focus - click to focus):
//!   - Arrow Up/Right: increase volume
//!   - Arrow Down/Left: decrease volume
//!   - M key: toggle mute
//! - Mute state support
//! - Customizable colors and theme support

use crate::theme::{Theme, ThemeExt};
use gpui::*;

/// Theme colors for volume knob styling
#[derive(Debug, Clone)]
pub struct VolumeKnobTheme {
    /// Accent color (ring and fill when active)
    pub accent: Hsla,
    /// Color when muted
    pub muted: Hsla,
    /// Background color
    pub background: Hsla,
    /// Text color for label
    pub text: Hsla,
}

impl Default for VolumeKnobTheme {
    fn default() -> Self {
        Self {
            accent: hsla(0.0, 0.0, 0.5, 1.0),
            muted: hsla(0.0, 0.0, 0.3, 1.0),
            background: hsla(0.0, 0.0, 0.1, 1.0),
            text: hsla(0.0, 0.0, 0.9, 1.0),
        }
    }
}

impl From<&Theme> for VolumeKnobTheme {
    fn from(theme: &Theme) -> Self {
        Self {
            accent: theme.accent.into(),
            muted: theme.text_muted.into(),
            background: theme.surface.into(),
            text: theme.text_primary.into(),
        }
    }
}

/// A circular volume knob with fill indicator.
#[derive(IntoElement)]
pub struct VolumeKnob {
    id: ElementId,
    value: f32,
    label: SharedString,
    size: Pixels,
    muted: bool,
    /// Optional theme (uses global theme if not set)
    theme: Option<VolumeKnobTheme>,
    /// Override: accent color
    accent_color: Option<Hsla>,
    /// Override: muted color
    muted_color: Option<Hsla>,
    /// Override: background color
    bg_color: Option<Hsla>,
    /// Override: text color
    text_color: Option<Hsla>,
    on_change: Option<Box<dyn Fn(f32, &mut Window, &mut App) + 'static>>,
    on_mute_toggle: Option<Box<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
}

static VOLUME_KNOB_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl VolumeKnob {
    pub fn new() -> Self {
        let counter = VOLUME_KNOB_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Self {
            id: ElementId::Name(format!("volume-knob-{}", counter).into()),
            value: 0.0,
            label: "".into(),
            size: px(40.0),
            muted: false,
            theme: None,
            accent_color: None,
            muted_color: None,
            bg_color: None,
            text_color: None,
            on_change: None,
            on_mute_toggle: None,
        }
    }

    /// Set the theme
    pub fn theme(mut self, theme: VolumeKnobTheme) -> Self {
        self.theme = Some(theme);
        self
    }

    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
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

    /// Override accent color (ring and fill when active)
    pub fn accent_color(mut self, color: impl Into<Hsla>) -> Self {
        self.accent_color = Some(color.into());
        self
    }

    /// Override muted color
    pub fn muted_color(mut self, color: impl Into<Hsla>) -> Self {
        self.muted_color = Some(color.into());
        self
    }

    /// Override background color
    pub fn bg_color(mut self, color: impl Into<Hsla>) -> Self {
        self.bg_color = Some(color.into());
        self
    }

    /// Override text color
    pub fn text_color(mut self, color: impl Into<Hsla>) -> Self {
        self.text_color = Some(color.into());
        self
    }

    /// Set value change handler (called on scroll wheel)
    pub fn on_change(mut self, handler: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// Set mute toggle handler (called on double-click)
    pub fn on_mute_toggle(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_mute_toggle = Some(Box::new(handler));
        self
    }
}

impl Default for VolumeKnob {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for VolumeKnob {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Get theme: use explicit theme, or derive from global theme
        let global_theme = cx.theme();
        let theme = self
            .theme
            .clone()
            .unwrap_or_else(|| VolumeKnobTheme::from(&global_theme));

        // Apply color overrides or use theme defaults
        let accent_color = self.accent_color.unwrap_or(theme.accent);
        let muted_color = self.muted_color.unwrap_or(theme.muted);
        let bg_color = self.bg_color.unwrap_or(theme.background);
        let text_color = self.text_color.unwrap_or(theme.text);

        let display_value = if self.muted {
            0.0
        } else {
            self.value.clamp(0.0, 1.0)
        };
        let ring_color = if self.muted {
            muted_color
        } else {
            accent_color
        };
        let text_color_final = if self.muted { muted_color } else { text_color };

        // Make fill color slightly lighter than the background
        let fill_color = if self.muted {
            muted_color
        } else {
            // Lighten the background color by increasing lightness
            let mut lighter = bg_color;
            lighter.l = (lighter.l + 0.15).min(1.0);
            lighter
        };

        // Calculate the fill height as a percentage of the circle
        // At 0%, no fill visible
        // At 100%, circle fully filled
        let fill_height = self.size * display_value;

        // Capture values for closures
        let current_value = self.value;
        let current_muted = self.muted;

        let mut container = div()
            .id(self.id)
            .relative()
            .w(self.size)
            .h(self.size)
            .cursor_pointer()
            .focusable(); // Make focusable for keyboard events

        // Convert handlers to Rc for sharing between closures
        let on_change_rc = self.on_change.map(std::rc::Rc::new);
        let on_mute_rc = self.on_mute_toggle.map(std::rc::Rc::new);

        // Scroll wheel - adjust value
        if let Some(ref change_handler) = on_change_rc {
            let scroll_handler = change_handler.clone();
            container = container.on_scroll_wheel(move |event, window, cx| {
                let delta = event.delta.pixel_delta(px(20.0)).y;
                let scroll_up = delta < px(0.0);
                let step = 0.05;
                let change = if scroll_up { step } else { -step };
                let new_value = (current_value + change).clamp(0.0, 1.0);
                scroll_handler(new_value, window, cx);
            });
        }

        // Double-click - toggle mute
        if let Some(ref mute_handler) = on_mute_rc {
            let click_mute = mute_handler.clone();
            container = container.on_click(move |event, window, cx| {
                if event.click_count() == 2 {
                    click_mute(!current_muted, window, cx);
                }
            });
        }

        // Keyboard support
        if on_change_rc.is_some() || on_mute_rc.is_some() {
            let key_change = on_change_rc.clone();
            let key_mute = on_mute_rc.clone();

            container = container.on_key_down(move |event, window, cx| {
                match event.keystroke.key.as_str() {
                    "up" | "right" => {
                        // Increase volume
                        if let Some(ref handler) = key_change {
                            let step = 0.05;
                            let new_value = (current_value + step).clamp(0.0, 1.0);
                            handler(new_value, window, cx);
                        }
                    }
                    "down" | "left" => {
                        // Decrease volume
                        if let Some(ref handler) = key_change {
                            let step = 0.05;
                            let new_value = (current_value - step).clamp(0.0, 1.0);
                            handler(new_value, window, cx);
                        }
                    }
                    "m" => {
                        // Toggle mute
                        if let Some(ref handler) = key_mute {
                            handler(!current_muted, window, cx);
                        }
                    }
                    _ => {}
                }
            });
        }

        container
            // Background circle
            .child(div().absolute().inset_0().rounded_full().bg(bg_color))
            // Filled portion - use a circle that's clipped from bottom
            // We create a full circle and move it up/down to show the fill level
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded_full()
                    .overflow_hidden()
                    .child(
                        // Inner circle that slides up from bottom
                        div()
                            .absolute()
                            .left_0()
                            .w(self.size)
                            .h(self.size)
                            .rounded_full()
                            .bg(fill_color)
                            .bottom(-(self.size - fill_height)),
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
    }
}
