//! Component integration tests
//!
//! Tests that verify component behavior and creation.

use gpui_ui_kit::accordion::{Accordion, AccordionItem, AccordionMode, AccordionOrientation};
use gpui_ui_kit::badge::{Badge, BadgeVariant};
use gpui_ui_kit::button::{Button, ButtonSize, ButtonVariant};
use gpui_ui_kit::select::{Select, SelectOption, SelectSize};
use gpui_ui_kit::theme::Theme;

#[test]
fn test_button_creation() {
    // Test that buttons can be created with all variants
    let variants = [
        ButtonVariant::Primary,
        ButtonVariant::Secondary,
        ButtonVariant::Destructive,
        ButtonVariant::Ghost,
        ButtonVariant::Outline,
    ];

    for variant in &variants {
        let button = Button::new("test-button", "Click me").variant(*variant);
        // If this compiles, the API works correctly
        drop(button);
    }
}

#[test]
fn test_button_sizes() {
    // Test that buttons can be created with all sizes
    let sizes = [
        ButtonSize::Xs,
        ButtonSize::Sm,
        ButtonSize::Md,
        ButtonSize::Lg,
    ];

    for size in &sizes {
        let button = Button::new("test-button", "Click me").size(*size);
        drop(button);
    }
}

#[test]
fn test_button_configuration() {
    // Test that button configuration methods work
    let button = Button::new("test", "Test")
        .variant(ButtonVariant::Primary)
        .size(ButtonSize::Lg)
        .disabled(true)
        .selected(true)
        .full_width(true);

    drop(button);
}

#[test]
fn test_badge_variants() {
    // Test that badges can be created with all variants
    let variants = [
        BadgeVariant::Default,
        BadgeVariant::Primary,
        BadgeVariant::Success,
        BadgeVariant::Warning,
        BadgeVariant::Error,
    ];

    for variant in &variants {
        let badge = Badge::new("test").variant(*variant);
        drop(badge);
    }
}

#[test]
fn test_select_creation() {
    // Test that select can be created with options
    let options = vec![
        SelectOption::new("apple", "Apple"),
        SelectOption::new("banana", "Banana"),
        SelectOption::new("orange", "Orange"),
    ];

    let select = Select::new("test-select")
        .options(options)
        .selected("apple")
        .placeholder("Choose a fruit");

    drop(select);
}

#[test]
fn test_select_configuration() {
    // Test that select configuration methods work
    let select = Select::new("test")
        .size(SelectSize::Lg)
        .label("Fruit Selection")
        .placeholder("Choose")
        .disabled(true);

    drop(select);
}

#[test]
fn test_select_sizes() {
    // Test that selects can be created with all sizes
    let sizes = [SelectSize::Sm, SelectSize::Md, SelectSize::Lg];

    for size in &sizes {
        let select = Select::new("test").size(*size);
        drop(select);
    }
}

#[test]
fn test_accordion_modes() {
    // Test that accordions can be created with different modes
    let single = Accordion::new().mode(AccordionMode::Single);
    drop(single);

    let multiple = Accordion::new().mode(AccordionMode::Multiple);
    drop(multiple);
}

#[test]
fn test_accordion_orientations() {
    // Test that accordions can be created with all orientations
    let vertical = Accordion::new().orientation(AccordionOrientation::Vertical);
    drop(vertical);

    let horizontal = Accordion::new().orientation(AccordionOrientation::Horizontal);
    drop(horizontal);

    let side = Accordion::new().orientation(AccordionOrientation::Side);
    drop(side);
}

#[test]
fn test_accordion_configuration() {
    // Test that accordion configuration works
    let items = vec![
        AccordionItem::new("item-1", "Item 1").content("Content 1"),
        AccordionItem::new("item-2", "Item 2").content("Content 2").disabled(true),
        AccordionItem::new("item-3", "Item 3").content("Content 3"),
    ];

    let accordion = Accordion::new()
        .items(items)
        .mode(AccordionMode::Multiple)
        .orientation(AccordionOrientation::Vertical)
        .expanded(vec!["item-1".into(), "item-2".into()]);

    drop(accordion);
}

#[test]
fn test_theme_creation() {
    // Test that themes can be created
    let dark = Theme::dark();
    let light = Theme::light();

    // Themes should have different backgrounds
    assert_ne!(dark.background, light.background);

    // Dark theme should have darker background (lower luminance)
    let dark_lum = dark.background.r + dark.background.g + dark.background.b;
    let light_lum = light.background.r + light.background.g + light.background.b;
    assert!(dark_lum < light_lum, "Dark theme should be darker than light theme");
}

#[test]
fn test_select_option_creation() {
    // Test that select options can be created
    let option = SelectOption::new("value", "Label");
    assert_eq!(option.value, "value");
    assert_eq!(option.label, "Label");
    assert!(!option.disabled);

    let disabled_option = SelectOption::new("value", "Label").disabled(true);
    assert!(disabled_option.disabled);
}

#[test]
fn test_accordion_item_creation() {
    use gpui::SharedString;

    // Test that accordion items can be created
    let item = AccordionItem::new("id", "Title");
    let expected_id: SharedString = "id".into();
    assert_eq!(item.id(), &expected_id);

    let item_with_content = AccordionItem::new("id", "Title")
        .content("Content text");
    assert_eq!(item_with_content.id(), &expected_id);
}

#[test]
fn test_button_with_icons() {
    // Test that buttons can be created with icons
    let button = Button::new("test", "Label")
        .icon_left("←")
        .icon_right("→");

    drop(button);
}
