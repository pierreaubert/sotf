//! UI Kit - A local component library for GPUI
//!
//! Inspired by adabraka-ui patterns but compatible with zed's gpui version.
//! Provides reusable, composable UI components with consistent styling.

// Allow complex callback types - common in UI code
#![allow(clippy::type_complexity)]
// Allow to_* methods that take self by reference - matches GPUI patterns
#![allow(clippy::wrong_self_convention)]
#![recursion_limit = "8192"]

// Theme, animation, i18n, and accessibility
pub mod accessibility;
pub mod animation;
pub mod color_tokens;
pub mod design;
pub mod i18n;
pub mod mobile;
pub mod theme;

// Core components
pub mod button;
pub mod button_set;
pub mod card;
pub mod confirm_dialog;
pub mod context_menu;
pub mod dialog;
pub mod focus;
pub mod icon_button;
pub mod menu;
pub mod popover;
pub mod tabs;
pub mod toast;

pub mod size;

// Form components
pub mod checkbox;
pub mod color;
pub mod color_picker;
pub mod input;
pub mod number_input;
pub mod select;
pub mod slider;
pub mod toggle;

// Data display
pub mod avatar;
pub mod badge;
pub mod collection_diff;
pub mod empty_state;
pub mod image_view;
pub mod keyboard_shortcut_label;
pub mod progress;
pub mod qr;
pub mod spinner;
pub mod step_indicator;
pub mod table;
pub mod text;

// Feedback
pub mod alert;
pub mod search_bar;
pub mod tooltip;

// Navigation
pub mod accordion;
pub mod breadcrumbs;
pub mod wizard;

// Layout
pub mod loading_overlay;
pub mod pane_divider;
pub mod settings_form;
pub mod sidebar;
pub mod split_pane;
pub mod stack;
pub mod status_bar;

// Tier 3 components
pub mod command_palette;
pub mod drag_list;
pub mod notification;
pub mod tag;
pub mod toolbar;
pub mod tree_view;

// Workflow canvas
pub mod workflow;

// Showcase (library-embeddable version)
pub mod showcase;

// Re-export commonly used types

// Buttons
pub use button::{ButtonSize, ButtonVariant, ButtonTheme, Button};
pub use button_set::{ButtonSetTheme, ButtonSetSize, ButtonSetOption, ButtonSet};
pub use icon_button::{IconButtonTheme, IconButtonSize, IconButtonVariant, IconButton};
// Containers
pub use card::{SlotFactory, Card};
pub use confirm_dialog::{ConfirmDialogVariant, ConfirmDialogTheme, ConfirmDialog};
pub use context_menu::{ContextMenuTheme, ContextMenu};
pub use dialog::{DialogSlotFactory, DialogTheme, DialogSize, Dialog};
pub use popover::{PopoverPlacement, PopoverSlotFactory, PopoverTheme, Popover};
// Navigation
pub use accordion::{AccordionItem, AccordionTheme, AccordionMode, AccordionOrientation, Accordion};
pub use breadcrumbs::{BreadcrumbItem, BreadcrumbSeparator, Breadcrumbs};
pub use menu::{MenuBar, MenuBarItem, MenuItem, MenuTheme, menu_bar_button, Menu};
pub use tabs::{TabItem, TabsTheme, TabVariant, IconFactory, Tabs};
pub use wizard::{StepStatus, WizardTheme, WizardVariant, WizardHeader, WizardNavigation, WizardStep, Wizard};
// Focus management
pub use focus::{FocusDirection, FocusGroup, FocusGroupExt};
// Notifications
pub use alert::{AlertVariant, Alert, InlineAlert};
pub use toast::{ToastVariant, ToastPosition, Toast, ToastContainer};
// Form
pub use checkbox::{CheckboxTheme, CheckboxSize, Checkbox};
pub use color::Color;
pub use color_picker::{ColorPickerMode, ColorPickerView};
pub use input::{cleanup_input_state, cleanup_stale_input_states, InputSize, is_input_editing, input_state_count, clear_all_input_states, InputTheme, InputVariant, Input};
pub use number_input::{is_number_input_editing, cleanup_number_input_state, NumberInputSize, NumberInputTheme, NumberInput};
pub use select::{SelectOption, SelectSize, SelectTheme, Select};
pub use slider::{SliderSize, SliderTheme, Slider};
pub use toggle::{ToggleSize, ToggleStyle, ToggleTheme, Toggle};
// Data display
pub use avatar::{AvatarSize, AvatarShape, Avatar, AvatarStatus, AvatarGroup};
pub use badge::{BadgeVariant, BadgeSize, Badge, BadgeDot};
pub use empty_state::EmptyState;
pub use image_view::{ImageFit, ImageViewTheme, ImageView};
pub use keyboard_shortcut_label::{KeyboardShortcutSize, KeyboardShortcutLabel};
pub use progress::{ProgressVariant, ProgressSize, Progress, CircularProgress};
pub use qr::QrCode;
pub use spinner::{SpinnerSize, Spinner, LoadingDots};
pub use step_indicator::{StepItemStatus, StepOrientation, StepIndicatorSize, StepIndicatorTheme, StepItem, StepIndicator};
pub use table::{Column, PaginationState, SortDirection, TableTheme, SortState, SelectionMode, Table};
pub use text::{Code, Heading, Link, code_text_color, TextSize, TextWeight, Text};
// Feedback
pub use search_bar::{SearchBarTheme, SearchBarSize, SearchBar};
pub use tooltip::{TooltipPlacement, Tooltip, WithTooltip};
// Layout
pub use loading_overlay::{LoadingOverlayTheme, LoadingOverlay};
pub use pane_divider::{CollapseDirection, PaneDividerTheme, PaneDivider};
pub use settings_form::{SettingsFormTheme, SettingsRow, SettingsForm};
pub use sidebar::{SidebarSide, SidebarSlotFactory, SidebarTheme, Sidebar};
pub use split_pane::{SplitDirection, SplitPaneTheme, SplitPane};
pub use stack::{Divider, HStack, Spacer, StackSpacing, StackAlign, StackJustify, StackOverflow, StackSize, VStack};
// Status bar
pub use status_bar::{StatusBarPosition, StatusBarTheme, StatusBar};
// Animation
pub use animation::{ease, interpolate, interpolate_color, Keyframe, KeyframeAnimation, evaluate_keyframes, Spring, Easing, Animation};
// Accessibility
pub use accessibility::{AriaRole, AriaState, AriaLive, AriaProps, AccessibilityNode, AccessibilityTree, AccessibilityExt};
pub use collection_diff::{CollectionPatch, diff_by_key, is_content_only_update};
pub use mobile::{EdgeInsets, PullToRefreshState, SwipeDirection, SwipeAction, ContextPreview, DynamicTypePolicy, WaveformScrubber};
// Theme and i18n
pub use color_tokens::{BackgroundColors, BorderColors, ColorPalette, ColorToken, with_alpha, lighten, darken, saturate, desaturate, SemanticColors, TextColors};
pub use gpui_design::{DesignExt, DesignSystem, DesignSystemState};
pub use i18n::{I18nExt, I18nState, Language, Translations, TranslationKey};
pub use theme::{glow_shadow, ThemeExt, ThemeState, ThemeVariant, Theme};
// Builder/layout solver integration
pub mod layout_builder {
    pub use gpui_builder::{LayoutSnapshot, LayoutSnapshotMatrix, LayoutViewport, solve_snapshot_matrix, LayoutDebugReport, LayoutDebugWarning, LayoutDebugWarningKind, SolvedNode, solve};
}

// Workflow canvas
pub use workflow::{WorkflowCanvas, Command, AddNodeCommand, RemoveNodeCommand, MoveNodesCommand, AddConnectionCommand, RemoveConnectionCommand, ChangePortCountsCommand, CompositeCommand, HistoryManager, HitTestResult, HitTester, NodeContent, DefaultNodeContent, WorkflowNode, PortDirection, Port, BoxSelection, CanvasState, Connection, Position, SelectionState, NodeId, ConnectionId, LinkType, InteractionMode, NodeDragState, ConnectionDrag, BulkConnectDrag, ContextMenuState, ViewportState, WorkflowGraph, WorkflowNodeData, WorkflowTheme};
// Tier 3 components
pub use command_palette::{CommandPaletteTheme, CommandItem, CommandPalette};
pub use drag_list::{DragListTheme, DragItem, DragListOrientation, DragList};
pub use notification::{NotificationVariant, NotificationTheme, Notification};
pub use tag::{TagVariant, TagSize, TagTheme, Tag};
pub use toolbar::{ToolbarTheme, ToolbarItem, Toolbar};
pub use tree_view::{TreeViewTheme, TreeNode, TreeView};
// Shared size definitions
pub use size::ComponentSize;

// Derive macros for theme and builder generation
pub use gpui_ui_kit_macros::{ComponentBuilder, ComponentTheme, FormField};
