//! UI Kit - A local component library for GPUI
//!
//! Inspired by adabraka-ui patterns but compatible with zed's gpui version.
//! Provides reusable, composable UI components with consistent styling.

// Core components
pub mod button;
pub mod icon_button;
pub mod card;
pub mod dialog;
pub mod menu;
pub mod tabs;
pub mod toast;

// Form components
pub mod input;
pub mod checkbox;
pub mod toggle;
pub mod select;

// Data display
pub mod badge;
pub mod progress;
pub mod spinner;
pub mod avatar;
pub mod text;

// Feedback
pub mod alert;
pub mod tooltip;

// Navigation
pub mod breadcrumbs;
pub mod accordion;

// Layout
pub mod stack;

// Re-export commonly used types

// Buttons
pub use button::{Button, ButtonVariant, ButtonSize, ButtonTheme};
pub use icon_button::{IconButton, IconButtonSize, IconButtonVariant};

// Containers
pub use card::Card;
pub use dialog::{Dialog, DialogSize};

// Navigation
pub use menu::{menu_bar_button, Menu, MenuBar, MenuBarItem, MenuItem};
pub use tabs::{Tabs, TabItem, TabVariant};
pub use breadcrumbs::{Breadcrumbs, BreadcrumbItem, BreadcrumbSeparator};
pub use accordion::{Accordion, AccordionItem, AccordionMode, AccordionTheme};

// Notifications
pub use toast::{Toast, ToastContainer, ToastVariant, ToastPosition};
pub use alert::{Alert, AlertVariant, InlineAlert};

// Form
pub use input::{Input, InputSize, InputVariant};
pub use checkbox::{Checkbox, CheckboxSize};
pub use toggle::{Toggle, ToggleSize};
pub use select::{Select, SelectOption, SelectSize};

// Data display
pub use badge::{Badge, BadgeDot, BadgeVariant, BadgeSize};
pub use progress::{Progress, CircularProgress, ProgressVariant, ProgressSize};
pub use spinner::{Spinner, LoadingDots, SpinnerSize};
pub use avatar::{Avatar, AvatarGroup, AvatarSize, AvatarShape, AvatarStatus};
pub use text::{Text, TextSize, TextWeight, Heading, Code, Link};

// Feedback
pub use tooltip::{Tooltip, TooltipPlacement, WithTooltip};

// Layout
pub use stack::{VStack, HStack, Spacer, Divider, StackSpacing, StackAlign, StackJustify};
