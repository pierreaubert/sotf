//! Accordion component
//!
//! Collapsible content sections.

use crate::theme::{Theme, ThemeExt};
use gpui::prelude::*;
use gpui::*;

/// Theme colors for accordion styling
#[derive(Debug, Clone)]
pub struct AccordionTheme {
    pub header_bg: Rgba,
    pub header_hover_bg: Rgba,
    pub content_bg: Rgba,
    pub border: Rgba,
    pub title_color: Rgba,
    pub indicator_color: Rgba,
}

impl Default for AccordionTheme {
    fn default() -> Self {
        Self {
            header_bg: rgb(0x252525),
            header_hover_bg: rgb(0x2a2a2a),
            content_bg: rgb(0x1e1e1e),
            border: rgb(0x3a3a3a),
            title_color: rgb(0xffffff),
            indicator_color: rgb(0x888888),
        }
    }
}

impl From<&Theme> for AccordionTheme {
    fn from(theme: &Theme) -> Self {
        Self {
            header_bg: theme.muted,
            header_hover_bg: theme.surface_hover,
            content_bg: theme.background,
            border: theme.border,
            title_color: theme.text_primary,
            indicator_color: theme.text_muted,
        }
    }
}

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
    theme: Option<AccordionTheme>,
    on_change: Option<Box<dyn Fn(&SharedString, bool, &mut Window, &mut App) + 'static>>,
}

impl Accordion {
    /// Create a new accordion
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            expanded: Vec::new(),
            mode: AccordionMode::default(),
            theme: None,
            on_change: None,
        }
    }

    /// Set items
    pub fn items(mut self, items: Vec<AccordionItem>) -> Self {
        self.items = items;
        self
    }

    /// Add a single item
    pub fn item(mut self, item: AccordionItem) -> Self {
        self.items.push(item);
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

    /// Set theme
    pub fn theme(mut self, theme: AccordionTheme) -> Self {
        self.theme = Some(theme);
        self
    }

    /// Set change handler (receives item ID and new expanded state)
    pub fn on_change(
        mut self,
        handler: impl Fn(&SharedString, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// Build into element with theme
    pub fn build_with_theme(self, theme: &AccordionTheme) -> Div {
        let theme = self.theme.clone().unwrap_or_else(|| theme.clone());
        let on_change = self.on_change.map(|h| std::rc::Rc::new(h));

        let mut container = div()
            .flex()
            .flex_col()
            .border_1()
            .border_color(theme.border)
            .rounded_lg();

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
                .bg(theme.header_bg)
                .cursor_pointer();

            if !is_first {
                header = header.border_t_1().border_color(theme.border);
            }

            if item.disabled {
                header = header.opacity(0.5).cursor_not_allowed();
            } else {
                let hover_bg = theme.header_hover_bg;
                header = header.hover(move |s| s.bg(hover_bg));

                // Click handler
                if let Some(handler) = on_change.clone() {
                    let id = item_id.clone();
                    let new_state = !is_expanded;
                    header = header.on_mouse_up(MouseButton::Left, move |_event, window, cx| {
                        (handler)(&id, new_state, window, cx);
                    });
                }
            }

            // Title
            header = header.child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.title_color)
                    .child(item.title),
            );

            // Expand/collapse indicator
            let indicator = if is_expanded { "▼" } else { "▶" };
            header = header.child(
                div()
                    .text_xs()
                    .text_color(theme.indicator_color)
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
                            .bg(theme.content_bg)
                            .border_t_1()
                            .border_color(theme.border)
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

impl RenderOnce for Accordion {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let global_theme = cx.theme();
        let accordion_theme = AccordionTheme::from(&global_theme);
        self.build_with_theme(&accordion_theme)
    }
}

impl IntoElement for Accordion {
    type Element = gpui::Component<Self>;

    fn into_element(self) -> Self::Element {
        gpui::Component::new(self)
    }
}
