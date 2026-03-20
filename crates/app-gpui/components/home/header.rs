//! Header component rendering with dropdown menus

use crate::app::actions::QuitApp;
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
                state.app.ui_state.theme.clone(),
                state.app.ui_state.active_menu,
                state.app.library_state.scan_in_progress,
                state.app.library_state.scan_progress_tracks,
                state.app.ui_state.translations.clone(),
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
                        translations.menu_show,
                        ActiveMenu::Show,
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
                                translations.library_scanning, scan_progress_tracks
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
            (
                state.app.ui_state.theme.clone(),
                state.app.ui_state.active_menu,
            )
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
                            state.app.ui_state.active_menu = ActiveMenu::None;
                        });
                        cx.notify();
                    }),
                )
            })
            .when(active_menu == ActiveMenu::File, |el| {
                el.child(self.render_file_dropdown(theme.clone(), cx))
            })
            .when(active_menu == ActiveMenu::Show, |el| {
                el.child(self.render_show_dropdown(theme.clone(), cx))
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
                    if state.app.ui_state.active_menu == menu_id {
                        state.app.ui_state.active_menu = ActiveMenu::None;
                    } else {
                        state.app.ui_state.active_menu = menu_id;
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
        let translations = self.state.read(cx).app.ui_state.translations.clone();

        Menu::new(
            "file-menu",
            vec![
                MenuItem::new("settings", translations.screen_settings).with_shortcut("⌘,"),
                MenuItem::separator(),
                MenuItem::new("quit", translations.menu_quit)
                    .with_shortcut("⌘Q")
                    .danger(),
            ],
        )
        .theme(theme.to_menu_theme())
        .on_select(move |id, window, cx| {
            state.update(cx, |state, _cx| {
                state.app.ui_state.active_menu = ActiveMenu::None;
            });
            match id.as_ref() {
                "settings" => {
                    state.update(cx, |state, _cx| {
                        state.app.ui_state.current_screen = Screen::Settings;
                    });
                }
                "quit" => {
                    // Dispatch QuitApp action to properly save config and quit
                    window.dispatch_action(Box::new(QuitApp), cx);
                }
                _ => {}
            }
        })
        .build_with_theme(&theme.to_menu_theme())
        .absolute()
        .top(rems(1.75))
        .left(rems(1.0))
    }

    /// Render View menu dropdown
    fn render_show_dropdown(&self, theme: Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let app_state = self.state.read(cx);
        let translations = app_state.app.ui_state.translations.clone();
        let channel = app_state.app.ui_state.release_channel;
        let _ = app_state;

        let state = self.state.clone();

        Menu::new(
            "view-menu",
            vec![
                MenuItem::new("studio", translations.screen_studio)
                    .with_shortcut("⌘1")
                    .disabled(!channel.allows(Screen::Studio.maturity())),
                MenuItem::new("recording", translations.screen_recording).with_shortcut("⌘2"),
                MenuItem::new("roomeq", translations.screen_room_eq)
                    .with_shortcut("⌘3")
                    .disabled(!channel.allows(Screen::RoomEq.maturity())),
                MenuItem::new("headphoneeq", translations.screen_headphone_eq).with_shortcut("⌘4"),
                MenuItem::new("spinorama", translations.screen_spinorama).with_shortcut("⌘5"),
                MenuItem::separator(),
                MenuItem::new("library", translations.screen_library).with_shortcut("⌘0"),
                MenuItem::separator(),
                MenuItem::new("queue", translations.screen_queue),
                MenuItem::new("settings", translations.screen_settings),
            ],
        )
        .theme(theme.to_menu_theme())
        .on_select(move |id, _window, cx| {
            state.update(cx, |state, _cx| {
                state.app.ui_state.active_menu = ActiveMenu::None;
                match id.as_ref() {
                    "studio" => state.app.ui_state.current_screen = Screen::Studio,
                    "recording" => state.app.ui_state.current_screen = Screen::Recording,
                    "roomeq" => state.app.ui_state.current_screen = Screen::RoomEq,
                    "headphoneeq" => state.app.ui_state.current_screen = Screen::HeadphoneEq,
                    "spinorama" => state.app.ui_state.current_screen = Screen::Spinorama,
                    "library" => state.app.ui_state.current_screen = Screen::Library,
                    "queue" => state.app.ui_state.current_screen = Screen::Queue,
                    "settings" => state.app.ui_state.current_screen = Screen::Settings,
                    _ => {}
                }
            });
        })
        .build_with_theme(&theme.to_menu_theme())
        .absolute()
        .top(rems(1.75))
        .left(rems(3.25))
    }

    /// Render Help menu dropdown
    fn render_help_dropdown(&self, theme: Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.clone();
        let translations = self.state.read(cx).app.ui_state.translations.clone();

        Menu::new(
            "help-menu",
            vec![
                MenuItem::new("screen-guide", "Screen Guide").with_shortcut("F1"),
                MenuItem::new("shortcuts", translations.menu_keyboard_shortcuts).with_shortcut("?"),
                MenuItem::separator(),
                MenuItem::new("tutorial", "Show Tutorial"),
                MenuItem::separator(),
                MenuItem::new("about", translations.menu_about),
            ],
        )
        .theme(theme.to_menu_theme())
        .on_select(move |id, _window, cx| {
            state.update(cx, |state, _cx| {
                state.app.ui_state.active_menu = ActiveMenu::None;
                match id.as_ref() {
                    "screen-guide" => {
                        state.app.ui_state.input_mode = crate::app::InputMode::ScreenGuide;
                    }
                    "shortcuts" => {
                        state.app.ui_state.input_mode = crate::app::InputMode::KeyboardShortcuts;
                    }
                    "tutorial" => {
                        state.app.ui_state.input_mode = crate::app::InputMode::Tutorial;
                        state.app.ui_state.tutorial_screen = 0;
                    }
                    "about" => {
                        state.app.ui_state.input_mode = crate::app::InputMode::About;
                    }
                    _ => {}
                }
            });
        })
        .build_with_theme(&theme.to_menu_theme())
        .absolute()
        .top(rems(1.75))
        .left(rems(6.0))
    }

    /// Render the tab bar header (for compact mode)
    pub(crate) fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (theme, layout_mode, current_screen, translations) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state.app.ui_state.layout_mode,
                state.app.ui_state.current_screen,
                state.app.ui_state.translations.clone(),
            )
        };

        // Only show tabs in compact mode
        if layout_mode == LayoutMode::Expanded {
            return div().into_any_element();
        }

        let screens = [Screen::Library, Screen::Queue];
        let selected_index = screens
            .iter()
            .position(|s| *s == current_screen)
            .unwrap_or(0);

        let state_entity = self.state.clone();

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
                Tabs::new("nav-tabs")
                    .tabs(vec![
                        TabItem::new("library", translations.screen_library),
                        TabItem::new("queue", translations.screen_queue),
                    ])
                    .selected_index(selected_index)
                    .variant(TabVariant::Pills)
                    .theme(theme.to_tabs_theme())
                    .on_change(move |index, _window, cx| {
                        if let Some(screen) = screens.get(index).copied() {
                            state_entity.update(cx, |state, _cx| {
                                state.app.ui_state.current_screen = screen;
                            });
                        }
                    }),
            )
            .into_any_element()
    }
}
