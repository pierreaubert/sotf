//! Accordion component
//!
//! Collapsible content sections.

use gpui::prelude::*;
use gpui::*;

/// A single accordion item
pub struct AccordionItem {
    id: SharedString,
    title: SharedString,
    content: Option<AnyElement>,
    disabled: bool,
}

impl AccordionItem {
    /// Create a new accordion item
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            content: None,
            disabled: false,
        }
    }

    /// Set content
    pub fn content(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }

    /// Set disabled state
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Get the item ID
    pub fn id(&self) -> &SharedString {
        &self.id
    }
}

/// Accordion behavior mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AccordionMode {
    /// Only one item can be open at a time
    #[default]
    Single,
    /// Multiple items can be open
    Multiple,
}

/// An accordion component
pub struct Accordion {
    items: Vec<AccordionItem>,
    expanded: Vec<SharedString>,
    mode: AccordionMode,
    on_change: Option<Box<dyn Fn(&SharedString, bool, &mut Window, &mut App) + 'static>>,
}

impl Accordion {
    /// Create a new accordion
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            expanded: Vec::new(),
            mode: AccordionMode::default(),
            on_change: None,
        }
    }

    /// Set items
    pub fn items(mut self, items: Vec<AccordionItem>) -> Self {
        self.items = items;
        self
    }

    /// Set expanded item IDs
    pub fn expanded(mut self, expanded: Vec<SharedString>) -> Self {
        self.expanded = expanded;
        self
    }

    /// Set mode
    pub fn mode(mut self, mode: AccordionMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set change handler (receives item ID and new expanded state)
    pub fn on_change(mut self, handler: impl Fn(&SharedString, bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// Build into element
    pub fn build(self) -> Div {
        let mut container = div()
            .flex()
            .flex_col()
            .border_1()
            .border_color(rgb(0x3a3a3a))
            .rounded_lg()
            .overflow_hidden();

        for (idx, item) in self.items.into_iter().enumerate() {
            let is_expanded = self.expanded.contains(&item.id);
            let item_id = item.id.clone();
            let is_first = idx == 0;

            // Header
            let mut header = div()
                .id(SharedString::from(format!("accordion-header-{}", item_id)))
                .flex()
                .items_center()
                .justify_between()
                .px_4()
                .py_3()
                .bg(rgb(0x252525))
                .cursor_pointer();

            if !is_first {
                header = header.border_t_1().border_color(rgb(0x3a3a3a));
            }

            if item.disabled {
                header = header.opacity(0.5).cursor_not_allowed();
            } else {
                header = header.hover(|s| s.bg(rgb(0x2a2a2a)));

                // Click handler
                if let Some(ref handler) = self.on_change {
                    let handler_ptr: *const dyn Fn(&SharedString, bool, &mut Window, &mut App) =
                        handler.as_ref();
                    let id = item_id.clone();
                    let new_state = !is_expanded;
                    header = header.on_mouse_up(MouseButton::Left, move |_event, window, cx| {
                        unsafe {
                            (*handler_ptr)(&id, new_state, window, cx);
                        }
                    });
                }
            }

            // Title
            header = header.child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(0xffffff))
                    .child(item.title),
            );

            // Expand/collapse indicator
            let indicator = if is_expanded { "▼" } else { "▶" };
            header = header.child(
                div()
                    .text_xs()
                    .text_color(rgb(0x888888))
                    .child(indicator),
            );

            container = container.child(header);

            // Content (only if expanded)
            if is_expanded {
                if let Some(content) = item.content {
                    container = container.child(
                        div()
                            .px_4()
                            .py_3()
                            .bg(rgb(0x1e1e1e))
                            .border_t_1()
                            .border_color(rgb(0x3a3a3a))
                            .child(content),
                    );
                }
            }
        }

        container
    }
}

impl Default for Accordion {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoElement for Accordion {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        self.build()
    }
}
