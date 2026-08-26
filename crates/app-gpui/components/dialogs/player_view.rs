use super::misc::get_keybindings_for_screen;
use crate::app::Screen;
use crate::app::i18n::{
    ContextMenuTranslations, DialogTranslations, FileDialogTranslations, KeybindingTranslations,
    MetadataEditorTranslations, RuntimeMessageTranslations,
};
use crate::app::keybindings::{KeybindingCategory, get_documented_keybindings};
use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName};
use crate::components::plugins::editing::PluginEditingManager;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, CommandItem, CommandPalette, Dialog,
    DialogSize, HStack, Input, InputSize, StackAlign, StackJustify, StackSize, StackSpacing, Text,
    TextSize, TextWeight, ToastVariant, VStack,
};
use sotf_audio_player::QueuePlaybackEffect;

impl PlayerView {
    pub(crate) fn render_command_palette(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(palette) = self.command_palette.as_ref() else {
            return div().into_any_element();
        };
        let state = self.state.read(cx);
        let text = DialogTranslations::for_language(state.app.ui_state.language);
        let query = palette.query.clone();
        let selected_index = palette.selected_index;
        let focus_handle = palette.focus_handle.clone();

        let commands = self.command_palette_commands(cx);
        let items = if commands.is_empty() {
            vec![
                CommandItem::new("command-palette-empty", text.command_palette_empty)
                    .disabled(true),
            ]
        } else {
            commands
                .into_iter()
                .map(|command| {
                    CommandItem::new(command.action_name, command.description)
                        .shortcut(command.key)
                        .category(command.category)
                })
                .collect()
        };

        // Search is performed across localized action, category, and shortcut
        // by gpui-keybinding before items reach the component. Keep the
        // component query empty to avoid applying a second label-only filter.
        let query_display = if query.is_empty() {
            text.command_palette_placeholder.to_string()
        } else {
            query
        };
        let highlight_view = cx.entity().clone();
        let select_view = cx.entity().clone();
        let dismiss_view = cx.entity().clone();

        CommandPalette::new("sotf-command-palette", items)
            .placeholder(query_display)
            .query("")
            .selected_index(selected_index)
            .focus_handle(focus_handle)
            .max_visible(12)
            .on_highlight_change(move |index, _window, cx| {
                highlight_view.update(cx, |view, cx| {
                    view.set_command_palette_selection(index);
                    cx.notify();
                });
            })
            .on_select(move |id, window, cx| {
                select_view.update(cx, |view, cx| {
                    view.execute_command_palette_action(id.as_ref(), window, cx);
                });
            })
            .on_dismiss(move |window, cx| {
                dismiss_view.update(cx, |view, cx| {
                    view.close_command_palette(window, cx);
                });
            })
            .into_any_element()
    }

    pub(crate) fn render_help_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let text = DialogTranslations::for_language(state.app.ui_state.language);
        let screen_name = text.screen_name(state.app.ui_state.current_screen);

        // Get keybindings for current screen
        let keybindings = get_keybindings_for_screen(
            state.app.ui_state.current_screen,
            state.app.ui_state.language,
            state.app.ui_state.keymap_preset,
        );

        Dialog::new("help-modal")
            .title(text.keyboard_shortcuts_for(screen_name))
            .size(DialogSize::Full)
            .content(
                VStack::new()
                    .spacing(StackSpacing::Xs)
                    // Global keybindings section
                    .child(Text::section_header(text.global_keybindings).color(theme.accent))
                    .child(self.render_keybinding_row(
                        "Shift-L/Q/P/O/D",
                        text.jump_to_screens,
                        &theme,
                    ))
                    .child(self.render_keybinding_row("+/=", text.increase_volume, &theme))
                    .child(self.render_keybinding_row("-/_", text.decrease_volume, &theme))
                    .child(self.render_keybinding_row("?", text.show_keyboard_shortcuts, &theme))
                    .child(self.render_keybinding_row("Shift-?", text.show_help_support, &theme))
                    .child(div().h_4()) // Spacer
                    // Screen-specific keybindings section
                    .child(
                        Text::section_header(text.screen_keybindings(screen_name))
                            .color(theme.accent),
                    )
                    .children(keybindings.iter().map(|(key, desc)| {
                        self.render_keybinding_row(key, desc, &theme)
                            .into_any_element()
                    })),
            )
            .footer(Text::caption(text.press_escape_or_question_to_close))
    }

    pub(crate) fn render_about_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let text = DialogTranslations::for_language(state.app.ui_state.language);

        Dialog::new("about-dialog")
            .title(text.about.about_title)
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
                            Text::new(text.about.app_name)
                                    .size(TextSize::Lg)
                                    .weight(TextWeight::Bold)
                                    .color(theme.text_primary),
                            )
                            .child(
                            Text::new(text.version(env!("CARGO_PKG_VERSION")))
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
                            text.about.github_repository,
                            text.about.source_code_and_docs,
                                "https://github.com/pierreaubert/sotf",
                                &theme,
                                d,
                            ))
                            .child(self.render_external_link(
                            "🐛",
                            text.about.report_issues,
                            text.about.bug_tracker,
                                "https://github.com/pierreaubert/sotf/discussions/116",
                                &theme,
                                d,
                            ))
                            .child(self.render_external_link(
                            "💬",
                            text.about.feature_requests,
                            text.about.github_discussions,
                                "https://github.com/pierreaubert/sotf/discussions/117",
                                &theme,
                                d,
                            ))
                            .child(self.render_external_link(
                            "🔊",
                            text.about.community_forum,
                            text.about.audio_science_review,
                                "https://www.audiosciencereview.com/forum/index.php?threads/autoeq-for-speaker-and-headphone.66460/",
                                &theme,
                                d,
                            ))
                            .child(self.render_external_link(
                            "⚖️",
                            text.about.license_gpl,
                            text.about.open_source_license,
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
                    .child(Text::caption(text.about.press_escape_to_close))
                    .child(
                        gpui_ui_kit::Button::new("about-close", text.about.close)
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
        let text = DialogTranslations::for_language(state.app.ui_state.language);

        Dialog::new("help-support-dialog")
            .title(text.about.help_support_title)
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
                                text.about.request_new_features,
                                text.about.share_feature_ideas,
                                "https://github.com/pierreaubert/sotf/discussions/117",
                                &theme,
                                d,
                            ))
                            .child(self.render_external_link(
                                "🐛",
                                text.about.report_bugs,
                                text.help_fix_issues,
                                "https://github.com/pierreaubert/sotf/discussions/116",
                                &theme,
                                d,
                            ))
                            .child(self.render_external_link(
                                "📦",
                                text.about.github_repository,
                                text.view_source_and_docs,
                                "https://github.com/pierreaubert/sotf",
                                &theme,
                                d,
                            )),
                    ),
            )
            .footer(Text::caption(text.press_escape_or_help_to_close))
    }

    /// Render modal for empty library prompt shown on startup
    pub(crate) fn render_empty_library_prompt(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let text = DialogTranslations::for_language(state.app.ui_state.language);
        let file_text = FileDialogTranslations::for_language(state.app.ui_state.language);

        Dialog::new("empty-library-prompt")
            .title(text.empty_library_welcome)
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
                            .child(Text::section_header(text.empty_library_title))
                            .child(
                                Text::new(text.empty_library_description)
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
                                        Text::new(text.not_now)
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
                                        Text::new(text.add_music_folders)
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
                                        Text::new(text.add_remote_source)
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
            .footer(Text::caption(file_text.press_escape_to_skip))
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
        let runtime_text = RuntimeMessageTranslations::for_language(state.app.ui_state.language);

        let toast_data = state.app.ui_state.toast_message.as_ref().map(|t| {
            let variant = match t.toast_type {
                crate::app::ToastType::Success => ToastVariant::Success,
                crate::app::ToastType::Error => ToastVariant::Error,
                crate::app::ToastType::Info => ToastVariant::Info,
                crate::app::ToastType::Warning => ToastVariant::Warning,
            };
            (
                runtime_text.translate(&t.message).into_owned(),
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
        let text = ContextMenuTranslations::for_language(state.app.ui_state.language);

        if let Some(menu) = &state.app.ui_state.context_menu {
            let menu_type = menu.menu_type.clone();
            let item_idx = menu.item_index;

            let items = match menu.menu_type {
                crate::app::ContextMenuType::Album => vec![
                    gpui_ui_kit::MenuItem::new("play-now", text.play_now),
                    gpui_ui_kit::MenuItem::new("add-to-queue", text.add_to_queue),
                    gpui_ui_kit::MenuItem::separator(),
                    gpui_ui_kit::MenuItem::new("edit-metadata", text.edit_metadata),
                ],
                crate::app::ContextMenuType::QueueItem => vec![
                    gpui_ui_kit::MenuItem::new("play-from-here", text.play_from_here),
                    gpui_ui_kit::MenuItem::new("remove-from-queue", text.remove_from_queue),
                    gpui_ui_kit::MenuItem::separator(),
                    gpui_ui_kit::MenuItem::new("edit-metadata", text.edit_metadata),
                ],
                crate::app::ContextMenuType::Plugin => vec![
                    gpui_ui_kit::MenuItem::new("toggle-enabled", text.toggle_enabled),
                    gpui_ui_kit::MenuItem::new("move-up", text.move_up),
                    gpui_ui_kit::MenuItem::new("move-down", text.move_down),
                    gpui_ui_kit::MenuItem::separator(),
                    gpui_ui_kit::MenuItem::new("remove-plugin", text.remove_plugin).danger(),
                ],
                crate::app::ContextMenuType::Directory => vec![
                    gpui_ui_kit::MenuItem::new("remove-directory", text.remove_directory).danger(),
                    gpui_ui_kit::MenuItem::new("rescan-library", text.rescan_library),
                ],
            };

            let state_entity = self.state.clone();
            let view_entity = cx.entity().clone();

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
                                                if let Err(e) = state.player.stop() {
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
                            // The PlayerView does not observe AppState, so event handlers that
                            // mutate state must notify the view explicitly to render the update
                            // (e.g. menu closing, queue length changing) immediately.
                            view_entity.update(cx, |_, cx| cx.notify());
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
        let text = DialogTranslations::for_language(state.app.ui_state.language);
        let metadata_text = MetadataEditorTranslations::for_language(state.app.ui_state.language);
        let runtime_text = RuntimeMessageTranslations::for_language(state.app.ui_state.language);
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
            .title(text.edit_metadata)
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
                        Text::caption(metadata_text.target_label(&editor.target_label))
                            .color(theme.text_secondary),
                    )
                    .child(
                        div()
                            .grid()
                            .grid_cols(2)
                            .gap(d.gap)
                            .child(self.render_metadata_input(
                                metadata_text.fields.title,
                                "title",
                                editor.fields.title.clone(),
                                metadata_text.fields.title,
                                cx,
                            ))
                            .child(self.render_metadata_input(
                                metadata_text.fields.artist,
                                "artist",
                                editor.fields.artist.clone(),
                                metadata_text.fields.artist,
                                cx,
                            ))
                            .child(self.render_metadata_input(
                                metadata_text.fields.album_artist,
                                "album_artist",
                                editor.fields.album_artist.clone(),
                                metadata_text.fields.album_artist,
                                cx,
                            ))
                            .child(self.render_metadata_input(
                                metadata_text.fields.year,
                                "year",
                                editor.fields.year.clone(),
                                metadata_text.fields.year_placeholder,
                                cx,
                            ))
                            .child(self.render_metadata_input(
                                metadata_text.fields.genre,
                                "genre",
                                editor.fields.genre.clone(),
                                metadata_text.fields.genre,
                                cx,
                            ))
                            .child(self.render_metadata_input(
                                metadata_text.fields.composer,
                                "composer",
                                editor.fields.composer.clone(),
                                metadata_text.fields.composer,
                                cx,
                            ))
                            .child(self.render_metadata_input(
                                metadata_text.fields.disc,
                                "disc_number",
                                editor.fields.disc_number.clone(),
                                metadata_text.fields.disc,
                                cx,
                            ))
                            .child(self.render_metadata_input(
                                metadata_text.fields.track,
                                "track_number",
                                editor.fields.track_number.clone(),
                                metadata_text.fields.track,
                                cx,
                            ))
                            .child(self.render_metadata_input(
                                metadata_text.fields.conductor,
                                "conductor",
                                editor.fields.conductor.clone(),
                                metadata_text.fields.conductor,
                                cx,
                            ))
                            .child(self.render_metadata_input(
                                metadata_text.fields.performer,
                                "performer",
                                editor.fields.performer.clone(),
                                metadata_text.fields.performer,
                                cx,
                            ))
                            .child(self.render_metadata_input(
                                metadata_text.fields.isrc,
                                "isrc",
                                editor.fields.isrc.clone(),
                                metadata_text.fields.isrc,
                                cx,
                            ))
                            .child(self.render_metadata_input(
                                metadata_text.fields.ensemble,
                                "ensemble",
                                editor.fields.ensemble.clone(),
                                metadata_text.fields.ensemble,
                                cx,
                            ))
                            .child(self.render_metadata_input(
                                metadata_text.fields.edition,
                                "edition",
                                editor.fields.edition.clone(),
                                metadata_text.fields.edition,
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
                            .child(Text::section_header(metadata_text.preview))
                            .when_some(preview.clone(), |el, preview| {
                                el.child(Text::caption(metadata_text.preview_summary(
                                    preview.affected_files.len(),
                                    preview.unsupported_writes.len(),
                                    preview.sidecar_path.is_some(),
                                )))
                                .children(
                                    preview.unsupported_writes.iter().take(3).map(|file| {
                                        let reason = file
                                            .reason
                                            .as_deref()
                                            .map(|reason| {
                                                runtime_text.translate(reason).into_owned()
                                            })
                                            .unwrap_or_else(|| {
                                                metadata_text.tag_writing_unsupported.to_string()
                                            });
                                        Text::caption(format!(
                                            "{}: {}",
                                            file.path.display(),
                                            reason
                                        ))
                                        .color(theme.warning)
                                        .into_any_element()
                                    }),
                                )
                            })
                            .when(preview.is_none(), |el| {
                                el.child(Text::caption(metadata_text.preview_before_applying))
                            })
                            .when_some(editor.error.clone(), |el, error| {
                                el.child(
                                    Text::caption(runtime_text.translate(&error).into_owned())
                                        .color(theme.error),
                                )
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
                                            .placeholder(text.search_musicbrainz)
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
                                                            editor.search_query = value.to_string();
                                                            editor.search_error = None;
                                                        }
                                                    });
                                                }
                                            }),
                                    )
                                    .child(
                                        Button::new(
                                            "metadata-search-musicbrainz",
                                            metadata_text.search_musicbrainz,
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
                                el.child(Text::caption(metadata_text.searching_musicbrainz))
                            })
                            .when_some(editor.search_error.clone(), |el, error| {
                                el.child(
                                    Text::caption(runtime_text.translate(&error).into_owned())
                                        .color(theme.error),
                                )
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
                                            .unwrap_or(metadata_text.untitled),
                                        candidate
                                            .album_artist
                                            .as_deref()
                                            .or(candidate.artist.as_deref())
                                            .unwrap_or(metadata_text.unknown),
                                        candidate
                                            .year
                                            .map(|year| year.to_string())
                                            .unwrap_or_else(|| metadata_text.unknown.to_string())
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
                            metadata_text.unsupported_before_apply
                        } else {
                            metadata_text.backups_created
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
                                Button::new("metadata-preview", metadata_text.preview)
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
                                Button::new("metadata-cancel", metadata_text.cancel)
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
                                Button::new("metadata-apply", metadata_text.apply_changes)
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
                                    "title" => editor.fields.title = value.to_string(),
                                    "artist" => editor.fields.artist = value.to_string(),
                                    "album_artist" => {
                                        editor.fields.album_artist = value.to_string()
                                    }
                                    "year" => editor.fields.year = value.to_string(),
                                    "genre" => editor.fields.genre = value.to_string(),
                                    "composer" => editor.fields.composer = value.to_string(),
                                    "disc_number" => editor.fields.disc_number = value.to_string(),
                                    "track_number" => {
                                        editor.fields.track_number = value.to_string()
                                    }
                                    "conductor" => editor.fields.conductor = value.to_string(),
                                    "performer" => editor.fields.performer = value.to_string(),
                                    "isrc" => editor.fields.isrc = value.to_string(),
                                    "ensemble" => editor.fields.ensemble = value.to_string(),
                                    "edition" => editor.fields.edition = value.to_string(),
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
        let text = DialogTranslations::for_language(state.app.ui_state.language);
        let file_text = FileDialogTranslations::for_language(state.app.ui_state.language);

        Dialog::new("apo-file-dialog")
            .title(text.load_apo)
            .size(DialogSize::Md)
            .content(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::caption(file_text.enter_apo_path))
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
            .footer(Text::caption(file_text.load_or_cancel))
    }

    pub(crate) fn render_sofa_file_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let text = DialogTranslations::for_language(state.app.ui_state.language);
        let file_text = FileDialogTranslations::for_language(state.app.ui_state.language);

        Dialog::new("sofa-file-dialog")
            .title(text.load_sofa)
            .size(DialogSize::Md)
            .content(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::caption(file_text.enter_sofa_path))
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
            .footer(Text::caption(file_text.load_or_cancel))
    }

    pub(crate) fn render_save_plugins_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let text = DialogTranslations::for_language(state.app.ui_state.language);
        let file_text = FileDialogTranslations::for_language(state.app.ui_state.language);
        let presets = state.app.plugin_state.available_presets.clone();
        let selected_preset = state.app.plugin_state.selected_preset_index;
        let input = state.app.input_state.plugin_file_input.clone();

        Dialog::new("save-plugins-dialog")
            .title(text.save_plugin_preset)
            .size(DialogSize::Lg)
            .content(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::caption(file_text.save_name_or_overwrite))
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
                        el.child(Text::caption(file_text.existing_presets)).child(
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
                                            el.bg(theme.accent_muted).text_color(theme.text_primary)
                                        })
                                        .when(!is_selected, |el| {
                                            el.text_color(theme.text_secondary)
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
            .footer(Text::caption(file_text.save_hint))
    }

    pub(crate) fn render_load_plugins_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let text = DialogTranslations::for_language(state.app.ui_state.language);
        let file_text = FileDialogTranslations::for_language(state.app.ui_state.language);
        let presets = state.app.plugin_state.available_presets.clone();
        let selected_preset = state.app.plugin_state.selected_preset_index;
        let input = state.app.input_state.plugin_file_input.clone();

        Dialog::new("load-plugins-dialog")
            .title(text.load_plugin_preset)
            .size(DialogSize::Lg)
            .content(
                VStack::new()
                    .spacing(StackSpacing::Sm)
                    .child(Text::caption(file_text.preset_name_or_select))
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
                        el.child(Text::caption(file_text.available_presets)).child(
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
                                            el.bg(theme.accent_muted).text_color(theme.text_primary)
                                        })
                                        .when(!is_selected, |el| {
                                            el.text_color(theme.text_secondary)
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
                            div()
                                .p(d.card)
                                .text_center()
                                .child(Text::caption(file_text.no_presets_found)),
                        )
                    }),
            )
            .footer(Text::caption(file_text.load_hint))
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
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let text = DialogTranslations::for_language(state.app.ui_state.language);
        let keybinding_text = KeybindingTranslations::for_language(state.app.ui_state.language);
        let documented = get_documented_keybindings(state.app.ui_state.keymap_preset);
        let sections = KeybindingCategory::all()
            .iter()
            .filter_map(|category| {
                let shortcuts = documented
                    .iter()
                    .filter(|binding| binding.category == *category)
                    .map(|binding| {
                        (
                            binding.key.as_str(),
                            keybinding_text.action_description(binding.description),
                        )
                    })
                    .collect::<Vec<_>>();
                (!shortcuts.is_empty())
                    .then_some((keybinding_text.category_name(*category), shortcuts))
            })
            .collect::<Vec<_>>();

        Dialog::new("shortcuts-dialog")
            .title(text.keyboard_shortcuts)
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
                    .children(sections.iter().map(|(category, shortcuts)| {
                        self.render_shortcut_section(category, shortcuts, &theme)
                    })),
            )
            .footer(
                HStack::new()
                    .width(StackSize::Full)
                    .justify(StackJustify::SpaceBetween)
                    .child(Text::caption(text.press_escape_or_question_to_close))
                    .child(
                        gpui_ui_kit::Button::new("shortcuts-close", text.about.close)
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
        let text = DialogTranslations::for_language(state.app.ui_state.language);
        let action_text = ContextMenuTranslations::for_language(state.app.ui_state.language);
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
            .title(text.channel_conflict)
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
                                    action_text.suspend_incompatible_and_play,
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
                                    action_text.remove_incompatible_and_play,
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
                                gpui_ui_kit::Button::new(
                                    "conflict-cancel",
                                    action_text.cancel_playback,
                                )
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
