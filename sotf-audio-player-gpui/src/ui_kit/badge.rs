//! Badge component
//!
//! Small status indicators and labels.

use gpui::prelude::*;
use gpui::*;

/// Badge variant
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BadgeVariant {
    /// Default gray
    #[default]
    Default,
    /// Primary blue
    Primary,
    /// Success green
    Success,
    /// Warning yellow
    Warning,
    /// Error red
    Error,
    /// Info cyan
    Info,
}

impl BadgeVariant {
    fn colors(&self) -> (Rgba, Rgba) {
        // Returns (background, text_color)
        match self {
            BadgeVariant::Default => (rgb(0x3a3a3a), rgb(0xcccccc)),
            BadgeVariant::Primary => (rgb(0x1a4a7a), rgb(0x7cc4ff)),
            BadgeVariant::Success => (rgb(0x1a3a1a), rgb(0x7ccc7c)),
            BadgeVariant::Warning => (rgb(0x3a3a1a), rgb(0xcccc7c)),
            BadgeVariant::Error => (rgb(0x3a1a1a), rgb(0xcc7c7c)),
            BadgeVariant::Info => (rgb(0x1a3a3a), rgb(0x7ccccc)),
        }
    }
}

/// Badge size
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BadgeSize {
    /// Small
    Sm,
    /// Medium (default)
    #[default]
    Md,
    /// Large
    Lg,
}

/// A badge component
pub struct Badge {
    label: SharedString,
    variant: BadgeVariant,
    size: BadgeSize,
    rounded: bool,
    icon: Option<SharedString>,
}

impl Badge {
    /// Create a new badge
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            variant: BadgeVariant::default(),
            size: BadgeSize::default(),
            rounded: false,
            icon: None,
        }
    }

    /// Set variant
    pub fn variant(mut self, variant: BadgeVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Set size
    pub fn size(mut self, size: BadgeSize) -> Self {
        self.size = size;
        self
    }

    /// Make fully rounded (pill shape)
    pub fn rounded(mut self, rounded: bool) -> Self {
        self.rounded = rounded;
        self
    }

    /// Add icon
    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Build into element
    pub fn build(self) -> Div {
        let (bg, text_color) = self.variant.colors();

        let (px_val, py_val) = match self.size {
            BadgeSize::Sm => (px(6.0), px(2.0)),
            BadgeSize::Md => (px(8.0), px(3.0)),
            BadgeSize::Lg => (px(12.0), px(4.0)),
        };

        let mut badge = div()
            .flex()
            .items_center()
            .gap_1()
            .px(px_val)
            .py(py_val)
            .bg(bg)
            .text_color(text_color);

        // Apply text size
        badge = match self.size {
            BadgeSize::Sm => badge.text_xs(),
            BadgeSize::Md => badge.text_xs(),
            BadgeSize::Lg => badge.text_sm(),
        };

        // Apply rounding
        if self.rounded {
            badge = badge.rounded_full();
        } else {
            badge = badge.rounded(px(3.0));
        }

        // Icon
        if let Some(icon) = self.icon {
            badge = badge.child(div().child(icon));
        }

        // Label
        badge = badge.child(self.label);

        badge
    }
}

impl IntoElement for Badge {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        self.build()
    }
}

/// A dot indicator (no text)
pub struct BadgeDot {
    variant: BadgeVariant,
    size: Pixels,
}

impl BadgeDot {
    /// Create a new badge dot
    pub fn new() -> Self {
        Self {
            variant: BadgeVariant::default(),
            size: px(8.0),
        }
    }

    /// Set variant
    pub fn variant(mut self, variant: BadgeVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Set size in pixels
    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    /// Build into element
    pub fn build(self) -> Div {
        let (bg, _) = self.variant.colors();

        div()
            .w(self.size)
            .h(self.size)
            .rounded_full()
            .bg(bg)
    }
}

impl Default for BadgeDot {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoElement for BadgeDot {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        self.build()
    }
}
