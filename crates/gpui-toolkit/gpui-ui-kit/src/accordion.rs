//! Accordion component
//!
//! Collapsible content sections with support for both vertical and horizontal orientations.

use crate::ComponentTheme;
use crate::accessibility::{AccessibilityExt, AccessibilityNode, AriaProps, AriaRole};
use crate::theme::{ThemeExt, glow_shadow};
use gpui::prelude::*;
use gpui::*;

/// Theme colors for accordion styling
#[derive(Debug, Clone, ComponentTheme)]
pub struct AccordionTheme {
    #[theme(default = 0x252525, from = muted)]
    pub header_bg: Rgba,
    #[theme(default = 0x2a2a2a, from = surface_hover)]
    pub header_hover_bg: Rgba,
    #[theme(default = 0x007acc33, from = accent_muted)]
    pub header_active_bg: Rgba,
    #[theme(default = 0x1e1e1e, from = background)]
    pub content_bg: Rgba,
    #[theme(default = 0x3a3a3a, from = border)]
    pub border: Rgba,
    #[theme(default = 0x007acc33, from = accent_muted)]
    pub accent_tint: Rgba,
    #[theme(default = 0x007acc, from = accent)]
    pub accent: Rgba,
    #[theme(default = 0xffffff, from = text_primary)]
    pub title_color: Rgba,
    #[theme(default = 0x888888, from = text_muted)]
    pub indicator_color: Rgba,
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

/// Accordion orientation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AccordionOrientation {
    /// Vertical layout: headers stacked vertically, content expands downward (default)
    #[default]
    Vertical,
    /// Horizontal layout: headers arranged horizontally, content expands downward
    Horizontal,
    /// Side layout: headers stacked vertically on left, content expands to right
    Side,
}

/// An accordion component
pub struct Accordion {
    items: Vec<AccordionItem>,
    expanded: Vec<SharedString>,
    mode: AccordionMode,
    orientation: AccordionOrientation,
    theme: Option<AccordionTheme>,
    on_change: Option<Box<dyn Fn(&SharedString, bool, &mut Window, &mut App) + 'static>>,
    aria_label: Option<SharedString>,
    aria_role: Option<AriaRole>,
}

impl Accordion {
    /// Create a new accordion
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            expanded: Vec::new(),
            mode: AccordionMode::default(),
            orientation: AccordionOrientation::default(),
            theme: None,
            on_change: None,
            aria_label: None,
            aria_role: None,
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

    /// Set orientation
    pub fn orientation(mut self, orientation: AccordionOrientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Set theme
    pub fn theme(mut self, theme: AccordionTheme) -> Self {
        self.theme = Some(theme);
        self
    }

    /// Set an explicit ARIA label
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    /// Override the default ARIA role (Group)
    pub fn aria_role(mut self, role: AriaRole) -> Self {
        self.aria_role = Some(role);
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
        // Use self.theme if provided, otherwise clone the passed theme
        let theme = self.theme.unwrap_or_else(|| theme.clone());

        // Handle Side layout separately since it needs different structure
        let is_side = matches!(self.orientation, AccordionOrientation::Side);
        if is_side {
            let Accordion {
                items,
                expanded,
                on_change,
                ..
            } = self;
            let on_change = on_change.map(|h| std::rc::Rc::new(h));
            return Self::build_side_layout_static(items, expanded, theme, on_change);
        }

        let on_change = self.on_change.map(|h| std::rc::Rc::new(h));
        let is_vertical = matches!(self.orientation, AccordionOrientation::Vertical);

        let mut container = div()
            .flex()
            .border_1()
            .border_color(theme.border)
            .rounded_lg();

        // Set flex direction based on orientation
        container = if is_vertical {
            container.flex_col()
        } else {
            container.flex_row()
        };

        for (idx, item) in self.items.into_iter().enumerate() {
            let is_expanded = self.expanded.contains(&item.id);
            let item_id = item.id.clone();
            let is_first = idx == 0;

            // Create item wrapper for horizontal layout
            let mut item_wrapper = div();
            if !is_vertical {
                item_wrapper = item_wrapper.flex().flex_col();
            }

            // Header
            let mut header = div()
                .id(SharedString::from(format!("accordion-header-{}", item_id)))
                .flex()
                .items_center()
                .gap_2()
                .px_4()
                .py_3()
                .bg(if is_expanded {
                    theme.header_active_bg
                } else {
                    theme.header_bg
                })
                .cursor_pointer();

            // Add border based on orientation
            if !is_first {
                header = if is_vertical {
                    header.border_t_1().border_color(theme.border)
                } else {
                    header.border_l_1().border_color(theme.border)
                };
            }

            if item.disabled {
                header = header.opacity(0.5).cursor_not_allowed();
            } else {
                let hover_bg = theme.header_hover_bg;
                let hover_accent = theme.accent_tint;
                header = header.hover(move |style| {
                    style
                        .bg(hover_bg)
                        .border_color(hover_accent)
                        .shadow(glow_shadow(hover_bg))
                });

                // Click handler
                if let Some(handler) = on_change.clone() {
                    let id = item_id.clone();
                    let new_state = !is_expanded;
                    header = header.on_mouse_up(MouseButton::Left, move |_event, window, cx| {
                        (handler)(&id, new_state, window, cx);
                    });
                }
            }

            let rail_bg = if is_expanded {
                theme.accent
            } else {
                theme.accent_tint
            };
            header = header.child(div().w(px(3.0)).h(px(22.0)).rounded(px(1.5)).bg(rail_bg));

            // Title
            header = header.child(
                div()
                    .flex_1()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.title_color)
                    .child(item.title),
            );

            // Expand/collapse indicator (different for vertical vs horizontal)
            let indicator = if is_vertical {
                if is_expanded { "▼" } else { "▶" }
            } else if is_expanded {
                "▼"
            } else {
                "▲"
            };
            header = header.child(
                div()
                    .text_xs()
                    .text_color(if is_expanded {
                        theme.accent
                    } else {
                        theme.indicator_color
                    })
                    .child(indicator),
            );

            item_wrapper = item_wrapper.child(header);

            // Content (only if expanded)
            if is_expanded && let Some(content) = item.content {
                let content_div = div()
                    .px_4()
                    .py_3()
                    .bg(theme.content_bg)
                    .border_t_1()
                    .border_color(if is_expanded {
                        theme.accent_tint
                    } else {
                        theme.border
                    });

                item_wrapper = item_wrapper.child(content_div.child(content));
            }

            container = container.child(item_wrapper);
        }

        container
    }

    /// Build side layout: headers vertically on left, content expands to right
    fn build_side_layout_static(
        items: Vec<AccordionItem>,
        expanded: Vec<SharedString>,
        theme: AccordionTheme,
        on_change: Option<
            std::rc::Rc<Box<dyn Fn(&SharedString, bool, &mut Window, &mut App) + 'static>>,
        >,
    ) -> Div {
        let mut container = div()
            .flex()
            .flex_row()
            .border_1()
            .border_color(theme.border)
            .rounded_lg();

        // Left side: vertical header tabs
        let mut headers_container = div()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(theme.border);

        for (idx, item) in items.iter().enumerate() {
            let is_expanded = expanded.contains(&item.id);
            let item_id = item.id.clone();
            let is_first = idx == 0;

            let mut header = div()
                .id(SharedString::from(format!(
                    "accordion-header-side-{}",
                    item_id
                )))
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .w(px(42.0))
                .h(side_label_height(&item.title))
                .bg(if is_expanded {
                    theme.header_active_bg
                } else {
                    theme.header_bg
                })
                .cursor_pointer();

            if !is_first {
                header = header.border_t_1().border_color(theme.border);
            }

            if is_expanded {
                header = header.bg(theme.header_active_bg);
            }

            if item.disabled {
                header = header.opacity(0.5).cursor_not_allowed();
            } else {
                let hover_bg = theme.header_hover_bg;
                let hover_accent = theme.accent_tint;
                header = header.hover(move |style| {
                    style
                        .bg(hover_bg)
                        .border_color(hover_accent)
                        .shadow(glow_shadow(hover_bg))
                });

                // Click handler
                if let Some(handler) = on_change.clone() {
                    let id = item_id.clone();
                    let new_state = !is_expanded;
                    header = header.on_mouse_up(MouseButton::Left, move |_event, window, cx| {
                        (handler)(&id, new_state, window, cx);
                    });
                }
            }

            let label_text = item.title.to_string();
            let label_height = side_label_height(&item.title);
            let label_svg = rotated_side_label_svg(&label_text);
            let label_path =
                SharedString::from(format!("accordion-side-label:{item_id}:{label_text}"));
            let label_color = if is_expanded {
                theme.accent
            } else {
                theme.title_color
            };

            header =
                header
                    .child(div().absolute().top_0().bottom_0().left_0().w(px(3.0)).bg(
                        if is_expanded {
                            theme.accent
                        } else {
                            theme.accent_tint
                        },
                    ))
                    .child(
                        canvas(
                            move |_bounds, _window, _cx| label_svg,
                            move |bounds, label_svg, window, cx| {
                                let _ = window.paint_svg(
                                    bounds,
                                    label_path,
                                    Some(label_svg.as_bytes()),
                                    TransformationMatrix::unit(),
                                    Hsla::from(label_color),
                                    cx,
                                );
                            },
                        )
                        .w(px(18.0))
                        .h(label_height),
                    );

            headers_container = headers_container.child(header);
        }

        container = container.child(headers_container);

        // Right side: content area - show all expanded items side by side
        let mut content_container = div().flex().flex_row().flex_1();

        for item in items.into_iter() {
            let is_expanded = expanded.contains(&item.id);

            if is_expanded && let Some(content) = item.content {
                let content_div = div()
                    .flex_1()
                    .px_4()
                    .py_3()
                    .bg(theme.content_bg)
                    .border_r_1()
                    .border_color(theme.border)
                    .child(content);

                content_container = content_container.child(content_div);
            }
        }

        container = container.child(content_container);

        container
    }
}

fn side_label_height(label: &str) -> Pixels {
    px((label.chars().count() as f32 * 6.0 + 28.0).clamp(54.0, 126.0))
}

fn rotated_side_label_svg(label: &str) -> String {
    let escaped = escape_side_label_svg_text(label);
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="18" height="126" viewBox="0 0 18 126">
<text x="0" y="0" transform="translate(9 63) rotate(-90)" text-anchor="middle" dominant-baseline="middle" font-family="system-ui, -apple-system, BlinkMacSystemFont, sans-serif" font-size="11" font-weight="600" fill="black">{escaped}</text>
</svg>"#
    )
}

fn escape_side_label_svg_text(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

impl Default for Accordion {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Accordion {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Register in accessibility tree
        cx.register_accessible(AccessibilityNode {
            element_id: ElementId::Name("accordion".into()),
            label: self.aria_label.clone().unwrap_or_default(),
            props: AriaProps::with_role(self.aria_role.unwrap_or(AriaRole::Group)),
        });

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
