//! Dialog and modal rendering components

pub mod tutorial;

use crate::app::Screen;
use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName};
use crate::components::plugins::editing::PluginEditingManager;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Badge, BadgeVariant, Dialog, DialogSize, HStack, Heading, Spinner, SpinnerSize, StackAlign,
    StackJustify, StackSize, StackSpacing, Text, TextSize, TextWeight, ToastVariant, VStack,
};
use sotf_audio_player::QueuePlaybackEffect;

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

    fn render_external_link(
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
        Screen::Playlists => vec![
            ("↑/↓ or K/J", "Navigate playlists"),
            ("Enter", "Open playlist"),
            ("D/Delete", "Remove playlist"),
        ],
    }
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

    fn render_shortcut_section(
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

    /// Render the scan progress modal
    pub(crate) fn render_scan_progress_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
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

        // Library scans don't have an upfront total, so we can't render a
        // determinate progress bar — show an indeterminate Spinner instead
        // (see the conditional `.child` on the progress widget below). For
        // other scan types we have a real fraction; clamp it.
        let show_indeterminate = !is_complete && is_library_scan;
        let progress_width = if is_complete {
            100.0
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
                    .rounded(d.r_lg)
                    .shadow_lg()
                    .p(d.section_lg)
                    .flex()
                    .flex_col()
                    .gap(d.section)
                    // Title
                    .child(Heading::h4(scan_type.title()))
                    // Description
                    .child(
                        Text::new(scan_type.description())
                            .size(TextSize::Xs)
                            .color(theme.text_secondary),
                    )
                    // Progress indicator: an indeterminate Spinner for
                    // library scans (no upfront total), or a determinate
                    // bar driven by `progress_width` for the other scan
                    // types where we have a real fraction.
                    .child(if show_indeterminate {
                        div()
                            .flex()
                            .justify_center()
                            .child(Spinner::new().size(SpinnerSize::Md))
                            .into_any_element()
                    } else {
                        div()
                            .w_full()
                            .h(rems(0.5))
                            .bg(theme.background_secondary)
                            .rounded_full()
                            .overflow_hidden()
                            .child(
                                div()
                                    .h_full()
                                    // Calculate width as a fraction of the container
                                    // 400px modal - 48px padding (24px each side) = 352px container
                                    .w(px(352.0 * (progress_width / 100.0))) // intentional: progress-bar width computed from runtime percentage
                                    .bg(theme.accent)
                                    .rounded_full(),
                            )
                            .into_any_element()
                    })
                    // Status text
                    .child(Text::caption(status_text))
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
                                            .on_click_event(cx.listener(
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
                                            .on_click_event(cx.listener(
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
                                    .on_click_event(cx.listener(
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
                                                        .graph
                                                        .remove_plugin_by_index(*idx).ok();
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
