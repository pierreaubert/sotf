//! Card component for content containers
//!
//! A flexible card component with optional header, content, and footer sections.

use gpui::prelude::*;
use gpui::*;

/// A card container with optional sections
pub struct Card {
    header: Option<AnyElement>,
    content: Option<AnyElement>,
    footer: Option<AnyElement>,
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

    /// Build the card into an element
    pub fn build(self) -> Div {
        let mut card = div()
            .flex()
            .flex_col()
            .bg(rgb(0x1e1e1e))
            .border_1()
            .border_color(rgb(0x3a3a3a))
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
                    .border_b_1()
                    .border_color(rgb(0x3a3a3a))
                    .child(header),
            );
        }

        // Content section
        if let Some(content) = self.content {
            card = card.child(div().px_4().py_4().child(content));
        }

        // Footer section
        if let Some(footer) = self.footer {
            card = card.child(
                div()
                    .px_4()
                    .py_3()
                    .border_t_1()
                    .border_color(rgb(0x3a3a3a))
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

impl IntoElement for Card {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        self.build()
    }
}
