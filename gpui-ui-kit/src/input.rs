//! Input component
//!
//! Text input field with optional label, placeholder, and validation.
//!
//! Features:
//! - Full keyboard text editing support (self-contained)
//! - Click to focus and start editing
//! - Enter to confirm, Escape to cancel
//! - Cursor navigation and text selection
//! - Mouse drag to select text, double-click to select all
//! - Clipboard support: Cmd+C (copy), Cmd+X (cut), Cmd+V (paste), Cmd+A (select all)
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
//!
//! # Thread-Local State Pattern
//!
//! This component uses `thread_local!` storage to persist focus handles and
//! edit state across renders. This is necessary because GPUI's `RenderOnce`
//! components are recreated on each render, but we need state to persist:
//!
//! - **Focus handles**: Must be the same instance across renders or focus is lost
//! - **Edit state**: Cursor position, text, and selection must persist during editing
//!
//! ## Memory Considerations
//!
//! The thread-local `HashMap` entries grow as new element IDs are used and are
//! never automatically cleaned up. For most applications this is fine because:
//! - Element IDs are typically static or part of a bounded set
//! - The stored data is small (FocusHandle, EditState)
//!
//! If you have dynamic element IDs (e.g., from a virtualized list), consider:
//! 1. Using a stable ID scheme that reuses IDs
//! 2. Calling `cleanup_input_state(id)` when components are removed
//!
//! ## Cleanup Function
//!
//! To manually clean up state for a removed element:
//! ```rust,ignore
//! cleanup_input_state(&element_id);
//! ```

use crate::ComponentTheme;
use crate::theme::ThemeExt;
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

/// Clean up thread-local state for an Input element.
///
/// Call this when removing an Input with a dynamic element ID to prevent
/// memory leaks. For static element IDs, cleanup is not necessary.
///
/// # Example
/// ```rust,ignore
/// // When removing a dynamically-created Input
/// cleanup_input_state(&ElementId::Name(format!("input-{}", item_id).into()));
/// ```
pub fn cleanup_input_state(id: &ElementId) {
    FOCUS_HANDLES.with(|handles| {
        handles.borrow_mut().remove(id);
    });
    EDIT_STATES.with(|states| {
        states.borrow_mut().remove(id);
    });
}

/// Theme colors for input styling
#[derive(Debug, Clone, ComponentTheme)]
pub struct InputTheme {
    /// Background color
    #[theme(default = 0x1e1e1e, from = background)]
    pub background: Rgba,
    /// Filled variant background
    #[theme(default = 0x2a2a2a, from = surface)]
    pub filled_bg: Rgba,
    /// Text color
    #[theme(default = 0xffffff, from = text_primary)]
    pub text: Rgba,
    /// Placeholder color
    #[theme(default = 0x666666, from = text_muted)]
    pub placeholder: Rgba,
    /// Label color
    #[theme(default = 0xcccccc, from = text_secondary)]
    pub label: Rgba,
    /// Border color
    #[theme(default = 0x3a3a3a, from = border)]
    pub border: Rgba,
    /// Border hover color
    #[theme(default = 0x007acc, from = accent)]
    pub border_hover: Rgba,
    /// Border focus color
    #[theme(default = 0x007acc, from = accent)]
    pub border_focus: Rgba,
    /// Error color
    #[theme(default = 0xcc3333, from = error)]
    pub error: Rgba,
    /// Cursor color
    #[theme(default = 0x007acc, from = accent)]
    pub cursor: Rgba,
    /// Selection background
    #[theme(
        default = 0x007acc44,
        from_expr = "Rgba { r: theme.accent.r, g: theme.accent.g, b: theme.accent.b, a: 0.3 }"
    )]
    pub selection_bg: Rgba,
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

impl From<crate::ComponentSize> for InputSize {
    fn from(size: crate::ComponentSize) -> Self {
        match size {
            crate::ComponentSize::Xs | crate::ComponentSize::Sm => Self::Sm,
            crate::ComponentSize::Md => Self::Md,
            crate::ComponentSize::Lg | crate::ComponentSize::Xl => Self::Lg,
        }
    }
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
    /// Selection anchor (where selection started). If Some, selection is from anchor to cursor.
    selection_anchor: Option<usize>,
    /// Whether currently dragging to select
    is_dragging: bool,
}

impl EditState {
    fn new(value: &str) -> Self {
        let len = value.chars().count();
        Self {
            editing: true,
            text: value.to_string(),
            cursor: len,
            selection_anchor: Some(0), // Select all by default
            is_dragging: false,
        }
    }

    /// Check if there's any selection
    #[allow(dead_code)]
    fn has_selection(&self) -> bool {
        if let Some(anchor) = self.selection_anchor {
            anchor != self.cursor
        } else {
            false
        }
    }

    /// Get selection range (start, end) where start <= end
    fn selection_range(&self) -> Option<(usize, usize)> {
        self.selection_anchor.map(|anchor| {
            let start = anchor.min(self.cursor);
            let end = anchor.max(self.cursor);
            (start, end)
        })
    }

    /// Check if all text is selected
    #[allow(dead_code)]
    fn is_all_selected(&self) -> bool {
        if let Some((start, end)) = self.selection_range() {
            start == 0 && end == self.text.chars().count()
        } else {
            false
        }
    }

    /// Get the currently selected text
    fn get_selected_text(&self) -> Option<String> {
        if let Some((start, end)) = self.selection_range()
            && start != end
        {
            let chars: Vec<char> = self.text.chars().collect();
            return Some(chars[start..end].iter().collect());
        }
        None
    }

    fn clear_selection(&mut self) {
        self.selection_anchor = None;
    }

    fn move_to_start(&mut self) {
        self.cursor = 0;
        self.clear_selection();
    }

    fn move_to_end(&mut self) {
        self.cursor = self.text.chars().count();
        self.clear_selection();
    }

    fn move_forward(&mut self) {
        let len = self.text.chars().count();
        if self.cursor < len {
            self.cursor += 1;
        }
        self.clear_selection();
    }

    fn move_backward(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
        self.clear_selection();
    }

    fn select_all(&mut self) {
        self.selection_anchor = Some(0);
        self.cursor = self.text.chars().count();
    }

    fn kill_to_end(&mut self) {
        let chars: Vec<char> = self.text.chars().collect();
        self.text = chars[..self.cursor].iter().collect();
        self.clear_selection();
    }

    fn kill_to_start(&mut self) {
        let chars: Vec<char> = self.text.chars().collect();
        self.text = chars[self.cursor..].iter().collect();
        self.cursor = 0;
        self.clear_selection();
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
        self.clear_selection();
    }

    /// Delete selected text, returning true if something was deleted
    fn delete_selection(&mut self) -> bool {
        if let Some((start, end)) = self.selection_range()
            && start != end
        {
            let chars: Vec<char> = self.text.chars().collect();
            let mut new_chars = chars[..start].to_vec();
            new_chars.extend_from_slice(&chars[end..]);
            self.text = new_chars.into_iter().collect();
            self.cursor = start;
            self.clear_selection();
            return true;
        }
        false
    }

    fn do_backspace(&mut self) {
        if self.delete_selection() {
            return;
        }
        if self.cursor > 0 {
            // Find byte positions for character before cursor
            let byte_pos = self
                .text
                .char_indices()
                .nth(self.cursor - 1)
                .map(|(i, _)| i)
                .unwrap_or(0);
            let next_byte = self
                .text
                .char_indices()
                .nth(self.cursor)
                .map(|(i, _)| i)
                .unwrap_or(self.text.len());
            self.text.replace_range(byte_pos..next_byte, "");
            self.cursor -= 1;
        }
    }

    fn do_delete(&mut self) {
        if self.delete_selection() {
            return;
        }
        let len = self.text.chars().count();
        if self.cursor < len {
            // Find byte positions for character at cursor
            let byte_pos = self
                .text
                .char_indices()
                .nth(self.cursor)
                .map(|(i, _)| i)
                .unwrap_or(self.text.len());
            let next_byte = self
                .text
                .char_indices()
                .nth(self.cursor + 1)
                .map(|(i, _)| i)
                .unwrap_or(self.text.len());
            self.text.replace_range(byte_pos..next_byte, "");
        }
    }

    fn insert_text(&mut self, char_text: &str) {
        self.delete_selection();
        // Find byte position for insertion
        let byte_pos = self
            .text
            .char_indices()
            .nth(self.cursor)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len());
        self.text.insert_str(byte_pos, char_text);
        self.cursor += char_text.chars().count();
    }

    /// Start a selection at the given position
    fn start_selection(&mut self, pos: usize) {
        self.cursor = pos;
        self.selection_anchor = Some(pos);
        self.is_dragging = true;
    }

    /// Update selection during drag
    fn update_selection(&mut self, pos: usize) {
        self.cursor = pos;
    }

    /// End selection drag
    fn end_selection(&mut self) {
        self.is_dragging = false;
        // If no actual selection (anchor == cursor), clear the anchor
        if let Some(anchor) = self.selection_anchor
            && anchor == self.cursor
        {
            self.selection_anchor = None;
        }
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
        let selection_range = if editing {
            state.selection_range()
        } else {
            None
        };
        let _cursor_pos = state.cursor; // TODO: use for cursor rendering
        let _is_dragging = state.is_dragging;
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
        // Double-click selects all text
        // Single click positions cursor, drag selects text
        if !disabled && !readonly {
            let focus_handle_for_click = focus_handle.clone();
            let edit_state_for_click = edit_state.clone();
            let value_for_click = current_value.to_string();
            let on_edit_start_click = on_edit_start_rc.clone();
            let edit_text_for_click = edit_text.clone();

            input_wrapper =
                input_wrapper.on_mouse_down(MouseButton::Left, move |event, window, cx| {
                    // Focus the input
                    window.focus(&focus_handle_for_click, cx);

                    let mut state = edit_state_for_click.borrow_mut();

                    // Double-click: select all text
                    if event.click_count == 2 {
                        if state.editing {
                            state.select_all();
                        } else {
                            // Start editing with all text selected
                            *state = EditState::new(&value_for_click);
                        }
                        drop(state);
                        window.refresh();
                        return;
                    }

                    // Single click: start editing if not already, position cursor
                    if !state.editing {
                        *state = EditState::new(&value_for_click);
                        // Clear default selection, will be set by drag or stay at click position
                        state.clear_selection();
                        drop(state);

                        // Call on_edit_start callback
                        if let Some(ref handler) = on_edit_start_click {
                            handler(window, cx);
                        }
                    } else {
                        // Already editing - calculate cursor position from click
                        // Use a simple heuristic: assume monospace ~8px per character
                        let text_len = edit_text_for_click.chars().count();
                        let char_width = 8.0_f32; // Approximate width per character
                        let click_x: f32 = event.position.x.into();
                        // Estimate position (this is approximate without actual text metrics)
                        let char_pos = ((click_x / char_width).round() as usize).min(text_len);
                        state.start_selection(char_pos);
                        drop(state);
                    }
                    window.refresh();
                });

            // Mouse move handler for drag selection
            let edit_state_for_move = edit_state.clone();
            let edit_text_for_move = edit_text.clone();

            input_wrapper = input_wrapper.on_mouse_move(move |event, window, _cx| {
                let mut state = edit_state_for_move.borrow_mut();
                if state.is_dragging && state.editing {
                    let text_len = edit_text_for_move.chars().count();
                    let char_width = 8.0_f32;
                    let move_x: f32 = event.position.x.into();
                    let char_pos = ((move_x / char_width).round() as usize).min(text_len);
                    state.update_selection(char_pos);
                    drop(state);
                    window.refresh();
                }
            });

            // Mouse up handler to end drag selection
            let edit_state_for_up = edit_state.clone();

            input_wrapper =
                input_wrapper.on_mouse_up(MouseButton::Left, move |_event, window, _cx| {
                    let mut state = edit_state_for_up.borrow_mut();
                    if state.is_dragging {
                        state.end_selection();
                        drop(state);
                        window.refresh();
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
                let cmd = event.keystroke.modifiers.platform; // Cmd on macOS, Ctrl on other platforms

                // Get edit state - only initialize from props if not yet editing
                // (e.g., if focus was gained via tab navigation instead of click)
                let mut state = edit_state_for_key.borrow_mut();
                if !state.editing {
                    // Initialize state from props value
                    state.text = current_value_for_key.clone();
                    state.editing = true;
                    state.cursor = state.text.chars().count();
                    state.selection_anchor = Some(0); // Select all by default
                }

                // Clipboard operations (Cmd+C, Cmd+V, Cmd+X)
                if cmd {
                    match key {
                        "c" => {
                            // Copy selected text to clipboard
                            if let Some(selected) = state.get_selected_text() {
                                drop(state);
                                cx.write_to_clipboard(ClipboardItem::new_string(selected));
                            }
                            return;
                        }
                        "x" => {
                            // Cut selected text (copy + delete)
                            if let Some(selected) = state.get_selected_text() {
                                cx.write_to_clipboard(ClipboardItem::new_string(selected));
                                state.delete_selection();
                                let text = state.text.clone();
                                drop(state);
                                if let Some(ref handler) = on_text_change_key {
                                    handler(text, window, cx);
                                }
                                window.refresh();
                            }
                            return;
                        }
                        "v" => {
                            // Paste from clipboard
                            if let Some(clipboard) = cx.read_from_clipboard()
                                && let Some(paste_text) = clipboard.text()
                            {
                                state.insert_text(&paste_text);
                                let text = state.text.clone();
                                drop(state);
                                if let Some(ref handler) = on_text_change_key {
                                    handler(text, window, cx);
                                }
                                window.refresh();
                            }
                            return;
                        }
                        "a" => {
                            // Select all (Cmd+A)
                            state.select_all();
                            drop(state);
                            window.refresh();
                            return;
                        }
                        _ => {}
                    }
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
                        state.clear_selection();
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
                        state.clear_selection();
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

        // Build the text element with partial selection support
        let mut text_el = div().id(field_id).flex_1().flex().items_center();

        // Apply text size
        text_el = match self.size {
            InputSize::Sm => text_el.text_xs(),
            InputSize::Md => text_el.text_sm(),
            InputSize::Lg => text_el,
        };

        // Render text with selection highlighting
        if editing {
            if let Some((sel_start, sel_end)) = selection_range {
                if sel_start != sel_end {
                    // Partial or full selection - render in parts
                    let chars: Vec<char> = display_text.chars().collect();
                    let before: String = chars[..sel_start].iter().collect();
                    let selected: String = chars[sel_start..sel_end].iter().collect();
                    let after: String = chars[sel_end..].iter().collect();

                    if !before.is_empty() {
                        text_el = text_el.child(div().text_color(text_color).child(before));
                    }
                    text_el = text_el.child(
                        div()
                            .bg(selection_bg)
                            .text_color(text_color)
                            .child(selected),
                    );
                    if !after.is_empty() {
                        text_el = text_el.child(div().text_color(text_color).child(after));
                    }
                } else {
                    // Cursor only, no selection
                    text_el = text_el.text_color(text_color).child(display_text);
                }
            } else {
                // No selection anchor
                text_el = text_el.text_color(text_color).child(display_text);
            }
        } else if !editing && current_value.is_empty() {
            // Placeholder text
            text_el = text_el.text_color(placeholder_color).child(display_text);
        } else {
            // Normal text (not editing)
            text_el = text_el.text_color(text_color).child(display_text);
        }

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
