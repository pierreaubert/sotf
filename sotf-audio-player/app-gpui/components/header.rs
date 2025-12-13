//! Header component rendering with dropdown menus

use crate::app::{ActiveMenu, LayoutMode, Screen};
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    HStack, LoadingDots, Menu, MenuItem, StackSpacing, TabItem, TabVariant, Tabs, menu_bar_button,
};

impl PlayerView {
    /// Render the application menu bar with dropdown menus
    pub(crate) fn render_menu_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (theme, active_menu, scan_in_progress, scan_progress_tracks, translations) = {
            let state = self.state.read(cx);
            (
                state.app.theme.clone(),
                state.app.active_menu,
                state.app.scan_in_progress,
                state.app.scan_progress_tracks,
                state.app.translations.clone(),
            )
        };

        HStack::new()
            .spacing(StackSpacing::None)
            .justify(gpui_ui_kit::StackJustify::SpaceBetween)
            .child(
                // Left side: menus only (no title)
                HStack::new()
                    .spacing(StackSpacing::Xs)
                    .child(self.render_menu_button(
                        translations.menu_file,
                        ActiveMenu::File,
                        active_menu,
                        theme.clone(),
                        cx,
                    ))
                    .child(self.render_menu_button(
                        translations.menu_view,
                        ActiveMenu::View,
                        active_menu,
                        theme.clone(),
                        cx,
                    ))
                    .child(self.render_menu_button(
                        translations.menu_help,
                        ActiveMenu::Help,
                        active_menu,
                        theme.clone(),
                        cx,
                    )),
            )
            .child(
                // Right side: Quick status with loading indicator
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .text_color(theme.text_muted)
                    .when(scan_in_progress, |el| {
                        el.child(LoadingDots::new().color(theme.accent))
                            .child(format!(
                                "{}: {} files",
                                translations.library_scanning,
                                scan_progress_tracks
                            ))
                    }),
            )
            .build()
            .px_4()
            .py_1()
            .bg(theme.background_secondary)
            .border_b_1()
            .border_color(theme.border)
    }

    /// Render the dropdown menus overlay (called separately for z-ordering)
    pub(crate) fn render_menu_dropdowns(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (theme, active_menu) = {
            let state = self.state.read(cx);
            (state.app.theme.clone(), state.app.active_menu)
        };

        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .when(active_menu != ActiveMenu::None, |el| {
                el.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|view, _: &MouseDownEvent, _window, cx| {
                        view.state.update(cx, |state, _cx| {
                            state.app.active_menu = ActiveMenu::None;
                        });
                        cx.notify();
                    }),
                )
            })
            .when(active_menu == ActiveMenu::File, |el| {
                el.child(self.render_file_dropdown(theme.clone(), cx))
            })
            .when(active_menu == ActiveMenu::View, |el| {
                el.child(self.render_view_dropdown(theme.clone(), cx))
            })
            .when(active_menu == ActiveMenu::Help, |el| {
                el.child(self.render_help_dropdown(theme.clone(), cx))
            })
    }

    /// Render a single menu button using menu_bar_button from ui_kit
    fn render_menu_button(
        &self,
        label: &'static str,
        menu_id: ActiveMenu,
        active_menu: ActiveMenu,
        _theme: Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_open = active_menu == menu_id;

        let menu_theme = _theme.to_menu_theme();
        menu_bar_button(format!("menu-{}", label), label, is_open, &menu_theme).on_mouse_up(
            MouseButton::Left,
            cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                view.state.update(cx, |state, _cx| {
                    if state.app.active_menu == menu_id {
                        state.app.active_menu = ActiveMenu::None;
                    } else {
                        state.app.active_menu = menu_id;
                    }
                });
                cx.notify();
            }),
        )
    }

    /// Render File menu dropdown
    fn render_file_dropdown(&self, theme: Theme, cx: &mut Context<Self>) -> impl IntoElement {
        // Create a weak reference to self for the callback
        let state = self.state.clone();
        let translations = self.state.read(cx).app.translations.clone();

        Menu::new(vec![
            MenuItem::new("settings", translations.screen_settings).with_shortcut("⌘,"),
            MenuItem::separator(),
            MenuItem::new("quit", translations.menu_quit)
                .with_shortcut("⌘Q")
                .danger(),
        ])
        .theme(theme.to_menu_theme())
        .on_select(move |id, window, cx| {
            state.update(cx, |state, _cx| {
                state.app.active_menu = ActiveMenu::None;
            });
            match id.as_ref() {
                "settings" => {
                    state.update(cx, |state, _cx| {
                        state.app.current_screen = Screen::Settings;
                    });
                }
                "quit" => {
                    window.remove_window();
                }
                _ => {}
            }
        })
        .build_with_theme(&theme.to_menu_theme())
        .absolute()
        .top(px(28.0))
        .left(px(16.0))
    }

    /// Render View menu dropdown
    fn render_view_dropdown(&self, theme: Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let app_state = self.state.read(cx);
        let layout_mode = app_state.app.layout_mode;
        let translations = app_state.app.translations.clone();
        drop(app_state);

        let state = self.state.clone();

        let layout_label = format!(
            "Layout: {}",
            match layout_mode {
                LayoutMode::Compact => "Compact",
                LayoutMode::Expanded => "Expanded",
            }
        );

        Menu::new(vec![
            MenuItem::new("library", translations.screen_library).with_shortcut("1"),
            MenuItem::new("queue", translations.screen_queue).with_shortcut("2"),
            MenuItem::separator(),
            MenuItem::new("settings", translations.screen_settings).with_shortcut("3"),
            MenuItem::separator(),
            MenuItem::new("layout-info", layout_label).disabled(true),
        ])
        .theme(theme.to_menu_theme())
        .on_select(move |id, _window, cx| {
            state.update(cx, |state, _cx| {
                state.app.active_menu = ActiveMenu::None;
                match id.as_ref() {
                    "library" => state.app.current_screen = Screen::Library,
                    "queue" => state.app.current_screen = Screen::Queue,
                    "settings" => state.app.current_screen = Screen::Settings,
                    _ => {}
                }
            });
        })
        .build_with_theme(&theme.to_menu_theme())
        .absolute()
        .top(px(28.0))
        .left(px(52.0))
    }

    /// Render Help menu dropdown
    fn render_help_dropdown(&self, theme: Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.clone();
        let translations = self.state.read(cx).app.translations.clone();

        Menu::new(vec![
            MenuItem::new("shortcuts", translations.menu_keyboard_shortcuts).with_shortcut("?"),
            MenuItem::separator(),
            MenuItem::new("about", translations.menu_about),
        ])
        .theme(theme.to_menu_theme())
        .on_select(move |id, _window, cx| {
            state.update(cx, |state, _cx| {
                state.app.active_menu = ActiveMenu::None;
                match id.as_ref() {
                    "shortcuts" => {
                        state.app.input_mode = crate::app::InputMode::KeyboardShortcuts;
                    }
                    "about" => {
                        state.app.input_mode = crate::app::InputMode::About;
                    }
                    _ => {}
                }
            });
        })
        .build_with_theme(&theme.to_menu_theme())
        .absolute()
        .top(px(28.0))
        .left(px(96.0))
    }

    /// Render the tab bar header (for compact mode)
    pub(crate) fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (theme, layout_mode, current_screen, translations) = {
            let state = self.state.read(cx);
            (
                state.app.theme.clone(),
                state.app.layout_mode,
                state.app.current_screen,
                state.app.translations.clone(),
            )
        };

        // Only show tabs in compact mode
        if layout_mode == LayoutMode::Expanded {
            return div().into_any_element();
        }

        // Map screen to tab index
        let selected_index = match current_screen {
            Screen::Library => 0,
            Screen::Queue => 1,
            Screen::Settings
            | Screen::DirectoryManager
            | Screen::Spectrum
            | Screen::Recording
            | Screen::RoomEq
            | Screen::HeadphoneEq => 0, // Default to library for other screens
        };

        // Get a weak handle for the state to use in the callback
        let state_handle = self.state.downgrade();

        div()
            .flex()
            .items_center()
            .justify_between()
            .p_4()
            .bg(theme.surface)
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::BOLD)
                    .child(translations.app_title),
            )
            .child(
                Tabs::new()
                    .tabs(vec![
                        TabItem::new("library", translations.screen_library),
                        TabItem::new("queue", translations.screen_queue),
                    ])
                    .selected_index(selected_index)
                    .variant(TabVariant::Pills)
                    .theme(theme.to_tabs_theme())
                    .on_change(move |index, _window, cx| {
                        let screen = match index {
                            0 => Screen::Library,
                            1 => Screen::Queue,
                            _ => Screen::Library,
                        };
                        if let Some(state) = state_handle.upgrade() {
                            state.update(cx, |state, _cx| {
                                state.app.current_screen = screen;
                            });
                        }
                    }),
            )
            .into_any_element()
    }
}
