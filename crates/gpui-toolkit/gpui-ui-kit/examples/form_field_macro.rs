//! Example: Using the FormField derive macro
//!
//! This example demonstrates how to use the `FormField` macro to reduce
//! boilerplate when creating form components.

use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::FormField;

/// A custom text input component with reduced boilerplate
#[derive(FormField)]
pub struct CustomTextInput {
    #[field(required)]
    id: ElementId,
    
    #[field(optional, into)]
    value: Option<SharedString>,
    
    #[field(optional, into)]
    placeholder: Option<SharedString>,
    
    #[field(optional, into)]
    label: Option<SharedString>,
    
    disabled: bool,
    
    readonly: bool,
    
    #[field(default = "false")]
    required: bool,
}

/// A custom checkbox component
#[derive(FormField)]
pub struct CustomCheckbox {
    #[field(required)]
    id: ElementId,
    
    #[field(default = "false")]
    checked: bool,
    
    #[field(optional, into)]
    label: Option<SharedString>,
    
    disabled: bool,
    
    #[field(builder = false)]
    on_change: Option<Box<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
}

/// A custom number input component
#[derive(FormField)]
pub struct CustomNumberInput {
    #[field(required)]
    id: ElementId,
    
    #[field(default = "0.0")]
    value: f64,
    
    #[field(default = "0.0")]
    min: f64,
    
    #[field(default = "100.0")]
    max: f64,
    
    #[field(default = "1.0")]
    step: f64,
    
    #[field(optional, into)]
    label: Option<SharedString>,
    
    disabled: bool,
}

fn main() {
    // Example usage:
    
    // Using the generated builder methods
    let text_input = CustomTextInput::new("username")
        .value("John Doe")
        .placeholder("Enter your name")
        .label("Username")
        .disabled(false)
        .readonly(false)
        .required(true);
    
    let checkbox = CustomCheckbox::new("subscribe")
        .checked(true)
        .label("Subscribe to newsletter")
        .disabled(false);
    
    let number_input = CustomNumberInput::new("age")
        .value(25.0)
        .min(0.0)
        .max(120.0)
        .step(1.0)
        .label("Age")
        .disabled(false);
    
    println!("Form components created successfully!");
    println!("TextInput: id={:?}, value={:?}, label={:?}", 
        text_input.id, text_input.value, text_input.label);
    println!("Checkbox: id={:?}, checked={}", 
        checkbox.id, checkbox.checked);
    println!("NumberInput: id={:?}, value={}, min={}, max={}", 
        number_input.id, number_input.value, number_input.min, number_input.max);
}
