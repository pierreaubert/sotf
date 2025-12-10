//! Select/Dropdown component
//!
//! A dropdown select component for choosing from options with theming support.
//!
//! Features:
//! - Keyboard navigation:
//!   - Arrow Up/Down: navigate options
//!   - Enter: select highlighted option
//!   - Escape: close dropdown
//!   - Space: toggle dropdown open/closed
//! - Mouse support: click to toggle, hover to highlight

use gpui::prelude::*;
use gpui::*;

use crate::theme::{Theme, ThemeExt};

/// Theme colors for select styling
#[derive(Debug, Clone)]
pub struct SelectTheme {
    /// Trigger background color
    pub trigger_bg: Rgba,
    /// Trigger border color
    pub trigger_border: Rgba,
    /// Trigger border color on hover
    pub trigger_border_hover: Rgba,
    /// Trigger border color when focused/open
    pub trigger_border_focused: Rgba,
    /// Dropdown background color
    pub dropdown_bg: Rgba,
    /// Dropdown border color
    pub dropdown_border: Rgba,
    /// Selected option background
    pub selected_bg: Rgba,
    /// Option hover background
    pub option_hover_bg: Rgba,
    /// Label text color
    pub label_color: Rgba,
    /// Text color for selected value
    pub text_color: Rgba,
    /// Placeholder text color
    pub placeholder_color: Rgba,
    /// Option text color
    pub option_text_color: Rgba,
    /// Selected option text color
    pub selected_text_color: Rgba,
    /// Disabled text color
    pub disabled_color: Rgba,
    /// Arrow/chevron color
    pub arrow_color: Rgba,
}

impl Default for SelectTheme {
    fn default() -> Self {
        Self {
            trigger_bg: rgba(0x1e1e1eff),
            trigger_border: rgba(0x3a3a3aff),
            trigger_border_hover: rgba(0x007accff),
            trigger_border_focused: rgba(0x007accff),
            dropdown_bg: rgba(0x2a2a2aff),
            dropdown_border: rgba(0x3a3a3aff),
            selected_bg: rgba(0x007accff),
            option_hover_bg: rgba(0x3a3a3aff),
            label_color: rgba(0xccccccff),
            text_color: rgba(0xffffffff),
            placeholder_color: rgba(0x666666ff),
            option_text_color: rgba(0xccccccff),
            selected_text_color: rgba(0xffffffff),
            disabled_color: rgba(0x666666ff),
            arrow_color: rgba(0x666666ff),
        }
    }
}

impl From<&Theme> for SelectTheme {
    fn from(theme: &Theme) -> Self {
        Self {
            trigger_bg: theme.background,
            trigger_border: theme.border,
            trigger_border_hover: theme.accent,
            trigger_border_focused: theme.accent,
            dropdown_bg: theme.surface,
            dropdown_border: theme.border,
            selected_bg: theme.accent,
            option_hover_bg: theme.surface_hover,
            label_color: theme.text_secondary,
            text_color: theme.text_primary,
            placeholder_color: theme.text_muted,
            option_text_color: theme.text_secondary,
            selected_text_color: theme.text_primary,
            disabled_color: theme.text_muted,
            arrow_color: theme.text_muted,
        }
    }
}

/// Select size variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectSize {
    /// Small
    Sm,
    /// Medium (default)
    #[default]
    Md,
    /// Large
    Lg,
}

/// A select option
#[derive(Clone)]
pub struct SelectOption {
    /// Option value
    pub value: SharedString,
    /// Display label
    pub label: SharedString,
    /// Whether option is disabled
    pub disabled: bool,
}

impl SelectOption {
    /// Create a new select option
    pub fn new(value: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            disabled: false,
        }
    }

    /// Set disabled state
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// A select dropdown component with theming support
pub struct Select {
    id: ElementId,
    options: Vec<SelectOption>,
    selected: Option<SharedString>,
    placeholder: Option<SharedString>,
    label: Option<SharedString>,
    size: SelectSize,
    disabled: bool,
    is_open: bool,
    highlighted_index: Option<usize>,
    theme: Option<SelectTheme>,
    on_change: Option<Box<dyn Fn(&SharedString, &mut Window, &mut App) + 'static>>,
    on_toggle: Option<Box<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
    on_highlight: Option<Box<dyn Fn(Option<usize>, &mut Window, &mut App) + 'static>>,
}

impl Select {
    /// Create a new select
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            options: Vec::new(),
            selected: None,
            placeholder: None,
            label: None,
            size: SelectSize::default(),
            disabled: false,
            is_open: false,
            highlighted_index: None,
            theme: None,
            on_change: None,
            on_toggle: None,
            on_highlight: None,
        }
    }

    /// Set options
    pub fn options(mut self, options: Vec<SelectOption>) -> Self {
        self.options = options;
        self
    }

    /// Set selected value
    pub fn selected(mut self, value: impl Into<SharedString>) -> Self {
        self.selected = Some(value.into());
        self
    }

    /// Set placeholder
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Set label
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set size
    pub fn size(mut self, size: SelectSize) -> Self {
        self.size = size;
        self
    }

    /// Set disabled state
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set open state (for controlled component)
    pub fn is_open(mut self, is_open: bool) -> Self {
        self.is_open = is_open;
        self
    }

    /// Set highlighted index (for keyboard navigation)
    pub fn highlighted_index(mut self, index: Option<usize>) -> Self {
        self.highlighted_index = index;
        self
    }

    /// Set theme
    pub fn theme(mut self, theme: SelectTheme) -> Self {
        self.theme = Some(theme);
        self
    }

    /// Set change handler
    pub fn on_change(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// Set toggle handler (called when trigger is clicked)
    pub fn on_toggle(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Box::new(handler));
        self
    }

    /// Set highlight handler (called when highlighted option changes during keyboard navigation)
    pub fn on_highlight(
        mut self,
        handler: impl Fn(Option<usize>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_highlight = Some(Box::new(handler));
        self
    }

    /// Build into element
    fn build(self, theme: &SelectTheme) -> Div {
        let (py, _text_size_class) = match self.size {
            SelectSize::Sm => (px(4.0), "sm"),
            SelectSize::Md => (px(8.0), "md"),
            SelectSize::Lg => (px(12.0), "lg"),
        };

        let mut container = div().relative().flex().flex_col().gap_1();

        // Label
        if let Some(label) = self.label {
            container = container.child(
                div()
                    .text_sm()
                    .text_color(theme.label_color)
                    .font_weight(FontWeight::MEDIUM)
                    .child(label),
            );
        }

        // Find selected option label
        let selected_label = self.selected.as_ref().and_then(|val| {
            self.options
                .iter()
                .find(|o| &o.value == val)
                .map(|o| o.label.clone())
        });

        // Select trigger
        let border_color = if self.is_open {
            theme.trigger_border_focused
        } else {
            theme.trigger_border
        };

        let mut trigger = div()
            .id(self.id)
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .py(py)
            .min_w(px(120.0))
            .bg(theme.trigger_bg)
            .border_1()
            .border_color(border_color)
            .rounded_md()
            .cursor_pointer();

        // Apply text size
        trigger = match self.size {
            SelectSize::Sm => trigger.text_xs(),
            SelectSize::Md => trigger.text_sm(),
            SelectSize::Lg => trigger,
        };

        // Convert handlers to Rc upfront so we can use them in closures
        let on_toggle_rc = self.on_toggle.map(std::rc::Rc::new);
        let on_change_rc = self.on_change.map(std::rc::Rc::new);
        let on_highlight_rc = self.on_highlight.map(std::rc::Rc::new);

        let currently_open = self.is_open;
        let num_options = self.options.len();
        let current_highlight = self.highlighted_index;

        if self.disabled {
            trigger = trigger.opacity(0.5).cursor_not_allowed();
        } else {
            let hover_border = theme.trigger_border_hover;
            trigger = trigger.hover(move |s| s.border_color(hover_border));

            // Mouse click handler - use on_mouse_down for more reliable response
            if let Some(ref handler) = on_toggle_rc {
                let handler = handler.clone();
                trigger = trigger.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    (handler)(!currently_open, window, cx);
                });
            }

            // Keyboard handler
            if let Some(ref toggle_handler) = on_toggle_rc {
                let toggle_rc = toggle_handler.clone();
                let change_rc = on_change_rc.clone();
                let highlight_rc = on_highlight_rc.clone();
                let options_clone = self.options.clone();

                trigger = trigger.on_key_down(move |event, window, cx| {
                    match event.keystroke.key.as_str() {
                        "space" | " " => {
                            // Toggle open/closed
                            toggle_rc(!currently_open, window, cx);
                        }
                        "escape" if currently_open => {
                            // Close dropdown
                            toggle_rc(false, window, cx);
                        }
                        "enter" if currently_open => {
                            // Select highlighted option
                            if let Some(idx) = current_highlight {
                                if idx < options_clone.len() && !options_clone[idx].disabled {
                                    if let Some(ref change_handler) = change_rc {
                                        change_handler(&options_clone[idx].value, window, cx);
                                    }
                                    toggle_rc(false, window, cx);
                                }
                            }
                        }
                        "down" | "up" if currently_open => {
                            // Navigate options
                            let delta = if event.keystroke.key == "down" {
                                1
                            } else {
                                -1_i32
                            };
                            let new_idx = if let Some(idx) = current_highlight {
                                let new = idx as i32 + delta;
                                if new < 0 {
                                    Some(num_options.saturating_sub(1))
                                } else if new >= num_options as i32 {
                                    Some(0)
                                } else {
                                    Some(new as usize)
                                }
                            } else {
                                // No highlight yet, start at first/last
                                if delta > 0 {
                                    Some(0)
                                } else {
                                    Some(num_options.saturating_sub(1))
                                }
                            };

                            if let Some(ref highlight_handler) = highlight_rc {
                                highlight_handler(new_idx, window, cx);
                            }
                        }
                        _ => {}
                    }
                });
            }
        }

        // Display value or placeholder
        let display_text = if let Some(label) = selected_label {
            div().text_color(theme.text_color).child(label)
        } else if let Some(placeholder) = self.placeholder {
            div().text_color(theme.placeholder_color).child(placeholder)
        } else {
            div().text_color(theme.placeholder_color).child("Select...")
        };

        trigger = trigger.child(display_text);

        // Dropdown arrow
        trigger = trigger.child(div().text_xs().text_color(theme.arrow_color).child("▼"));

        container = container.child(trigger);

        // Dropdown menu (only shown when open)
        if self.is_open {
            let mut dropdown = div()
                .id("select-dropdown")
                .absolute()
                .top_full()
                .left_0()
                .right_0()
                .mt_1()
                .bg(theme.dropdown_bg)
                .opacity(1.0)
                .border_1()
                .border_color(theme.dropdown_border)
                .rounded_md()
                .shadow_lg()
                .max_h(px(200.0))
                .overflow_y_scroll()
                .py_1();

            for (idx, option) in self.options.iter().enumerate() {
                let is_selected = self.selected.as_ref() == Some(&option.value);
                let is_highlighted = self.highlighted_index == Some(idx);
                let option_value = option.value.clone();

                let mut option_el = div()
                    .id(SharedString::from(format!("select-option-{}", idx)))
                    .px_3()
                    .py(px(6.0))
                    .cursor_pointer();

                // Apply text size
                option_el = match self.size {
                    SelectSize::Sm => option_el.text_xs(),
                    SelectSize::Md => option_el.text_sm(),
                    SelectSize::Lg => option_el,
                };

                if option.disabled {
                    option_el = option_el
                        .bg(theme.dropdown_bg)
                        .opacity(1.0)
                        .text_color(theme.disabled_color)
                        .cursor_not_allowed();
                } else if is_selected {
                    option_el = option_el
                        .bg(theme.selected_bg)
                        .text_color(theme.selected_text_color);
                } else if is_highlighted {
                    // Highlight option for keyboard navigation
                    option_el = option_el
                        .bg(theme.option_hover_bg)
                        .text_color(theme.option_text_color);
                } else {
                    let hover_bg = theme.option_hover_bg;
                    option_el = option_el
                        .bg(theme.dropdown_bg)
                        .opacity(1.0)
                        .text_color(theme.option_text_color)
                        .hover(move |s| s.bg(hover_bg));

                    // Add click handler for non-disabled, non-selected options
                    if let Some(ref handler) = on_change_rc {
                        let handler_click = handler.clone();
                        option_el =
                            option_el.on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                                handler_click(&option_value, window, cx);
                            });
                    }
                }

                option_el = option_el.child(option.label.clone());
                dropdown = dropdown.child(option_el);
            }

            container = container.child(dropdown);
        }

        container
    }
}

impl RenderOnce for Select {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let global_theme = cx.theme();
        let theme = self
            .theme
            .clone()
            .unwrap_or_else(|| SelectTheme::from(&global_theme));

        self.build(&theme)
    }
}

impl IntoElement for Select {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        // When used without context, fall back to default theme
        let theme = self.theme.clone().unwrap_or_default();
        self.build(&theme)
    }
}
