//! Input Debug Example
//!
//! A minimal example to test the text input component:
//! 1. Click to focus and start editing
//! 2. Type to enter text
//! 3. Enter to confirm, Escape to cancel
//! 4. Different variants and sizes

use gpui::*;
use gpui_ui_kit::input::{Input, InputSize, InputVariant};
use gpui_ui_kit::theme::ThemeExt;
use gpui_ui_kit::*;

pub struct InputDebug {
    // Basic input state
    input1_value: String,
    input1_editing: bool,
    input1_edit_text: String,
    input1_text_selected: bool,
    input1_cursor: usize,
    input1_focus: FocusHandle,

    // Second input for comparison
    input2_value: String,
    input2_editing: bool,
    input2_edit_text: String,
    input2_text_selected: bool,
    input2_cursor: usize,
    input2_focus: FocusHandle,

    // Filled variant input
    input3_value: String,
    input3_editing: bool,
    input3_edit_text: String,
    input3_text_selected: bool,
    input3_cursor: usize,
    input3_focus: FocusHandle,

    // Flushed variant input
    input4_value: String,
    input4_editing: bool,
    input4_edit_text: String,
    input4_text_selected: bool,
    input4_cursor: usize,
    input4_focus: FocusHandle,

    root_focus: FocusHandle,
    needs_initial_focus: bool,
    entity: Entity<Self>,
}

impl InputDebug {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            input1_value: "Hello World".to_string(),
            input1_editing: false,
            input1_edit_text: String::new(),
            input1_text_selected: false,
            input1_cursor: 0,
            input1_focus: cx.focus_handle(),

            input2_value: String::new(),
            input2_editing: false,
            input2_edit_text: String::new(),
            input2_text_selected: false,
            input2_cursor: 0,
            input2_focus: cx.focus_handle(),

            input3_value: "Filled variant".to_string(),
            input3_editing: false,
            input3_edit_text: String::new(),
            input3_text_selected: false,
            input3_cursor: 0,
            input3_focus: cx.focus_handle(),

            input4_value: "Flushed variant".to_string(),
            input4_editing: false,
            input4_edit_text: String::new(),
            input4_text_selected: false,
            input4_cursor: 0,
            input4_focus: cx.focus_handle(),

            root_focus: cx.focus_handle(),
            needs_initial_focus: true,
            entity: cx.entity().clone(),
        }
    }
}

impl InputDebug {
    /// Get which input is currently editing (1-4, or 0 if none)
    fn editing_input(&self) -> usize {
        if self.input1_editing { 1 }
        else if self.input2_editing { 2 }
        else if self.input3_editing { 3 }
        else if self.input4_editing { 4 }
        else { 0 }
    }

    /// Handle keyboard input for whichever input is currently editing
    fn handle_key_input(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let editing = self.editing_input();
        if editing == 0 {
            return;
        }

        let key = event.keystroke.key.as_str();
        let ctrl = event.keystroke.modifiers.control;

        // Emacs-style keybindings when Ctrl is held
        if ctrl {
            match key {
                "a" => self.move_to_start(editing),
                "e" => self.move_to_end(editing),
                "k" => self.kill_to_end(editing),
                "u" => self.kill_to_start(editing),
                "w" => self.kill_word_backward(editing),
                "h" => self.do_backspace(editing),
                "d" => self.do_delete(editing),
                "f" => self.move_forward(editing),
                "b" => self.move_backward(editing),
                _ => {}
            }
            cx.notify();
            return;
        }

        // Regular key handling
        match key {
            "enter" => self.confirm_edit(editing),
            "escape" => self.cancel_edit(editing),
            "backspace" => self.do_backspace(editing),
            "delete" => self.do_delete(editing),
            "left" => self.move_backward(editing),
            "right" => self.move_forward(editing),
            "home" => self.move_to_start(editing),
            "end" => self.move_to_end(editing),
            _ => {
                // Handle text input using key_char for IME support
                if let Some(char_text) = event.keystroke.key_char.as_ref() {
                    self.insert_text(editing, char_text);
                }
            }
        }
        cx.notify();
    }

    fn get_text_and_cursor(&self, editing: usize) -> (String, usize, bool) {
        match editing {
            1 => (self.input1_edit_text.clone(), self.input1_cursor, self.input1_text_selected),
            2 => (self.input2_edit_text.clone(), self.input2_cursor, self.input2_text_selected),
            3 => (self.input3_edit_text.clone(), self.input3_cursor, self.input3_text_selected),
            4 => (self.input4_edit_text.clone(), self.input4_cursor, self.input4_text_selected),
            _ => (String::new(), 0, false),
        }
    }

    fn set_text_and_cursor(&mut self, editing: usize, text: String, cursor: usize) {
        match editing {
            1 => { self.input1_edit_text = text; self.input1_cursor = cursor; self.input1_text_selected = false; }
            2 => { self.input2_edit_text = text; self.input2_cursor = cursor; self.input2_text_selected = false; }
            3 => { self.input3_edit_text = text; self.input3_cursor = cursor; self.input3_text_selected = false; }
            4 => { self.input4_edit_text = text; self.input4_cursor = cursor; self.input4_text_selected = false; }
            _ => {}
        }
    }

    fn move_to_start(&mut self, editing: usize) {
        self.set_text_and_cursor(editing, self.get_text_and_cursor(editing).0, 0);
    }

    fn move_to_end(&mut self, editing: usize) {
        let (text, _, _) = self.get_text_and_cursor(editing);
        let len = text.chars().count();
        self.set_text_and_cursor(editing, text, len);
    }

    fn move_forward(&mut self, editing: usize) {
        let (text, cursor, _) = self.get_text_and_cursor(editing);
        let len = text.chars().count();
        let new_cursor = if cursor < len { cursor + 1 } else { cursor };
        self.set_text_and_cursor(editing, text, new_cursor);
    }

    fn move_backward(&mut self, editing: usize) {
        let (text, cursor, _) = self.get_text_and_cursor(editing);
        let new_cursor = if cursor > 0 { cursor - 1 } else { cursor };
        self.set_text_and_cursor(editing, text, new_cursor);
    }

    fn kill_to_end(&mut self, editing: usize) {
        let (text, cursor, _) = self.get_text_and_cursor(editing);
        let chars: Vec<char> = text.chars().collect();
        let new_text: String = chars[..cursor].iter().collect();
        self.set_text_and_cursor(editing, new_text, cursor);
    }

    fn kill_to_start(&mut self, editing: usize) {
        let (text, cursor, _) = self.get_text_and_cursor(editing);
        let chars: Vec<char> = text.chars().collect();
        let new_text: String = chars[cursor..].iter().collect();
        self.set_text_and_cursor(editing, new_text, 0);
    }

    fn kill_word_backward(&mut self, editing: usize) {
        let (text, cursor, _) = self.get_text_and_cursor(editing);
        if cursor == 0 {
            return;
        }
        let chars: Vec<char> = text.chars().collect();
        let mut new_pos = cursor;
        // Skip trailing spaces
        while new_pos > 0 && chars[new_pos - 1].is_whitespace() {
            new_pos -= 1;
        }
        // Skip word characters
        while new_pos > 0 && !chars[new_pos - 1].is_whitespace() {
            new_pos -= 1;
        }
        let mut new_chars = chars[..new_pos].to_vec();
        new_chars.extend_from_slice(&chars[cursor..]);
        let new_text: String = new_chars.into_iter().collect();
        self.set_text_and_cursor(editing, new_text, new_pos);
    }

    fn do_backspace(&mut self, editing: usize) {
        let (text, cursor, selected) = self.get_text_and_cursor(editing);
        if selected {
            // Delete selection (entire text when selected)
            self.set_text_and_cursor(editing, String::new(), 0);
        } else if cursor > 0 {
            let mut chars: Vec<char> = text.chars().collect();
            chars.remove(cursor - 1);
            let new_text: String = chars.into_iter().collect();
            self.set_text_and_cursor(editing, new_text, cursor - 1);
        }
    }

    fn do_delete(&mut self, editing: usize) {
        let (text, cursor, selected) = self.get_text_and_cursor(editing);
        if selected {
            self.set_text_and_cursor(editing, String::new(), 0);
        } else {
            let len = text.chars().count();
            if cursor < len {
                let mut chars: Vec<char> = text.chars().collect();
                chars.remove(cursor);
                let new_text: String = chars.into_iter().collect();
                self.set_text_and_cursor(editing, new_text, cursor);
            }
        }
    }

    fn insert_text(&mut self, editing: usize, char_text: &str) {
        let (text, cursor, selected) = self.get_text_and_cursor(editing);
        let (text, cursor) = if selected {
            (String::new(), 0)
        } else {
            (text, cursor)
        };
        let mut chars: Vec<char> = text.chars().collect();
        for (i, c) in char_text.chars().enumerate() {
            chars.insert(cursor + i, c);
        }
        let new_text: String = chars.into_iter().collect();
        let new_cursor = cursor + char_text.chars().count();
        self.set_text_and_cursor(editing, new_text, new_cursor);
    }

    fn confirm_edit(&mut self, editing: usize) {
        match editing {
            1 => {
                self.input1_value = self.input1_edit_text.clone();
                self.input1_editing = false;
                self.input1_text_selected = false;
            }
            2 => {
                self.input2_value = self.input2_edit_text.clone();
                self.input2_editing = false;
                self.input2_text_selected = false;
            }
            3 => {
                self.input3_value = self.input3_edit_text.clone();
                self.input3_editing = false;
                self.input3_text_selected = false;
            }
            4 => {
                self.input4_value = self.input4_edit_text.clone();
                self.input4_editing = false;
                self.input4_text_selected = false;
            }
            _ => {}
        }
    }

    fn cancel_edit(&mut self, editing: usize) {
        match editing {
            1 => {
                self.input1_editing = false;
                self.input1_text_selected = false;
            }
            2 => {
                self.input2_editing = false;
                self.input2_text_selected = false;
            }
            3 => {
                self.input3_editing = false;
                self.input3_text_selected = false;
            }
            4 => {
                self.input4_editing = false;
                self.input4_text_selected = false;
            }
            _ => {}
        }
    }
}

impl Render for InputDebug {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Focus on first render to enable keyboard handling
        if self.needs_initial_focus {
            self.needs_initial_focus = false;
            self.root_focus.focus(window, cx);
        }

        let entity = self.entity.clone();
        let theme = cx.theme();

        let root_focus = self.root_focus.clone();

        // Always have keyboard handler - check inside if we're editing
        div()
            .id("input-debug-root")
            .track_focus(&root_focus)
            .focusable()
            .key_context("InputDebug")
            .on_mouse_down(MouseButton::Left, {
                let root_focus = root_focus.clone();
                move |_event, window, cx| {
                    // Always capture focus on click
                    window.focus(&root_focus, cx);
                }
            })
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_key_input(event, window, cx);
            }))
            .w_full()
            .h_full()
            .bg(theme.background)
            .text_color(theme.text_primary)
            .p_8()
            .flex()
            .flex_col()
            .gap_6()
            // Header
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(Heading::h1("Text Input Debug"))
                    .child(Text::new(
                        "Testing: Click to edit, Enter to confirm, Escape to cancel",
                    )),
            )
            .child(Divider::new().build())
            // Instructions
            .child(
                div()
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.border)
                    .rounded_md()
                    .p_4()
                    .child(
                        VStack::new()
                            .spacing(StackSpacing::Sm)
                            .child(Text::new("How to use:").weight(TextWeight::Bold))
                            .child(Text::new("1. Click on an input field to start editing"))
                            .child(Text::new("2. Type to enter text (backspace to delete)"))
                            .child(Text::new("3. Press Enter to confirm your changes"))
                            .child(Text::new("4. Press Escape to cancel and restore original value")),
                    ),
            )
            // Basic input with value
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(Text::new("Default Input (with initial value):").weight(TextWeight::Medium))
                    .child({
                        let value = self.input1_value.clone();
                        let editing = self.input1_editing;
                        let edit_text = self.input1_edit_text.clone();
                        let text_selected = self.input1_text_selected;
                        let focus = self.input1_focus.clone();

                        div().w(px(300.0)).child(
                            Input::new("input-1")
                                .value(value.clone())
                                .placeholder("Enter some text...")
                                .label("Username")
                                .size(InputSize::Md)
                                .editing(editing)
                                .edit_text(edit_text)
                                .text_selected(text_selected)
                                .focus_handle(focus)
                                .on_edit_start({
                                    let entity = entity.clone();
                                    let value = value.clone();
                                    let root_focus = root_focus.clone();
                                    move |window, cx| {
                                        // Focus root to capture keyboard events
                                        window.focus(&root_focus, cx);
                                        entity.update(cx, |this, _cx| {
                                            this.input1_editing = true;
                                            this.input1_edit_text = value.clone();
                                            this.input1_cursor = value.chars().count();
                                            this.input1_text_selected = true;
                                        });
                                    }
                                }),
                        )
                    })
                    .child(
                        Text::new(format!(
                            "Value: \"{}\" | Editing: {} | Edit text: \"{}\"",
                            self.input1_value,
                            self.input1_editing,
                            self.input1_edit_text
                        ))
                        .size(TextSize::Sm)
                        .muted(true),
                    ),
            )
            // Empty input with placeholder
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(Text::new("Empty Input (with placeholder):").weight(TextWeight::Medium))
                    .child({
                        let value = self.input2_value.clone();
                        let editing = self.input2_editing;
                        let edit_text = self.input2_edit_text.clone();
                        let text_selected = self.input2_text_selected;
                        let focus = self.input2_focus.clone();

                        div().w(px(300.0)).child(
                            Input::new("input-2")
                                .value(value.clone())
                                .placeholder("Type something here...")
                                .label("Email")
                                .size(InputSize::Md)
                                .editing(editing)
                                .edit_text(edit_text)
                                .text_selected(text_selected)
                                .focus_handle(focus)
                                .on_edit_start({
                                    let entity = entity.clone();
                                    let value = value.clone();
                                    let root_focus = root_focus.clone();
                                    move |window, cx| {
                                        window.focus(&root_focus, cx);
                                        entity.update(cx, |this, _cx| {
                                            this.input2_editing = true;
                                            this.input2_edit_text = value.clone();
                                            this.input2_cursor = value.chars().count();
                                            this.input2_text_selected = true;
                                        });
                                    }
                                }),
                        )
                    })
                    .child(
                        Text::new(format!(
                            "Value: {} | Editing: {} | Edit text: \"{}\"",
                            if self.input2_value.is_empty() {
                                "(empty)".to_string()
                            } else {
                                format!("\"{}\"", self.input2_value)
                            },
                            self.input2_editing,
                            self.input2_edit_text
                        ))
                        .size(TextSize::Sm)
                        .muted(true),
                    ),
            )
            .child(Divider::new().build())
            // Variants section
            .child(Heading::h2("Input Variants"))
            // Filled variant
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(Text::new("Filled Variant:").weight(TextWeight::Medium))
                    .child({
                        let value = self.input3_value.clone();
                        let editing = self.input3_editing;
                        let edit_text = self.input3_edit_text.clone();
                        let text_selected = self.input3_text_selected;
                        let focus = self.input3_focus.clone();

                        div().w(px(300.0)).child(
                            Input::new("input-3")
                                .value(value.clone())
                                .placeholder("Filled input...")
                                .variant(InputVariant::Filled)
                                .editing(editing)
                                .edit_text(edit_text)
                                .text_selected(text_selected)
                                .focus_handle(focus)
                                .on_edit_start({
                                    let entity = entity.clone();
                                    let value = value.clone();
                                    let root_focus = root_focus.clone();
                                    move |window, cx| {
                                        window.focus(&root_focus, cx);
                                        entity.update(cx, |this, _cx| {
                                            this.input3_editing = true;
                                            this.input3_edit_text = value.clone();
                                            this.input3_cursor = value.chars().count();
                                            this.input3_text_selected = true;
                                        });
                                    }
                                }),
                        )
                    }),
            )
            // Flushed variant
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(Text::new("Flushed Variant (bottom border only):").weight(TextWeight::Medium))
                    .child({
                        let value = self.input4_value.clone();
                        let editing = self.input4_editing;
                        let edit_text = self.input4_edit_text.clone();
                        let text_selected = self.input4_text_selected;
                        let focus = self.input4_focus.clone();

                        div().w(px(300.0)).child(
                            Input::new("input-4")
                                .value(value.clone())
                                .placeholder("Flushed input...")
                                .variant(InputVariant::Flushed)
                                .editing(editing)
                                .edit_text(edit_text)
                                .text_selected(text_selected)
                                .focus_handle(focus)
                                .on_edit_start({
                                    let entity = entity.clone();
                                    let value = value.clone();
                                    let root_focus = root_focus.clone();
                                    move |window, cx| {
                                        window.focus(&root_focus, cx);
                                        entity.update(cx, |this, _cx| {
                                            this.input4_editing = true;
                                            this.input4_edit_text = value.clone();
                                            this.input4_cursor = value.chars().count();
                                            this.input4_text_selected = true;
                                        });
                                    }
                                }),
                        )
                    }),
            )
            .child(Divider::new().build())
            // Sizes section
            .child(Heading::h2("Input Sizes"))
            .child(
                div()
                    .flex()
                    .gap_4()
                    .items_end()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(Text::new("Small").size(TextSize::Sm))
                            .child(
                                div().w(px(150.0)).child(
                                    Input::new("size-sm")
                                        .value("Small size")
                                        .size(InputSize::Sm)
                                        .readonly(true),
                                ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(Text::new("Medium").size(TextSize::Sm))
                            .child(
                                div().w(px(150.0)).child(
                                    Input::new("size-md")
                                        .value("Medium size")
                                        .size(InputSize::Md)
                                        .readonly(true),
                                ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(Text::new("Large").size(TextSize::Sm))
                            .child(
                                div().w(px(150.0)).child(
                                    Input::new("size-lg")
                                        .value("Large size")
                                        .size(InputSize::Lg)
                                        .readonly(true),
                                ),
                            ),
                    ),
            )
            .child(Divider::new().build())
            // States section
            .child(Heading::h2("Input States"))
            .child(
                div()
                    .flex()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(Text::new("Disabled").size(TextSize::Sm))
                            .child(
                                div().w(px(200.0)).child(
                                    Input::new("state-disabled")
                                        .value("Cannot edit")
                                        .disabled(true),
                                ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(Text::new("Readonly").size(TextSize::Sm))
                            .child(
                                div().w(px(200.0)).child(
                                    Input::new("state-readonly")
                                        .value("Read only text")
                                        .readonly(true),
                                ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(Text::new("With Error").size(TextSize::Sm))
                            .child(
                                div().w(px(200.0)).child(
                                    Input::new("state-error")
                                        .value("Invalid value")
                                        .error("This field has an error")
                                        .readonly(true),
                                ),
                            ),
                    ),
            )
    }
}

fn main() {
    MiniApp::run(
        MiniAppConfig::new("Input Debug")
            .size(800.0, 900.0)
            .scrollable(true)
            .with_theme(true),
        |cx| cx.new(InputDebug::new),
    );
}
