//! IconButton component
//!
//! A button that displays only an icon, with optional tooltip.

use gpui::prelude::*;
use gpui::*;

/// IconButton size variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IconButtonSize {
    /// Extra small (16px)
    Xs,
    /// Small (20px)
    Sm,
    /// Medium (24px, default)
    #[default]
    Md,
    /// Large (32px)
    Lg,
}

impl IconButtonSize {
    fn size(&self) -> Pixels {
        match self {
            IconButtonSize::Xs => px(16.0),
            IconButtonSize::Sm => px(20.0),
            IconButtonSize::Md => px(24.0),
            IconButtonSize::Lg => px(32.0),
        }
    }
}

/// IconButton variant
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IconButtonVariant {
    /// Ghost button (transparent, default)
    #[default]
    Ghost,
    /// Filled background
    Filled,
    /// Outline border
    Outline,
}

/// An icon-only button component
pub struct IconButton {
    id: ElementId,
    icon: SharedString,
    size: IconButtonSize,
    variant: IconButtonVariant,
    disabled: bool,
    selected: bool,
    on_click: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl IconButton {
    /// Create a new icon button
    pub fn new(id: impl Into<ElementId>, icon: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            icon: icon.into(),
            size: IconButtonSize::default(),
            variant: IconButtonVariant::default(),
            disabled: false,
            selected: false,
            on_click: None,
        }
    }

    /// Set the button size
    pub fn size(mut self, size: IconButtonSize) -> Self {
        self.size = size;
        self
    }

    /// Set the button variant
    pub fn variant(mut self, variant: IconButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Set disabled state
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set selected state
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Set click handler
    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }

    /// Build into element
    pub fn build(self) -> Stateful<Div> {
        let size = self.size.size();

        let (bg, bg_hover, text_color, border) = match self.variant {
            IconButtonVariant::Ghost => {
                if self.selected {
                    (rgb(0x3a3a3a), rgb(0x4a4a4a), rgb(0xffffff), None)
                } else {
                    (rgba(0x00000000), rgb(0x3a3a3a), rgb(0xcccccc), None)
                }
            }
            IconButtonVariant::Filled => {
                if self.selected {
                    (rgb(0x007acc), rgb(0x0098ff), rgb(0xffffff), None)
                } else {
                    (rgb(0x3a3a3a), rgb(0x4a4a4a), rgb(0xcccccc), None)
                }
            }
            IconButtonVariant::Outline => {
                if self.selected {
                    (
                        rgb(0x2a2a2a),
                        rgb(0x3a3a3a),
                        rgb(0xffffff),
                        Some(rgb(0x007acc)),
                    )
                } else {
                    (
                        rgba(0x00000000),
                        rgb(0x2a2a2a),
                        rgb(0xcccccc),
                        Some(rgb(0x555555)),
                    )
                }
            }
        };

        let mut el = div()
            .id(self.id)
            .flex()
            .items_center()
            .justify_center()
            .w(size)
            .h(size)
            .rounded_md()
            .bg(bg)
            .text_color(text_color)
            .cursor_pointer();

        if let Some(border_color) = border {
            el = el.border_1().border_color(border_color);
        }

        if self.disabled {
            el = el.opacity(0.5).cursor_not_allowed();
        } else {
            el = el.hover(|s| s.bg(bg_hover));

            if let Some(handler) = self.on_click {
                el = el.on_mouse_up(MouseButton::Left, move |_event, window, cx| {
                    handler(window, cx);
                });
            }
        }

        el.child(self.icon)
    }
}

impl IntoElement for IconButton {
    type Element = Stateful<Div>;

    fn into_element(self) -> Self::Element {
        self.build()
    }
}
