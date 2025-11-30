//! IconButton component
//!
//! A button that displays only an icon, with optional tooltip.

use gpui::prelude::*;
use gpui::*;

/// Theme colors for icon button styling
#[derive(Debug, Clone)]
pub struct IconButtonTheme {
    /// Background color for ghost variant
    pub ghost_bg: Rgba,
    /// Background color on hover for ghost variant
    pub ghost_hover_bg: Rgba,
    /// Background color when selected
    pub selected_bg: Rgba,
    /// Background color on hover when selected
    pub selected_hover_bg: Rgba,
    /// Filled variant background
    pub filled_bg: Rgba,
    /// Filled variant hover background
    pub filled_hover_bg: Rgba,
    /// Accent color (for filled selected, outline border)
    pub accent: Rgba,
    /// Accent hover color
    pub accent_hover: Rgba,
    /// Default text/icon color
    pub text: Rgba,
    /// Text color when selected or on accent background
    pub text_on_accent: Rgba,
    /// Border color for outline variant
    pub border: Rgba,
}

impl Default for IconButtonTheme {
    fn default() -> Self {
        Self {
            ghost_bg: rgba(0x00000000),
            ghost_hover_bg: rgba(0x3a3a3aff),
            selected_bg: rgba(0x3a3a3aff),
            selected_hover_bg: rgba(0x4a4a4aff),
            filled_bg: rgba(0x3a3a3aff),
            filled_hover_bg: rgba(0x4a4a4aff),
            accent: rgba(0x007accff),
            accent_hover: rgba(0x0098ffff),
            text: rgba(0xccccccff),
            text_on_accent: rgba(0xffffffff),
            border: rgba(0x555555ff),
        }
    }
}

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
    /// Extra large (48px)
    Xl,
}

impl IconButtonSize {
    fn size(&self) -> Pixels {
        match self {
            IconButtonSize::Xs => px(16.0),
            IconButtonSize::Sm => px(20.0),
            IconButtonSize::Md => px(24.0),
            IconButtonSize::Lg => px(32.0),
            IconButtonSize::Xl => px(48.0),
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
    rounded_full: bool,
    theme: Option<IconButtonTheme>,
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
            rounded_full: false,
            theme: None,
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

    /// Use fully rounded corners (circular button)
    pub fn rounded_full(mut self) -> Self {
        self.rounded_full = true;
        self
    }

    /// Set the button theme
    pub fn theme(mut self, theme: IconButtonTheme) -> Self {
        self.theme = Some(theme);
        self
    }

    /// Build into element
    pub fn build(self) -> Stateful<Div> {
        let size = self.size.size();
        let default_theme = IconButtonTheme::default();
        let theme = self.theme.as_ref().unwrap_or(&default_theme);

        let (bg, bg_hover, text_color, border): (Rgba, Rgba, Rgba, Option<Rgba>) =
            match self.variant {
                IconButtonVariant::Ghost => {
                    if self.selected {
                        (
                            theme.selected_bg,
                            theme.selected_hover_bg,
                            theme.text_on_accent,
                            None,
                        )
                    } else {
                        (theme.ghost_bg, theme.ghost_hover_bg, theme.text, None)
                    }
                }
                IconButtonVariant::Filled => {
                    if self.selected {
                        (
                            theme.accent,
                            theme.accent_hover,
                            theme.text_on_accent,
                            None,
                        )
                    } else {
                        (theme.filled_bg, theme.filled_hover_bg, theme.text, None)
                    }
                }
                IconButtonVariant::Outline => {
                    if self.selected {
                        (
                            theme.selected_bg,
                            theme.selected_hover_bg,
                            theme.text_on_accent,
                            Some(theme.accent),
                        )
                    } else {
                        (
                            theme.ghost_bg,
                            theme.ghost_hover_bg,
                            theme.text,
                            Some(theme.border),
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
            .bg(bg)
            .text_color(text_color)
            .cursor_pointer();

        // Apply rounding
        if self.rounded_full {
            el = el.rounded_full();
        } else {
            el = el.rounded_md();
        }

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
