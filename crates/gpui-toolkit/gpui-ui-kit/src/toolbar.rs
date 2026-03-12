//! Toolbar component
//!
//! A horizontal action bar with grouped buttons and separators.
//!
//! # Usage
//!
//! ```ignore
//! Toolbar::new("editor-toolbar")
//!     .item(ToolbarItem::button("bold", "B"))
//!     .item(ToolbarItem::button("italic", "I"))
//!     .separator()
//!     .item(ToolbarItem::button("align-left", "<"))
//!     .item(ToolbarItem::button("align-center", "="))
//! ```

use crate::ComponentTheme;
use crate::theme::ThemeExt;
use gpui::prelude::*;
use gpui::*;

/// Theme colors for toolbar
#[derive(Debug, Clone, ComponentTheme)]
pub struct ToolbarTheme {
    /// Toolbar background
    #[theme(default = 0x1e1e1eff, from = surface)]
    pub background: Rgba,
    /// Toolbar border
    #[theme(default = 0x3a3a3aff, from = border)]
    pub border: Rgba,
    /// Button text color
    #[theme(default = 0xccccccff, from = text_secondary)]
    pub button_text: Rgba,
    /// Button hover background
    #[theme(default = 0x2a2a2aff, from = surface_hover)]
    pub button_hover: Rgba,
    /// Active button background
    #[theme(default = 0x007accff, from = accent)]
    pub button_active: Rgba,
    /// Active button text
    #[theme(default = 0xffffffff, from = text_on_accent)]
    pub button_active_text: Rgba,
    /// Separator color
    #[theme(default = 0x3a3a3aff, from = border)]
    pub separator: Rgba,
}

/// A toolbar item (button or separator)
pub enum ToolbarItem {
    /// A button with id, label, optional active state
    Button {
        id: ElementId,
        label: SharedString,
        active: bool,
        disabled: bool,
        on_click: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
    },
    /// A visual separator
    Separator,
    /// Custom content
    Custom(AnyElement),
}

impl ToolbarItem {
    /// Create a button item
    pub fn button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        ToolbarItem::Button {
            id: id.into(),
            label: label.into(),
            active: false,
            disabled: false,
            on_click: None,
        }
    }

    /// Set active state (only applies to Button variant)
    pub fn active(mut self, active: bool) -> Self {
        if let ToolbarItem::Button {
            active: ref mut a, ..
        } = self
        {
            *a = active;
        }
        self
    }

    /// Set disabled state (only applies to Button variant)
    pub fn disabled(mut self, disabled: bool) -> Self {
        if let ToolbarItem::Button {
            disabled: ref mut d,
            ..
        } = self
        {
            *d = disabled;
        }
        self
    }

    /// Set click handler (only applies to Button variant)
    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        if let ToolbarItem::Button {
            on_click: ref mut h,
            ..
        } = self
        {
            *h = Some(Box::new(handler));
        }
        self
    }

    /// Create a custom content item
    pub fn custom(element: impl IntoElement) -> Self {
        ToolbarItem::Custom(element.into_any_element())
    }
}

/// A toolbar component
pub struct Toolbar {
    id: ElementId,
    items: Vec<ToolbarItem>,
    bordered: bool,
}

impl Toolbar {
    /// Create a new toolbar
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            bordered: true,
        }
    }

    /// Add a toolbar item
    pub fn item(mut self, item: ToolbarItem) -> Self {
        self.items.push(item);
        self
    }

    /// Add a separator
    pub fn separator(mut self) -> Self {
        self.items.push(ToolbarItem::Separator);
        self
    }

    /// Show/hide the border
    pub fn bordered(mut self, bordered: bool) -> Self {
        self.bordered = bordered;
        self
    }

    /// Build the toolbar with theme
    pub fn build_with_theme(self, theme: &ToolbarTheme) -> Stateful<Div> {
        let mut toolbar = div()
            .id(self.id)
            .flex()
            .items_center()
            .gap_0p5()
            .px_2()
            .py_1()
            .bg(theme.background);

        if self.bordered {
            toolbar = toolbar.border_1().border_color(theme.border).rounded(px(6.0));
        }

        for item in self.items {
            match item {
                ToolbarItem::Button {
                    id,
                    label,
                    active,
                    disabled,
                    on_click,
                } => {
                    let hover_bg = theme.button_hover;
                    let mut btn = div()
                        .id(id)
                        .px_2()
                        .py_1()
                        .rounded(px(4.0))
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM);

                    if active {
                        btn = btn
                            .bg(theme.button_active)
                            .text_color(theme.button_active_text);
                    } else if disabled {
                        btn = btn
                            .text_color(theme.button_text)
                            .opacity(0.5);
                    } else {
                        btn = btn
                            .text_color(theme.button_text)
                            .cursor_pointer()
                            .hover(move |s| s.bg(hover_bg));
                    }

                    btn = btn.child(label);

                    if let (false, Some(handler)) = (disabled, on_click) {
                        btn = btn.on_mouse_up(
                            MouseButton::Left,
                            move |_event, window, cx| {
                                handler(window, cx);
                            },
                        );
                    }

                    toolbar = toolbar.child(btn);
                }
                ToolbarItem::Separator => {
                    toolbar = toolbar.child(
                        div()
                            .w(px(1.0))
                            .h(px(16.0))
                            .mx_1()
                            .bg(theme.separator),
                    );
                }
                ToolbarItem::Custom(element) => {
                    toolbar = toolbar.child(element);
                }
            }
        }

        toolbar
    }
}

impl RenderOnce for Toolbar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let global_theme = cx.theme();
        let theme = ToolbarTheme::from(&global_theme);
        self.build_with_theme(&theme)
    }
}

impl IntoElement for Toolbar {
    type Element = gpui::Component<Self>;

    fn into_element(self) -> Self::Element {
        gpui::Component::new(self)
    }
}
