//! Dialog and modal rendering components

use crate::app::Screen;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Badge, BadgeVariant, Dialog, DialogSize, HStack, StackAlign, StackJustify, StackSpacing, Text,
    TextSize, TextWeight, VStack,
};

impl PlayerView {
    pub(crate) fn render_help_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let screen_name = match state.app.current_screen {
            Screen::Library => "Library",
            Screen::DirectoryManager => "Directories",
            Screen::Queue => "Queue",
            Screen::Spectrum => "Spectrum",
            Screen::Settings => "Settings",
            Screen::Studio => "Studio",
            Screen::Recording => "Recording",
            Screen::RoomEq => "Room EQ",
            Screen::HeadphoneEq => "Headphone EQ",
            Screen::Spinorama => "Spinorama",
            Screen::PluginGraph => "Plugin Graph",
        };

        // Get keybindings for current screen
        let keybindings = get_keybindings_for_screen(state.app.current_screen);

        Dialog::new("help-modal")
            .title(format!("Help - {} Screen", screen_name))
            .size(DialogSize::Full)
            .content(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    // Global keybindings section
                    .child(
                        Text::new("GLOBAL KEYBINDINGS")
                            .size(TextSize::Lg)
                            .weight(TextWeight::Semibold)
                            .color(theme.accent),
                    )
                    .child(self.render_keybinding_row(
                        "Shift-L/Q/P/O/D",
                        "Jump to Library/Queue/Plugins/Devices/Directories",
                        &theme,
                    ))
                    .child(self.render_keybinding_row("+/=", "Increase volume", &theme))
                    .child(self.render_keybinding_row("-/_", "Decrease volume", &theme))
                    .child(self.render_keybinding_row("?", "Show this help", &theme))
                    .child(div().h_4()) // Spacer
                    // Screen-specific keybindings section
                    .child(
                        Text::new(format!("{} KEYBINDINGS", screen_name.to_uppercase()))
                            .size(TextSize::Lg)
                            .weight(TextWeight::Semibold)
                            .color(theme.accent),
                    )
                    .children(keybindings.iter().map(|(key, desc)| {
                        self.render_keybinding_row(key, desc, &theme)
                            .into_any_element()
                    })),
            )
            .footer(
                Text::new("Press ESC or ? to close")
                    .size(TextSize::Xs)
                    .muted(true),
            )
    }

    pub(crate) fn render_about_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();

        Dialog::new("about-dialog")
            .title("About SotF Player")
            .size(DialogSize::Md)
            .content(
                VStack::new()
                    .spacing(StackSpacing::Lg)
                    .align(StackAlign::Center)
                    .child(
                        div()
                            .w(px(128.0))
                            .h(px(128.0))
                            .rounded_xl()
                            .overflow_hidden()
                            .child(
                                img("sotf.jpg")
                                    .w_full()
                                    .h_full()
                                    .object_fit(ObjectFit::Cover),
                            ),
                    )
                    .child(
                        VStack::new()
                            .spacing(StackSpacing::Sm)
                            .align(StackAlign::Center)
                            .child(
                                Text::new("SotF Player")
                                    .size(TextSize::Xl)
                                    .weight(TextWeight::Bold)
                                    .color(theme.text_primary),
                            )
                            .child(
                                Text::new("Version 0.1.0")
                                    .size(TextSize::Sm)
                                    .color(theme.text_secondary),
                            )
                            .child(
                                Text::new("© 2025 Spinorama")
                                    .size(TextSize::Sm)
                                    .color(theme.text_muted),
                            ),
                    ),
            )
            .footer(
                Text::new("Press ESC to close")
                    .size(TextSize::Xs)
                    .muted(true),
            )
    }

    fn render_keybinding_row(
        &self,
        key: &str,
        description: &str,
        theme: &crate::theme::Theme,
    ) -> impl IntoElement {
        HStack::new()
            .spacing(StackSpacing::Md)
            .child(
                div()
                    .w(Rems(12.0))
                    .child(Badge::new(key.to_string()).variant(BadgeVariant::Primary)),
            )
            .child(
                Text::new(description.to_string())
                    .size(TextSize::Sm)
                    .color(theme.text_secondary),
            )
    }

    #[allow(dead_code)]
    pub(crate) fn render_keybinding(
        &self,
        key: &str,
        description: &str,
        theme: &crate::theme::Theme,
    ) -> impl IntoElement {
        div()
            .flex()
            .gap_4()
            .mb_1()
            .child(
                div()
                    .w(Rems(12.0))
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.info)
                    .child(format!("  {}", key)),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.text_secondary)
                    .child(description.to_string()),
            )
    }

    pub(crate) fn render_toast(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();

        if let Some(toast) = &state.app.toast_message {
            let (bg_color, border_color, icon) = match toast.toast_type {
                crate::app::ToastType::Success => (theme.toast_success_bg, theme.success, "✓"),
                crate::app::ToastType::Error => (theme.toast_error_bg, theme.error, "✗"),
                crate::app::ToastType::Info => (theme.toast_info_bg, theme.accent, "ℹ"),
                crate::app::ToastType::Warning => (theme.toast_warning_bg, theme.warning, "⚠"),
            };

            div()
                .absolute()
                .top(px(20.0))
                .left_1_2()
                .min_w(Rems(25.0))
                .max_w(Rems(50.0))
                .bg(bg_color)
                .border_2()
                .border_color(border_color)
                .rounded_md()
                .shadow_lg()
                .p_3()
                .child(
                    HStack::new()
                        .spacing(StackSpacing::Md)
                        .align(StackAlign::Center)
                        .child(
                            Text::new(icon)
                                .size(TextSize::Lg)
                                .weight(TextWeight::Bold)
                                .color(border_color),
                        )
                        .child(
                            div().flex_1().child(
                                Text::new(toast.message.clone())
                                    .size(TextSize::Sm)
                                    .color(theme.text_primary),
                            ),
                        )
                        .child(Text::new("ESC to dismiss").size(TextSize::Xs).muted(true)),
                )
        } else {
            div() // Return empty div if no toast
        }
    }

    pub(crate) fn render_context_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();

        if let Some(menu) = &state.app.context_menu {
            let menu_items: Vec<(&'static str, &'static str)> = match menu.menu_type {
                crate::app::ContextMenuType::Album => {
                    vec![("Add to Queue", "a"), ("Play Now", "enter")]
                }
                crate::app::ContextMenuType::QueueItem => {
                    vec![("Remove from Queue", "d"), ("Play from Here", "enter")]
                }
                crate::app::ContextMenuType::Plugin => vec![
                    ("Toggle Enabled", "shift-t"),
                    ("Move Up", "u"),
                    ("Move Down", "shift-n"),
                    ("Remove Plugin", "d"),
                ],
                crate::app::ContextMenuType::Directory => {
                    vec![("Remove Directory", "d"), ("Rescan Library", "shift-s")]
                }
            };

            div()
                .absolute()
                .top(px(menu.position_y))
                .left(px(menu.position_x))
                .w(Rems(15.0))
                .bg(theme.surface)
                .text_color(theme.text_primary)
                .border_1()
                .border_color(theme.accent)
                .rounded_md()
                .shadow_lg()
                .overflow_hidden()
                .children(menu_items.into_iter().map(|(label, shortcut)| {
                    let theme = theme.clone();
                    div()
                        .px_3()
                        .py_2()
                        .text_color(theme.text_primary)
                        .hover(|style| style.bg(theme.surface_hover))
                        .cursor_pointer()
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                // Close menu and execute action based on menu type
                                view.state.update(cx, |state, _cx| {
                                    let menu_type = state
                                        .app
                                        .context_menu
                                        .as_ref()
                                        .map(|m| m.menu_type.clone());
                                    let item_idx = state
                                        .app
                                        .context_menu
                                        .as_ref()
                                        .map(|m| m.item_index)
                                        .unwrap_or(0);
                                    state.app.context_menu = None;

                                    if let Some(mt) = menu_type {
                                        match (mt, label) {
                                            (
                                                crate::app::ContextMenuType::Album,
                                                "Add to Queue",
                                            ) => {
                                                if let Some(path) = state.app.add_album_to_queue() {
                                                    Self::play_track(state, path);
                                                }
                                            }
                                            (crate::app::ContextMenuType::Album, "Play Now") => {
                                                if let Some(path) = state.app.play_album_now() {
                                                    Self::play_track(state, path);
                                                }
                                            }
                                            (
                                                crate::app::ContextMenuType::QueueItem,
                                                "Remove from Queue",
                                            ) => {
                                                state.app.remove_from_queue(item_idx);
                                            }
                                            (
                                                crate::app::ContextMenuType::QueueItem,
                                                "Play from Here",
                                            ) => {
                                                state.app.current_queue_index = Some(item_idx);
                                                // Play the first track of the queue item
                                                if let Some(queue_item) =
                                                    state.app.queue.get(item_idx)
                                                {
                                                    if let Some(first_track) =
                                                        queue_item.album.tracks.first()
                                                    {
                                                        Self::play_track(
                                                            state,
                                                            first_track.path.clone(),
                                                        );
                                                    }
                                                }
                                            }
                                            (
                                                crate::app::ContextMenuType::Plugin,
                                                "Toggle Enabled",
                                            ) => {
                                                state.app.plugin_chain.toggle_plugin(item_idx);
                                            }
                                            (crate::app::ContextMenuType::Plugin, "Move Up") => {
                                                state.app.move_plugin_up(item_idx);
                                            }
                                            (crate::app::ContextMenuType::Plugin, "Move Down") => {
                                                state.app.move_plugin_down(item_idx);
                                            }
                                            (
                                                crate::app::ContextMenuType::Plugin,
                                                "Remove Plugin",
                                            ) => {
                                                state.app.plugin_chain.remove_plugin(item_idx);
                                            }
                                            (
                                                crate::app::ContextMenuType::Directory,
                                                "Remove Directory",
                                            ) => {
                                                state.app.selected_directory_index = item_idx;
                                                state.app.remove_selected_directory();
                                            }
                                            (
                                                crate::app::ContextMenuType::Directory,
                                                "Rescan Library",
                                            ) => {
                                                if let Err(e) = state.app.scan_library() {
                                                    log::error!("Scan failed: {}", e);
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                });
                                cx.notify();
                            }),
                        )
                        .child(
                            HStack::new()
                                .justify(StackJustify::SpaceBetween)
                                .align(StackAlign::Center)
                                .child(
                                    Text::new(label)
                                        .size(TextSize::Sm)
                                        .color(theme.text_primary),
                                )
                                .child(
                                    Text::new(shortcut)
                                        .size(TextSize::Xs)
                                        .color(theme.text_muted),
                                ),
                        )
                }))
        } else {
            div() // Return empty div if no menu
        }
    }

    pub(crate) fn render_apo_file_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();

        Dialog::new("apo-file-dialog")
            .title("Load APO File for EQ Plugin")
            .size(DialogSize::Lg)
            .content(
                VStack::new()
                    .spacing(StackSpacing::Md)
                    .child(
                        Text::new("Enter path to APO file:")
                            .size(TextSize::Sm)
                            .muted(true),
                    )
                    .child(
                        div()
                            .p_2()
                            .rounded_md()
                            .bg(theme.surface)
                            .border_1()
                            .border_color(theme.accent)
                            .child(
                                Text::new(format!("{}█", state.app.apo_file_input))
                                    .size(TextSize::Sm),
                            ),
                    ),
            )
            .footer(
                Text::new("Enter: Load file | ESC: Cancel")
                    .size(TextSize::Xs)
                    .muted(true),
            )
    }

    pub(crate) fn render_sofa_file_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();

        Dialog::new("sofa-file-dialog")
            .title("Load SOFA File for Binaural Decoder")
            .size(DialogSize::Lg)
            .content(
                VStack::new()
                    .spacing(StackSpacing::Md)
                    .child(
                        Text::new("Enter path to SOFA file:")
                            .size(TextSize::Sm)
                            .muted(true),
                    )
                    .child(
                        div()
                            .p_2()
                            .rounded_md()
                            .bg(theme.surface)
                            .border_1()
                            .border_color(theme.accent)
                            .child(
                                Text::new(format!("{}█", state.app.sofa_file_input))
                                    .size(TextSize::Sm),
                            ),
                    ),
            )
            .footer(
                Text::new("Enter: Load file | ESC: Cancel")
                    .size(TextSize::Xs)
                    .muted(true),
            )
    }

    pub(crate) fn render_save_plugins_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let presets = state.app.available_plugin_presets.clone();
        let selected_preset = state.app.selected_preset_index;
        let input = state.app.plugin_file_input.clone();

        Dialog::new("save-plugins-dialog")
            .title("Save Plugin Preset")
            .size(DialogSize::Xl)
            .content(
                VStack::new()
                    .spacing(StackSpacing::Md)
                    .child(
                        Text::new("Enter preset name (or select existing to overwrite):")
                            .size(TextSize::Sm)
                            .muted(true),
                    )
                    .child(
                        div()
                            .p_2()
                            .rounded_md()
                            .bg(theme.surface)
                            .border_1()
                            .border_color(theme.accent)
                            .child(Text::new(format!("{}█", input)).size(TextSize::Sm)),
                    )
                    // Show existing presets if available
                    .when(!presets.is_empty(), |el| {
                        el.child(
                            Text::new("Existing presets (↑/↓ to select):")
                                .size(TextSize::Sm)
                                .muted(true),
                        )
                        .child(
                            div()
                                .id("save-plugins-presets-list")
                                .max_h(Rems(12.0))
                                .overflow_y_scroll()
                                .bg(theme.surface)
                                .rounded_md()
                                .p_2()
                                .children(presets.iter().enumerate().map(|(idx, preset)| {
                                    let is_selected = idx == selected_preset;
                                    let theme = theme.clone();
                                    div()
                                        .p_1()
                                        .rounded_md()
                                        .text_sm()
                                        .when(is_selected, |d| {
                                            d.bg(theme.accent_muted).text_color(theme.text_primary)
                                        })
                                        .when(!is_selected, |d| d.text_color(theme.text_secondary))
                                        .child(preset.clone())
                                })),
                        )
                    }),
            )
            .footer(
                Text::new("Enter: Save | ↑/↓: Select preset | Tab: Autocomplete | ESC: Cancel")
                    .size(TextSize::Xs)
                    .muted(true),
            )
    }

    pub(crate) fn render_load_plugins_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        let presets = state.app.available_plugin_presets.clone();
        let selected_preset = state.app.selected_preset_index;
        let input = state.app.plugin_file_input.clone();

        Dialog::new("load-plugins-dialog")
            .title("Load Plugin Preset")
            .size(DialogSize::Xl)
            .content(
                VStack::new()
                    .spacing(StackSpacing::Md)
                    .child(
                        Text::new("Enter preset name or select from list:")
                            .size(TextSize::Sm)
                            .muted(true),
                    )
                    .child(
                        div()
                            .p_2()
                            .rounded_md()
                            .bg(theme.surface)
                            .border_1()
                            .border_color(theme.success)
                            .child(Text::new(format!("{}█", input)).size(TextSize::Sm)),
                    )
                    // Show existing presets
                    .when(!presets.is_empty(), |el| {
                        el.child(
                            Text::new("Available presets (↑/↓ to select):")
                                .size(TextSize::Sm)
                                .muted(true),
                        )
                        .child(
                            div()
                                .id("load-plugins-presets-list")
                                .max_h(Rems(12.0))
                                .overflow_y_scroll()
                                .bg(theme.surface)
                                .rounded_md()
                                .p_2()
                                .children(presets.iter().enumerate().map(|(idx, preset)| {
                                    let is_selected = idx == selected_preset;
                                    let theme = theme.clone();
                                    div()
                                        .p_1()
                                        .rounded_md()
                                        .text_sm()
                                        .when(is_selected, |d| {
                                            d.bg(theme.accent_muted).text_color(theme.text_primary)
                                        })
                                        .when(!is_selected, |d| d.text_color(theme.text_secondary))
                                        .child(preset.clone())
                                })),
                        )
                    })
                    .when(presets.is_empty(), |el| {
                        el.child(
                            div().p_4().text_center().child(
                                Text::new("No presets found. Save a preset first with 's'.")
                                    .size(TextSize::Sm)
                                    .muted(true),
                            ),
                        )
                    }),
            )
            .footer(
                Text::new("Enter: Load | ↑/↓: Select preset | Tab: Autocomplete | ESC: Cancel")
                    .size(TextSize::Xs)
                    .muted(true),
            )
    }
}

fn get_keybindings_for_screen(screen: Screen) -> Vec<(&'static str, &'static str)> {
    match screen {
        Screen::Library => vec![
            ("↑/↓ or K/J", "Navigate albums/artists"),
            ("PageUp/PageDown", "Jump by page"),
            ("/", "Search albums"),
            ("T", "Toggle tree view / flat view"),
            ("H/L or ←/→", "Collapse/expand artists in tree view"),
            ("S or 1/2/3/4", "Sort by Artist/Album/Title/Year"),
            ("C or 5/6/7/8/9", "Filter: All/Mono/Stereo/Multi/Mixed"),
            ("A or Enter", "Add album to queue"),
            ("Shift-Q", "Go to queue screen"),
        ],
        Screen::DirectoryManager => vec![
            ("↑/↓ or K/J", "Navigate directories"),
            ("PageUp/PageDown", "Jump by page"),
            ("Enter/→/L", "Expand/collapse directory"),
            ("Shift-A", "Add directory"),
            ("D/Delete", "Remove selected directory"),
            ("Shift-S", "Scan library"),
        ],
        Screen::Queue => vec![
            ("↑/↓ or K/J", "Navigate queue items"),
            ("Enter", "Play selected album from start"),
            ("H/L or ←/→", "Expand/collapse album tracks"),
            ("Space", "Play/Pause"),
            ("N or >", "Next track"),
            ("B or <", "Previous track"),
            ("D/Delete", "Remove from queue"),
            ("C", "Clear entire queue"),
        ],
        Screen::Spectrum => vec![("Space", "Play/Pause"), ("N", "Next track")],
        Screen::Settings => vec![("T", "Cycle theme"), ("Alt-L", "Cycle language")],
        Screen::Studio => vec![
            ("E/U/G/L/O/B", "Add plugins"),
            ("Enter/e", "Edit plugin"),
            ("D/Delete", "Delete plugin"),
            ("Space", "Toggle on/off"),
            ("Shift-U/N", "Move up/down"),
            ("Shift-S/l", "Save/Load preset"),
        ],
        Screen::Recording => vec![
            ("Back/Close", "Navigate between steps"),
            ("Next/Finish", "Proceed to next step or finish"),
        ],
        Screen::RoomEq => vec![
            ("Back/Close", "Navigate between steps"),
            ("Next/Finish", "Proceed to next step or finish"),
        ],
        Screen::HeadphoneEq => vec![
            ("Back/Close", "Navigate between steps"),
            ("Next/Finish", "Proceed to next step or finish"),
        ],
        Screen::Spinorama => vec![
            ("Back/Close", "Navigate between steps"),
            ("Next/Finish", "Proceed to next step or finish"),
        ],
        Screen::PluginGraph => vec![
            ("Click+Drag", "Move nodes"),
            ("Drag port", "Create connection"),
            ("Delete", "Remove selected"),
            ("Space", "Toggle selected plugin"),
        ],
    }
}

impl PlayerView {
    /// Render a comprehensive keyboard shortcuts dialog using Dialog component
    pub(crate) fn render_keyboard_shortcuts_dialog(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.state.read(cx).app.theme.clone();

        let global_shortcuts = vec![
            ("Space", "Play / Pause"),
            ("N", "Next track"),
            ("P", "Previous track"),
            ("+/-", "Volume up/down"),
            ("M", "Toggle mute"),
            ("1-5", "Switch screens"),
            ("?", "Toggle help"),
            ("Esc", "Close / Cancel"),
            ("T", "Cycle theme"),
            ("Alt-L", "Cycle language"),
            ("Cmd-Q", "Quit"),
        ];

        let library_shortcuts = vec![
            ("↑/↓ K/J", "Navigate albums"),
            ("Enter", "Add & play"),
            ("Q", "Add to queue"),
            ("/", "Search"),
            ("V", "Toggle view"),
            ("S", "Cycle sort"),
            ("C", "Channel filter"),
        ];

        let queue_shortcuts = vec![
            ("↑/↓ K/J", "Navigate queue"),
            ("X", "Remove item"),
            ("Shift-X", "Clear queue"),
            ("Tab", "Select meter"),
            ("Shift-M", "Mute group"),
            ("Shift-S", "Solo group"),
        ];

        let plugin_shortcuts = vec![
            ("E/U/G/L/O/B", "Add plugins"),
            ("Enter/e", "Edit plugin"),
            ("D/Delete", "Delete plugin"),
            ("Space", "Toggle on/off"),
            ("Shift-U/N", "Move up/down"),
            ("Shift-S/l", "Save/Load preset"),
        ];

        div()
            .child(
                Dialog::new("shortcuts-dialog")
                    .title("Keyboard Shortcuts")
                    .size(DialogSize::Full)
                    .show_close_button(false)
                    .content(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_6()
                            .child(self.render_shortcut_section(
                                "Global",
                                &global_shortcuts,
                                &theme,
                            ))
                            .child(self.render_shortcut_section(
                                "Library",
                                &library_shortcuts,
                                &theme,
                            ))
                            .child(self.render_shortcut_section("Queue", &queue_shortcuts, &theme))
                            .child(self.render_shortcut_section(
                                "Plugins",
                                &plugin_shortcuts,
                                &theme,
                            )),
                    )
                    .footer(
                        Text::new("Press ESC or ? to close")
                            .size(TextSize::Xs)
                            .muted(true),
                    ),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _: &MouseDownEvent, _window, cx| {
                    view.state.update(cx, |state, _cx| {
                        state.app.input_mode = crate::app::InputMode::Normal;
                    });
                    cx.notify();
                }),
            )
    }

    fn render_shortcut_section(
        &self,
        title: &str,
        shortcuts: &[(&str, &str)],
        theme: &crate::theme::Theme,
    ) -> impl IntoElement {
        VStack::new()
            .spacing(StackSpacing::Xs)
            .child(
                Text::new(title.to_string())
                    .size(TextSize::Sm)
                    .weight(TextWeight::Semibold)
                    .color(theme.accent),
            )
            .children(shortcuts.iter().map(|(key, desc)| {
                HStack::new()
                    .spacing(StackSpacing::Md)
                    .child(
                        div()
                            .w(Rems(8.0))
                            .child(Badge::new(key.to_string()).variant(BadgeVariant::Primary)),
                    )
                    .child(
                        Text::new(desc.to_string())
                            .size(TextSize::Xs)
                            .color(theme.text_secondary),
                    )
                    .into_any_element()
            }))
            .build()
            .min_w(px(260.0))
    }

    /// Render the scan progress modal
    pub(crate) fn render_scan_progress_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();

        // Get the active modal info
        let modal = match &state.app.scan_progress_modal {
            Some(m) if m.visible => m.clone(),
            _ => return div().into_any_element(),
        };

        // Get progress info based on scan type
        let (progress, processed, total, succeeded, failed) = match modal.scan_type {
            crate::app::types::ScanType::Library => {
                let albums = state.app.scan_progress_albums;
                let tracks = state.app.scan_progress_tracks;
                // For library scan, we don't have a total count upfront
                (0.0, tracks, 0usize, albums, 0usize)
            }
            crate::app::types::ScanType::ReplayGain => {
                let mgr = &state.app.replay_gain_manager;
                (
                    mgr.progress(),
                    mgr.processed,
                    mgr.total,
                    mgr.succeeded,
                    mgr.failed,
                )
            }
            crate::app::types::ScanType::Bliss => {
                let mgr = &state.app.bliss_manager;
                (
                    mgr.progress(),
                    mgr.processed,
                    mgr.total,
                    mgr.succeeded,
                    mgr.failed,
                )
            }
            crate::app::types::ScanType::Waveform => {
                let mgr = &state.app.waveform_manager;
                (
                    mgr.progress(),
                    mgr.processed,
                    mgr.total,
                    mgr.succeeded,
                    mgr.failed,
                )
            }
        };

        let scan_type = modal.scan_type;
        let is_library_scan = matches!(scan_type, crate::app::types::ScanType::Library);

        // Check if scan is complete
        let is_complete = match scan_type {
            crate::app::types::ScanType::Library => !state.app.scan_in_progress,
            crate::app::types::ScanType::ReplayGain => !state.app.replay_gain_manager.in_progress,
            crate::app::types::ScanType::Bliss => !state.app.bliss_manager.in_progress,
            crate::app::types::ScanType::Waveform => !state.app.waveform_manager.in_progress,
        };

        // Progress bar width (out of 100%)
        let progress_width = if is_complete {
            100.0
        } else if is_library_scan {
            // For library scan, show an indeterminate animation-like effect
            50.0
        } else {
            progress.clamp(0.0, 100.0)
        };

        // Status text
        let status_text = if is_complete {
            if is_library_scan {
                format!("Complete: {} albums, {} tracks found", succeeded, processed)
            } else {
                format!("Complete: {} succeeded, {} failed", succeeded, failed)
            }
        } else if is_library_scan {
            format!("{} albums, {} tracks found", succeeded, processed)
        } else if total > 0 {
            format!(
                "{} / {} processed ({} succeeded, {} failed)",
                processed, total, succeeded, failed
            )
        } else {
            "Initializing...".to_string()
        };

        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::rgba(0x00000080))
            .child(
                div()
                    .w(px(400.0))
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.border)
                    .rounded_lg()
                    .shadow_lg()
                    .p_6()
                    .flex()
                    .flex_col()
                    .gap_4()
                    // Title
                    .child(
                        Text::new(scan_type.title())
                            .size(TextSize::Lg)
                            .weight(TextWeight::Bold)
                            .color(theme.text_primary),
                    )
                    // Description
                    .child(
                        Text::new(scan_type.description())
                            .size(TextSize::Sm)
                            .color(theme.text_secondary),
                    )
                    // Progress bar
                    .child(
                        div()
                            .w_full()
                            .h(px(8.0))
                            .bg(theme.background_secondary)
                            .rounded_full()
                            .overflow_hidden()
                            .child(
                                div()
                                    .h_full()
                                    // Calculate width as a fraction of the container
                                    // 400px modal - 48px padding (24px each side) = 352px container
                                    .w(px(352.0 * (progress_width / 100.0)))
                                    .bg(theme.accent)
                                    .rounded_full(),
                            ),
                    )
                    // Status text
                    .child(
                        Text::new(status_text)
                            .size(TextSize::Sm)
                            .color(theme.text_muted),
                    )
                    // Buttons
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Md)
                            .justify(StackJustify::End)
                            .when(!is_complete, |stack| {
                                stack
                                    .child(
                                        gpui_ui_kit::Button::new("scan-cancel", "Cancel")
                                            .variant(gpui_ui_kit::ButtonVariant::Secondary)
                                            .size(gpui_ui_kit::ButtonSize::Md)
                                            .theme(theme.to_button_theme())
                                            .build()
                                            .on_click(cx.listener(
                                                move |view, _: &ClickEvent, _window, cx| {
                                                    view.cancel_scan(scan_type, cx);
                                                },
                                            )),
                                    )
                                    .child(
                                        gpui_ui_kit::Button::new("scan-background", "Background")
                                            .variant(gpui_ui_kit::ButtonVariant::Secondary)
                                            .size(gpui_ui_kit::ButtonSize::Md)
                                            .theme(theme.to_button_theme())
                                            .build()
                                            .on_click(cx.listener(
                                                move |view, _: &ClickEvent, _window, cx| {
                                                    view.dismiss_scan_modal(cx);
                                                },
                                            )),
                                    )
                            })
                            .child(
                                gpui_ui_kit::Button::new("scan-done", "Done")
                                    .variant(gpui_ui_kit::ButtonVariant::Primary)
                                    .size(gpui_ui_kit::ButtonSize::Md)
                                    .disabled(!is_complete)
                                    .theme(theme.to_button_theme())
                                    .build()
                                    .on_click(cx.listener(
                                        move |view, _: &ClickEvent, _window, cx| {
                                            view.close_scan_modal(cx);
                                        },
                                    )),
                            ),
                    ),
            )
            .into_any_element()
    }

    /// Cancel the active scan
    fn cancel_scan(&mut self, scan_type: crate::app::types::ScanType, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            match scan_type {
                crate::app::types::ScanType::Library => {
                    state.app.cancel_library_scan();
                }
                crate::app::types::ScanType::ReplayGain => {
                    state.app.replay_gain_manager.stop();
                }
                crate::app::types::ScanType::Bliss => {
                    state.app.bliss_manager.stop();
                }
                crate::app::types::ScanType::Waveform => {
                    state.app.waveform_manager.stop();
                }
            }
            state.app.scan_progress_modal = None;
        });
        cx.notify();
    }

    /// Dismiss the scan modal but keep the scan running in background
    fn dismiss_scan_modal(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            if let Some(modal) = &mut state.app.scan_progress_modal {
                modal.visible = false;
            }
        });
        cx.notify();
    }

    /// Close the scan modal completely (used when scan is done)
    fn close_scan_modal(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.scan_progress_modal = None;
        });
        cx.notify();
    }
}
