//! Tabs component for tabbed navigation
//!
//! Provides a horizontal tab bar with content panels and theming support.

use gpui::prelude::*;
use gpui::*;

/// Theme colors for tabs styling
#[derive(Debug, Clone)]
pub struct TabsTheme {
    /// Background color for the container (Pills variant)
    pub container_bg: Rgba,
    /// Border color for the container (Underline variant)
    pub container_border: Rgba,
    /// Background color for selected tab
    pub selected_bg: Rgba,
    /// Background color for selected tab on hover
    pub selected_hover_bg: Rgba,
    /// Background color for unselected tab on hover
    pub hover_bg: Rgba,
    /// Accent color (underline, selected pill)
    pub accent: Rgba,
    /// Text color for selected tab
    pub text_selected: Rgba,
    /// Text color for unselected tab
    pub text_unselected: Rgba,
    /// Text color on hover
    pub text_hover: Rgba,
    /// Badge background color
    pub badge_bg: Rgba,
    /// Close button color
    pub close_color: Rgba,
    /// Close button hover color
    pub close_hover_color: Rgba,
}

impl Default for TabsTheme {
    fn default() -> Self {
        Self {
            container_bg: rgba(0x2a2a2aff),
            container_border: rgba(0x3a3a3aff),
            selected_bg: rgba(0x3a3a3aff),
            selected_hover_bg: rgba(0x4a4a4aff),
            hover_bg: rgba(0x2a2a2aff),
            accent: rgba(0x007accff),
            text_selected: rgba(0xffffffff),
            text_unselected: rgba(0x888888ff),
            text_hover: rgba(0xccccccff),
            badge_bg: rgba(0x555555ff),
            close_color: rgba(0x888888ff),
            close_hover_color: rgba(0xffffffff),
        }
    }
}

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
pub struct TabItem {
    id: SharedString,
    label: SharedString,
    icon: Option<SharedString>,
    custom_icon: Option<AnyElement>,
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
            custom_icon: None,
            badge: None,
            disabled: false,
            closeable: false,
        }
    }

    /// Add a text/emoji icon
    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Add a custom icon element (e.g., SVG)
    pub fn custom_icon(mut self, icon: impl IntoElement) -> Self {
        self.custom_icon = Some(icon.into_any_element());
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

/// A tabs component with theming support
pub struct Tabs {
    tabs: Vec<TabItem>,
    selected_index: usize,
    variant: TabVariant,
    theme: Option<TabsTheme>,
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
            theme: None,
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

    /// Set the theme
    pub fn theme(mut self, theme: TabsTheme) -> Self {
        self.theme = Some(theme);
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
        let default_theme = TabsTheme::default();
        let theme = self.theme.as_ref().unwrap_or(&default_theme);

        let mut container = div().flex().items_center();

        // Apply variant-specific container styling
        match self.variant {
            TabVariant::Underline => {
                container = container.border_b_1().border_color(theme.container_border);
            }
            TabVariant::Enclosed => {
                container = container.gap_1();
            }
            TabVariant::Pills => {
                container = container
                    .gap_2()
                    .p_1()
                    .bg(theme.container_bg)
                    .rounded_lg();
            }
        }

        for (index, tab) in self.tabs.into_iter().enumerate() {
            let is_selected = index == self.selected_index;
            let tab_id = tab.id.clone();
            let label = tab.label;
            let icon = tab.icon;
            let custom_icon = tab.custom_icon;
            let badge = tab.badge;
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

            // Apply variant-specific tab styling with theme colors
            match self.variant {
                TabVariant::Underline => {
                    if is_selected {
                        tab_el = tab_el
                            .border_b_2()
                            .border_color(theme.accent)
                            .text_color(theme.text_selected)
                            .font_weight(FontWeight::SEMIBOLD);
                    } else {
                        let hover_color = theme.text_hover;
                        tab_el = tab_el
                            .text_color(theme.text_unselected)
                            .hover(move |s| s.text_color(hover_color));
                    }
                }
                TabVariant::Enclosed => {
                    if is_selected {
                        tab_el = tab_el
                            .bg(theme.selected_bg)
                            .rounded_t_md()
                            .text_color(theme.text_selected);
                    } else {
                        let hover_bg = theme.hover_bg;
                        let hover_text = theme.text_hover;
                        tab_el = tab_el
                            .text_color(theme.text_unselected)
                            .hover(move |s| s.bg(hover_bg).text_color(hover_text));
                    }
                }
                TabVariant::Pills => {
                    if is_selected {
                        tab_el = tab_el
                            .bg(theme.accent)
                            .rounded_md()
                            .text_color(theme.text_selected);
                    } else {
                        let hover_bg = theme.selected_bg;
                        let hover_text = theme.text_hover;
                        tab_el = tab_el
                            .rounded_md()
                            .text_color(theme.text_unselected)
                            .hover(move |s| s.bg(hover_bg).text_color(hover_text));
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

            // Custom icon (takes precedence)
            if let Some(custom_icon) = custom_icon {
                tab_el = tab_el.child(custom_icon);
            } else if let Some(icon) = icon {
                // Text/emoji icon
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
                        .bg(theme.badge_bg)
                        .rounded(px(3.0))
                        .child(badge),
                );
            }

            // Close button
            if closeable {
                let id = tab_id.clone();
                let close_color = theme.close_color;
                let close_hover = theme.close_hover_color;
                let mut close_btn = div()
                    .id(SharedString::from(format!("tab-close-{}", tab_id)))
                    .text_xs()
                    .text_color(close_color)
                    .hover(move |s| s.text_color(close_hover));

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
