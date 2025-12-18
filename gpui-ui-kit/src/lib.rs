//! UI Kit - A local component library for GPUI
//!
//! Inspired by adabraka-ui patterns but compatible with zed's gpui version.
//! Provides reusable, composable UI components with consistent styling.

// Allow complex callback types - common in UI code
#![allow(clippy::type_complexity)]
// Allow to_* methods that take self by reference - matches GPUI patterns
#![allow(clippy::wrong_self_convention)]

// Application templates
#[path = "../src-app/mod.rs"]
pub mod app;

// Theme and i18n
pub mod i18n;
pub mod theme;

// Core components
pub mod button;
pub mod button_set;
pub mod card;
pub mod dialog;
pub mod icon_button;
pub mod menu;
pub mod tabs;
pub mod toast;

// Shared utilities
pub mod scale;

// Form components
pub mod autoeq_form;
pub mod checkbox;
pub mod color;
pub mod color_picker;
pub mod input;
pub mod number_input;
pub mod potentiometer;
pub mod select;
pub mod slider;
pub mod toggle;
pub mod vertical_slider;
pub mod volume_knob;

// Data display
pub mod avatar;
pub mod badge;
pub mod progress;
pub mod spinner;
pub mod text;

// Feedback
pub mod alert;
pub mod tooltip;

// Navigation
pub mod accordion;
pub mod breadcrumbs;
pub mod wizard;

// Layout
pub mod pane_divider;
pub mod stack;

// Re-export commonly used types

// Buttons
pub use button::{Button, ButtonSize, ButtonTheme, ButtonVariant};
pub use button_set::{ButtonSet, ButtonSetOption, ButtonSetSize, ButtonSetTheme};
pub use icon_button::{IconButton, IconButtonSize, IconButtonTheme, IconButtonVariant};

// Containers
pub use card::Card;
pub use dialog::{Dialog, DialogSize};

// Navigation
pub use accordion::{Accordion, AccordionItem, AccordionMode, AccordionTheme};
pub use breadcrumbs::{BreadcrumbItem, BreadcrumbSeparator, Breadcrumbs};
pub use menu::{Menu, MenuBar, MenuBarItem, MenuItem, MenuTheme, menu_bar_button};
pub use tabs::{TabItem, TabVariant, Tabs, TabsTheme};
pub use wizard::{
    StepStatus, Wizard, WizardHeader, WizardNavigation, WizardStep, WizardTheme, WizardVariant,
};

// Notifications
pub use alert::{Alert, AlertVariant, InlineAlert};
pub use toast::{Toast, ToastContainer, ToastPosition, ToastVariant};

// Form
pub use autoeq_form::{
    ALGORITHM_OPTIONS, AutoEqConfig, AutoEqForm, AutoEqFormTheme, AutoEqFormUiState,
    DE_STRATEGY_OPTIONS, LOCAL_ALGO_OPTIONS, PEQ_MODEL_OPTIONS, ParamLimits,
};
pub use checkbox::{Checkbox, CheckboxSize};
pub use color::Color;
pub use color_picker::{ColorPickerMode, ColorPickerView};
pub use input::{Input, InputSize, InputVariant};
pub use number_input::{NumberInput, NumberInputSize, NumberInputTheme};
pub use potentiometer::{Potentiometer, PotentiometerScale, PotentiometerSize, PotentiometerTheme};
pub use select::{Select, SelectOption, SelectSize, SelectTheme};
pub use slider::{Slider, SliderSize, SliderTheme};
pub use toggle::{Toggle, ToggleSize, ToggleStyle, ToggleTheme};
pub use vertical_slider::{
    VerticalSlider, VerticalSliderScale, VerticalSliderSize, VerticalSliderTheme,
};
pub use volume_knob::{VolumeKnob, VolumeKnobTheme};

// Data display
pub use avatar::{Avatar, AvatarGroup, AvatarShape, AvatarSize, AvatarStatus};
pub use badge::{Badge, BadgeDot, BadgeSize, BadgeVariant};
pub use progress::{CircularProgress, Progress, ProgressSize, ProgressVariant};
pub use spinner::{LoadingDots, Spinner, SpinnerSize};
pub use text::{Code, Heading, Link, Text, TextSize, TextWeight};

// Feedback
pub use tooltip::{Tooltip, TooltipPlacement, WithTooltip};

// Layout
pub use pane_divider::{CollapseDirection, PaneDivider, PaneDividerTheme};
pub use stack::{
    Divider, HStack, Spacer, StackAlign, StackJustify, StackOverflow, StackSize, StackSpacing,
    VStack,
};

// Application templates
pub use app::{MiniApp, MiniAppConfig};

// Theme and i18n
pub use i18n::{I18nExt, I18nState, Language, TranslationKey, Translations};
pub use theme::{Theme, ThemeExt, ThemeState, ThemeVariant};
