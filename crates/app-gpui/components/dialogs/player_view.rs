use super::misc::get_keybindings_for_screen;
use crate::app::Screen;
use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName};
use crate::components::plugins::editing::PluginEditingManager;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, Dialog, DialogSize, HStack, Input,
    InputSize, StackAlign, StackJustify, StackSize, StackSpacing, Text, TextSize, TextWeight,
    ToastVariant, VStack,
};
use sotf_audio_player::QueuePlaybackEffect;

impl PlayerView {
    pub(crate) fn render_help_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let screen_name = match state.app.ui_state.current_screen {
            Screen::Home => "Home",
            Screen::NowPlaying => "Now Playing",
            Screen::Library => "Library",
            Screen::Streams => "Streams",
            Screen::Queue => "Queue",
            Screen::Spectrum => "Spectrum",
            Screen::Settings => "Settings",
            Screen::Studio => "Studio",
            Screen::Recording => "Recording",
            Screen::RoomEq => "Room EQ",
            Screen::HeadphoneEq => "Headphone EQ",
            Screen::Spinorama => "Spinorama",
            Screen::PluginGraph => "Plugin Graph",
            Screen::Playlists => "Playlists",
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
                    .child(Text::section_header("GLOBAL KEYBINDINGS").color(theme.accent))
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
                        Text::section_header(format!("{} KEYBINDINGS", screen_name.to_uppercase()))
                            .color(theme.accent),
                    )
                    .children(keybindings.iter().map(|(key, desc)| {
                        self.render_keybinding_row(key, desc, &theme)
                            .into_any_element()
                    })),
            )
            .footer(Text::caption("Press ESC or ? to close"))
    }

    pub(crate) fn render_about_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
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
                            .rounded(d.r_xl)
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
                            .child(Text::caption("© 2026 Spinorama")),
                    )
                    .child(div().w_full().h(px(1.0)).bg(theme.border)) // intentional: 1px hairline divider, no matching token
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
                                d,
                            ))
                            .child(self.render_external_link(
                                "🐛",
                                "Report Issues",
                                "Bug tracker",
                                "https://github.com/pierreaubert/sotf/discussions/116",
                                &theme,
                                d,
                            ))
                            .child(self.render_external_link(
                                "💬",
                                "Feature Requests",
                                "GitHub Discussions",
                                "https://github.com/pierreaubert/sotf/discussions/117",
                                &theme,
                                d,
                            ))
                            .child(self.render_external_link(
                                "🔊",
                                "Community Forum",
                                "Audio Science Review",
                                "https://www.audiosciencereview.com/forum/index.php?threads/autoeq-for-speaker-and-headphone.66460/",
                                &theme,
                                d,
                            ))
                            .child(self.render_external_link(
                                "⚖️",
                                "License (GPL v3)",
                                "Open Source License",
                                "https://github.com/pierreaubert/sotf/blob/main/LICENCE.md",
                                &theme,
                                d,
                            )),
                    ),
            )
            .footer(
                HStack::new()
                    .width(StackSize::Full)
                    .justify(StackJustify::SpaceBetween)
                    .child(Text::caption("Press ESC to close"))
                    .child(
                        gpui_ui_kit::Button::new("about-close", "Close")
                            .variant(gpui_ui_kit::ButtonVariant::Primary)
                            .size(gpui_ui_kit::ButtonSize::Xs)
                            .theme(theme.to_button_theme())
                            .on_click_event(cx.listener(|view, _event: &ClickEvent, _window, cx| {
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

    pub(super) fn render_external_link(
        &self,
        icon: &str,
        title: &str,
        subtitle: &str,
        url: &str,
        theme: &crate::theme::Theme,
        d: Ds,
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
            .gap(d.gap_md)
            .p(d.pad_y)
            .w_full()
            .rounded(d.r_md)
            .bg(theme.surface_hover)
            .cursor_pointer()
            .hover(move |s| s.bg(theme.accent_muted))
            .child(Text::new(icon.to_string()).size(TextSize::Md))
            .child(
                VStack::new()
                    .spacing(StackSpacing::Xs)
                    .child(Text::label(title.to_string()).color(theme.text_primary))
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
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();

        Dialog::new("help-support-dialog")
            .title("Help & Support")
            .size(DialogSize::Sm)
            .content(
                VStack::new()
                    .spacing(StackSpacing::Md)
                    .align(StackAlign::Center)
                    // Links section — uses render_external_link for consistency
                    .child(
                        VStack::new()
                            .spacing(StackSpacing::Sm)
                            .align(StackAlign::Start)
                            .child(self.render_external_link(
                                "🚀",
                                "Request New Features",
                                "Share your ideas for new features",
                                "https://github.com/pierreaubert/sotf/discussions/117",
                                &theme,
                                d,
                            ))
                            .child(self.render_external_link(
                                "🐛",
                                "Report Bugs",
                                "Help us fix issues you encounter",
                                "https://github.com/pierreaubert/sotf/discussions/116",
                                &theme,
                                d,
                            ))
                            .child(self.render_external_link(
                                "📦",
                                "GitHub Repository",
                                "View source code and documentation",
                                "https://github.com/pierreaubert/sotf",
                                &theme,
                                d,
                            )),
                    ),
            )
            .footer(Text::caption("Press ESC or Shift-? to close"))
    }

    /// Render modal for empty library prompt shown on startup
    pub(crate) fn render_empty_library_prompt(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();

        Dialog::new("empty-library-prompt")
            .title("Welcome to SotF Player")
            .size(DialogSize::Md)
            .content(
                VStack::new()
                    .spacing(StackSpacing::Md)
                    .align(StackAlign::Center)
                    .child(
                        div()
                            .w(rems(5.0))
                            .h(rems(5.0))
                            .rounded(d.r_xl)
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
                            .child(Text::section_header("Your music library is empty"))
                            .child(
                                Text::new("Would you like to add music folders or remote sources?")
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
                                    .px(d.card)
                                    .py(d.pad_y)
                                    .rounded(d.r_md)
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
                                    .px(d.card)
                                    .py(d.pad_y)
                                    .rounded(d.r_md)
                                    .bg(theme.accent)
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.accent_muted))
                                    // intentional: button caption inside accent-bg div, not a header
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
                            )
                            .child(
                                div()
                                    .id("empty-library-remote")
                                    .px(d.card)
                                    .py(d.pad_y)
                                    .rounded(d.r_md)
                                    .bg(theme.surface_hover)
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.border))
                                    .child(
                                        Text::new("Add Remote Source")
                                            .size(TextSize::Sm)
                                            .color(theme.text_secondary),
                                    )
                                    .on_click({
                                        let state = self.state.clone();
                                        move |_, _window, cx| {
                                            state.update(cx, |state, _| {
                                                // Navigate to Settings > Federation Sources tab
                                                state.app.ui_state.input_mode =
                                                    crate::app::InputMode::Normal;
                                                state.app.ui_state.current_screen =
                                                    Screen::Settings;
                                                state.app.ui_state.active_settings_tab =
                                                    crate::app::SettingsTab::Federation;
                                            });
                                        }
                                    }),
                            ),
                    ),
            )
            .footer(Text::caption("Press ESC to skip"))
    }

    pub(super) fn render_keybinding_row(
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
        d: Ds,
    ) -> impl IntoElement {
        div()
            .flex()
            .gap(d.section)
            .mb(d.grid)
            .child(
                div()
                    .w(Rems(12.0))
                    .text_size(d.text_sm)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.info)
                    .child(format!("  {}", key)),
            )
            .child(
                div()
                    .text_size(d.text_sm)
                    .text_color(theme.text_secondary)
                    .child(description.to_string()),
            )
    }

    pub(crate) fn render_toast(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();

        let toast_data = state.app.ui_state.toast_message.as_ref().map(|t| {
            let variant = match t.toast_type {
                crate::app::ToastType::Success => ToastVariant::Success,
                crate::app::ToastType::Error => ToastVariant::Error,
                crate::app::ToastType::Info => ToastVariant::Info,
                crate::app::ToastType::Warning => ToastVariant::Warning,
            };
            (
                t.message.clone(),
                variant,
                t.action.as_ref().map(|a| a.label.clone()),
            )
        });

        if let Some((message, variant, action_label)) = toast_data {
            let (bg_color, border_color, text_color) = match variant {
                ToastVariant::Success => (
                    theme.feedback.toast_success_bg,
                    theme.success,
                    theme.success,
                ),
                ToastVariant::Error => (theme.feedback.toast_error_bg, theme.error, theme.error),
                ToastVariant::Info => (theme.feedback.toast_info_bg, theme.info, theme.info),
                ToastVariant::Warning => (
                    theme.feedback.toast_warning_bg,
                    theme.warning,
                    theme.warning,
                ),
            };

            div()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .flex()
                .flex_col()
                .items_center()
                .p(d.card)
                .child(
                    div()
                        .max_w_96()
                        .rounded(d.r_lg)
                        .bg(bg_color)
                        .border_1()
                        .border_color(border_color)
                        .p(d.pad_x)
                        .flex()
                        .items_start()
                        .gap(d.gap)
                        .child(
                            div().flex_1().child(
                                Text::new(message.clone())
                                    .size(TextSize::Sm)
                                    .color(text_color),
                            ),
                        )
                        .when_some(action_label, |el, label| {
                            el.child(
                                div()
                                    .id("toast-action")
                                    .cursor_pointer()
                                    .px(d.pad_y)
                                    .py(d.pad_y_half)
                                    .rounded(d.r_md)
                                    .text_size(d.text_xs)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(text_color)
                                    .border_1()
                                    .border_color(text_color)
                                    .hover(move |s| s.bg(Theme::with_opacity(text_color, 0.15)))
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                            let action_id = view
                                                .state
                                                .read(cx)
                                                .app
                                                .ui_state
                                                .toast_message
                                                .as_ref()
                                                .and_then(|t| t.action.as_ref())
                                                .map(|a| a.action_id.clone());
                                            if let Some(id) = action_id {
                                                view.state.update(cx, |state, _cx| {
                                                    state.app.ui_state.toast_message = None;
                                                    state.app.handle_toast_action(&id);
                                                });
                                                cx.notify();
                                            }
                                        }),
                                    )
                                    .child(label),
                            )
                        })
                        .child(
                            div()
                                .id("toast-dismiss")
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .justify_center()
                                .w(rems(1.5))
                                .h(rems(1.5))
                                .rounded(d.r_md)
                                .hover(move |s| s.bg(Theme::with_opacity(text_color, 0.15)))
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
                    gpui_ui_kit::MenuItem::new("play-now", "Play Now"),
                    gpui_ui_kit::MenuItem::new("add-to-queue", "Add to Queue"),
                    gpui_ui_kit::MenuItem::separator(),
                    gpui_ui_kit::MenuItem::new("edit-metadata", "Edit Metadata"),
                ],
                crate::app::ContextMenuType::QueueItem => vec![
                    gpui_ui_kit::MenuItem::new("play-from-here", "Play from Here"),
                    gpui_ui_kit::MenuItem::new("remove-from-queue", "Remove from Queue"),
                    gpui_ui_kit::MenuItem::separator(),
                    gpui_ui_kit::MenuItem::new("edit-metadata", "Edit Metadata"),
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
                                    (crate::app::ContextMenuType::Album, "edit-metadata") => {
                                        if let Some(album) =
                                            state.app.library_state.selected_album().cloned()
                                        {
                                            match crate::app::MetadataEditorState::for_album(&album)
                                            {
                                                Ok(editor) => {
                                                    state.app.modal.metadata_editor = Some(editor);
                                                    state.app.ui_state.input_mode =
                                                        crate::app::InputMode::MetadataEditor;
                                                }
                                                Err(err) => {
                                                    state.app.ui_state.toast_message = Some(
                                                        crate::app::ToastMessage::error(err),
                                                    );
                                                }
                                            }
                                        }
                                    }
                                    (
                                        crate::app::ContextMenuType::QueueItem,
                                        "edit-metadata",
                                    ) => {
                                        if let Some(queue_item) =
                                            state.app.queue_state.get(item_idx)
                                            && let Some(track) = queue_item
                                                .current_track()
                                                .or_else(|| queue_item.album.tracks.first())
                                        {
                                            state.app.modal.metadata_editor =
                                                Some(crate::app::MetadataEditorState::for_track(
                                                    track,
                                                ));
                                            state.app.ui_state.input_mode =
                                                crate::app::InputMode::MetadataEditor;
                                        }
                                    }
                                    (crate::app::ContextMenuType::Album, "add-to-queue") => {
                                        match state.app.add_album_to_queue() {
                                            Ok(Some(path)) => Self::play_track(state, path),
                                            Err(e) => {
                                                state.app.ui_state.toast_message =
                                                    Some(crate::app::ToastMessage::error(e));
                                            }
                                            _ => {}
                                        }
                                    }
                                    (crate::app::ContextMenuType::Album, "play-now") => {
                                        match state.app.play_album_now() {
                                            Ok(Some(path)) => Self::play_track(state, path),
                                            Err(e) => {
                                                state.app.ui_state.toast_message =
                                                    Some(crate::app::ToastMessage::error(e));
                                            }
                                            _ => {}
                                        }
                                    }
                                    (
                                        crate::app::ContextMenuType::QueueItem,
                                        "remove-from-queue",
                                    ) => {
                                        let effect = state.app.remove_from_queue(item_idx);
                                        match effect {
                                            QueuePlaybackEffect::Reload(source)
                                            | QueuePlaybackEffect::Play(source) => {
                                                Self::play_track(state, source);
                                            }
                                            QueuePlaybackEffect::Stop => {
                                                if let Err(e) = state.player.lock().stop() {
                                                    log::warn!(
                                                        "[ContextMenu] Failed to stop player after queue removal: {}",
                                                        e
                                                    );
                                                }
                                            }
                                            QueuePlaybackEffect::None => {}
                                        }
                                    }
                                    (crate::app::ContextMenuType::QueueItem, "play-from-here") => {
                                        state.app.playback.current_queue_index = Some(item_idx);
                                        if let Some(queue_item) =
                                            state.app.queue_state.get(item_idx)
                                            && let Some(first_track) =
                                                queue_item.album.tracks.first()
                                        {
                                            Self::play_track(state, first_track.audio_source());
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

    pub(crate) fn render_metadata_editor_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let Some(editor) = state.app.modal.metadata_editor.clone() else {
            return div().into_any_element();
        };

        let preview = editor.preview.clone();
        let unsupported_count = preview
            .as_ref()
            .map(|preview| preview.unsupported_writes.len())
            .unwrap_or(0);
        let can_apply = preview.as_ref().is_some_and(|preview| preview.can_apply());
        let button_theme = theme.to_button_theme();

        Dialog::new("metadata-editor-dialog")
            .title("Edit Metadata")
            .size(DialogSize::Xl)
            .close_on_backdrop(false)
            .on_close({
                let state = self.state.clone();
                move |_window, cx| {
                    state.update(cx, |state, _cx| {
                        state.app.modal.metadata_editor = None;
                        state.app.ui_state.input_mode = crate::app::InputMode::Normal;
                    });
                }
            })
            .content(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(
                        Text::caption(format!("Target: {}", editor.target_label))
                            .color(theme.text_secondary),
                    )
                    .child(
                        div()
                            .grid()
                            .grid_cols(2)
                            .gap(d.gap)
                            .child(self.render_metadata_input(
                                "Title",
                                "title",
                                editor.fields.title.clone(),
                                "Title",
                                cx,
                            ))
                            .child(self.render_metadata_input(
                                "Artist",
                                "artist",
                                editor.fields.artist.clone(),
                                "Artist",
                                cx,
                            ))
                            .child(self.render_metadata_input(
                                "Album Artist",
                                "album_artist",
                                editor.fields.album_artist.clone(),
                                "Album artist",
                                cx,
                            ))
                            .child(self.render_metadata_input(
                                "Year",
                                "year",
                                editor.fields.year.clone(),
                                "YYYY",
                                cx,
                            ))
                            .child(self.render_metadata_input(
                                "Genre",
                                "genre",
                                editor.fields.genre.clone(),
                                "Genre",
                                cx,
                            ))
                            .child(self.render_metadata_input(
                                "Composer",
                                "composer",
                                editor.fields.composer.clone(),
                                "Composer",
                                cx,
                            ))
                            .child(self.render_metadata_input(
                                "Disc",
                                "disc_number",
                                editor.fields.disc_number.clone(),
                                "Disc",
                                cx,
                            ))
                            .child(self.render_metadata_input(
                                "Track",
                                "track_number",
                                editor.fields.track_number.clone(),
                                "Track",
                                cx,
                            ))
                            .child(self.render_metadata_input(
                                "Conductor",
                                "conductor",
                                editor.fields.conductor.clone(),
                                "Conductor",
                                cx,
                            ))
                            .child(self.render_metadata_input(
                                "Performer",
                                "performer",
                                editor.fields.performer.clone(),
                                "Performer",
                                cx,
                            ))
                            .child(self.render_metadata_input(
                                "ISRC",
                                "isrc",
                                editor.fields.isrc.clone(),
                                "ISRC",
                                cx,
                            ))
                            .child(self.render_metadata_input(
                                "Ensemble",
                                "ensemble",
                                editor.fields.ensemble.clone(),
                                "Ensemble",
                                cx,
                            ))
                            .child(self.render_metadata_input(
                                "Edition",
                                "edition",
                                editor.fields.edition.clone(),
                                "Edition",
                                cx,
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(d.grid)
                            .p(d.pad_x)
                            .rounded(d.r_md)
                            .bg(theme.background_secondary)
                            .border_1()
                            .border_color(theme.border)
                            .child(Text::section_header("Preview"))
                            .when_some(preview.clone(), |el, preview| {
                                el.child(Text::caption(format!(
                                    "{} file(s), {} unsupported, sidecar {}",
                                    preview.affected_files.len(),
                                    preview.unsupported_writes.len(),
                                    if preview.sidecar_path.is_some() {
                                        "yes"
                                    } else {
                                        "no"
                                    }
                                )))
                                .children(
                                    preview.unsupported_writes.iter().take(3).map(|file| {
                                        Text::caption(format!(
                                            "{}: {}",
                                            file.path.display(),
                                            file.reason
                                                .as_deref()
                                                .unwrap_or("tag writing unsupported")
                                        ))
                                        .color(theme.warning)
                                        .into_any_element()
                                    }),
                                )
                            })
                            .when(preview.is_none(), |el| {
                                el.child(Text::caption("Preview before applying changes"))
                            })
                            .when_some(editor.error.clone(), |el, error| {
                                el.child(Text::caption(error).color(theme.error))
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(d.grid)
                            .p(d.pad_x)
                            .rounded(d.r_md)
                            .bg(theme.background_secondary)
                            .border_1()
                            .border_color(theme.border)
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Sm)
                                    .child(
                                        Input::new("musicbrainz-query")
                                            .value(SharedString::from(editor.search_query.clone()))
                                            .placeholder("Search MusicBrainz")
                                            .size(InputSize::Sm)
                                            .bg_color(theme.surface)
                                            .text_color(theme.text_primary)
                                            .placeholder_color(theme.text_muted)
                                            .on_text_change({
                                                let state = self.state.clone();
                                                move |value, _window, cx| {
                                                    state.update(cx, |state, _cx| {
                                                        if let Some(editor) =
                                                            &mut state.app.modal.metadata_editor
                                                        {
                                                            editor.search_query = value;
                                                            editor.search_error = None;
                                                        }
                                                    });
                                                }
                                            }),
                                    )
                                    .child(
                                        Button::new(
                                            "metadata-search-musicbrainz",
                                            "Search MusicBrainz",
                                        )
                                        .variant(ButtonVariant::Secondary)
                                        .size(ButtonSize::Sm)
                                        .disabled(editor.search_in_progress)
                                        .theme(button_theme.clone())
                                        .on_click_event(
                                            cx.listener(|view, _: &ClickEvent, _window, cx| {
                                                view.search_metadata_musicbrainz(cx);
                                            }),
                                        ),
                                    ),
                            )
                            .when(editor.search_in_progress, |el| {
                                el.child(Text::caption("Searching MusicBrainz..."))
                            })
                            .when_some(editor.search_error.clone(), |el, error| {
                                el.child(Text::caption(error).color(theme.error))
                            })
                            .children(editor.search_results.iter().enumerate().map(
                                |(idx, candidate)| {
                                    let selected = idx == editor.selected_result;
                                    let label = format!(
                                        "{}  {} - {} ({})",
                                        candidate.score,
                                        candidate
                                            .album_title
                                            .as_deref()
                                            .or(candidate.title.as_deref())
                                            .unwrap_or("Untitled"),
                                        candidate
                                            .album_artist
                                            .as_deref()
                                            .or(candidate.artist.as_deref())
                                            .unwrap_or("Unknown"),
                                        candidate
                                            .year
                                            .map(|year| year.to_string())
                                            .unwrap_or_else(|| "unknown".to_string())
                                    );
                                    Button::new(
                                        SharedString::from(format!("metadata-candidate-{idx}")),
                                        label,
                                    )
                                    .variant(if selected {
                                        ButtonVariant::Primary
                                    } else {
                                        ButtonVariant::Ghost
                                    })
                                    .size(ButtonSize::Sm)
                                    .theme(button_theme.clone())
                                    .on_click_event(cx.listener(
                                        move |view, _: &ClickEvent, _window, cx| {
                                            view.import_metadata_candidate(idx, cx);
                                        },
                                    ))
                                    .into_any_element()
                                },
                            )),
                    ),
            )
            .footer(
                HStack::new()
                    .width(StackSize::Full)
                    .justify(StackJustify::SpaceBetween)
                    .child(
                        Text::caption(if unsupported_count > 0 {
                            "Unsupported files must be fixed before applying"
                        } else {
                            "Backups are created beside edited files"
                        })
                        .color(if unsupported_count > 0 {
                            theme.warning
                        } else {
                            theme.text_secondary
                        }),
                    )
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Sm)
                            .child(
                                Button::new("metadata-preview", "Preview")
                                    .variant(ButtonVariant::Secondary)
                                    .size(ButtonSize::Sm)
                                    .theme(button_theme.clone())
                                    .on_click_event(cx.listener(
                                        |view, _: &ClickEvent, _window, cx| {
                                            view.refresh_metadata_preview(cx);
                                        },
                                    )),
                            )
                            .child(
                                Button::new("metadata-cancel", "Cancel")
                                    .variant(ButtonVariant::Secondary)
                                    .size(ButtonSize::Sm)
                                    .theme(button_theme.clone())
                                    .on_click_event(cx.listener(
                                        |view, _: &ClickEvent, _window, cx| {
                                            view.close_metadata_editor(cx);
                                        },
                                    )),
                            )
                            .child(
                                Button::new("metadata-apply", "Apply Changes")
                                    .variant(ButtonVariant::Primary)
                                    .size(ButtonSize::Sm)
                                    .disabled(!can_apply)
                                    .theme(button_theme)
                                    .on_click_event(cx.listener(
                                        |view, _: &ClickEvent, _window, cx| {
                                            view.apply_metadata_editor(cx);
                                        },
                                    )),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_metadata_input(
        &self,
        label: &'static str,
        field: &'static str,
        value: String,
        placeholder: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();

        VStack::new()
            .spacing(StackSpacing::Xs)
            .child(Text::label(label))
            .child(
                Input::new(SharedString::from(format!("metadata-field-{field}")))
                    .value(SharedString::from(value))
                    .placeholder(placeholder)
                    .size(InputSize::Sm)
                    .bg_color(theme.surface)
                    .text_color(theme.text_primary)
                    .placeholder_color(theme.text_muted)
                    .on_text_change({
                        let state = self.state.clone();
                        move |value, _window, cx| {
                            state.update(cx, |state, _cx| {
                                let Some(editor) = &mut state.app.modal.metadata_editor else {
                                    return;
                                };
                                match field {
                                    "title" => editor.fields.title = value,
                                    "artist" => editor.fields.artist = value,
                                    "album_artist" => editor.fields.album_artist = value,
                                    "year" => editor.fields.year = value,
                                    "genre" => editor.fields.genre = value,
                                    "composer" => editor.fields.composer = value,
                                    "disc_number" => editor.fields.disc_number = value,
                                    "track_number" => editor.fields.track_number = value,
                                    "conductor" => editor.fields.conductor = value,
                                    "performer" => editor.fields.performer = value,
                                    "isrc" => editor.fields.isrc = value,
                                    "ensemble" => editor.fields.ensemble = value,
                                    "edition" => editor.fields.edition = value,
                                    _ => {}
                                }
                                editor.preview = None;
                                editor.error = None;
                            });
                        }
                    }),
            )
            .into_any_element()
    }

    pub(crate) fn close_metadata_editor(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.modal.metadata_editor = None;
            state.app.ui_state.input_mode = crate::app::InputMode::Normal;
        });
        cx.notify();
    }

    pub(crate) fn refresh_metadata_preview(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            let Some(editor) = state.app.modal.metadata_editor.clone() else {
                return;
            };
            let result = editor.patch().and_then(|patch| {
                state
                    .app
                    .library_state
                    .preview_metadata_edit(editor.target.clone(), patch)
                    .map_err(|err| err.to_string())
            });
            if let Some(current) = &mut state.app.modal.metadata_editor {
                match result {
                    Ok(preview) => {
                        current.preview = Some(preview);
                        current.error = None;
                    }
                    Err(err) => {
                        current.preview = None;
                        current.error = Some(err);
                    }
                }
            }
        });
        cx.notify();
    }

    pub(crate) fn apply_metadata_editor(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            let Some(editor) = state.app.modal.metadata_editor.clone() else {
                return;
            };
            let result = editor.patch().and_then(|patch| {
                state
                    .app
                    .library_state
                    .apply_metadata_edit(editor.target.clone(), patch)
                    .map_err(|err| err.to_string())
            });
            match result {
                Ok(preview) => {
                    state.app.modal.metadata_editor = None;
                    state.app.ui_state.input_mode = crate::app::InputMode::Normal;
                    state.app.ui_state.toast_message =
                        Some(crate::app::ToastMessage::success(format!(
                            "Metadata updated for {} file(s)",
                            preview.affected_files.len()
                        )));
                    state.app.invalidate_library_stats();
                }
                Err(err) => {
                    if let Some(current) = &mut state.app.modal.metadata_editor {
                        current.error = Some(err);
                    }
                }
            }
        });
        cx.notify();
    }

    pub(crate) fn search_metadata_musicbrainz(&mut self, cx: &mut Context<Self>) {
        let (scope, query) = self
            .state
            .update(cx, |state, _cx| {
                let Some(editor) = &mut state.app.modal.metadata_editor else {
                    return None;
                };
                let query = editor.search_query.trim().to_string();
                if query.is_empty() {
                    editor.search_error = Some("Enter a MusicBrainz search query".to_string());
                    return None;
                }
                editor.search_in_progress = true;
                editor.search_error = None;
                Some((editor.scope, query))
            })
            .unwrap_or((crate::app::MetadataEditorScope::Track, String::new()));

        if query.is_empty() {
            cx.notify();
            return;
        }

        let (tx, rx) = smol::channel::bounded(1);
        std::thread::spawn(move || {
            let config = sotf_audio_player::config::load_metadata_services_config()
                .unwrap_or_else(|_| sotf_audio_player::MetadataServicesConfig::default());
            let provider_config = config
                .providers
                .iter()
                .find(|provider| provider.provider_id == "musicbrainz")
                .cloned()
                .unwrap_or_default();
            let result = if !provider_config.enabled {
                Err(sotf_audio_player::MetadataError::Provider(
                    "MusicBrainz is disabled in Metadata settings".to_string(),
                ))
            } else {
                run_musicbrainz_search_on_tokio(
                    scope,
                    query,
                    provider_config.endpoint,
                    config.user_agent,
                )
            };
            let _ = tx.send_blocking(result);
        });

        let state_entity = self.state.clone();
        cx.spawn(async move |_: WeakEntity<PlayerView>, cx| {
            let Ok(result) = rx.recv().await else {
                return;
            };
            state_entity.update(cx, |state, cx| {
                if let Some(editor) = &mut state.app.modal.metadata_editor {
                    editor.search_in_progress = false;
                    match result {
                        Ok(candidates) => {
                            editor.search_results = candidates;
                            editor.selected_result = 0;
                            editor.search_error = None;
                        }
                        Err(err) => {
                            editor.search_results.clear();
                            editor.search_error = Some(err.to_string());
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn import_metadata_candidate(&mut self, index: usize, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            let Some(editor) = &mut state.app.modal.metadata_editor else {
                return;
            };
            let Some(candidate) = editor.search_results.get(index).cloned() else {
                return;
            };
            editor.selected_result = index;
            editor.apply_candidate(candidate);
            editor.preview = None;
        });
        self.refresh_metadata_preview(cx);
    }

    pub(crate) fn render_apo_file_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();

        Dialog::new("apo-file-dialog")
            .title("Load APO File for EQ Plugin")
            .size(DialogSize::Md)
            .content(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::caption("Enter path to APO file:"))
                    .child(
                        div()
                            .p(d.pad_y)
                            .rounded(d.r_md)
                            .bg(theme.surface)
                            .border_1()
                            .border_color(theme.accent)
                            .child(
                                Text::new(format!("{}█", state.app.input_state.apo_file_input))
                                    .size(TextSize::Xs),
                            ),
                    ),
            )
            .footer(Text::caption("Enter: Load file | ESC: Cancel"))
    }

    pub(crate) fn render_sofa_file_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();

        Dialog::new("sofa-file-dialog")
            .title("Load SOFA File for Binaural Decoder")
            .size(DialogSize::Md)
            .content(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::caption("Enter path to SOFA file:"))
                    .child(
                        div()
                            .p(d.pad_y)
                            .rounded(d.r_md)
                            .bg(theme.surface)
                            .border_1()
                            .border_color(theme.accent)
                            .child(
                                Text::new(format!("{}█", state.app.input_state.sofa_file_input))
                                    .size(TextSize::Xs),
                            ),
                    ),
            )
            .footer(Text::caption("Enter: Load file | ESC: Cancel"))
    }

    pub(crate) fn render_save_plugins_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
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
                    .child(Text::caption(
                        "Enter preset name (or select existing to overwrite):",
                    ))
                    .child(
                        div()
                            .p(d.pad_y)
                            .rounded(d.r_md)
                            .bg(theme.surface)
                            .border_1()
                            .border_color(theme.accent)
                            .child(Text::new(format!("{}█", input)).size(TextSize::Xs)),
                    )
                    // Show existing presets if available
                    .when(!presets.is_empty(), |el| {
                        el.child(Text::caption("Existing presets (↑/↓ to select):"))
                            .child(
                                div()
                                    .id("save-plugins-presets-list")
                                    .max_h(Rems(12.0))
                                    .overflow_y_scroll()
                                    .bg(theme.surface)
                                    .rounded(d.r_md)
                                    .p(d.pad_y)
                                    .children(presets.iter().enumerate().map(|(idx, preset)| {
                                        let is_selected = idx == selected_preset;
                                        let theme = theme.clone();
                                        let state_click = self.state.clone();
                                        let hover_bg = theme.surface_hover;
                                        div()
                                            .id(("save-preset-item", idx))
                                            .p(d.grid)
                                            .rounded(d.r_md)
                                            .text_size(d.text_sm)
                                            .cursor_pointer()
                                            .when(is_selected, |el| {
                                                el.bg(theme.accent_muted)
                                                    .text_color(theme.text_primary)
                                            })
                                            .when(!is_selected, |el| {
                                                el.text_color(theme.text_secondary)
                                                    .hover(move |s| s.bg(hover_bg))
                                            })
                                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                                state_click.update(cx, |state, _cx| {
                                                    state.app.plugin_state.selected_preset_index =
                                                        idx;
                                                    state.app.input_state.plugin_file_input.clear();
                                                });
                                            })
                                            .child(preset.clone())
                                    })),
                            )
                    }),
            )
            .footer(Text::caption(
                "Enter: Save | Click/↑/↓: Select preset | Tab: Autocomplete | ESC: Cancel",
            ))
    }

    pub(crate) fn render_load_plugins_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
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
                    .child(Text::caption("Enter preset name or select from list:"))
                    .child(
                        div()
                            .p(d.pad_y)
                            .rounded(d.r_md)
                            .bg(theme.surface)
                            .border_1()
                            .border_color(theme.success)
                            .child(Text::new(format!("{}█", input)).size(TextSize::Xs)),
                    )
                    // Show existing presets
                    .when(!presets.is_empty(), |el| {
                        el.child(Text::caption("Available presets (↑/↓ to select):"))
                            .child(
                                div()
                                    .id("load-plugins-presets-list")
                                    .max_h(Rems(12.0))
                                    .overflow_y_scroll()
                                    .bg(theme.surface)
                                    .rounded(d.r_md)
                                    .p(d.pad_y)
                                    .children(presets.iter().enumerate().map(|(idx, preset)| {
                                        let is_selected = idx == selected_preset;
                                        let theme = theme.clone();
                                        let state_click = self.state.clone();
                                        let hover_bg = theme.surface_hover;
                                        div()
                                            .id(("load-preset-item", idx))
                                            .p(d.grid)
                                            .rounded(d.r_md)
                                            .text_size(d.text_sm)
                                            .cursor_pointer()
                                            .when(is_selected, |el| {
                                                el.bg(theme.accent_muted)
                                                    .text_color(theme.text_primary)
                                            })
                                            .when(!is_selected, |el| {
                                                el.text_color(theme.text_secondary)
                                                    .hover(move |s| s.bg(hover_bg))
                                            })
                                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                                state_click.update(cx, |state, _cx| {
                                                    state.app.plugin_state.selected_preset_index =
                                                        idx;
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
                        el.child(div().p(d.card).text_center().child(Text::caption(
                            "No presets found. Save a preset first with 's'.",
                        )))
                    }),
            )
            .footer(Text::caption(
                "Enter: Load | ↑/↓: Select preset | Tab: Autocomplete | ESC: Cancel",
            ))
    }
}

fn run_musicbrainz_search_on_tokio(
    scope: crate::app::MetadataEditorScope,
    query: String,
    endpoint: String,
    user_agent: String,
) -> Result<Vec<sotf_audio_player::MetadataImportCandidate>, sotf_audio_player::MetadataError> {
    use sotf_audio_player::metadata::MetadataProvider;

    let provider = sotf_audio_player::MusicBrainzProvider::with_endpoint(endpoint, user_agent)?;
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|err| sotf_audio_player::MetadataError::Provider(err.to_string()))?;

    runtime.block_on(async {
        match scope {
            crate::app::MetadataEditorScope::Album => provider.search_album(None, &query).await,
            crate::app::MetadataEditorScope::Track => provider.search_track(None, &query).await,
        }
    })
}

impl PlayerView {
    /// Render a comprehensive keyboard shortcuts dialog using Dialog component
    pub(crate) fn render_keyboard_shortcuts_dialog(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
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
                    .gap(d.section_lg)
                    .child(self.render_shortcut_section("Global", &global_shortcuts, &theme))
                    .child(self.render_shortcut_section("Library", &library_shortcuts, &theme))
                    .child(self.render_shortcut_section("Queue", &queue_shortcuts, &theme))
                    .child(self.render_shortcut_section("Plugins", &plugin_shortcuts, &theme)),
            )
            .footer(
                HStack::new()
                    .width(StackSize::Full)
                    .justify(StackJustify::SpaceBetween)
                    .child(Text::caption("Press ESC or ? to close"))
                    .child(
                        gpui_ui_kit::Button::new("shortcuts-close", "Close")
                            .variant(gpui_ui_kit::ButtonVariant::Primary)
                            .size(gpui_ui_kit::ButtonSize::Xs)
                            .theme(theme.to_button_theme())
                            .on_click_event(cx.listener(
                                |view, _event: &ClickEvent, _window, cx| {
                                    view.state.update(cx, |state, _cx| {
                                        state.app.ui_state.input_mode =
                                            crate::app::InputMode::Normal;
                                    });
                                    cx.notify();
                                },
                            )),
                    ),
            )
    }

    pub(super) fn render_shortcut_section(
        &self,
        title: &str,
        shortcuts: &[(&str, &str)],
        theme: &crate::theme::Theme,
    ) -> impl IntoElement {
        VStack::new()
            .spacing(StackSpacing::Xs)
            .child(Text::label(title.to_string()).color(theme.accent))
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

    pub(crate) fn render_channel_conflict_dialog(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let track_channels = state.app.modal.channel_conflict_track_channels;
        let conflict_names: Vec<String> = state
            .app
            .modal
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
                            state.app.modal.channel_conflict_path = None;
                            state.app.modal.channel_conflicts.clear();
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
                                                    std::mem::take(&mut state.app.modal.channel_conflicts);
                                                let indices: Vec<usize> =
                                                    conflicts.iter().map(|c| c.index).collect();
                                                state
                                                    .app
                                                    .plugin_state
                                                    .graph
                                                    .suspend_plugins(&indices);
                                                state
                                                    .app
                                                    .plugin_state
                                                    .graph
                                                    .update_channel_dependent_plugins();
                                                state.app.ui_state.input_mode =
                                                    crate::app::InputMode::Normal;
                                            });
                                            // Play the pending track
                                            let path = state_entity
                                                .update(cx, |state, _| {
                                                    state.app.modal.channel_conflict_path.take()
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
                                                    std::mem::take(&mut state.app.modal.channel_conflicts);
                                                let mut indices: Vec<usize> =
                                                    conflicts.iter().map(|c| c.index).collect();
                                                indices.sort_unstable_by(|a, b| b.cmp(a));
                                                for idx in &indices {
                                                    state
                                                        .app
                                                        .plugin_state
                                                        .graph
                                                        .remove_plugin_by_index(*idx).ok();
                                                }
                                                state.app.ui_state.input_mode =
                                                    crate::app::InputMode::Normal;
                                            });
                                            let path = state_entity
                                                .update(cx, |state, _| {
                                                    state.app.modal.channel_conflict_path.take()
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
                                                    state.app.modal.channel_conflict_path = None;
                                                    state.app.modal.channel_conflicts.clear();
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
