//! Card component for content containers
//!
//! A flexible card component with optional header, content, and footer sections.

use crate::theme::{Theme, ThemeExt};
use gpui::prelude::*;
use gpui::*;

/// A card container with optional sections
#[derive(IntoElement)]
pub struct Card {
    header: Option<AnyElement>,
    content: Option<AnyElement>,
    footer: Option<AnyElement>,
    /// Custom background color (overrides theme)
    background: Option<Rgba>,
    /// Custom header background color (overrides theme)
    header_background: Option<Rgba>,
    /// Custom border color (overrides theme)
    border_color: Option<Rgba>,
    /// Additional styling
    extra_classes: Vec<Box<dyn FnOnce(Div) -> Div>>,
}

impl Card {
    /// Create a new empty card
    pub fn new() -> Self {
        Self {
            header: None,
            content: None,
            footer: None,
            background: None,
            header_background: None,
            border_color: None,
            extra_classes: Vec::new(),
        }
    }

    /// Set the card header
    pub fn header(mut self, element: impl IntoElement) -> Self {
        self.header = Some(element.into_any_element());
        self
    }

    /// Set the card content
    pub fn content(mut self, element: impl IntoElement) -> Self {
        self.content = Some(element.into_any_element());
        self
    }

    /// Set the card footer
    pub fn footer(mut self, element: impl IntoElement) -> Self {
        self.footer = Some(element.into_any_element());
        self
    }

    /// Add custom styling to the card container
    pub fn style(mut self, f: impl FnOnce(Div) -> Div + 'static) -> Self {
        self.extra_classes.push(Box::new(f));
        self
    }

    /// Set custom background color (overrides theme)
    pub fn background(mut self, color: Rgba) -> Self {
        self.background = Some(color);
        self
    }

    /// Set custom header background color (overrides theme)
    pub fn header_background(mut self, color: Rgba) -> Self {
        self.header_background = Some(color);
        self
    }

    /// Set custom border color (overrides theme)
    pub fn border(mut self, color: Rgba) -> Self {
        self.border_color = Some(color);
        self
    }

    /// Build the card into an element with theme
    pub fn build_with_theme(self, theme: &Theme) -> Div {
        let bg_color = self.background.unwrap_or(theme.surface);
        let border_color = self.border_color.unwrap_or(theme.border);
        let header_bg = self.header_background.unwrap_or(theme.muted);

        let mut card = div()
            .flex()
            .flex_col()
            .bg(bg_color)
            .text_color(theme.text_primary)
            .border_1()
            .border_color(border_color)
            .rounded_lg()
            .shadow_md()
            .overflow_hidden();

        // Apply extra classes
        for class_fn in self.extra_classes {
            card = class_fn(card);
        }

        // Header section
        if let Some(header) = self.header {
            card = card.child(
                div()
                    .px_4()
                    .py_3()
                    .bg(header_bg)
                    .text_color(theme.text_primary)
                    .border_b_1()
                    .border_color(border_color)
                    .child(header),
            );
        }

        // Content section
        if let Some(content) = self.content {
            card = card.child(
                div()
                    .px_4()
                    .py_4()
                    .text_color(theme.text_secondary)
                    .child(content),
            );
        }

        // Footer section
        if let Some(footer) = self.footer {
            card = card.child(
                div()
                    .px_4()
                    .py_3()
                    .bg(header_bg)
                    .text_color(theme.text_muted)
                    .border_t_1()
                    .border_color(border_color)
                    .child(footer),
            );
        }

        card
    }
}

impl Default for Card {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Card {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        self.build_with_theme(&theme)
    }
}
