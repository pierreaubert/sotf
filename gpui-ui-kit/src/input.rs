//! Input component
//!
//! Text input field with optional label, placeholder, and validation.
//!
//! Features:
//! - Full keyboard text editing support (self-contained)
//! - Click to focus and start editing
//! - Enter to confirm, Escape to cancel
//! - Cursor navigation and text selection
//! - Emacs-style keybindings (Ctrl+A/E/K/U/W/H/D/F/B)
//! - Disabled and readonly states
//!
//! # Simple Usage
//!
//! The Input component handles all focus and keyboard events internally.
//! Just provide callbacks for changes:
//!
//! ```ignore
//! Input::new("my-input")
//!     .value(current_value)
//!     .placeholder("Enter text...")
//!     .on_change(|new_value, _window, _cx| {
//!         // Called when user confirms with Enter
//!         println!("Value changed to: {}", new_value);
//!     })
//!     .on_text_change(|text, _window, _cx| {
//!         // Called on every keystroke (optional, for live updates)
//!         println!("Current text: {}", text);
//!     })
//! ```

use crate::theme::{Theme, ThemeExt};
use gpui::prelude::*;
use gpui::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

// Thread-local registry for focus handles, keyed by element ID.
// This ensures the same focus handle is reused across renders for Input components
// that don't provide their own focus handle. Without this, focus would be lost
// on every re-render since Input is a RenderOnce component.
thread_local! {
    static FOCUS_HANDLES: RefCell<HashMap<ElementId, FocusHandle>> = RefCell::new(HashMap::new());
}

// Thread-local registry for edit state, keyed by element ID.
// This ensures edit state (cursor position, current text, selection) persists
// across renders. Without this, every re-render would reset the editing state.
thread_local! {
    static EDIT_STATES: RefCell<HashMap<ElementId, Rc<RefCell<EditState>>>> = RefCell::new(HashMap::new());
}

/// Theme colors for input styling
#[derive(Debug, Clone)]
pub struct InputTheme {
    /// Background color
    pub background: Rgba,
    /// Filled variant background
    pub filled_bg: Rgba,
    /// Text color
    pub text: Rgba,
    /// Placeholder color
    pub placeholder: Rgba,
    /// Label color
    pub label: Rgba,
    /// Border color
    pub border: Rgba,
    /// Border hover color
    pub border_hover: Rgba,
    /// Border focus color
    pub border_focus: Rgba,
    /// Error color
    pub error: Rgba,
    /// Cursor color
    pub cursor: Rgba,
    /// Selection background
    pub selection_bg: Rgba,
}

impl Default for InputTheme {
    fn default() -> Self {
        Self {
            background: rgb(0x1e1e1e),
            filled_bg: rgb(0x2a2a2a),
            text: rgb(0xffffff),
            placeholder: rgb(0x666666),
            label: rgb(0xcccccc),
            border: rgb(0x3a3a3a),
            border_hover: rgb(0x007acc),
            border_focus: rgb(0x007acc),
            error: rgb(0xcc3333),
            cursor: rgb(0x007acc),
            selection_bg: rgba(0x007acc44),
        }
    }
}

impl From<&Theme> for InputTheme {
    fn from(theme: &Theme) -> Self {
        Self {
            background: theme.background,
            filled_bg: theme.surface,
            text: theme.text_primary,
            placeholder: theme.text_muted,
            label: theme.text_secondary,
            border: theme.border,
            border_hover: theme.accent,
            border_focus: theme.accent,
            error: theme.error,
            cursor: theme.accent,
            selection_bg: Rgba {
                r: theme.accent.r,
                g: theme.accent.g,
                b: theme.accent.b,
                a: 0.3,
            },
        }
    }
}

/// Input size variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputSize {
    /// Small input
    Sm,
    /// Medium input (default)
    #[default]
    Md,
    /// Large input
    Lg,
}

/// Input visual variant
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputVariant {
    /// Default input style
    #[default]
    Default,
    /// Filled background
    Filled,
    /// Flushed (bottom border only)
    Flushed,
}

/// Internal editing state for the input
#[derive(Clone, Default)]
struct EditState {
    /// Whether currently editing
    editing: bool,
    /// Current edit text
    text: String,
    /// Cursor position (character index)
    cursor: usize,
    /// Whether all text is selected
    text_selected: bool,
}

impl EditState {
    fn new(value: &str) -> Self {
        Self {
            editing: true,
            text: value.to_string(),
            cursor: value.chars().count(),
            text_selected: true,
        }
    }

    fn move_to_start(&mut self) {
        self.cursor = 0;
        self.text_selected = false;
    }

    fn move_to_end(&mut self) {
        self.cursor = self.text.chars().count();
        self.text_selected = false;
    }

    fn move_forward(&mut self) {
        let len = self.text.chars().count();
        if self.cursor < len {
            self.cursor += 1;
        }
        self.text_selected = false;
    }

    fn move_backward(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
        self.text_selected = false;
    }

    fn kill_to_end(&mut self) {
        let chars: Vec<char> = self.text.chars().collect();
        self.text = chars[..self.cursor].iter().collect();
        self.text_selected = false;
    }

    fn kill_to_start(&mut self) {
        let chars: Vec<char> = self.text.chars().collect();
        self.text = chars[self.cursor..].iter().collect();
        self.cursor = 0;
        self.text_selected = false;
    }

    fn kill_word_backward(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let chars: Vec<char> = self.text.chars().collect();
        let mut new_pos = self.cursor;
        // Skip trailing spaces
        while new_pos > 0 && chars[new_pos - 1].is_whitespace() {
            new_pos -= 1;
        }
        // Skip word characters
        while new_pos > 0 && !chars[new_pos - 1].is_whitespace() {
            new_pos -= 1;
        }
        let mut new_chars = chars[..new_pos].to_vec();
        new_chars.extend_from_slice(&chars[self.cursor..]);
        self.text = new_chars.into_iter().collect();
        self.cursor = new_pos;
        self.text_selected = false;
    }

    fn do_backspace(&mut self) {
        if self.text_selected {
            self.text.clear();
            self.cursor = 0;
            self.text_selected = false;
        } else if self.cursor > 0 {
            let mut chars: Vec<char> = self.text.chars().collect();
            chars.remove(self.cursor - 1);
            self.text = chars.into_iter().collect();
            self.cursor -= 1;
        }
    }

    fn do_delete(&mut self) {
        if self.text_selected {
            self.text.clear();
            self.cursor = 0;
            self.text_selected = false;
        } else {
            let len = self.text.chars().count();
            if self.cursor < len {
                let mut chars: Vec<char> = self.text.chars().collect();
                chars.remove(self.cursor);
                self.text = chars.into_iter().collect();
            }
        }
    }

    fn insert_text(&mut self, char_text: &str) {
        if self.text_selected {
            self.text.clear();
            self.cursor = 0;
            self.text_selected = false;
        }
        let mut chars: Vec<char> = self.text.chars().collect();
        for (i, c) in char_text.chars().enumerate() {
            chars.insert(self.cursor + i, c);
        }
        self.text = chars.into_iter().collect();
        self.cursor += char_text.chars().count();
    }
}

/// A text input component with full keyboard editing support
///
/// The Input handles all focus and keyboard events internally.
/// Parent components only need to provide callbacks for value changes.
pub struct Input {
    id: ElementId,
    value: SharedString,
    placeholder: Option<SharedString>,
    label: Option<SharedString>,
    size: InputSize,
    variant: InputVariant,
    disabled: bool,
    readonly: bool,
    error: Option<SharedString>,
    icon_left: Option<SharedString>,
    icon_right: Option<SharedString>,
    bg_color: Option<Rgba>,
    text_color: Option<Rgba>,
    border_color: Option<Rgba>,
    placeholder_color: Option<Rgba>,
    /// Called when value is confirmed (Enter pressed)
    on_change: Option<Box<dyn Fn(&str, &mut Window, &mut App) + 'static>>,
    /// Called when editing starts (click on input)
    on_edit_start: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
    /// Called when editing ends (Enter = Some(value), Escape = None)
    on_edit_end: Option<Box<dyn Fn(Option<String>, &mut Window, &mut App) + 'static>>,
    /// Called on every text change during editing (for live updates)
    on_text_change: Option<Box<dyn Fn(String, &mut Window, &mut App) + 'static>>,
    /// Focus handle for this input
    focus_handle: Option<FocusHandle>,
}

impl Input {
    /// Create a new input
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            value: "".into(),
            placeholder: None,
            label: None,
            size: InputSize::default(),
            variant: InputVariant::default(),
            disabled: false,
            readonly: false,
            error: None,
            icon_left: None,
            icon_right: None,
            bg_color: None,
            text_color: None,
            border_color: None,
            placeholder_color: None,
            on_change: None,
            on_edit_start: None,
            on_edit_end: None,
            on_text_change: None,
            focus_handle: None,
        }
    }

    /// Set the focus handle (optional - one is created internally if not provided)
    pub fn focus_handle(mut self, handle: FocusHandle) -> Self {
        self.focus_handle = Some(handle);
        self
    }

    /// Set the input value
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = value.into();
        self
    }

    /// Set placeholder text
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Set label text
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set input size
    pub fn size(mut self, size: InputSize) -> Self {
        self.size = size;
        self
    }

    /// Set input variant
    pub fn variant(mut self, variant: InputVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Set disabled state
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set readonly state
    pub fn readonly(mut self, readonly: bool) -> Self {
        self.readonly = readonly;
        self
    }

    /// Set error message
    pub fn error(mut self, error: impl Into<SharedString>) -> Self {
        self.error = Some(error.into());
        self
    }

    /// Set left icon
    pub fn icon_left(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon_left = Some(icon.into());
        self
    }

    /// Set right icon
    pub fn icon_right(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon_right = Some(icon.into());
        self
    }

    /// Set background color
    pub fn bg_color(mut self, color: impl Into<Rgba>) -> Self {
        self.bg_color = Some(color.into());
        self
    }

    /// Set text color
    pub fn text_color(mut self, color: impl Into<Rgba>) -> Self {
        self.text_color = Some(color.into());
        self
    }

    /// Set border color
    pub fn border_color(mut self, color: impl Into<Rgba>) -> Self {
        self.border_color = Some(color.into());
        self
    }

    /// Set placeholder color
    pub fn placeholder_color(mut self, color: impl Into<Rgba>) -> Self {
        self.placeholder_color = Some(color.into());
        self
    }

    /// Set change handler (called when input value is confirmed with Enter)
    pub fn on_change(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    /// Set edit start handler (called when user clicks on input to edit)
    pub fn on_edit_start(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_edit_start = Some(Box::new(handler));
        self
    }

    /// Set edit end handler (called when user confirms or cancels edit)
    /// The Option<String> is Some(value) if confirmed, None if cancelled
    pub fn on_edit_end(
        mut self,
        handler: impl Fn(Option<String>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_edit_end = Some(Box::new(handler));
        self
    }

    /// Set text change handler (called on every keystroke during editing)
    pub fn on_text_change(
        mut self,
        handler: impl Fn(String, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_text_change = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for Input {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let global_theme = cx.theme();
        let theme = InputTheme::from(&global_theme);

        let (py, _text_size) = match self.size {
            InputSize::Sm => (px(4.0), "text_xs"),
            InputSize::Md => (px(8.0), "text_sm"),
            InputSize::Lg => (px(12.0), "text_base"),
        };

        let has_error = self.error.is_some();
        let disabled = self.disabled;
        let readonly = self.readonly;
        let current_value = self.value.clone();

        // Use provided focus handle, or get/create one from the registry.
        // The registry ensures the same focus handle is reused across renders,
        // which is critical since Input is a RenderOnce component.
        let focus_handle = self.focus_handle.unwrap_or_else(|| {
            FOCUS_HANDLES.with(|handles| {
                let mut handles = handles.borrow_mut();
                handles
                    .entry(self.id.clone())
                    .or_insert_with(|| cx.focus_handle())
                    .clone()
            })
        });

        // Determine editing state from focus
        // The input is "editing" when it has focus
        let is_focused = focus_handle.is_focused(window);

        // When focused, we're always in editing mode
        let editing = is_focused && !disabled && !readonly;

        // Get or create edit state from registry (persists across renders)
        let edit_state = EDIT_STATES.with(|states| {
            let mut states = states.borrow_mut();
            states
                .entry(self.id.clone())
                .or_insert_with(|| Rc::new(RefCell::new(EditState::default())))
                .clone()
        });

        // Get display state from edit_state
        let state = edit_state.borrow();
        let text_selected = state.text_selected && editing;
        // When editing, display the internal state.text; otherwise display props value
        let edit_text = if editing && state.editing {
            state.text.clone()
        } else {
            current_value.to_string()
        };
        drop(state);

        let border_color = if has_error {
            theme.error
        } else if editing {
            theme.border_focus
        } else {
            self.border_color.unwrap_or(theme.border)
        };

        let mut container = div().flex().flex_col().gap_1();

        // Label
        if let Some(label) = &self.label {
            container = container.child(
                div()
                    .text_sm()
                    .text_color(theme.label)
                    .font_weight(FontWeight::MEDIUM)
                    .child(label.clone()),
            );
        }

        // Create a unique ID for the input field
        let field_id = ElementId::Name(format!("{:?}-field", self.id).into());

        // Input wrapper
        let mut input_wrapper = div()
            .id(self.id.clone())
            .track_focus(&focus_handle)
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .py(py)
            .rounded_md()
            .border_1()
            .border_color(border_color)
            .focusable();

        // Apply variant styling
        match self.variant {
            InputVariant::Default => {
                input_wrapper = input_wrapper.bg(self.bg_color.unwrap_or(theme.background));
            }
            InputVariant::Filled => {
                input_wrapper = input_wrapper
                    .bg(self.bg_color.unwrap_or(theme.filled_bg))
                    .border_color(rgba(0x00000000));
            }
            InputVariant::Flushed => {
                input_wrapper = input_wrapper
                    .bg(rgba(0x00000000))
                    .border_0()
                    .border_b_1()
                    .border_color(border_color)
                    .rounded_none();
            }
        }

        let border_hover = theme.border_hover;
        if disabled {
            input_wrapper = input_wrapper.opacity(0.5).cursor_not_allowed();
        } else if !readonly {
            input_wrapper = input_wrapper
                .cursor_text()
                .hover(move |s| s.border_color(border_hover));
        }

        let placeholder_color = self.placeholder_color.unwrap_or(theme.placeholder);
        let text_color = self.text_color.unwrap_or(theme.text);
        let selection_bg = theme.selection_bg;

        // Wrap handlers in Rc for sharing
        let on_change_rc = self.on_change.map(Rc::new);
        let on_edit_start_rc = self.on_edit_start.map(Rc::new);
        let on_edit_end_rc = self.on_edit_end.map(Rc::new);
        let on_text_change_rc = self.on_text_change.map(Rc::new);

        // Add click handler - focus and start editing
        if !disabled && !readonly {
            let focus_handle_for_click = focus_handle.clone();
            let edit_state_for_click = edit_state.clone();
            let value_for_click = current_value.to_string();
            let on_edit_start_click = on_edit_start_rc.clone();

            input_wrapper =
                input_wrapper.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    // Focus the input
                    window.focus(&focus_handle_for_click, cx);

                    // Start editing if not already
                    let mut state = edit_state_for_click.borrow_mut();
                    if !state.editing {
                        *state = EditState::new(&value_for_click);
                        drop(state);

                        // Call on_edit_start callback
                        if let Some(ref handler) = on_edit_start_click {
                            handler(window, cx);
                        }
                    }
                });
        }

        // Add keyboard event handling
        // The edit_state persists across renders (via registry), so we use state.text
        // as the source of truth during editing.
        if !disabled && !readonly {
            let edit_state_for_key = edit_state.clone();
            let on_edit_end_key = on_edit_end_rc.clone();
            let on_text_change_key = on_text_change_rc.clone();
            let on_change_key = on_change_rc.clone();
            let focus_handle_for_key = focus_handle.clone();
            let current_value_for_key = current_value.to_string();

            input_wrapper = input_wrapper.on_key_down(move |event, window, cx| {
                // Check if we're focused (editing)
                if !focus_handle_for_key.is_focused(window) {
                    return;
                }

                // Stop propagation to prevent other handlers from firing
                cx.stop_propagation();

                let key = event.keystroke.key.as_str();
                let ctrl = event.keystroke.modifiers.control;

                // Get edit state - only initialize from props if not yet editing
                // (e.g., if focus was gained via tab navigation instead of click)
                let mut state = edit_state_for_key.borrow_mut();
                if !state.editing {
                    // Initialize state from props value
                    state.text = current_value_for_key.clone();
                    state.editing = true;
                    state.cursor = state.text.chars().count();
                    state.text_selected = true;
                }

                // Emacs-style keybindings when Ctrl is held
                if ctrl {
                    match key {
                        "a" => state.move_to_start(),
                        "e" => state.move_to_end(),
                        "k" => state.kill_to_end(),
                        "u" => state.kill_to_start(),
                        "w" => state.kill_word_backward(),
                        "h" => state.do_backspace(),
                        "d" => state.do_delete(),
                        "f" => state.move_forward(),
                        "b" => state.move_backward(),
                        _ => {}
                    }
                    // Notify text change and refresh display
                    let text = state.text.clone();
                    drop(state);
                    if let Some(ref handler) = on_text_change_key {
                        handler(text, window, cx);
                    }
                    window.refresh();
                    return;
                }

                // Regular key handling
                match key {
                    "enter" => {
                        // Confirm edit - blur the input
                        let text = state.text.clone();
                        state.editing = false;
                        state.text_selected = false;
                        drop(state);

                        // Blur focus
                        window.blur();

                        // Call on_change callback
                        if let Some(ref handler) = on_change_key {
                            handler(&text, window, cx);
                        }
                        // Call on_edit_end callback
                        if let Some(ref handler) = on_edit_end_key {
                            handler(Some(text), window, cx);
                        }
                    }
                    "escape" => {
                        // Cancel edit - blur the input
                        state.editing = false;
                        state.text_selected = false;
                        drop(state);

                        // Blur focus
                        window.blur();

                        // Call on_edit_end callback
                        if let Some(ref handler) = on_edit_end_key {
                            handler(None, window, cx);
                        }
                    }
                    "backspace" => {
                        state.do_backspace();
                        let text = state.text.clone();
                        drop(state);
                        if let Some(ref handler) = on_text_change_key {
                            handler(text, window, cx);
                        }
                        window.refresh();
                    }
                    "delete" => {
                        state.do_delete();
                        let text = state.text.clone();
                        drop(state);
                        if let Some(ref handler) = on_text_change_key {
                            handler(text, window, cx);
                        }
                        window.refresh();
                    }
                    "left" => {
                        state.move_backward();
                        drop(state);
                        window.refresh();
                    }
                    "right" => {
                        state.move_forward();
                        drop(state);
                        window.refresh();
                    }
                    "home" => {
                        state.move_to_start();
                        drop(state);
                        window.refresh();
                    }
                    "end" => {
                        state.move_to_end();
                        drop(state);
                        window.refresh();
                    }
                    _ => {
                        // Handle text input using key_char for IME support
                        if let Some(char_text) = event.keystroke.key_char.as_ref() {
                            state.insert_text(char_text);
                            let text = state.text.clone();
                            drop(state);
                            if let Some(ref handler) = on_text_change_key {
                                handler(text, window, cx);
                            }
                            window.refresh();
                        }
                    }
                }
            });
        }

        // Left icon
        if let Some(icon) = &self.icon_left {
            input_wrapper =
                input_wrapper.child(div().text_color(placeholder_color).child(icon.clone()));
        }

        // Determine display text
        let display_text = if editing {
            edit_text
        } else if current_value.is_empty() {
            self.placeholder
                .as_ref()
                .map(|s| s.to_string())
                .unwrap_or_default()
        } else {
            current_value.to_string()
        };

        // Visual selection highlight: when text_selected is true, show accent background
        let (value_bg, value_text_color) = if editing && text_selected {
            // Selected text: selection background with text color
            (Some(selection_bg), text_color)
        } else if !editing && current_value.is_empty() {
            // Placeholder text
            (None, placeholder_color)
        } else {
            // Normal text
            (None, text_color)
        };

        let mut text_el = div()
            .id(field_id)
            .flex_1()
            .text_color(value_text_color)
            .child(display_text);

        // Apply selection background if selected
        if let Some(bg) = value_bg {
            text_el = text_el.bg(bg);
        }

        // Apply text size
        text_el = match self.size {
            InputSize::Sm => text_el.text_xs(),
            InputSize::Md => text_el.text_sm(),
            InputSize::Lg => text_el,
        };

        input_wrapper = input_wrapper.child(text_el);

        // Right icon
        if let Some(icon) = &self.icon_right {
            input_wrapper =
                input_wrapper.child(div().text_color(placeholder_color).child(icon.clone()));
        }

        container = container.child(input_wrapper);

        // Error message
        if let Some(error) = &self.error {
            container =
                container.child(div().text_xs().text_color(theme.error).child(error.clone()));
        }

        container
    }
}

impl IntoElement for Input {
    type Element = gpui::Component<Self>;

    fn into_element(self) -> Self::Element {
        gpui::Component::new(self)
    }
}
