//! Menu components - MenuItem, Menu, MenuBar, and ContextMenu
//!
//! Provides a complete menu system for application navigation and context menus.

use crate::theme::{ThemeExt, glow_shadow};
use crate::ComponentTheme;
use gpui::prelude::*;
use gpui::*;

/// Theme colors for menu styling
#[derive(Debug, Clone, ComponentTheme)]
pub struct MenuTheme {
    /// Menu background color
    #[theme(default = 0x2a2a2aff, from = surface)]
    pub background: Rgba,
    /// Menu border color
    #[theme(default = 0x444444ff, from = border)]
    pub border: Rgba,
    /// Separator color
    #[theme(default = 0x3a3a3aff, from = border)]
    pub separator: Rgba,
    /// Normal item text color
    #[theme(default = 0xccccccff, from = text_secondary)]
    pub text: Rgba,
    /// Hovered item text color
    #[theme(default = 0xffffffff, from = text_primary)]
    pub text_hover: Rgba,
    /// Disabled item text color
    #[theme(default = 0x666666ff, from = text_muted)]
    pub text_disabled: Rgba,
    /// Shortcut text color
    #[theme(default = 0x777777ff, from = text_muted)]
    pub text_shortcut: Rgba,
    /// Item hover background color
    #[theme(default = 0x3a3a3aff, from = surface_hover)]
    pub hover_bg: Rgba,
    /// Danger item hover background (for destructive actions like Quit)
    #[theme(default = 0xdc2626ff, from = error)]
    pub danger_hover_bg: Rgba,
}

/// A single menu item
#[derive(Clone)]
pub struct MenuItem {
    id: SharedString,
    label: SharedString,
    shortcut: Option<SharedString>,
    icon: Option<SharedString>,
    disabled: bool,
    is_separator: bool,
    is_checkbox: bool,
    checked: bool,
    is_danger: bool,
    children: Vec<MenuItem>,
}

impl MenuItem {
    /// Create a new menu item
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            shortcut: None,
            icon: None,
            disabled: false,
            is_separator: false,
            is_checkbox: false,
            checked: false,
            is_danger: false,
            children: Vec::new(),
        }
    }

    /// Create a separator item
    pub fn separator() -> Self {
        Self {
            id: "separator".into(),
            label: "".into(),
            shortcut: None,
            icon: None,
            disabled: true,
            is_separator: true,
            is_checkbox: false,
            checked: false,
            is_danger: false,
            children: Vec::new(),
        }
    }

    /// Create a checkbox menu item
    pub fn checkbox(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        checked: bool,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            shortcut: None,
            icon: None,
            disabled: false,
            is_separator: false,
            is_checkbox: true,
            checked,
            is_danger: false,
            children: Vec::new(),
        }
    }

    /// Add a keyboard shortcut display
    pub fn with_shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Add an icon
    pub fn with_icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Disable the menu item
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Add submenu items
    pub fn with_children(mut self, children: Vec<MenuItem>) -> Self {
        self.children = children;
        self
    }

    /// Get the item ID
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// Check if this is a separator
    pub fn is_separator(&self) -> bool {
        self.is_separator
    }

    /// Mark as a danger/destructive action (e.g., Quit, Delete)
    pub fn danger(mut self) -> Self {
        self.is_danger = true;
        self
    }

    /// Check if this is a danger item
    pub fn is_danger(&self) -> bool {
        self.is_danger
    }
}

/// A dropdown menu containing menu items
pub struct Menu {
    items: Vec<MenuItem>,
    min_width: Pixels,
    theme: Option<MenuTheme>,
    on_select: Option<Box<dyn Fn(&SharedString, &mut Window, &mut App) + 'static>>,
}

impl Menu {
    /// Create a new menu with items
    pub fn new(items: Vec<MenuItem>) -> Self {
        Self {
            items,
            min_width: px(180.0),
            theme: None,
            on_select: None,
        }
    }

    /// Set minimum width
    pub fn min_width(mut self, width: Pixels) -> Self {
        self.min_width = width;
        self
    }

    /// Set theme
    pub fn theme(mut self, theme: MenuTheme) -> Self {
        self.theme = Some(theme);
        self
    }

    /// Set the selection handler
    pub fn on_select(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Box::new(handler));
        self
    }

    /// Build into element with theme
    pub fn build_with_theme(self, menu_theme: &MenuTheme) -> Stateful<Div> {
        let min_width = self.min_width;
        let theme = self.theme.as_ref().unwrap_or(menu_theme);

        // Use Rc pattern instead of unsafe pointer for on_select handler
        let on_select_rc = self.on_select.map(|f| std::rc::Rc::new(f));

        let mut menu = div()
            .id("menu-container")
            .min_w(min_width)
            .max_h(px(600.0))
            .bg(theme.background)
            .border_1()
            .border_color(theme.border)
            .rounded(px(4.0))
            .shadow_lg()
            .py_1()
            .overflow_y_scroll();

        for item in self.items {
            if item.is_separator {
                menu = menu.child(div().my_1().h(px(1.0)).bg(theme.separator).mx_2());
            } else {
                let item_id = item.id.clone();
                let label = item.label.clone();
                let shortcut = item.shortcut.clone();
                let icon = item.icon.clone();
                let disabled = item.disabled;
                let is_checkbox = item.is_checkbox;
                let checked = item.checked;
                let is_danger = item.is_danger;

                let mut row = div()
                    .id(SharedString::from(format!("menu-item-{}", item_id)))
                    .px_3()
                    .py(px(6.0))
                    .mx_1()
                    .rounded(px(3.0))
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_sm();

                if disabled {
                    row = row.text_color(theme.text_disabled).cursor_not_allowed();
                } else {
                    let text_color = theme.text;
                    let text_hover = theme.text_hover;
                    let hover_bg = if is_danger {
                        theme.danger_hover_bg
                    } else {
                        theme.hover_bg
                    };

                    row = row
                        .text_color(text_color)
                        .cursor_pointer()
                        .hover(move |style| {
                            style
                                .bg(hover_bg)
                                .text_color(text_hover)
                                .shadow(glow_shadow(hover_bg))
                        });

                    if let Some(ref handler) = on_select_rc {
                        let handler = handler.clone();
                        let id = item_id.clone();
                        row = row.on_mouse_up(MouseButton::Left, move |_event, window, cx| {
                            handler(&id, window, cx);
                        });
                    }
                }

                // Checkbox indicator
                if is_checkbox {
                    row = row.child(div().w(px(16.0)).text_xs().child(if checked {
                        "✓"
                    } else {
                        " "
                    }));
                }

                // Icon
                if let Some(icon) = icon {
                    row = row.child(div().w(px(16.0)).child(icon));
                }

                // Label (flex-1 to push shortcut to right)
                row = row.child(div().flex_1().child(label));

                // Shortcut
                if let Some(shortcut) = shortcut {
                    let shortcut_color = theme.text_shortcut;
                    row = row.child(div().text_xs().text_color(shortcut_color).child(shortcut));
                }

                menu = menu.child(row);
            }
        }

        menu
    }
}

impl RenderOnce for Menu {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let global_theme = cx.theme();
        let menu_theme = MenuTheme::from(&global_theme);
        self.build_with_theme(&menu_theme)
    }
}

impl IntoElement for Menu {
    type Element = gpui::Component<Self>;

    fn into_element(self) -> Self::Element {
        gpui::Component::new(self)
    }
}

/// A menu bar item (top-level menu)
pub struct MenuBarItem {
    id: SharedString,
    label: SharedString,
    items: Vec<MenuItem>,
}

impl MenuBarItem {
    /// Create a new menu bar item
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            items: Vec::new(),
        }
    }

    /// Set the dropdown items
    pub fn with_items(mut self, items: Vec<MenuItem>) -> Self {
        self.items = items;
        self
    }

    /// Get the menu ID
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// Get the menu label
    pub fn label(&self) -> &SharedString {
        &self.label
    }

    /// Get the menu items
    pub fn items(&self) -> &[MenuItem] {
        &self.items
    }
}

/// A horizontal menu bar
pub struct MenuBar {
    items: Vec<MenuBarItem>,
    active_menu: Option<SharedString>,
    on_select: Option<Box<dyn Fn(&SharedString, &mut Window, &mut App) + 'static>>,
    on_menu_toggle: Option<Box<dyn Fn(Option<&SharedString>, &mut Window, &mut App) + 'static>>,
}

impl MenuBar {
    /// Create a new menu bar
    pub fn new(items: Vec<MenuBarItem>) -> Self {
        Self {
            items,
            active_menu: None,
            on_select: None,
            on_menu_toggle: None,
        }
    }

    /// Set the currently active (open) menu
    pub fn active_menu(mut self, id: Option<SharedString>) -> Self {
        self.active_menu = id;
        self
    }

    /// Set the item selection handler
    pub fn on_select(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Box::new(handler));
        self
    }

    /// Set the menu toggle handler
    pub fn on_menu_toggle(
        mut self,
        handler: impl Fn(Option<&SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_menu_toggle = Some(Box::new(handler));
        self
    }

    /// Get menu bar items (for external rendering with custom handlers)
    pub fn items(&self) -> &[MenuBarItem] {
        &self.items
    }

    /// Get active menu ID
    pub fn get_active_menu(&self) -> Option<&SharedString> {
        self.active_menu.as_ref()
    }

    /// Build into element with theme
    pub fn build_with_theme(self, theme: &MenuTheme) -> Div {
        // Use Rc pattern instead of unsafe pointer for on_menu_toggle handler
        let on_toggle_rc = self.on_menu_toggle.map(|f| std::rc::Rc::new(f));

        let mut bar = div().flex().items_center().gap_1();

        for item in &self.items {
            let is_open = self.active_menu.as_ref() == Some(&item.id);
            let menu_id = item.id.clone();
            let label = item.label.clone();

            let mut button = div()
                .id(SharedString::from(format!("menubar-{}", menu_id)))
                .px_3()
                .py_1()
                .rounded(px(3.0))
                .text_sm()
                .cursor_pointer();

            if is_open {
                button = button
                    .bg(theme.hover_bg)
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.text_hover);
            } else {
                let hover_bg = theme.hover_bg;
                button = button
                    .text_color(theme.text)
                    .hover(move |style| style.bg(hover_bg).shadow(glow_shadow(hover_bg)));
            }

            if let Some(ref handler) = on_toggle_rc {
                let handler = handler.clone();
                let id = menu_id.clone();
                let currently_open = is_open;
                button = button.on_mouse_up(MouseButton::Left, move |_event, window, cx| {
                    if currently_open {
                        handler(None, window, cx);
                    } else {
                        handler(Some(&id), window, cx);
                    }
                });
            }

            button = button.child(label);
            bar = bar.child(button);
        }

        bar
    }
}

impl RenderOnce for MenuBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let global_theme = cx.theme();
        let menu_theme = MenuTheme::from(&global_theme);
        self.build_with_theme(&menu_theme)
    }
}

impl IntoElement for MenuBar {
    type Element = gpui::Component<Self>;

    fn into_element(self) -> Self::Element {
        gpui::Component::new(self)
    }
}

/// Helper to build a single menu bar button without handlers
/// Use this when you need to add cx.listener() handlers
pub fn menu_bar_button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    is_open: bool,
    theme: &MenuTheme,
) -> Stateful<Div> {
    let id = id.into();
    let label = label.into();

    let mut button = div()
        .id(SharedString::from(format!("menubar-{}", id)))
        .px_3()
        .py_1()
        .rounded(px(3.0))
        .text_sm()
        .cursor_pointer();

    if is_open {
        button = button
            .bg(theme.hover_bg)
            .font_weight(FontWeight::BOLD)
            .text_color(theme.text_hover);
    } else {
        let hover_bg = theme.hover_bg;
        button = button
            .text_color(theme.text)
            .hover(move |style| style.bg(hover_bg).shadow(glow_shadow(hover_bg)));
    }

    button.child(label)
}
