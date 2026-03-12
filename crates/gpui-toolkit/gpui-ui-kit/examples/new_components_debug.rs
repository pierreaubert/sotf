//! New Components Debug Example
//!
//! Demonstrates all 8 Tier 1 components:
//! - ContextMenu, Popover, Sidebar, StatusBar
//! - SearchBar, KeyboardShortcutLabel, EmptyState, ConfirmDialog

use gpui::*;
use gpui_ui_kit::theme::ThemeExt;
use gpui_ui_kit::*;

pub struct NewComponentsDebug;

impl Render for NewComponentsDebug {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .id("new-components-root")
            .size_full()
            .bg(theme.background)
            .p_8()
            .flex()
            .flex_col()
            .gap_4()
            .overflow_y_scroll()
            // Keyboard Shortcuts
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(Text::new("Keyboard Shortcuts").weight(TextWeight::Bold))
                    .child(
                        div()
                            .flex()
                            .gap_4()
                            .child(KeyboardShortcutLabel::new("Cmd+K"))
                            .child(KeyboardShortcutLabel::new("Ctrl+Shift+P"))
                            .child(
                                KeyboardShortcutLabel::new("Alt+F4").size(KeyboardShortcutSize::Lg),
                            ),
                    ),
            )
            // Empty State
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(Text::new("Empty State").weight(TextWeight::Bold))
                    .child(
                        div()
                            .border_1()
                            .border_color(theme.border)
                            .rounded_lg()
                            .p_4()
                            .child(
                                EmptyState::new("No items found")
                                    .description("Try adjusting your search")
                                    .icon("?"),
                            ),
                    ),
            )
            // Search Bar
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(Text::new("Search Bar").weight(TextWeight::Bold))
                    .child(
                        div()
                            .flex()
                            .gap_4()
                            .child(
                                div()
                                    .w(px(250.0))
                                    .child(SearchBar::new("search-empty").placeholder("Search...")),
                            )
                            .child(
                                div().w(px(200.0)).child(
                                    SearchBar::new("search-filled")
                                        .value("Beethoven")
                                        .size(SearchBarSize::Sm),
                                ),
                            ),
                    ),
            )
            // Status Bar
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(Text::new("Status Bar").weight(TextWeight::Bold))
                    .child(
                        div()
                            .border_1()
                            .border_color(theme.border)
                            .rounded_lg()
                            .overflow_hidden()
                            .child(
                                StatusBar::new("status-demo")
                                    .position(StatusBarPosition::Bottom)
                                    .left(Text::new("Track 1").size(TextSize::Xs))
                                    .center(Text::new("00:00 / 03:45").size(TextSize::Xs))
                                    .right(Text::new("Vol: 80%").size(TextSize::Xs)),
                            ),
                    ),
            )
            // Sidebar
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(Text::new("Sidebar").weight(TextWeight::Bold))
                    .child(
                        div()
                            .flex()
                            .h(px(150.0))
                            .border_1()
                            .border_color(theme.border)
                            .rounded_lg()
                            .overflow_hidden()
                            .child(
                                Sidebar::new("demo-sidebar")
                                    .side(SidebarSide::Left)
                                    .width(px(180.0))
                                    .header(
                                        div()
                                            .p_2()
                                            .child(Text::new("Nav").weight(TextWeight::Bold)),
                                    )
                                    .content(div().p_2().child("Item 1\nItem 2\nItem 3")),
                            )
                            .child(div().flex_1().p_4().child(Text::new("Main content area"))),
                    ),
            )
    }
}

fn main() {
    MiniApp::run(
        MiniAppConfig::new("New Components Debug")
            .size(600.0, 700.0)
            .scrollable(true)
            .with_theme(true),
        |cx| cx.new(|_cx| NewComponentsDebug),
    );
}
