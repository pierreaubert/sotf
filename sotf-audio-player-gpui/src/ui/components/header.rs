//! Header component rendering with dropdown menus

use crate::app::{ActiveMenu, LayoutMode, Screen};
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui_ui_kit::{menu_bar_button, Button, ButtonVariant, ButtonSize, HStack, VStack, StackSpacing, Divider};
use gpui::prelude::*;
use gpui::*;

impl PlayerView {
    /// Render the application menu bar with dropdown menus
    pub(crate) fn render_menu_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (theme, active_menu, scan_in_progress, scan_progress_tracks) = {
            let state = self.state.read(cx);
            (
                state.app.theme.clone(),
                state.app.active_menu,
                state.app.scan_in_progress,
                state.app.scan_progress_tracks,
            )
        };

        HStack::new()
            .spacing(StackSpacing::None)
            .justify(gpui_ui_kit::StackJustify::SpaceBetween)
            .child(
                // Left side: menus only (no title)
                HStack::new()
                    .spacing(StackSpacing::Xs)
                    .child(self.render_menu_button("File", ActiveMenu::File, active_menu, theme.clone(), cx))
                    .child(self.render_menu_button("View", ActiveMenu::View, active_menu, theme.clone(), cx))
                    .child(self.render_menu_button("Help", ActiveMenu::Help, active_menu, theme.clone(), cx))
            )
            .child(
                // Right side: Quick status
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .text_color(theme.text_muted)
                    .when(scan_in_progress, |el| {
                        el.child(format!("Scanning: {} files", scan_progress_tracks))
                    })
            )
            .build()
            .px_4()
            .py_1()
            .bg(rgb(0x2a2a2a))
            .border_b_1()
            .border_color(rgb(0x3a3a3a))
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

        menu_bar_button(format!("menu-{}", label), label, is_open)
            .on_mouse_up(
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
        VStack::new()
            .spacing(StackSpacing::None)
            .child(self.render_menu_item_simple("Settings", Some("⌘,"), theme.clone(), Screen::Settings, cx))
            .child(Divider::new())
            .child(self.render_quit_item(theme, cx))
            .build()
            .absolute()
            .top(px(28.0))
            .left(px(16.0))
            .min_w(px(180.0))
            .bg(rgb(0x2a2a2a))
            .border_1()
            .border_color(rgb(0x444444))
            .rounded(px(4.0))
            .shadow_lg()
            .py_1()
    }

    /// Render View menu dropdown
    fn render_view_dropdown(&self, theme: Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let layout_mode = self.state.read(cx).app.layout_mode;

        VStack::new()
            .spacing(StackSpacing::None)
            .child(self.render_menu_item_simple("Library", Some("1"), theme.clone(), Screen::Library, cx))
            .child(self.render_menu_item_simple("Queue", Some("2"), theme.clone(), Screen::Queue, cx))
            .child(Divider::new())
            .child(self.render_menu_item_simple("Settings", Some("3"), theme.clone(), Screen::Settings, cx))
            .child(Divider::new())
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_sm()
                    .text_color(theme.text_muted)
                    .child(format!(
                        "Layout: {}",
                        match layout_mode {
                            LayoutMode::Compact => "Compact",
                            LayoutMode::Expanded => "Expanded",
                        }
                    ))
            )
            .build()
            .absolute()
            .top(px(28.0))
            .left(px(52.0))
            .min_w(px(180.0))
            .bg(rgb(0x2a2a2a))
            .border_1()
            .border_color(rgb(0x444444))
            .rounded(px(4.0))
            .shadow_lg()
            .py_1()
    }

    /// Render Help menu dropdown
    fn render_help_dropdown(&self, theme: Theme, cx: &mut Context<Self>) -> impl IntoElement {
        VStack::new()
            .spacing(StackSpacing::None)
            .child(self.render_help_item(theme.clone(), cx))
            .child(Divider::new())
            .child(self.render_about_item(theme, cx))
            .build()
            .absolute()
            .top(px(28.0))
            .left(px(96.0))
            .min_w(px(180.0))
            .bg(rgb(0x2a2a2a))
            .border_1()
            .border_color(rgb(0x444444))
            .rounded(px(4.0))
            .shadow_lg()
            .py_1()
    }

    /// Render a simple menu item that navigates to a screen
    fn render_menu_item_simple(
        &self,
        label: &'static str,
        shortcut: Option<&'static str>,
        _theme: Theme,
        screen: Screen,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(SharedString::from(format!("menu-item-{}", label)))
            .px_3()
            .py(px(6.0))
            .mx_1()
            .rounded(px(3.0))
            .flex()
            .justify_between()
            .items_center()
            .cursor_pointer()
            .text_color(rgb(0xcccccc))
            .hover(|style| style.bg(rgb(0x3a3a3a)).text_color(rgb(0xffffff)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                    view.switch_screen(screen, cx);
                    view.state.update(cx, |state, _cx| {
                        state.app.active_menu = ActiveMenu::None;
                    });
                }),
            )
            .child(div().text_sm().child(label))
            .when(shortcut.is_some(), |el| {
                el.child(
                    gpui::div()
                        .text_xs()
                        .text_color(rgb(0x777777))
                        .child(shortcut.unwrap_or("")),
                )
            })
    }

    /// Render help menu item (Keyboard Shortcuts)
    fn render_help_item(&self, _theme: Theme, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("menu-item-shortcuts")
            .px_3()
            .py(px(6.0))
            .mx_1()
            .rounded(px(3.0))
            .flex()
            .justify_between()
            .items_center()
            .cursor_pointer()
            .text_color(rgb(0xcccccc))
            .hover(|style| style.bg(rgb(0x3a3a3a)).text_color(rgb(0xffffff)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, _cx| {
                        state.app.input_mode = crate::app::InputMode::KeyboardShortcuts;
                        state.app.active_menu = ActiveMenu::None;
                    });
                    cx.notify();
                }),
            )
            .child(div().text_sm().child("Keyboard Shortcuts"))
            .child(div().text_xs().text_color(rgb(0x777777)).child("?"))
    }

    /// Render about menu item
    fn render_about_item(&self, _theme: Theme, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("menu-item-about")
            .px_3()
            .py(px(6.0))
            .mx_1()
            .rounded(px(3.0))
            .flex()
            .justify_between()
            .items_center()
            .cursor_pointer()
            .text_color(rgb(0xcccccc))
            .hover(|style| style.bg(rgb(0x3a3a3a)).text_color(rgb(0xffffff)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, _cx| {
                        state.app.input_mode = crate::app::InputMode::About;
                        state.app.active_menu = ActiveMenu::None;
                    });
                    cx.notify();
                }),
            )
            .child(div().text_sm().child("About"))
    }

    /// Render quit menu item
    fn render_quit_item(&self, _theme: Theme, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_3()
            .py(px(6.0))
            .mx_1()
            .rounded(px(3.0))
            .flex()
            .justify_between()
            .items_center()
            .cursor_pointer()
            .text_color(rgb(0xcccccc))
            .hover(|style| style.bg(rgb(0x5a2a2a)).text_color(rgb(0xffffff)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _: &MouseUpEvent, window, cx| {
                    view.quit_app(&crate::actions::QuitApp, window, cx);
                }),
            )
            .child(div().text_sm().child("Quit"))
            .child(div().text_xs().text_color(rgb(0x777777)).child("⌘Q"))
    }

    /// Render a menu separator line (kept for backward compatibility)
    #[allow(dead_code)]
    fn render_menu_separator(&self, _theme: Theme) -> impl IntoElement {
        Divider::new().build().mx_2()
    }

    /// Render the tab bar header (for compact mode)
    pub(crate) fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (theme, layout_mode, current_screen) = {
            let state = self.state.read(cx);
            (
                state.app.theme.clone(),
                state.app.layout_mode,
                state.app.current_screen,
            )
        };

        // Only show tabs in compact mode
        if layout_mode == LayoutMode::Expanded {
            return div().into_any_element();
        }

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
                    .child("SOTF Audio Player"),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.render_tab_button_inner(
                        "Library",
                        Screen::Library,
                        current_screen == Screen::Library,
                        theme.clone(),
                        cx,
                    ))
                    .child(self.render_tab_button_inner(
                        "Queue",
                        Screen::Queue,
                        current_screen == Screen::Queue,
                        theme,
                        cx,
                    )),
            )
            .into_any_element()
    }

    fn render_tab_button_inner(
        &self,
        label: &str,
        screen: Screen,
        is_active: bool,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let label_string = SharedString::from(label.to_string());
        let btn = Button::new(SharedString::from(format!("tab-{}", label)), label_string)
            .variant(if is_active { ButtonVariant::Primary } else { ButtonVariant::Secondary })
            .size(ButtonSize::Md)
            .selected(is_active)
            .theme(theme.to_button_theme())
            .build();

        if is_active {
            btn
        } else {
            btn.on_mouse_up(
                MouseButton::Left,
                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                    view.switch_screen(screen, cx);
                }),
            )
        }
    }

    pub(crate) fn render_tab_button(
        &self,
        label: &str,
        screen: Screen,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_active = self.state.read(cx).app.current_screen == screen;
        self.render_tab_button_inner(label, screen, is_active, theme, cx)
    }
}
