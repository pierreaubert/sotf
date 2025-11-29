//! Stack layout components
//!
//! Vertical and horizontal stack layouts with spacing.

use gpui::prelude::*;
use gpui::*;

/// Spacing values
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StackSpacing {
    /// No spacing
    None,
    /// Extra small (2px)
    Xs,
    /// Small (4px)
    Sm,
    /// Medium (8px, default)
    #[default]
    Md,
    /// Large (16px)
    Lg,
    /// Extra large (24px)
    Xl,
    /// 2X large (32px)
    Xxl,
}

impl StackSpacing {
    fn to_pixels(&self) -> Pixels {
        match self {
            StackSpacing::None => px(0.0),
            StackSpacing::Xs => px(2.0),
            StackSpacing::Sm => px(4.0),
            StackSpacing::Md => px(8.0),
            StackSpacing::Lg => px(16.0),
            StackSpacing::Xl => px(24.0),
            StackSpacing::Xxl => px(32.0),
        }
    }
}

/// Alignment options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StackAlign {
    /// Align to start
    Start,
    /// Center alignment (default)
    #[default]
    Center,
    /// Align to end
    End,
    /// Stretch to fill
    Stretch,
}

/// Justify options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StackJustify {
    /// Justify to start (default)
    #[default]
    Start,
    /// Center justify
    Center,
    /// Justify to end
    End,
    /// Space between items
    SpaceBetween,
    /// Space around items
    SpaceAround,
    /// Space evenly
    SpaceEvenly,
}

/// A vertical stack (column) layout
pub struct VStack {
    children: Vec<AnyElement>,
    spacing: StackSpacing,
    align: StackAlign,
    justify: StackJustify,
}

impl VStack {
    /// Create a new vertical stack
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            spacing: StackSpacing::default(),
            align: StackAlign::Stretch,
            justify: StackJustify::default(),
        }
    }

    /// Add a child element
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }

    /// Add multiple children
    pub fn children(mut self, children: impl IntoIterator<Item = impl IntoElement>) -> Self {
        self.children
            .extend(children.into_iter().map(|c| c.into_any_element()));
        self
    }

    /// Set spacing
    pub fn spacing(mut self, spacing: StackSpacing) -> Self {
        self.spacing = spacing;
        self
    }

    /// Set alignment
    pub fn align(mut self, align: StackAlign) -> Self {
        self.align = align;
        self
    }

    /// Set justify
    pub fn justify(mut self, justify: StackJustify) -> Self {
        self.justify = justify;
        self
    }

    /// Build into element
    pub fn build(self) -> Div {
        let mut stack = div().flex().flex_col().gap(self.spacing.to_pixels());

        // Apply alignment
        stack = match self.align {
            StackAlign::Start => stack.items_start(),
            StackAlign::Center => stack.items_center(),
            StackAlign::End => stack.items_end(),
            StackAlign::Stretch => stack,
        };

        // Apply justify
        stack = match self.justify {
            StackJustify::Start => stack.justify_start(),
            StackJustify::Center => stack.justify_center(),
            StackJustify::End => stack.justify_end(),
            StackJustify::SpaceBetween => stack.justify_between(),
            StackJustify::SpaceAround => stack.justify_around(),
            StackJustify::SpaceEvenly => stack,
        };

        for child in self.children {
            stack = stack.child(child);
        }

        stack
    }
}

impl Default for VStack {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoElement for VStack {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        self.build()
    }
}

/// A horizontal stack (row) layout
pub struct HStack {
    children: Vec<AnyElement>,
    spacing: StackSpacing,
    align: StackAlign,
    justify: StackJustify,
    wrap: bool,
}

impl HStack {
    /// Create a new horizontal stack
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            spacing: StackSpacing::default(),
            align: StackAlign::Center,
            justify: StackJustify::default(),
            wrap: false,
        }
    }

    /// Add a child element
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }

    /// Add multiple children
    pub fn children(mut self, children: impl IntoIterator<Item = impl IntoElement>) -> Self {
        self.children
            .extend(children.into_iter().map(|c| c.into_any_element()));
        self
    }

    /// Set spacing
    pub fn spacing(mut self, spacing: StackSpacing) -> Self {
        self.spacing = spacing;
        self
    }

    /// Set alignment
    pub fn align(mut self, align: StackAlign) -> Self {
        self.align = align;
        self
    }

    /// Set justify
    pub fn justify(mut self, justify: StackJustify) -> Self {
        self.justify = justify;
        self
    }

    /// Enable flex wrap
    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    /// Build into element
    pub fn build(self) -> Div {
        let mut stack = div().flex().gap(self.spacing.to_pixels());

        if self.wrap {
            stack = stack.flex_wrap();
        }

        // Apply alignment
        stack = match self.align {
            StackAlign::Start => stack.items_start(),
            StackAlign::Center => stack.items_center(),
            StackAlign::End => stack.items_end(),
            StackAlign::Stretch => stack,
        };

        // Apply justify
        stack = match self.justify {
            StackJustify::Start => stack.justify_start(),
            StackJustify::Center => stack.justify_center(),
            StackJustify::End => stack.justify_end(),
            StackJustify::SpaceBetween => stack.justify_between(),
            StackJustify::SpaceAround => stack.justify_around(),
            StackJustify::SpaceEvenly => stack,
        };

        for child in self.children {
            stack = stack.child(child);
        }

        stack
    }
}

impl Default for HStack {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoElement for HStack {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        self.build()
    }
}

/// A spacer element that fills available space
pub struct Spacer;

impl Spacer {
    /// Create a new spacer
    pub fn new() -> Self {
        Self
    }

    /// Build into element
    pub fn build(self) -> Div {
        div().flex_1()
    }
}

impl Default for Spacer {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoElement for Spacer {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        self.build()
    }
}

/// A divider line
pub struct Divider {
    id: Option<SharedString>,
    vertical: bool,
    color: Option<Rgba>,
    hover_color: Option<Rgba>,
    thickness: Option<Pixels>,
    interactive: bool,
}

impl Divider {
    /// Create a new horizontal divider
    pub fn new() -> Self {
        Self {
            id: None,
            vertical: false,
            color: None,
            hover_color: None,
            thickness: None,
            interactive: false,
        }
    }

    /// Create a vertical divider
    pub fn vertical() -> Self {
        Self {
            id: None,
            vertical: true,
            color: None,
            hover_color: None,
            thickness: None,
            interactive: false,
        }
    }

    /// Set an ID for the divider (required for interactive dividers)
    pub fn id(mut self, id: impl Into<SharedString>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set custom color
    pub fn color(mut self, color: Rgba) -> Self {
        self.color = Some(color);
        self
    }

    /// Set hover color (for interactive dividers)
    pub fn hover_color(mut self, color: Rgba) -> Self {
        self.hover_color = Some(color);
        self
    }

    /// Set custom thickness
    pub fn thickness(mut self, thickness: Pixels) -> Self {
        self.thickness = Some(thickness);
        self
    }

    /// Make this an interactive resize divider
    pub fn interactive(mut self) -> Self {
        self.interactive = true;
        self
    }

    /// Build into a stateful element that can have handlers attached
    /// Use this when you need to add event handlers (e.g., for resize dividers)
    pub fn build(self) -> Stateful<Div> {
        let color = self.color.unwrap_or(rgb(0x3a3a3a));
        let id = self.id.unwrap_or_else(|| SharedString::from("divider"));

        let base = if self.vertical {
            let thickness = self.thickness.unwrap_or(px(1.0));
            div().id(id).w(thickness).h_full().bg(color)
        } else {
            let thickness = self.thickness.unwrap_or(px(1.0));
            div().id(id).h(thickness).w_full().bg(color)
        };

        if self.interactive {
            let hover_color = self.hover_color.unwrap_or(rgb(0x007acc));
            let cursor = if self.vertical {
                gpui::CursorStyle::ResizeLeftRight
            } else {
                gpui::CursorStyle::ResizeUpDown
            };
            base.cursor(cursor)
                .hover(move |style| style.bg(hover_color))
        } else {
            base
        }
    }

    /// Build into a non-stateful element (for simple visual dividers)
    pub fn build_simple(self) -> Div {
        let color = self.color.unwrap_or(rgb(0x3a3a3a));

        if self.vertical {
            let thickness = self.thickness.unwrap_or(px(1.0));
            div().w(thickness).h_full().bg(color)
        } else {
            let thickness = self.thickness.unwrap_or(px(1.0));
            div().h(thickness).w_full().bg(color)
        }
    }
}

impl Default for Divider {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoElement for Divider {
    type Element = Stateful<Div>;

    fn into_element(self) -> Self::Element {
        self.build()
    }
}
