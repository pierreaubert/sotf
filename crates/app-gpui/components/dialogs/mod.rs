//! Dialog and modal rendering components

mod tutorial;

use crate::app::Screen;
use crate::components::icons::{Icon, IconName};
use crate::components::plugins::editing::PluginEditingManager;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Badge, BadgeVariant, Dialog, DialogSize, HStack, StackAlign, StackJustify, StackSize,
    StackSpacing, Text, TextSize, TextWeight, ToastVariant, VStack,
};

impl PlayerView {
    pub(crate) fn render_help_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let screen_name = match state.app.ui_state.current_screen {
            Screen::Library => "Library",
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
        let keybindings = get_keybindings_for_screen(state.app.ui_state.current_screen);

        Dialog::new("help-modal")
            .title(format!("Keyboard Shortcuts - {} Screen", screen_name))
            .size(DialogSize::Full)
            .content(
                VStack::new()
                    .spacing(StackSpacing::Xs)
                    // Global keybindings section
                    .child(
                        Text::new("GLOBAL KEYBINDINGS")
                            .size(TextSize::Md)
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
                    .child(self.render_keybinding_row("?", "Show keyboard shortcuts", &theme))
                    .child(self.render_keybinding_row("Shift-?", "Show help & support", &theme))
                    .child(div().h_4()) // Spacer
                    // Screen-specific keybindings section
                    .child(
                        Text::new(format!("{} KEYBINDINGS", screen_name.to_uppercase()))
                            .size(TextSize::Md)
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
        let theme = state.app.ui_state.theme.clone();

        Dialog::new("about-dialog")
            .title("About SotF Player")
            .size(DialogSize::Sm)
            .on_close({
                let state = self.state.clone();
                move |_window, cx| {
                    // Defer state update to avoid re-entrant update issues
                    let state = state.clone();
                    cx.defer(move |cx| {
                        state.update(cx, |state, _| {
                            state.app.ui_state.input_mode = crate::app::InputMode::Normal;
                        });
                    });
                }
            })
            .content(
                VStack::new()
                    .spacing(StackSpacing::Md)
                    .align(StackAlign::Center)
                    .child(
                        div()
                            .w(rems(8.0))
                            .h(rems(8.0))
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
                            .spacing(StackSpacing::Xs)
                            .align(StackAlign::Center)
                            .child(
                                Text::new("SotF Player")
                                    .size(TextSize::Lg)
                                    .weight(TextWeight::Bold)
                                    .color(theme.text_primary),
                            )
                            .child(
                                Text::new(format!("Version {}", env!("CARGO_PKG_VERSION")))
                                    .size(TextSize::Xs)
                                    .color(theme.text_secondary),
                            )
                            .child(
                                Text::new("© 2026 Spinorama")
                                    .size(TextSize::Xs)
                                    .color(theme.text_muted),
                            ),
                    )
                    .child(div().w_full().h(px(1.0)).bg(theme.border))
                    .child(
                        VStack::new()
                            .spacing(StackSpacing::Xs)
                            .width(StackSize::Full)
                            .child(self.render_external_link(
                                "📦",
                                "GitHub Repository",
                                "Source code and documentation",
                                "https://github.com/pierreaubert/sotf",
                                &theme,
                            ))
                            .child(self.render_external_link(
                                "🐛",
                                "Report Issues",
                                "Bug tracker",
                                "https://github.com/pierreaubert/sotf/discussions/116",
                                &theme,
                            ))
                            .child(self.render_external_link(
                                "💬",
                                "Feature Requests",
                                "GitHub Discussions",
                                "https://github.com/pierreaubert/sotf/discussions/117",
                                &theme,
                            ))
                            .child(self.render_external_link(
                                "🔊",
                                "Community Forum",
                                "Audio Science Review",
                                "https://www.audiosciencereview.com/forum/index.php?threads/autoeq-for-speaker-and-headphone.66460/",
                                &theme,
                            ))
                            .child(self.render_external_link(
                                "⚖️",
                                "License (GPL v3)",
                                "Open Source License",
                                "https://github.com/pierreaubert/sotf/blob/main/LICENCE.md",
                                &theme,
                            )),
                    ),
            )
            .footer(
                HStack::new()
                    .width(StackSize::Full)
                    .justify(StackJustify::SpaceBetween)
                    .child(
                        Text::new("Press ESC to close")
                            .size(TextSize::Xs)
                            .muted(true),
                    )
                    .child(
                        gpui_ui_kit::Button::new("about-close", "Close")
                            .variant(gpui_ui_kit::ButtonVariant::Primary)
                            .size(gpui_ui_kit::ButtonSize::Xs)
                            .theme(theme.to_button_theme())
                            .build()
                            .on_click(cx.listener(|view, _event: &ClickEvent, _window, cx| {
                                // Defer state update to avoid re-entrant update issues
                                let state = view.state.clone();
                                cx.defer(move |cx| {
                                    state.update(cx, |state, _| {
                                        state.app.ui_state.input_mode = crate::app::InputMode::Normal;
                                    });
                                });
                            })),
                    ),
            )
    }

    fn render_external_link(
        &self,
        icon: &str,
        title: &str,
        subtitle: &str,
        url: &str,
        theme: &crate::theme::Theme,
    ) -> impl IntoElement {
        let url = url.to_string();
        let theme = theme.clone();
        let id = SharedString::from(format!(
            "external-link-{}",
            title.replace(' ', "-").to_lowercase()
        ));
        div()
            .id(id)
            .flex()
            .items_center()
            .gap_3()
            .p_2()
            .w_full()
            .rounded_md()
            .bg(theme.surface_hover)
            .cursor_pointer()
            .hover(move |s| s.bg(theme.accent_muted))
            .child(Text::new(icon.to_string()).size(TextSize::Md))
            .child(
                VStack::new()
                    .spacing(StackSpacing::Xs)
                    .child(
                        Text::new(title.to_string())
                            .size(TextSize::Xs)
                            .weight(TextWeight::Semibold)
                            .color(theme.text_primary),
                    )
                    .child(
                        Text::new(subtitle.to_string())
                            .size(TextSize::Xs)
                            .color(theme.text_secondary),
                    ),
            )
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                cx.open_url(&url);
            })
    }

    pub(crate) fn render_help_support_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();

        Dialog::new("help-support-dialog")
            .title("Help & Support")
            .size(DialogSize::Sm)
            .content(
                VStack::new()
                    .spacing(StackSpacing::Md)
                    .align(StackAlign::Center)
                    // Links section
                    .child(
                        VStack::new()
                            .spacing(StackSpacing::Sm)
                            .align(StackAlign::Start)
                            .child(
                                div()
                                    .id("link-new-features")
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .p_3()
                                    .w_full()
                                    .rounded_md()
                                    .bg(theme.surface_hover)
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.accent_muted))
                                    .child(Text::new("🚀").size(TextSize::Lg))
                                    .child(
                                        VStack::new()
                                            .spacing(StackSpacing::Xs)
                                            .child(
                                                Text::new("Request New Features")
                                                    .size(TextSize::Sm)
                                                    .weight(TextWeight::Semibold)
                                                    .color(theme.text_primary),
                                            )
                                            .child(
                                                Text::new("Share your ideas for new features")
                                                    .size(TextSize::Xs)
                                                    .color(theme.text_secondary),
                                            ),
                                    )
                                    .on_mouse_up(MouseButton::Left, |_, _window, cx| {
                                        cx.open_url(
                                            "https://github.com/pierreaubert/sotf/discussions/117",
                                        );
                                    }),
                            )
                            .child(
                                div()
                                    .id("link-report-bugs")
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .p_3()
                                    .w_full()
                                    .rounded_md()
                                    .bg(theme.surface_hover)
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.accent_muted))
                                    .child(Text::new("🐛").size(TextSize::Lg))
                                    .child(
                                        VStack::new()
                                            .spacing(StackSpacing::Xs)
                                            .child(
                                                Text::new("Report Bugs")
                                                    .size(TextSize::Sm)
                                                    .weight(TextWeight::Semibold)
                                                    .color(theme.text_primary),
                                            )
                                            .child(
                                                Text::new("Help us fix issues you encounter")
                                                    .size(TextSize::Xs)
                                                    .color(theme.text_secondary),
                                            ),
                                    )
                                    .on_mouse_up(MouseButton::Left, |_, _window, cx| {
                                        cx.open_url(
                                            "https://github.com/pierreaubert/sotf/discussions/116",
                                        );
                                    }),
                            )
                            .child(
                                div()
                                    .id("link-github")
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .p_3()
                                    .w_full()
                                    .rounded_md()
                                    .bg(theme.surface_hover)
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.accent_muted))
                                    .child(Text::new("📦").size(TextSize::Lg))
                                    .child(
                                        VStack::new()
                                            .spacing(StackSpacing::Xs)
                                            .child(
                                                Text::new("GitHub Repository")
                                                    .size(TextSize::Sm)
                                                    .weight(TextWeight::Semibold)
                                                    .color(theme.text_primary),
                                            )
                                            .child(
                                                Text::new("View source code and documentation")
                                                    .size(TextSize::Xs)
                                                    .color(theme.text_secondary),
                                            ),
                                    )
                                    .on_mouse_up(MouseButton::Left, |_, _window, cx| {
                                        cx.open_url("https://github.com/pierreaubert/sotf");
                                    }),
                            ),
                    ),
            )
            .footer(
                Text::new("Press ESC or Shift-? to close")
                    .size(TextSize::Xs)
                    .muted(true),
            )
    }

    /// Render modal for empty library prompt shown on startup
    pub(crate) fn render_empty_library_prompt(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();

        Dialog::new("empty-library-prompt")
            .title("Welcome to SotF Player")
            .size(DialogSize::Sm)
            .content(
                VStack::new()
                    .spacing(StackSpacing::Md)
                    .align(StackAlign::Center)
                    .child(
                        div()
                            .w(rems(5.0))
                            .h(rems(5.0))
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
                                Text::new("Your music library is empty")
                                    .size(TextSize::Md)
                                    .weight(TextWeight::Semibold)
                                    .color(theme.text_primary),
                            )
                            .child(
                                Text::new("Would you like to add some music folders to scan?")
                                    .size(TextSize::Sm)
                                    .color(theme.text_secondary),
                            ),
                    )
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Sm)
                            .justify(StackJustify::Center)
                            .child(
                                div()
                                    .id("empty-library-no")
                                    .px_4()
                                    .py_2()
                                    .rounded_md()
                                    .bg(theme.surface_hover)
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.border))
                                    .child(
                                        Text::new("Not Now")
                                            .size(TextSize::Sm)
                                            .color(theme.text_secondary),
                                    )
                                    .on_click({
                                        let state = self.state.clone();
                                        move |_, _window, cx| {
                                            state.update(cx, |state, _| {
                                                state.app.ui_state.input_mode =
                                                    crate::app::InputMode::Normal;
                                            });
                                        }
                                    }),
                            )
                            .child(
                                div()
                                    .id("empty-library-yes")
                                    .px_4()
                                    .py_2()
                                    .rounded_md()
                                    .bg(theme.accent)
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.accent_muted))
                                    .child(
                                        Text::new("Add Music Folders")
                                            .size(TextSize::Sm)
                                            .weight(TextWeight::Semibold)
                                            .color(theme.text_on_accent),
                                    )
                                    .on_click({
                                        let state = self.state.clone();
                                        move |_, _window, cx| {
                                            state.update(cx, |state, _| {
                                                // Navigate to Settings > Library tab
                                                state.app.ui_state.input_mode =
                                                    crate::app::InputMode::Normal;
                                                state.app.ui_state.current_screen =
                                                    Screen::Settings;
                                                state.app.ui_state.active_settings_tab =
                                                    crate::app::SettingsTab::Library;
                                            });
                                        }
                                    }),
                            ),
                    ),
            )
            .footer(
                Text::new("Press ESC to skip")
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
            .spacing(StackSpacing::Sm)
            .child(
                div()
                    .w(Rems(12.0))
                    .child(Badge::new(key.to_string()).variant(BadgeVariant::Primary)),
            )
            .child(
                Text::new(description.to_string())
                    .size(TextSize::Xs)
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
        let theme = state.app.ui_state.theme.clone();

        let toast_data = state.app.ui_state.toast_message.as_ref().map(|t| {
            let variant = match t.toast_type {
                crate::app::ToastType::Success => ToastVariant::Success,
                crate::app::ToastType::Error => ToastVariant::Error,
                crate::app::ToastType::Info => ToastVariant::Info,
                crate::app::ToastType::Warning => ToastVariant::Warning,
            };
            (t.message.clone(), variant)
        });

        if let Some((message, variant)) = toast_data {
            let (bg_color, border_color, text_color) = match variant {
                ToastVariant::Success => (theme.toast_success_bg, theme.success, theme.success),
                ToastVariant::Error => (theme.toast_error_bg, theme.error, theme.error),
                ToastVariant::Info => (theme.toast_info_bg, theme.info, theme.info),
                ToastVariant::Warning => (theme.toast_warning_bg, theme.warning, theme.warning),
            };

            div()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .flex()
                .flex_col()
                .items_center()
                .p_4()
                .child(
                    div()
                        .max_w_96()
                        .rounded_lg()
                        .bg(bg_color)
                        .border_1()
                        .border_color(border_color)
                        .p_3()
                        .flex()
                        .items_start()
                        .gap_2()
                        .child(
                            div()
                                .flex_1()
                                .child(
                                    Text::new(message.clone())
                                        .size(TextSize::Sm)
                                        .color(text_color),
                                ),
                        )
                        .child(
                            div()
                                .id("toast-dismiss")
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .justify_center()
                                .w(px(24.0))
                                .h(px(24.0))
                                .rounded_md()
                                .hover(move |s| {
                                    s.bg(Theme::with_opacity(text_color, 0.15))
                                })
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                        view.state.update(cx, |state, _cx| {
                                            state.app.ui_state.toast_message = None;
                                        });
                                        cx.notify();
                                    }),
                                )
                                .child(
                                    Icon::new(IconName::X)
                                        .color(text_color)
                                        .size(crate::components::icons::IconSize::Sm),
                                ),
                        ),
                )
        } else {
            div()
        }
    }

    pub(crate) fn render_context_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();

        if let Some(menu) = &state.app.ui_state.context_menu {
            let menu_type = menu.menu_type.clone();
            let item_idx = menu.item_index;

            let items = match menu.menu_type {
                crate::app::ContextMenuType::Album => vec![
                    gpui_ui_kit::MenuItem::new("add-to-queue", "Add to Queue"),
                    gpui_ui_kit::MenuItem::new("play-now", "Play Now"),
                ],
                crate::app::ContextMenuType::QueueItem => vec![
                    gpui_ui_kit::MenuItem::new("remove-from-queue", "Remove from Queue"),
                    gpui_ui_kit::MenuItem::new("play-from-here", "Play from Here"),
                ],
                crate::app::ContextMenuType::Plugin => vec![
                    gpui_ui_kit::MenuItem::new("toggle-enabled", "Toggle Enabled"),
                    gpui_ui_kit::MenuItem::new("move-up", "Move Up"),
                    gpui_ui_kit::MenuItem::new("move-down", "Move Down"),
                    gpui_ui_kit::MenuItem::separator(),
                    gpui_ui_kit::MenuItem::new("remove-plugin", "Remove Plugin").danger(),
                ],
                crate::app::ContextMenuType::Directory => vec![
                    gpui_ui_kit::MenuItem::new("remove-directory", "Remove Directory").danger(),
                    gpui_ui_kit::MenuItem::new("rescan-library", "Rescan Library"),
                ],
            };

            let state_entity = self.state.clone();

            div()
                .absolute()
                .left(px(menu.position_x))
                .top(px(menu.position_y))
                .child(
                    gpui_ui_kit::Menu::new("context-menu", items)
                        .theme(theme.to_menu_theme())
                        .on_select(move |id, _window, cx| {
                            let id = id.clone();
                            let menu_type = menu_type.clone();
                            state_entity.update(cx, |state, _cx| {
                                state.app.ui_state.context_menu = None;
                                state.app.ui_state.input_mode = crate::app::InputMode::Normal;
                                match (menu_type, id.as_ref()) {
                                    (crate::app::ContextMenuType::Album, "add-to-queue") => {
                                        if let Some(path) = state.app.add_album_to_queue() {
                                            Self::play_track(state, path);
                                        }
                                    }
                                    (crate::app::ContextMenuType::Album, "play-now") => {
                                        if let Some(path) = state.app.play_album_now() {
                                            Self::play_track(state, path);
                                        }
                                    }
                                    (crate::app::ContextMenuType::QueueItem, "remove-from-queue") => {
                                        state.app.remove_from_queue(item_idx);
                                    }
                                    (crate::app::ContextMenuType::QueueItem, "play-from-here") => {
                                        state.app.playback.current_queue_index = Some(item_idx);
                                        if let Some(queue_item) = state.app.queue.get(item_idx)
                                            && let Some(first_track) =
                                                queue_item.album.tracks.first()
                                        {
                                            Self::play_track(state, first_track.path.clone());
                                        }
                                    }
                                    (crate::app::ContextMenuType::Plugin, "toggle-enabled") => {
                                        state.app.toggle_plugin(item_idx);
                                    }
                                    (crate::app::ContextMenuType::Plugin, "move-up") => {
                                        state.app.move_plugin_up(item_idx);
                                    }
                                    (crate::app::ContextMenuType::Plugin, "move-down") => {
                                        state.app.move_plugin_down(item_idx);
                                    }
                                    (crate::app::ContextMenuType::Plugin, "remove-plugin") => {
                                        state.app.remove_plugin(item_idx);
                                    }
                                    _ => {}
                                }
                            });
                        }),
                )
        } else {
            div()
        }
    }

    pub(crate) fn render_apo_file_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();

        Dialog::new("apo-file-dialog")
            .title("Load APO File for EQ Plugin")
            .size(DialogSize::Md)
            .content(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(
                        Text::new("Enter path to APO file:")
                            .size(TextSize::Xs)
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
                                Text::new(format!("{}█", state.app.input_state.apo_file_input))
                                    .size(TextSize::Xs),
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
        let theme = state.app.ui_state.theme.clone();

        Dialog::new("sofa-file-dialog")
            .title("Load SOFA File for Binaural Decoder")
            .size(DialogSize::Md)
            .content(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(
                        Text::new("Enter path to SOFA file:")
                            .size(TextSize::Xs)
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
                                Text::new(format!("{}█", state.app.input_state.sofa_file_input))
                                    .size(TextSize::Xs),
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
        let theme = state.app.ui_state.theme.clone();
        let presets = state.app.plugin_state.available_presets.clone();
        let selected_preset = state.app.plugin_state.selected_preset_index;
        let input = state.app.input_state.plugin_file_input.clone();

        Dialog::new("save-plugins-dialog")
            .title("Save Plugin Preset")
            .size(DialogSize::Lg)
            .content(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(
                        Text::new("Enter preset name (or select existing to overwrite):")
                            .size(TextSize::Xs)
                            .muted(true),
                    )
                    .child(
                        div()
                            .p_2()
                            .rounded_md()
                            .bg(theme.surface)
                            .border_1()
                            .border_color(theme.accent)
                            .child(Text::new(format!("{}█", input)).size(TextSize::Xs)),
                    )
                    // Show existing presets if available
                    .when(!presets.is_empty(), |el| {
                        el.child(
                            Text::new("Existing presets (↑/↓ to select):")
                                .size(TextSize::Xs)
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
                                    let state_click = self.state.clone();
                                    let hover_bg = theme.surface_hover;
                                    div()
                                        .id(("save-preset-item", idx))
                                        .p_1()
                                        .rounded_md()
                                        .text_sm()
                                        .cursor_pointer()
                                        .when(is_selected, |d| {
                                            d.bg(theme.accent_muted).text_color(theme.text_primary)
                                        })
                                        .when(!is_selected, |d| {
                                            d.text_color(theme.text_secondary)
                                                .hover(move |s| s.bg(hover_bg))
                                        })
                                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                            state_click.update(cx, |state, _cx| {
                                                state.app.plugin_state.selected_preset_index = idx;
                                                state.app.input_state.plugin_file_input.clear();
                                            });
                                        })
                                        .child(preset.clone())
                                })),
                        )
                    }),
            )
            .footer(
                Text::new("Enter: Save | Click/↑/↓: Select preset | Tab: Autocomplete | ESC: Cancel")
                    .size(TextSize::Xs)
                    .muted(true),
            )
    }

    pub(crate) fn render_load_plugins_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let presets = state.app.plugin_state.available_presets.clone();
        let selected_preset = state.app.plugin_state.selected_preset_index;
        let input = state.app.input_state.plugin_file_input.clone();

        Dialog::new("load-plugins-dialog")
            .title("Load Plugin Preset")
            .size(DialogSize::Lg)
            .content(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(
                        Text::new("Enter preset name or select from list:")
                            .size(TextSize::Xs)
                            .muted(true),
                    )
                    .child(
                        div()
                            .p_2()
                            .rounded_md()
                            .bg(theme.surface)
                            .border_1()
                            .border_color(theme.success)
                            .child(Text::new(format!("{}█", input)).size(TextSize::Xs)),
                    )
                    // Show existing presets
                    .when(!presets.is_empty(), |el| {
                        el.child(
                            Text::new("Available presets (↑/↓ to select):")
                                .size(TextSize::Xs)
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
                                    let state_click = self.state.clone();
                                    let hover_bg = theme.surface_hover;
                                    div()
                                        .id(("load-preset-item", idx))
                                        .p_1()
                                        .rounded_md()
                                        .text_sm()
                                        .cursor_pointer()
                                        .when(is_selected, |d| {
                                            d.bg(theme.accent_muted).text_color(theme.text_primary)
                                        })
                                        .when(!is_selected, |d| {
                                            d.text_color(theme.text_secondary)
                                                .hover(move |s| s.bg(hover_bg))
                                        })
                                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                            state_click.update(cx, |state, _cx| {
                                                state.app.plugin_state.selected_preset_index = idx;
                                                state.app.input_state.plugin_file_input.clear();
                                                state.app.load_selected_preset();
                                                state.app.ui_state.input_mode =
                                                    crate::app::InputMode::Normal;
                                                state.app.clear_autocomplete();
                                            });
                                        })
                                        .child(preset.clone())
                                })),
                        )
                    })
                    .when(presets.is_empty(), |el| {
                        el.child(
                            div().p_4().text_center().child(
                                Text::new("No presets found. Save a preset first with 's'.")
                                    .size(TextSize::Xs)
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
        let theme = self.state.read(cx).app.ui_state.theme.clone();

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

        Dialog::new("shortcuts-dialog")
            .title("Keyboard Shortcuts")
            .size(DialogSize::Full)
            .show_close_button(true)
            .on_close({
                let state = self.state.clone();
                move |_window, cx| {
                    state.update(cx, |state, _| {
                        state.app.ui_state.input_mode = crate::app::InputMode::Normal;
                    });
                }
            })
            .content(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_6()
                    .child(self.render_shortcut_section("Global", &global_shortcuts, &theme))
                    .child(self.render_shortcut_section("Library", &library_shortcuts, &theme))
                    .child(self.render_shortcut_section("Queue", &queue_shortcuts, &theme))
                    .child(self.render_shortcut_section("Plugins", &plugin_shortcuts, &theme)),
            )
            .footer(
                HStack::new()
                    .width(StackSize::Full)
                    .justify(StackJustify::SpaceBetween)
                    .child(
                        Text::new("Press ESC or ? to close")
                            .size(TextSize::Xs)
                            .muted(true),
                    )
                    .child(
                        gpui_ui_kit::Button::new("shortcuts-close", "Close")
                            .variant(gpui_ui_kit::ButtonVariant::Primary)
                            .size(gpui_ui_kit::ButtonSize::Xs)
                            .theme(theme.to_button_theme())
                            .build()
                            .on_click(cx.listener(|view, _event: &ClickEvent, _window, cx| {
                                view.state.update(cx, |state, _cx| {
                                    state.app.ui_state.input_mode = crate::app::InputMode::Normal;
                                });
                                cx.notify();
                            })),
                    ),
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
                    .size(TextSize::Xs)
                    .weight(TextWeight::Semibold)
                    .color(theme.accent),
            )
            .children(shortcuts.iter().map(|(key, desc)| {
                HStack::new()
                    .spacing(StackSpacing::Sm)
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
            .min_w(rems(16.25))
    }

    /// Render the scan progress modal
    pub(crate) fn render_scan_progress_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();

        // Get the active modal info
        let modal = match &state.app.scan_progress_modal {
            Some(m) if m.visible => m.clone(),
            _ => return div().into_any_element(),
        };

        // Get progress info based on scan type
        let (progress, processed, total, succeeded, failed) = match modal.scan_type {
            crate::app::types::ScanType::Library => {
                let albums = state.app.library_state.scan_progress_albums;
                let tracks = state.app.library_state.scan_progress_tracks;
                // For library scan, we don't have a total count upfront
                (0.0, tracks, 0usize, albums, 0usize)
            }
            crate::app::types::ScanType::ReplayGain => {
                let mgr = &state.app.scan_ctrl.replay_gain_manager;
                (
                    mgr.progress(),
                    mgr.processed,
                    mgr.total,
                    mgr.succeeded,
                    mgr.failed,
                )
            }
            crate::app::types::ScanType::Bliss => {
                let mgr = &state.app.scan_ctrl.bliss_manager;
                (
                    mgr.progress(),
                    mgr.processed,
                    mgr.total,
                    mgr.succeeded,
                    mgr.failed,
                )
            }
            crate::app::types::ScanType::Waveform => {
                let mgr = &state.app.scan_ctrl.waveform_manager;
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
            crate::app::types::ScanType::Library => !state.app.library_state.scan_in_progress,
            crate::app::types::ScanType::ReplayGain => {
                !state.app.scan_ctrl.replay_gain_manager.in_progress
            }
            crate::app::types::ScanType::Bliss => !state.app.scan_ctrl.bliss_manager.in_progress,
            crate::app::types::ScanType::Waveform => {
                !state.app.scan_ctrl.waveform_manager.in_progress
            }
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
            .bg(theme.overlay_bg)
            .child(
                div()
                    .w(rems(25.0))
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
                            .size(TextSize::Md)
                            .weight(TextWeight::Bold)
                            .color(theme.text_primary),
                    )
                    // Description
                    .child(
                        Text::new(scan_type.description())
                            .size(TextSize::Xs)
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
                            .size(TextSize::Xs)
                            .color(theme.text_muted),
                    )
                    // Buttons
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Sm)
                            .justify(StackJustify::End)
                            .when(!is_complete, |stack| {
                                stack
                                    .child(
                                        gpui_ui_kit::Button::new("scan-cancel", "Cancel")
                                            .variant(gpui_ui_kit::ButtonVariant::Secondary)
                                            .size(gpui_ui_kit::ButtonSize::Sm)
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
                                            .size(gpui_ui_kit::ButtonSize::Sm)
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
                                    .size(gpui_ui_kit::ButtonSize::Sm)
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
                    state.app.scan_ctrl.replay_gain_manager.stop();
                }
                crate::app::types::ScanType::Bliss => {
                    state.app.scan_ctrl.bliss_manager.stop();
                }
                crate::app::types::ScanType::Waveform => {
                    state.app.scan_ctrl.waveform_manager.stop();
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

    pub(crate) fn render_channel_conflict_dialog(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let track_channels = state.app.channel_conflict_track_channels;
        let conflict_names: Vec<String> = state
            .app
            .channel_conflicts
            .iter()
            .map(|c| {
                format!(
                    "{} (requires {}ch, got {}ch)",
                    c.plugin_type.name(),
                    c.required_channels,
                    c.actual_channels
                )
            })
            .collect();

        Dialog::new("channel-conflict-dialog")
            .title("Channel Conflict")
            .size(DialogSize::Sm)
            .on_close({
                let state_entity = self.state.clone();
                move |_window, cx| {
                    let state_entity = state_entity.clone();
                    cx.defer(move |cx| {
                        state_entity.update(cx, |state, _| {
                            state.app.channel_conflict_path = None;
                            state.app.channel_conflicts.clear();
                            state.app.ui_state.input_mode = crate::app::InputMode::Normal;
                            state.app.playback.is_playing = false;
                        });
                    });
                }
            })
            .content(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(
                        Text::new(format!(
                            "This track has {} channels but the following plugins are incompatible:",
                            track_channels
                        ))
                        .size(TextSize::Sm)
                        .color(theme.text_primary),
                    )
                    .children(conflict_names.iter().map(|name| {
                        Text::new(format!("  {}", name))
                            .size(TextSize::Sm)
                            .color(theme.warning)
                            .into_any_element()
                    }))
                    .child(div().h_2())
                    .child(
                        VStack::new()
                            .spacing(StackSpacing::Xs)
                            .width(StackSize::Full)
                            .child(
                                gpui_ui_kit::Button::new(
                                    "conflict-suspend",
                                    "Suspend incompatible and play",
                                )
                                .variant(gpui_ui_kit::ButtonVariant::Primary)
                                .size(gpui_ui_kit::ButtonSize::Sm)
                                .full_width(true)
                                .on_click({
                                    let state_entity = self.state.clone();
                                    move |_event, cx| {
                                        let state_entity = state_entity.clone();
                                        cx.defer(move |cx| {
                                            state_entity.update(cx, |state, _| {
                                                let conflicts =
                                                    std::mem::take(&mut state.app.channel_conflicts);
                                                let indices: Vec<usize> =
                                                    conflicts.iter().map(|c| c.index).collect();
                                                state
                                                    .app
                                                    .plugin_state
                                                    .chain
                                                    .suspend_plugins(&indices);
                                                state
                                                    .app
                                                    .plugin_state
                                                    .chain
                                                    .update_channel_dependent_plugins();
                                                state.app.ui_state.input_mode =
                                                    crate::app::InputMode::Normal;
                                            });
                                            // Play the pending track
                                            let path = state_entity
                                                .update(cx, |state, _| {
                                                    state.app.channel_conflict_path.take()
                                                });
                                            if let Some(path) = path {
                                                state_entity.update(cx, |state, _| {
                                                    PlayerView::play_track(state, path);
                                                });
                                            }
                                        });
                                    }
                                }),
                            )
                            .child(
                                gpui_ui_kit::Button::new(
                                    "conflict-remove",
                                    "Remove incompatible and play",
                                )
                                .variant(gpui_ui_kit::ButtonVariant::Destructive)
                                .size(gpui_ui_kit::ButtonSize::Sm)
                                .full_width(true)
                                .on_click({
                                    let state_entity = self.state.clone();
                                    move |_event, cx| {
                                        let state_entity = state_entity.clone();
                                        cx.defer(move |cx| {
                                            state_entity.update(cx, |state, _| {
                                                let conflicts =
                                                    std::mem::take(&mut state.app.channel_conflicts);
                                                let mut indices: Vec<usize> =
                                                    conflicts.iter().map(|c| c.index).collect();
                                                indices.sort_unstable_by(|a, b| b.cmp(a));
                                                for idx in &indices {
                                                    state
                                                        .app
                                                        .plugin_state
                                                        .chain
                                                        .remove_plugin(*idx);
                                                }
                                                state.app.ui_state.input_mode =
                                                    crate::app::InputMode::Normal;
                                            });
                                            let path = state_entity
                                                .update(cx, |state, _| {
                                                    state.app.channel_conflict_path.take()
                                                });
                                            if let Some(path) = path {
                                                state_entity.update(cx, |state, _| {
                                                    PlayerView::play_track(state, path);
                                                });
                                            }
                                        });
                                    }
                                }),
                            )
                            .child(
                                gpui_ui_kit::Button::new("conflict-cancel", "Cancel playback")
                                    .variant(gpui_ui_kit::ButtonVariant::Ghost)
                                    .size(gpui_ui_kit::ButtonSize::Sm)
                                    .full_width(true)
                                    .on_click({
                                        let state_entity = self.state.clone();
                                        move |_event, cx| {
                                            let state_entity = state_entity.clone();
                                            cx.defer(move |cx| {
                                                state_entity.update(cx, |state, _| {
                                                    state.app.channel_conflict_path = None;
                                                    state.app.channel_conflicts.clear();
                                                    state.app.ui_state.input_mode =
                                                        crate::app::InputMode::Normal;
                                                    state.app.playback.is_playing = false;
                                                });
                                            });
                                        }
                                    }),
                            ),
                    ),
            )
    }
}
