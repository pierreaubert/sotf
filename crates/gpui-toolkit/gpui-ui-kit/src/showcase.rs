//! UI Kit Showcase (library module)
//!
//! A comprehensive demonstration of all gpui-ui-kit components with theme and i18n support.
//! This module exposes the Showcase component for embedding in other applications.

use gpui::*;

use crate::accordion::AccordionOrientation;
use crate::i18n::{I18nExt, TranslationKey};
use crate::menu::{Menu, MenuItem};
use crate::theme::ThemeExt;
use crate::wizard::StepStatus;
use crate::workflow::{Position, WorkflowCanvas, WorkflowGraph, WorkflowNodeData};
use crate::*;
use std::collections::HashSet;

#[derive(Clone, Debug)]
pub struct User {
    pub id: usize,
    pub name: String,
    pub email: String,
    pub role: String,
}

/// Component groups for organizing related sections
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShowcaseGroup {
    Actions,
    TextAndLabels,
    FormControls,
    Navigation,
    Feedback,
    DataDisplay,
    Overlays,
    LayoutAndStructure,
    Controls,
    MultiStep,
    Media,
}

impl ShowcaseGroup {
    pub fn all() -> &'static [ShowcaseGroup] {
        &[
            ShowcaseGroup::Actions,
            ShowcaseGroup::TextAndLabels,
            ShowcaseGroup::FormControls,
            ShowcaseGroup::Navigation,
            ShowcaseGroup::Feedback,
            ShowcaseGroup::DataDisplay,
            ShowcaseGroup::Overlays,
            ShowcaseGroup::LayoutAndStructure,
            ShowcaseGroup::Controls,
            ShowcaseGroup::MultiStep,
            ShowcaseGroup::Media,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            ShowcaseGroup::Actions => "Actions",
            ShowcaseGroup::TextAndLabels => "Text & Labels",
            ShowcaseGroup::FormControls => "Form Controls",
            ShowcaseGroup::Navigation => "Navigation",
            ShowcaseGroup::Feedback => "Feedback",
            ShowcaseGroup::DataDisplay => "Data Display",
            ShowcaseGroup::Overlays => "Overlays & Popups",
            ShowcaseGroup::LayoutAndStructure => "Layout & Structure",
            ShowcaseGroup::Controls => "Controls",
            ShowcaseGroup::MultiStep => "Multi-Step",
            ShowcaseGroup::Media => "Media & Visuals",
        }
    }

    pub fn sections(&self) -> &'static [ShowcaseSection] {
        match self {
            ShowcaseGroup::Actions => &[ShowcaseSection::Buttons, ShowcaseSection::IconButtons],
            ShowcaseGroup::TextAndLabels => &[
                ShowcaseSection::Text,
                ShowcaseSection::Badges,
                ShowcaseSection::Tag,
                ShowcaseSection::KeyboardShortcut,
            ],
            ShowcaseGroup::FormControls => &[
                ShowcaseSection::FormControls,
                ShowcaseSection::SettingsForm,
                ShowcaseSection::Accessibility,
            ],
            ShowcaseGroup::Navigation => &[
                ShowcaseSection::Menu,
                ShowcaseSection::ContextMenu,
                ShowcaseSection::CommandPalette,
                ShowcaseSection::Tabs,
                ShowcaseSection::Breadcrumbs,
                ShowcaseSection::SearchBar,
            ],
            ShowcaseGroup::Feedback => &[
                ShowcaseSection::Alerts,
                ShowcaseSection::Toasts,
                ShowcaseSection::Notification,
                ShowcaseSection::Progress,
                ShowcaseSection::Spinners,
                ShowcaseSection::LoadingOverlay,
                ShowcaseSection::EmptyState,
            ],
            ShowcaseGroup::DataDisplay => &[
                ShowcaseSection::Table,
                ShowcaseSection::Cards,
                ShowcaseSection::Avatars,
                ShowcaseSection::TreeView,
                ShowcaseSection::DragList,
            ],
            ShowcaseGroup::Overlays => &[
                ShowcaseSection::Dialog,
                ShowcaseSection::ConfirmDialog,
                ShowcaseSection::Popover,
                ShowcaseSection::Tooltips,
            ],
            ShowcaseGroup::LayoutAndStructure => &[
                ShowcaseSection::Layout,
                ShowcaseSection::SplitPane,
                ShowcaseSection::Sidebar,
                ShowcaseSection::StatusBar,
                ShowcaseSection::Toolbar,
            ],
            ShowcaseGroup::Controls => &[ShowcaseSection::Accordion],
            ShowcaseGroup::MultiStep => &[
                ShowcaseSection::Wizard,
                ShowcaseSection::StepIndicator,
                ShowcaseSection::Workflow,
            ],
            ShowcaseGroup::Media => &[ShowcaseSection::QrCode, ShowcaseSection::ImageView],
        }
    }

    /// Description explaining the differences between components in this group
    pub fn description(&self) -> &'static str {
        match self {
            ShowcaseGroup::Actions => {
                "\
Buttons trigger actions on click. \
Icon Buttons are compact, icon-only variants for toolbars and tight spaces where a text label would be too wide."
            }

            ShowcaseGroup::TextAndLabels => {
                "\
Text renders styled inline or block text. \
Badges are small status indicators (counts, labels) typically attached to other elements. \
Tags are removable, categorical labels for filtering and tagging content. \
Keyboard Shortcuts display key combinations like Cmd+S."
            }

            ShowcaseGroup::FormControls => {
                "\
Form Controls are individual input primitives (toggles, checkboxes, sliders, selects, number inputs). \
Settings Form composes multiple form controls into a labeled, grouped settings page with sections and descriptions."
            }

            ShowcaseGroup::Navigation => {
                "\
Menu is a bare dropdown panel of items you position yourself (used inside MenuBar or custom dropdowns). \
Context Menu wraps Menu with absolute positioning at the cursor and a click-outside-to-dismiss backdrop (right-click menus). \
Command Palette is a searchable, filterable command list triggered by a keyboard shortcut. \
Tabs switch between views in a fixed set. \
Breadcrumbs show hierarchical location for drill-down navigation. \
Search Bar provides a text input with search-specific affordances (icon, clear button)."
            }

            ShowcaseGroup::Feedback => {
                "\
Alerts are persistent, inline banners for important messages (info, warning, error). \
Toasts are temporary, auto-dismissing notifications that stack at a screen edge. \
Notifications are richer, persistent messages with actions (mark read, dismiss). \
Progress shows determinate completion (0-100%). \
Spinners show indeterminate loading with no known duration. \
Loading Overlay covers a region with a spinner and optional message, blocking interaction. \
Empty State is a placeholder shown when a list or view has no data."
            }

            ShowcaseGroup::DataDisplay => {
                "\
Table displays structured rows and columns with sorting and selection. \
Cards are flexible content containers for preview or summary information. \
Avatars represent users or entities with images or initials. \
Tree View shows hierarchical, expandable data (file trees, nested lists). \
Drag List is an orderable list where items can be reordered by dragging."
            }

            ShowcaseGroup::Overlays => {
                "\
Dialog is a general-purpose modal overlay for complex content (forms, detail views). \
Confirm Dialog is a focused, pre-built dialog for yes/no confirmation prompts with variant styling (info, warning, danger). \
Popover is a non-modal floating panel anchored to a trigger element (menus, pickers). \
Tooltips are small, hover-triggered text hints that describe an element."
            }

            ShowcaseGroup::LayoutAndStructure => {
                "\
Layout provides flex/grid containers and spacing primitives (VStack, HStack, Divider). \
Split Pane divides a region into resizable panels with a draggable divider. \
Sidebar is a fixed or collapsible side panel for navigation or tools. \
Status Bar is a narrow bar at the window bottom for status information. \
Toolbar is a horizontal bar of actions and controls, typically at the top of a view."
            }

            ShowcaseGroup::Controls => {
                "\
Accordion expands and collapses content sections, either one-at-a-time or multiple simultaneously."
            }

            ShowcaseGroup::MultiStep => {
                "\
Wizard guides users through a multi-step process with prev/next navigation and step validation. \
Step Indicator is a read-only progress display showing which step the user is on, without navigation logic. \
Workflow is a node-and-edge graph editor for visual pipeline or flowchart building."
            }

            ShowcaseGroup::Media => {
                "\
QR Code generates and renders a QR code from data. \
Image View displays images with optional zoom, pan, and loading states."
            }
        }
    }
}

/// Section identifiers for navigation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShowcaseSection {
    #[default]
    Buttons,
    Text,
    Badges,
    Avatars,
    FormControls,
    Progress,
    Alerts,
    Tabs,
    Cards,
    Breadcrumbs,
    Spinners,
    Layout,
    IconButtons,
    Toasts,
    Dialog,
    Menu,
    Table,
    Tooltips,
    Accordion,
    Wizard,
    Workflow,
    QrCode,
    ContextMenu,
    Popover,
    Sidebar,
    StatusBar,
    SearchBar,
    KeyboardShortcut,
    EmptyState,
    ConfirmDialog,
    SplitPane,
    ImageView,
    SettingsForm,
    StepIndicator,
    LoadingOverlay,
    Tag,
    Toolbar,
    Notification,
    TreeView,
    DragList,
    CommandPalette,
    Accessibility,
}

impl ShowcaseSection {
    pub fn label(&self) -> &'static str {
        match self {
            ShowcaseSection::Buttons => "Buttons",
            ShowcaseSection::Text => "Text",
            ShowcaseSection::Badges => "Badges",
            ShowcaseSection::Avatars => "Avatars",
            ShowcaseSection::FormControls => "Form Controls",
            ShowcaseSection::Progress => "Progress",
            ShowcaseSection::Alerts => "Alerts",
            ShowcaseSection::Tabs => "Tabs",
            ShowcaseSection::Cards => "Cards",
            ShowcaseSection::Breadcrumbs => "Breadcrumbs",
            ShowcaseSection::Spinners => "Spinners",
            ShowcaseSection::Layout => "Layout",
            ShowcaseSection::IconButtons => "Icon Buttons",
            ShowcaseSection::Toasts => "Toasts",
            ShowcaseSection::Dialog => "Dialog",
            ShowcaseSection::Menu => "Menu",
            ShowcaseSection::Table => "Table",
            ShowcaseSection::Tooltips => "Tooltips",
            ShowcaseSection::Accordion => "Accordion",
            ShowcaseSection::Wizard => "Wizard",
            ShowcaseSection::Workflow => "Workflow",
            ShowcaseSection::QrCode => "QR Code",
            ShowcaseSection::ContextMenu => "Context Menu",
            ShowcaseSection::Popover => "Popover",
            ShowcaseSection::Sidebar => "Sidebar",
            ShowcaseSection::StatusBar => "Status Bar",
            ShowcaseSection::SearchBar => "Search Bar",
            ShowcaseSection::KeyboardShortcut => "Keyboard Shortcuts",
            ShowcaseSection::EmptyState => "Empty State",
            ShowcaseSection::ConfirmDialog => "Confirm Dialog",
            ShowcaseSection::SplitPane => "Split Pane",
            ShowcaseSection::ImageView => "Image View",
            ShowcaseSection::SettingsForm => "Settings Form",
            ShowcaseSection::StepIndicator => "Step Indicator",
            ShowcaseSection::LoadingOverlay => "Loading Overlay",
            ShowcaseSection::Tag => "Tag",
            ShowcaseSection::Toolbar => "Toolbar",
            ShowcaseSection::Notification => "Notification",
            ShowcaseSection::TreeView => "Tree View",
            ShowcaseSection::DragList => "Drag List",
            ShowcaseSection::CommandPalette => "Command Palette",
            ShowcaseSection::Accessibility => "Accessibility",
        }
    }

    pub fn group(&self) -> ShowcaseGroup {
        for group in ShowcaseGroup::all() {
            if group.sections().contains(self) {
                return *group;
            }
        }
        ShowcaseGroup::Actions
    }
}

pub struct Showcase {
    // Toggle states
    pub toggle_on: bool,
    pub toggle_lg: bool,
    pub checkbox_checked: bool,
    // Slider value
    pub slider_value: f32,
    // Vertical slider value
    // Number input values
    pub number_value: f64,
    pub number_freq: f64,
    pub number_db: f64,
    // Number input editing state
    pub editing_number: Option<&'static str>,
    pub edit_text: String,
    pub text_selected: bool,
    // Text Input component states
    pub input_value: String,
    pub input_editing: bool,
    pub input_edit_text: String,
    pub input_selected: bool,
    // Select states
    pub select_value: Option<SharedString>,
    pub select_open: bool,
    pub select_highlighted: Option<usize>,
    // ButtonSet states
    pub buttonset_view_mode: SharedString,
    pub buttonset_alignment: SharedString,
    // Tabs state
    pub selected_tab: usize,
    // Accordion states
    pub accordion_vertical_single: Vec<SharedString>,
    pub accordion_vertical_multiple: Vec<SharedString>,
    pub accordion_horizontal_single: Vec<SharedString>,
    pub accordion_side_single: Vec<SharedString>,
    // Wizard state
    pub wizard_step: usize,
    pub wizard_statuses: Vec<StepStatus>,
    // Workflow state (simple graph, no persistent Entity)
    pub workflow_graph: WorkflowGraph,
    pub workflow_node_counter: usize,
    // Table states
    pub users: Vec<User>,
    pub selected_users: HashSet<usize>,
    pub sort_state: Option<SortState>,
    pub pagination: PaginationState,
    // Pane divider states
    pub pane_left_collapsed: bool,
    pub pane_left_width: f32,
    pub pane_dragging_left: bool,
    pub pane_drag_start_x: f32,
    pub pane_drag_start_width: f32,
    // Tooltip hover state
    pub tooltip_hovered: Option<&'static str>,
    // Popover open state
    pub popover_open: Option<&'static str>,
    // Current section for navigation
    pub current_section: ShowcaseSection,
    // Entity for updating self
    pub entity: Entity<Self>,
    // Focus handle for keyboard input
    pub focus_handle: FocusHandle,
}

impl Showcase {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let workflow_graph = WorkflowGraph::new();

        Self {
            toggle_on: true,
            toggle_lg: false,
            checkbox_checked: true,
            slider_value: 0.5,
            number_value: 42.0,
            number_freq: 1000.0,
            number_db: -3.0,
            editing_number: None,
            edit_text: String::new(),
            text_selected: false,
            input_value: String::from("Hello World!"),
            input_editing: false,
            input_edit_text: String::new(),
            input_selected: false,
            select_value: Some("apple".into()),
            select_open: false,
            select_highlighted: None,
            buttonset_view_mode: "grid".into(),
            buttonset_alignment: "center".into(),
            selected_tab: 0,
            accordion_vertical_single: vec!["v-single-1".into()],
            accordion_vertical_multiple: vec!["v-multi-1".into(), "v-multi-2".into()],
            accordion_horizontal_single: vec!["h-single-1".into()],
            accordion_side_single: vec!["side-single-1".into(), "side-single-2".into()],
            wizard_step: 0,
            wizard_statuses: vec![
                StepStatus::Active,
                StepStatus::NotVisited,
                StepStatus::NotVisited,
                StepStatus::NotVisited,
                StepStatus::NotVisited,
            ],
            users: vec![
                User {
                    id: 1,
                    name: "Alice Smith".into(),
                    email: "alice@example.com".into(),
                    role: "Admin".into(),
                },
                User {
                    id: 2,
                    name: "Bob Jones".into(),
                    email: "bob@example.com".into(),
                    role: "User".into(),
                },
                User {
                    id: 3,
                    name: "Charlie Brown".into(),
                    email: "charlie@example.com".into(),
                    role: "Editor".into(),
                },
                User {
                    id: 4,
                    name: "David Wilson".into(),
                    email: "david@example.com".into(),
                    role: "User".into(),
                },
                User {
                    id: 5,
                    name: "Eve Adams".into(),
                    email: "eve@example.com".into(),
                    role: "Admin".into(),
                },
            ],
            selected_users: HashSet::new(),
            sort_state: Some(SortState {
                column_id: "name".into(),
                direction: SortDirection::Ascending,
            }),
            pagination: PaginationState {
                current_page: 0,
                page_size: 5,
                total_items: 5,
            },
            workflow_graph,
            workflow_node_counter: 0,
            pane_left_collapsed: false,
            pane_left_width: 200.0,
            pane_dragging_left: false,
            pane_drag_start_x: 0.0,
            pane_drag_start_width: 0.0,
            tooltip_hovered: None,
            popover_open: None,
            current_section: ShowcaseSection::default(),
            entity: cx.entity().clone(),
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Render for Showcase {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = self.entity.clone();
        let toggle_on = self.toggle_on;
        let checkbox_checked = self.checkbox_checked;
        let slider_value = self.slider_value;
        let current_section = self.current_section;

        // Get theme colors
        let theme = cx.theme();
        let bg_color = theme.background;
        let text_color = theme.text_secondary;
        let accent_color = theme.accent;

        // Get translations
        let title = cx.t(TranslationKey::AppTitle);
        let subtitle = cx.t(TranslationKey::AppSubtitle);

        // Build navigation sidebar items grouped by category
        let mut nav_items = div().flex().flex_col().py_4().gap_1();
        let border_color = theme.border;

        for group in ShowcaseGroup::all() {
            // Group header
            nav_items = nav_items.child(
                div()
                    .px_4()
                    .pt_3()
                    .pb_1()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.text_muted)
                    .child(group.label().to_uppercase()),
            );

            // Section items within this group
            for section in group.sections() {
                let section = *section;
                let is_active = section == current_section;
                let entity_clone = entity.clone();

                let mut item = div()
                    .id(SharedString::from(format!("nav-{:?}", section)))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_4()
                    .py(px(5.0))
                    .mx_2()
                    .rounded_md()
                    .cursor_pointer()
                    .text_sm();

                if is_active {
                    item = item
                        .bg(accent_color)
                        .text_color(rgba(0xffffffff))
                        .font_weight(FontWeight::SEMIBOLD);
                } else {
                    let hover_bg = theme.surface_hover;
                    item = item.text_color(text_color).hover(move |s| s.bg(hover_bg));
                }

                item = item.child(div().child(section.label())).on_mouse_down(
                    MouseButton::Left,
                    move |_event, _window, cx| {
                        entity_clone.update(cx, |this, cx| {
                            this.current_section = section;
                            cx.notify();
                        });
                    },
                );

                nav_items = nav_items.child(item);
            }

            // Separator between groups
            nav_items = nav_items.child(div().mx_4().my_1().h(px(1.0)).bg(border_color));
        }

        let nav = Sidebar::new("showcase-nav")
            .side(SidebarSide::Left)
            .width(px(220.0))
            .content(nav_items);

        // Group description box
        let current_group = current_section.group();
        let group_description = current_group.description();
        let group_label = current_group.label();
        let info_bg = theme.surface;
        let info_border = theme.border;
        let info_text = theme.text_muted;

        let group_info = div()
            .mb_4()
            .p_4()
            .bg(info_bg)
            .border_1()
            .border_color(info_border)
            .rounded(px(6.0))
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(info_text)
                    .child(group_label),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(info_text)
                    .child(group_description),
            );

        // Main content area
        let content = match current_section {
            ShowcaseSection::Buttons => self.render_buttons_section(cx).into_any_element(),
            ShowcaseSection::Text => self.render_text_section(cx).into_any_element(),
            ShowcaseSection::Badges => self.render_badges_section(cx).into_any_element(),
            ShowcaseSection::Avatars => self.render_avatars_section(cx).into_any_element(),
            ShowcaseSection::FormControls => self
                .render_form_controls_section(
                    toggle_on,
                    self.toggle_lg,
                    checkbox_checked,
                    slider_value,
                    self.number_value,
                    self.number_freq,
                    self.number_db,
                    self.editing_number,
                    self.edit_text.clone(),
                    self.text_selected,
                    self.input_value.clone(),
                    self.input_editing,
                    self.input_edit_text.clone(),
                    self.input_selected,
                    self.buttonset_view_mode.clone(),
                    self.buttonset_alignment.clone(),
                    entity.clone(),
                    cx,
                )
                .into_any_element(),
            ShowcaseSection::Progress => self.render_progress_section(cx).into_any_element(),
            ShowcaseSection::Alerts => self.render_alerts_section(cx).into_any_element(),
            ShowcaseSection::Tabs => self.render_tabs_section(cx).into_any_element(),
            ShowcaseSection::Cards => self.render_card_section(cx).into_any_element(),
            ShowcaseSection::Breadcrumbs => self.render_breadcrumbs_section(cx).into_any_element(),
            ShowcaseSection::Spinners => self.render_spinners_section(cx).into_any_element(),
            ShowcaseSection::Layout => self.render_layout_section(cx).into_any_element(),
            ShowcaseSection::IconButtons => self.render_icon_buttons_section(cx).into_any_element(),
            ShowcaseSection::Toasts => self.render_toasts_section(cx).into_any_element(),
            ShowcaseSection::Dialog => self.render_dialog_section(cx).into_any_element(),
            ShowcaseSection::Menu => self.render_menu_section(cx).into_any_element(),
            ShowcaseSection::Table => self.render_table_section(cx).into_any_element(),
            ShowcaseSection::Tooltips => self.render_tooltip_section(cx).into_any_element(),
            ShowcaseSection::Accordion => self.render_accordion_section(cx).into_any_element(),
            ShowcaseSection::Wizard => self.render_wizard_section(cx).into_any_element(),
            ShowcaseSection::Workflow => self.render_workflow_section(cx).into_any_element(),
            ShowcaseSection::QrCode => self.render_qr_section(cx).into_any_element(),
            ShowcaseSection::ContextMenu => self.render_context_menu_section(cx).into_any_element(),
            ShowcaseSection::Popover => self.render_popover_section(cx).into_any_element(),
            ShowcaseSection::Sidebar => self.render_sidebar_section(cx).into_any_element(),
            ShowcaseSection::StatusBar => self.render_status_bar_section(cx).into_any_element(),
            ShowcaseSection::SearchBar => self.render_search_bar_section(cx).into_any_element(),
            ShowcaseSection::KeyboardShortcut => {
                self.render_keyboard_shortcut_section(cx).into_any_element()
            }
            ShowcaseSection::EmptyState => self.render_empty_state_section(cx).into_any_element(),
            ShowcaseSection::ConfirmDialog => {
                self.render_confirm_dialog_section(cx).into_any_element()
            }
            ShowcaseSection::SplitPane => self.render_split_pane_section(cx).into_any_element(),
            ShowcaseSection::ImageView => self.render_image_view_section(cx).into_any_element(),
            ShowcaseSection::SettingsForm => {
                self.render_settings_form_section(cx).into_any_element()
            }
            ShowcaseSection::StepIndicator => {
                self.render_step_indicator_section(cx).into_any_element()
            }
            ShowcaseSection::LoadingOverlay => {
                self.render_loading_overlay_section(cx).into_any_element()
            }
            ShowcaseSection::Tag => self.render_tag_section(cx).into_any_element(),
            ShowcaseSection::Toolbar => self.render_toolbar_section(cx).into_any_element(),
            ShowcaseSection::Notification => {
                self.render_notification_section(cx).into_any_element()
            }
            ShowcaseSection::TreeView => self.render_tree_view_section(cx).into_any_element(),
            ShowcaseSection::DragList => self.render_drag_list_section(cx).into_any_element(),
            ShowcaseSection::CommandPalette => {
                self.render_command_palette_section(cx).into_any_element()
            }
            ShowcaseSection::Accessibility => {
                self.render_accessibility_section(cx).into_any_element()
            }
        };

        div()
            .id("showcase-root")
            .track_focus(&self.focus_handle)
            .w_full()
            .h_full()
            .bg(bg_color)
            .text_color(text_color)
            .flex()
            .on_key_down(cx.listener(Self::handle_key_down))
            .child(nav)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(
                        // Header (fixed)
                        div()
                            .flex_shrink_0()
                            .p_8()
                            .pb_0()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(Heading::h1(title))
                            .child(Text::new(subtitle))
                            .child(Divider::new().build()),
                    )
                    .child(
                        // Scrollable content area
                        div()
                            .id("content-scroll")
                            .flex_1()
                            .overflow_y_scroll()
                            .p_8()
                            .pt_4()
                            .child(group_info)
                            .child(content),
                    ),
            )
    }
}

impl Showcase {
    pub fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Handle keys when editing text input
        if self.input_editing {
            match event.keystroke.key.as_str() {
                "enter" => {
                    self.input_value = self.input_edit_text.clone();
                    self.input_editing = false;
                    self.input_edit_text.clear();
                    self.input_selected = false;
                    cx.notify();
                }
                "escape" => {
                    self.input_editing = false;
                    self.input_edit_text.clear();
                    self.input_selected = false;
                    cx.notify();
                }
                "backspace" => {
                    if self.input_selected {
                        self.input_edit_text.clear();
                        self.input_selected = false;
                    } else {
                        self.input_edit_text.pop();
                    }
                    cx.notify();
                }
                key if key.len() == 1 => {
                    let ch = key.chars().next().unwrap();
                    if self.input_selected {
                        self.input_edit_text.clear();
                        self.input_selected = false;
                    }
                    self.input_edit_text.push(ch);
                    cx.notify();
                }
                _ => {}
            }
        }
        // Handle keys when editing a number input
        else if let Some(editing_id) = self.editing_number {
            match event.keystroke.key.as_str() {
                "enter" => {
                    if let Ok(value) = self.edit_text.parse::<f64>() {
                        match editing_id {
                            "basic" => self.number_value = value.clamp(0.0, 100.0),
                            "freq" => self.number_freq = value.clamp(20.0, 20000.0),
                            "db" => self.number_db = value.clamp(-12.0, 12.0),
                            _ => {}
                        }
                    }
                    self.editing_number = None;
                    self.edit_text.clear();
                    self.text_selected = false;
                    cx.notify();
                }
                "escape" => {
                    self.editing_number = None;
                    self.edit_text.clear();
                    self.text_selected = false;
                    cx.notify();
                }
                "backspace" => {
                    if self.text_selected {
                        self.edit_text.clear();
                        self.text_selected = false;
                    } else {
                        self.edit_text.pop();
                    }
                    cx.notify();
                }
                key if key.len() == 1 => {
                    let ch = key.chars().next().unwrap();
                    if ch.is_ascii_digit() || ch == '.' || ch == '-' {
                        if self.text_selected {
                            self.edit_text.clear();
                            self.text_selected = false;
                        }
                        self.edit_text.push(ch);
                        cx.notify();
                    }
                }
                _ => {}
            }
        }
    }
}

impl Showcase {
    pub fn section_header(&self, title: impl Into<SharedString>) -> impl IntoElement {
        Heading::h2(title)
    }
}

// --- QR section (inline, no animated QR entities) ---

impl Showcase {
    fn render_qr_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let section_title = cx.t(TranslationKey::SectionQrCode);

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(self.section_header(section_title))
            .child(Text::new("Static QR codes:").weight(TextWeight::Semibold))
            .child(
                HStack::new()
                    .spacing(StackSpacing::Lg)
                    .align(StackAlign::End)
                    .child(
                        VStack::new()
                            .spacing(StackSpacing::Sm)
                            .child(Text::new("Default (200px)").weight(TextWeight::Medium))
                            .child(QrCode::new("https://example.com")),
                    )
                    .child(
                        VStack::new()
                            .spacing(StackSpacing::Sm)
                            .child(Text::new("Small (120px)").weight(TextWeight::Medium))
                            .child(QrCode::new("https://example.com").size(px(120.0))),
                    )
                    .child(
                        VStack::new()
                            .spacing(StackSpacing::Sm)
                            .child(Text::new("Custom Colors").weight(TextWeight::Medium))
                            .child(
                                QrCode::new("https://example.com")
                                    .size(px(150.0))
                                    .fg(rgba(0x2da44eff))
                                    .bg(rgba(0x1a1a2eff)),
                            ),
                    ),
            )
    }
}

// --- Workflow section (inline, creates WorkflowCanvas entity on-the-fly) ---

impl Showcase {
    fn render_workflow_section(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let entity = self.entity.clone();

        // Create a WorkflowCanvas entity on-the-fly from the stored graph
        let graph = self.workflow_graph.clone();
        let workflow_canvas = cx.new(|cx| WorkflowCanvas::with_graph(graph, cx));

        // Get stats from canvas
        let (node_count, connection_count, _selected_count) = workflow_canvas.read(cx).stats();

        div()
            .flex()
            .flex_col()
            .gap_6()
            .h_full()
            .child(self.section_header("Workflow Canvas"))
            .child(
                Text::new("A node-based workflow editor with drag-and-drop connections, panning, and zooming.")
                    .color(theme.text_secondary)
            )

            // Canvas Container
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.border)
                    .rounded_xl()
                    .overflow_hidden()
                    .h(px(500.0))

                    // Toolbar
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .p_3()
                            .border_b_1()
                            .border_color(theme.border)
                            .bg(theme.surface)
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .child(
                                        Button::new("wf-add-node", "Add Node")
                                            .size(ButtonSize::Sm)
                                            .on_click({
                                                let entity = entity.clone();
                                                move |_, cx| {
                                                    entity.update(cx, |this, cx| this.workflow_add_node(cx));
                                                }
                                            }),
                                    )
                                    .child(
                                        Button::new("wf-clear", "Clear")
                                            .size(ButtonSize::Sm)
                                            .variant(ButtonVariant::Destructive)
                                            .on_click({
                                                let entity = entity.clone();
                                                move |_, cx| {
                                                    entity.update(cx, |this, _cx| {
                                                        this.workflow_graph = WorkflowGraph::new();
                                                        this.workflow_node_counter = 0;
                                                    });
                                                }
                                            }),
                                    )
                            )
                            .child(
                                Text::new(format!("Nodes: {} | Conns: {}", node_count, connection_count))
                                    .size(TextSize::Xs)
                                    .muted(true)
                            )
                    )

                    // Canvas
                    .child(
                        div()
                            .flex_1()
                            .relative()
                            .child(workflow_canvas)
                    )

                    // Footer instructions
                    .child(
                        div()
                            .p_2()
                            .bg(theme.background)
                            .border_t_1()
                            .border_color(theme.border)
                            .child(
                                Text::new("Drag nodes to move. Drag from ports to connect. Scroll to zoom. Middle-click to pan.")
                                    .size(TextSize::Xs)
                                    .muted(true)
                            )
                    )
            )
    }

    fn workflow_add_node(&mut self, _cx: &mut Context<Self>) {
        self.workflow_node_counter += 1;
        let id = self.workflow_node_counter;

        let x = 100.0 + (id as f32 * 30.0) % 400.0;
        let y = 100.0 + (id as f32 * 20.0) % 300.0;

        let node = WorkflowNodeData::new(format!("Node {}", id), Position::new(x, y))
            .with_ports(1, 1)
            .with_size(160.0, 70.0);

        self.workflow_graph.add_node(node);
    }
}

// --- Include all other render_*.inc.rs files with adjusted paths ---

include!("../examples/includes/render_accordion.inc.rs");
include!("../examples/includes/render_alert.inc.rs");
include!("../examples/includes/render_avatar.inc.rs");
include!("../examples/includes/render_badge.inc.rs");
include!("../examples/includes/render_breadcrumbs.inc.rs");
include!("../examples/includes/render_button.inc.rs");
include!("../examples/includes/render_card.inc.rs");
include!("../examples/includes/render_dialog.inc.rs");
include!("../examples/includes/render_form.inc.rs");
include!("../examples/includes/render_icon.inc.rs");
include!("../examples/includes/render_layout.inc.rs");
include!("../examples/includes/render_menu.inc.rs");
include!("../examples/includes/render_progress.inc.rs");
include!("../examples/includes/render_spinners.inc.rs");
include!("../examples/includes/render_table.inc.rs");
include!("../examples/includes/render_tabs.inc.rs");
include!("../examples/includes/render_text.inc.rs");
include!("../examples/includes/render_toast.inc.rs");
include!("../examples/includes/render_tooltip.inc.rs");
include!("../examples/includes/render_wizard.inc.rs");
include!("../examples/includes/render_context_menu.inc.rs");
include!("../examples/includes/render_popover.inc.rs");
include!("../examples/includes/render_sidebar.inc.rs");
include!("../examples/includes/render_status_bar.inc.rs");
include!("../examples/includes/render_search_bar.inc.rs");
include!("../examples/includes/render_keyboard_shortcut.inc.rs");
include!("../examples/includes/render_empty_state.inc.rs");
include!("../examples/includes/render_confirm_dialog.inc.rs");
include!("../examples/includes/render_split_pane.inc.rs");
include!("../examples/includes/render_image_view.inc.rs");
include!("../examples/includes/render_settings_form.inc.rs");
include!("../examples/includes/render_step_indicator.inc.rs");
include!("../examples/includes/render_loading_overlay.inc.rs");
include!("../examples/includes/render_tag.inc.rs");
include!("../examples/includes/render_toolbar.inc.rs");
include!("../examples/includes/render_notification.inc.rs");
include!("../examples/includes/render_tree_view.inc.rs");
include!("../examples/includes/render_drag_list.inc.rs");
include!("../examples/includes/render_command_palette.inc.rs");
include!("../examples/includes/render_accessibility.inc.rs");
