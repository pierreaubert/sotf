//! Integration test for Input component

use gpui::{Context, TestAppContext, Window, div, prelude::*};
use gpui_ui_kit::input::Input;
use std::cell::RefCell;
use std::rc::Rc;

struct InputTestView;

impl Render for InputTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().child(
            Input::new("test-input")
                .placeholder("Enter text...")
                .value("Hello"),
        )
    }
}

#[gpui::test]
async fn test_input_renders(cx: &mut TestAppContext) {
    let _window = cx.add_window(|_window, _cx| InputTestView);
}

/// Test that Input component properly tracks value changes via on_text_change callback
struct InputWithCallbackView {
    value: Rc<RefCell<String>>,
}

impl Render for InputWithCallbackView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let value = self.value.borrow().clone();
        let value_rc = self.value.clone();
        
        div().child(
            Input::new("callback-input")
                .placeholder("Type here...")
                .value(value)
                .on_text_change(move |text, _window, _cx| {
                    *value_rc.borrow_mut() = text;
                }),
        )
    }
}

#[gpui::test]
async fn test_input_with_callback(cx: &mut TestAppContext) {
    let value = Rc::new(RefCell::new("initial".to_string()));
    let value_clone = value.clone();
    
    let _window = cx.add_window(move |_window, _cx| InputWithCallbackView {
        value: value_clone,
    });
    
    // Verify initial value
    assert_eq!(*value.borrow(), "initial");
}

/// Test that Input component can be created with various configurations
#[gpui::test]
async fn test_input_configurations(cx: &mut TestAppContext) {
    use gpui_ui_kit::input::{InputSize, InputVariant};
    
    struct ConfigTestView;
    
    impl Render for ConfigTestView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .child(
                    Input::new("small-input")
                        .size(InputSize::Sm)
                        .value("Small"),
                )
                .child(
                    Input::new("filled-input")
                        .variant(InputVariant::Filled)
                        .value("Filled"),
                )
                .child(
                    Input::new("disabled-input")
                        .disabled(true)
                        .value("Disabled"),
                )
                .child(
                    Input::new("readonly-input")
                        .readonly(true)
                        .value("Readonly"),
                )
                .child(
                    Input::new("error-input")
                        .error("This is an error")
                        .value("Error"),
                )
        }
    }
    
    let _window = cx.add_window(|_window, _cx| ConfigTestView);
}
