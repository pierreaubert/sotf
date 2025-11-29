//! Tabs component for tabbed navigation
//!
//! Provides a horizontal tab bar with content panels.

use gpui::prelude::*;
use gpui::*;

/// Tab visual variant
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabVariant {
    /// Underline indicator (default)
    #[default]
    Underline,
    /// Enclosed tabs with background
    Enclosed,
    /// Pill-shaped tabs
    Pills,
}

/// A single tab item
#[derive(Clone)]
pub struct TabItem {
    id: SharedString,
    label: SharedString,
    icon: Option<SharedString>,
    badge: Option<SharedString>,
    disabled: bool,
    closeable: bool,
}

impl TabItem {
    /// Create a new tab item
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            badge: None,
            disabled: false,
            closeable: false,
        }
    }

    /// Add an icon
    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Add a badge
    pub fn badge(mut self, badge: impl Into<SharedString>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    /// Disable the tab
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Make the tab closeable
    pub fn closeable(mut self, closeable: bool) -> Self {
        self.closeable = closeable;
        self
    }

    /// Get the tab ID
    pub fn id(&self) -> &SharedString {
        &self.id
    }
}

/// A tabs component
pub struct Tabs {
    tabs: Vec<TabItem>,
    selected_index: usize,
    variant: TabVariant,
    on_change: Option<Box<dyn Fn(usize, &mut Window, &mut App) + 'static>>,
    on_close: Option<Box<dyn Fn(&SharedString, &mut Window, &mut App) + 'static>>,
}

impl Tabs {
    /// Create a new tabs component
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            selected_index: 0,
            variant: TabVariant::default(),
            on_change: None,
            on_close: None,
        }
    }

    /// Set the tab items
    pub fn tabs(mut self, tabs: Vec<TabItem>) -> Self {
        self.tabs = tabs;
        self
    }

    /// Set the selected tab index
    pub fn selected_index(mut self, index: usize) -> Self {
        self.selected_index = index;
        self
    }

    /// Set the visual variant
    pub fn variant(mut self, variant: TabVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Set the tab change handler
    pub fn on_change(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// Set the tab close handler
    pub fn on_close(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_close = Some(Box::new(handler));
        self
    }

    /// Build into element
    pub fn build(self) -> Div {
        let mut container = div().flex().items_center();

        // Apply variant-specific container styling
        match self.variant {
            TabVariant::Underline => {
                container = container.border_b_1().border_color(rgb(0x3a3a3a));
            }
            TabVariant::Enclosed => {
                container = container.gap_1();
            }
            TabVariant::Pills => {
                container = container.gap_2().p_1().bg(rgb(0x2a2a2a)).rounded_lg();
            }
        }

        for (index, tab) in self.tabs.iter().enumerate() {
            let is_selected = index == self.selected_index;
            let tab_id = tab.id.clone();
            let label = tab.label.clone();
            let icon = tab.icon.clone();
            let badge = tab.badge.clone();
            let disabled = tab.disabled;
            let closeable = tab.closeable;

            let on_change: Option<*const dyn Fn(usize, &mut Window, &mut App)> =
                self.on_change.as_ref().map(|f| f.as_ref() as *const _);
            let on_close: Option<*const dyn Fn(&SharedString, &mut Window, &mut App)> =
                self.on_close.as_ref().map(|f| f.as_ref() as *const _);

            let mut tab_el = div()
                .id(SharedString::from(format!("tab-{}", tab_id)))
                .flex()
                .items_center()
                .gap_2()
                .px_4()
                .py_2();

            // Apply variant-specific tab styling
            match self.variant {
                TabVariant::Underline => {
                    if is_selected {
                        tab_el = tab_el
                            .border_b_2()
                            .border_color(rgb(0x007acc))
                            .text_color(rgb(0xffffff))
                            .font_weight(FontWeight::SEMIBOLD);
                    } else {
                        tab_el = tab_el
                            .text_color(rgb(0x888888))
                            .hover(|s| s.text_color(rgb(0xcccccc)));
                    }
                }
                TabVariant::Enclosed => {
                    if is_selected {
                        tab_el = tab_el
                            .bg(rgb(0x3a3a3a))
                            .rounded_t_md()
                            .text_color(rgb(0xffffff));
                    } else {
                        tab_el = tab_el
                            .text_color(rgb(0x888888))
                            .hover(|s| s.bg(rgb(0x2a2a2a)).text_color(rgb(0xcccccc)));
                    }
                }
                TabVariant::Pills => {
                    if is_selected {
                        tab_el = tab_el
                            .bg(rgb(0x007acc))
                            .rounded_md()
                            .text_color(rgb(0xffffff));
                    } else {
                        tab_el = tab_el
                            .rounded_md()
                            .text_color(rgb(0x888888))
                            .hover(|s| s.bg(rgb(0x3a3a3a)).text_color(rgb(0xcccccc)));
                    }
                }
            }

            if disabled {
                tab_el = tab_el.opacity(0.5).cursor_not_allowed();
            } else {
                tab_el = tab_el.cursor_pointer();

                // Handle click
                if let Some(handler_ptr) = on_change {
                    let idx = index;
                    tab_el =
                        tab_el.on_mouse_up(MouseButton::Left, move |_event, window, cx| unsafe {
                            (*handler_ptr)(idx, window, cx);
                        });
                }
            }

            // Icon
            if let Some(icon) = icon {
                tab_el = tab_el.child(div().text_sm().child(icon));
            }

            // Label
            tab_el = tab_el.child(div().text_sm().child(label));

            // Badge
            if let Some(badge) = badge {
                tab_el = tab_el.child(
                    div()
                        .text_xs()
                        .px_1()
                        .py(px(1.0))
                        .bg(rgb(0x555555))
                        .rounded(px(3.0))
                        .child(badge),
                );
            }

            // Close button
            if closeable {
                let id = tab_id.clone();
                let mut close_btn = div()
                    .id(SharedString::from(format!("tab-close-{}", tab_id)))
                    .text_xs()
                    .text_color(rgb(0x888888))
                    .hover(|s| s.text_color(rgb(0xffffff)));

                if let Some(handler_ptr) = on_close {
                    close_btn = close_btn.on_mouse_up(
                        MouseButton::Left,
                        move |_event, window, cx| unsafe {
                            (*handler_ptr)(&id, window, cx);
                        },
                    );
                }

                tab_el = tab_el.child(close_btn.child("×"));
            }

            container = container.child(tab_el);
        }

        container
    }
}

impl Default for Tabs {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoElement for Tabs {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        self.build()
    }
}
