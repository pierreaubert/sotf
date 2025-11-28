//! Button component with variants and sizes
//!
//! Provides a flexible button component with different visual styles.

use gpui::prelude::*;
use gpui::*;

/// Button visual variant
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    /// Primary action button (accent color)
    #[default]
    Primary,
    /// Secondary action button (muted)
    Secondary,
    /// Destructive action (red)
    Destructive,
    /// Ghost button (transparent until hover)
    Ghost,
    /// Outline button (border only)
    Outline,
}

/// Button size
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonSize {
    /// Small button
    Sm,
    /// Medium button (default)
    #[default]
    Md,
    /// Large button
    Lg,
}

/// A styled button component
#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    variant: ButtonVariant,
    size: ButtonSize,
    disabled: bool,
    icon_left: Option<SharedString>,
    icon_right: Option<SharedString>,
    on_click: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl Button {
    /// Create a new button with a label
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            variant: ButtonVariant::default(),
            size: ButtonSize::default(),
            disabled: false,
            icon_left: None,
            icon_right: None,
            on_click: None,
        }
    }

    /// Set the button variant
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Set the button size
    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    /// Disable the button
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Add an icon to the left of the label
    pub fn icon_left(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon_left = Some(icon.into());
        self
    }

    /// Add an icon to the right of the label
    pub fn icon_right(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon_right = Some(icon.into());
        self
    }

    /// Set the click handler
    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let (bg, bg_hover, text_color, border_color) = match self.variant {
            ButtonVariant::Primary => (
                rgb(0x007acc),
                rgb(0x0098ff),
                rgb(0xffffff),
                rgb(0x007acc),
            ),
            ButtonVariant::Secondary => (
                rgb(0x3c3c3c),
                rgb(0x4a4a4a),
                rgb(0xcccccc),
                rgb(0x3c3c3c),
            ),
            ButtonVariant::Destructive => (
                rgb(0xcc3333),
                rgb(0xe64545),
                rgb(0xffffff),
                rgb(0xcc3333),
            ),
            ButtonVariant::Ghost => (
                rgba(0x00000000),
                rgb(0x3c3c3c),
                rgb(0xcccccc),
                rgba(0x00000000),
            ),
            ButtonVariant::Outline => (
                rgba(0x00000000),
                rgb(0x2a2a2a),
                rgb(0xcccccc),
                rgb(0x555555),
            ),
        };

        let (px_val, py_val) = match self.size {
            ButtonSize::Sm => (px(8.0), px(4.0)),
            ButtonSize::Md => (px(16.0), px(8.0)),
            ButtonSize::Lg => (px(24.0), px(12.0)),
        };

        let mut el = div()
            .id(self.id)
            .flex()
            .items_center()
            .justify_center()
            .gap_2()
            .px(px_val)
            .py(py_val)
            .rounded_md()
            .bg(bg)
            .text_color(text_color)
            .border_1()
            .border_color(border_color)
            .cursor_pointer();

        // Apply text size based on button size
        el = match self.size {
            ButtonSize::Sm => el.text_xs(),
            ButtonSize::Md => el.text_sm(),
            ButtonSize::Lg => el.text_lg(),
        };

        if self.disabled {
            el = el.opacity(0.5).cursor_not_allowed();
        } else {
            el = el.hover(|style| style.bg(bg_hover));

            if let Some(handler) = self.on_click {
                el = el.on_mouse_up(MouseButton::Left, move |_event, window, cx| {
                    handler(window, cx);
                });
            }
        }

        // Add icon left
        if let Some(icon) = self.icon_left {
            el = el.child(div().child(icon));
        }

        // Add label
        el = el.child(self.label);

        // Add icon right
        if let Some(icon) = self.icon_right {
            el = el.child(div().child(icon));
        }

        el
    }
}
