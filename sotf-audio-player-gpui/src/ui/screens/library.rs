//! Library screen rendering functions

use crate::ui::PlayerView;
use crate::ui::components::album_card::{AlbumCard, AlbumCardMode};
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant};
use std::sync::Arc;

impl PlayerView {
    pub(crate) fn render_library_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (
            albums_count,
            search_query,
            input_mode,
            sort_order,
            channel_filter,
            filtered_count,
            theme,
        ) = {
            let state = self.state.read(cx);
            let filtered_count = state.app.filtered_albums().len();
            (
                state.app.library.albums.len(),
                state.app.search_query.clone(),
                state.app.input_mode,
                state.app.library_sort_order,
                state.app.channel_filter,
                filtered_count,
                state.app.theme.clone(),
            )
        };

        let is_search_mode = input_mode == crate::app::InputMode::Search;

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .mb_4()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(if filtered_count == albums_count {
                                format!("Library ({} albums)", albums_count)
                            } else {
                                format!("Library ({}/{} albums)", filtered_count, albums_count)
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_4()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .bg(theme.surface)
                                    .rounded_md()
                                    .border_1()
                                    .when(is_search_mode, |div| div.border_color(theme.accent))
                                    .when(!is_search_mode, |div| div.border_color(theme.border))
                                    .px_2()
                                    .py_1()
                                    .w_64()
                                    .cursor_pointer()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                            view.state.update(cx, |state, _cx| {
                                                if state.app.input_mode != crate::app::InputMode::Search {
                                                    state.app.input_mode = crate::app::InputMode::Search;
                                                }
                                            });
                                            cx.notify();
                                        }),
                                    )
                                    .child(
                                        div()
                                            .mr_2()
                                            .text_color(if is_search_mode {
                                                theme.accent
                                            } else {
                                                theme.text_secondary
                                            })
                                            .child("🔍"),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(if search_query.is_empty() {
                                                if is_search_mode {
                                                    theme.text_secondary
                                                } else {
                                                    theme.text_muted
                                                }
                                            } else {
                                                theme.text_primary
                                            })
                                            .child(if search_query.is_empty() {
                                                if is_search_mode {
                                                    "Type to search...".to_string()
                                                } else {
                                                    "Click to search".to_string()
                                                }
                                            } else {
                                                format!(
                                                    "{}{}",
                                                    search_query,
                                                    if is_search_mode { "|" } else { "" }
                                                )
                                            }),
                                    ),
                            )
                            // Sort buttons
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.text_secondary)
                                            .child("Sort:"),
                                    )
                                    .child(self.render_sort_button(
                                        "Artist",
                                        crate::app::LibrarySortOrder::Artist,
                                        sort_order,
                                        theme.clone(),
                                        cx,
                                    ))
                                    .child(self.render_sort_button(
                                        "Album",
                                        crate::app::LibrarySortOrder::Album,
                                        sort_order,
                                        theme.clone(),
                                        cx,
                                    ))
                                    .child(self.render_sort_button(
                                        "Title",
                                        crate::app::LibrarySortOrder::Title,
                                        sort_order,
                                        theme.clone(),
                                        cx,
                                    ))
                                    .child(self.render_sort_button(
                                        "Year",
                                        crate::app::LibrarySortOrder::Year,
                                        sort_order,
                                        theme.clone(),
                                        cx,
                                    )),
                            )
                            // Filter buttons
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.text_secondary)
                                            .child("Filter:"),
                                    )
                                    .child(self.render_filter_button(
                                        "All",
                                        crate::app::ChannelFilter::All,
                                        channel_filter,
                                        theme.clone(),
                                        cx,
                                    ))
                                    .child(self.render_filter_button(
                                        "2.0",
                                        crate::app::ChannelFilter::Stereo,
                                        channel_filter,
                                        theme.clone(),
                                        cx,
                                    ))
                                    .child(self.render_filter_button(
                                        "5.1",
                                        crate::app::ChannelFilter::Multichannel,
                                        channel_filter,
                                        theme.clone(),
                                        cx,
                                    ))
                                    .child(self.render_filter_button(
                                        "7.1",
                                        crate::app::ChannelFilter::Mixed,
                                        channel_filter,
                                        theme.clone(),
                                        cx,
                                    )),
                            ),
                    ),
            )
            .child(
                div()
                    .id("library-content-container")
                    .flex_1()
                    .min_h_0() // Allow flex item to shrink below content size for scroll
                    .overflow_hidden()
                    .child(self.render_library_grid(cx)),
            )
            .child(self.render_pagination_controls(cx))
    }

    pub(crate) fn render_pagination_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (current_page, total_pages, items_per_page, theme) = {
            let state = self.state.read(cx);
            let current_page = state.app.library_page + 1;
            let total_pages = state.app.get_total_pages();
            (
                current_page,
                total_pages,
                state.app.library_items_per_page,
                state.app.theme.clone(),
            )
        };

        div()
            .flex()
            .justify_between()
            .items_center()
            .p_3()
            .bg(theme.background)
            .border_t_1()
            .border_color(theme.border)
            .child(
                div()
                    .text_sm()
                    .text_color(theme.text_secondary)
                    .child(format!(
                        "Page {} of {} ({} items/page)",
                        current_page, total_pages, items_per_page
                    )),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .when(current_page > 1, |div| {
                                div.bg(theme.surface)
                                    .hover(|style| style.bg(theme.surface_hover))
                                    .cursor_pointer()
                            })
                            .when(current_page == 1, |div| {
                                div.bg(theme.background).text_color(theme.text_muted)
                            })
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                    view.state.update(cx, |state, _cx| {
                                        state.app.prev_page();
                                    });
                                    cx.notify();
                                }),
                            )
                            .child("← Prev"),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .when(current_page < total_pages, |div| {
                                div.bg(theme.surface)
                                    .hover(|style| style.bg(theme.surface_hover))
                                    .cursor_pointer()
                            })
                            .when(current_page == total_pages, |div| {
                                div.bg(theme.background).text_color(theme.text_muted)
                            })
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                    view.state.update(cx, |state, _cx| {
                                        state.app.next_page();
                                    });
                                    cx.notify();
                                }),
                            )
                            .child("Next →"),
                    ),
            )
    }

    /// Render album grid view with thumbnails
    pub(crate) fn render_library_grid(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (albums, selected_album_index, theme) = {
            let state = self.state.read(cx);
            (
                state.app.get_paginated_albums(),
                state.app.selected_album_index,
                state.app.theme.clone(),
            )
        };

        div()
            .id("album-grid")
            .flex()
            .flex_wrap()
            .content_start() // Align items to start so they don't stretch
            .gap_4()
            .p_2()
            .size_full() // Ensure grid takes full parent size
            .overflow_y_scroll()
            .track_scroll(&self.grid_scroll_handle)
            .children(albums.iter().enumerate().map(|(idx, album)| {
                let is_selected = selected_album_index == idx;
                let theme = theme.clone();

                div()
                    .id(SharedString::from(format!("album-card-wrapper-{}", idx)))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                            view.state.update(cx, |state, _cx| {
                                state.app.selected_album_index = idx;
                            });
                            cx.notify();
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Right,
                        cx.listener(move |view, event: &MouseUpEvent, _window, cx| {
                            view.state.update(cx, |state, _cx| {
                                state.app.selected_album_index = idx;
                                state.app.context_menu = Some(crate::app::ContextMenuState {
                                    menu_type: crate::app::ContextMenuType::Album,
                                    position_x: event.position.x.into(),
                                    position_y: event.position.y.into(),
                                    item_index: idx,
                                });
                            });
                            cx.notify();
                        }),
                    )
                    .child(
                        AlbumCard::new(Arc::new((*album).clone()), idx, is_selected, theme.clone())
                            .mode(AlbumCardMode::Grid),
                    )
            }))
    }

    /// Render a sort button with active state styling
    fn render_sort_button(
        &self,
        label: &'static str,
        sort_order: crate::app::LibrarySortOrder,
        current_sort: crate::app::LibrarySortOrder,
        theme: crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_active = current_sort == sort_order;

        Button::new(SharedString::from(format!("sort-btn-{}", label)), label)
            .variant(if is_active {
                ButtonVariant::Primary
            } else {
                ButtonVariant::Secondary
            })
            .size(ButtonSize::Xs)
            .selected(is_active)
            .theme(theme.to_button_theme())
            .build()
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, _cx| {
                        state.app.set_library_sort_order(sort_order);
                    });
                    cx.notify();
                }),
            )
    }

    /// Render a filter button with active state styling
    fn render_filter_button(
        &self,
        label: &'static str,
        filter: crate::app::ChannelFilter,
        current_filter: crate::app::ChannelFilter,
        theme: crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_active = current_filter == filter;

        Button::new(SharedString::from(format!("filter-btn-{}", label)), label)
            .variant(if is_active {
                ButtonVariant::Primary
            } else {
                ButtonVariant::Secondary
            })
            .size(ButtonSize::Xs)
            .selected(is_active)
            .theme(theme.to_button_theme())
            .build()
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, _cx| {
                        state.app.set_channel_filter(filter);
                    });
                    cx.notify();
                }),
            )
    }
}
