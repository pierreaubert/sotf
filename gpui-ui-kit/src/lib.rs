//! UI Kit - A local component library for GPUI
//!
//! Inspired by adabraka-ui patterns but compatible with zed's gpui version.
//! Provides reusable, composable UI components with consistent styling.

// Core components
pub mod button;
pub mod card;
pub mod dialog;
pub mod icon_button;
pub mod menu;
pub mod tabs;
pub mod toast;

// Form components
pub mod checkbox;
pub mod input;
pub mod select;
pub mod slider;
pub mod toggle;

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

// Layout
pub mod stack;

// Re-export commonly used types

// Buttons
pub use button::{Button, ButtonSize, ButtonTheme, ButtonVariant};
pub use icon_button::{IconButton, IconButtonSize, IconButtonTheme, IconButtonVariant};

// Containers
pub use card::Card;
pub use dialog::{Dialog, DialogSize};

// Navigation
pub use accordion::{Accordion, AccordionItem, AccordionMode, AccordionTheme};
pub use breadcrumbs::{BreadcrumbItem, BreadcrumbSeparator, Breadcrumbs};
pub use menu::{Menu, MenuBar, MenuBarItem, MenuItem, menu_bar_button};
pub use tabs::{TabItem, TabVariant, Tabs};

// Notifications
pub use alert::{Alert, AlertVariant, InlineAlert};
pub use toast::{Toast, ToastContainer, ToastPosition, ToastVariant};

// Form
pub use checkbox::{Checkbox, CheckboxSize};
pub use input::{Input, InputSize, InputVariant};
pub use select::{Select, SelectOption, SelectSize};
pub use slider::{Slider, SliderSize, SliderTheme};
pub use toggle::{Toggle, ToggleSize};

// Data display
pub use avatar::{Avatar, AvatarGroup, AvatarShape, AvatarSize, AvatarStatus};
pub use badge::{Badge, BadgeDot, BadgeSize, BadgeVariant};
pub use progress::{CircularProgress, Progress, ProgressSize, ProgressVariant};
pub use spinner::{LoadingDots, Spinner, SpinnerSize};
pub use text::{Code, Heading, Link, Text, TextSize, TextWeight};

// Feedback
pub use tooltip::{Tooltip, TooltipPlacement, WithTooltip};

// Layout
pub use stack::{Divider, HStack, Spacer, StackAlign, StackJustify, StackSpacing, VStack};
