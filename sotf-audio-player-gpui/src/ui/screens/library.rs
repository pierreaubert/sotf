//! Library screen rendering functions

use crate::app::AppState;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;

impl PlayerView {
    pub(crate) fn render_library_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (
            library_view_mode,
            albums_count,
            search_query,
            scan_in_progress,
            input_mode,
            sort_order,
            channel_filter,
            filtered_count,
        ) = {
            let state = self.state.read(cx);
            let filtered_count = state.app.filtered_albums().len();
            (
                state.app.library_view_mode,
                state.app.library.albums.len(),
                state.app.search_query.clone(),
                state.app.scan_in_progress,
                state.app.input_mode,
                state.app.library_sort_order,
                state.app.channel_filter,
                filtered_count,
            )
        };

        let is_search_mode = input_mode == crate::app::InputMode::Search;

        let content = if library_view_mode == crate::app::LibraryViewMode::TreeView {
            self.render_library_tree(cx).into_any_element()
        } else {
            self.render_library_flat(cx).into_any_element()
        };

        let sort_label = match sort_order {
            crate::app::LibrarySortOrder::Artist => "Artist",
            crate::app::LibrarySortOrder::Album => "Album",
            crate::app::LibrarySortOrder::Title => "Title",
            crate::app::LibrarySortOrder::Year => "Year",
        };

        let filter_label = match channel_filter {
            crate::app::ChannelFilter::All => "All".to_string(),
            crate::app::ChannelFilter::Mono => "Mono".to_string(),
            crate::app::ChannelFilter::Stereo => "Stereo".to_string(),
            crate::app::ChannelFilter::Multichannel => "Multi".to_string(),
            crate::app::ChannelFilter::Mixed => "Mixed".to_string(),
            crate::app::ChannelFilter::Specific(n) => format!("{}ch", n),
        };

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
                                    .bg(rgb(0x2d2d2d))
                                    .rounded_md()
                                    .border_1()
                                    .when(is_search_mode, |div| div.border_color(rgb(0x007acc)))
                                    .when(!is_search_mode, |div| div.border_color(rgb(0x3e3e3e)))
                                    .px_2()
                                    .py_1()
                                    .w_64()
                                    .child(
                                        div()
                                            .mr_2()
                                            .text_color(if is_search_mode {
                                                rgb(0x007acc)
                                            } else {
                                                rgb(0x999999)
                                            })
                                            .child("🔍"),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(if search_query.is_empty() {
                                                if is_search_mode {
                                                    rgb(0x999999)
                                                } else {
                                                    rgb(0x666666)
                                                }
                                            } else {
                                                rgb(0xcccccc)
                                            })
                                            .child(if search_query.is_empty() {
                                                if is_search_mode {
                                                    "Type to search...".to_string()
                                                } else {
                                                    "Press / to search".to_string()
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
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x999999))
                                    .child(format!("Sort: {}", sort_label)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x999999))
                                    .child(format!("Filter: {}", filter_label)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .px_2()
                                            .py_1()
                                            .rounded_md()
                                            .bg(if library_view_mode
                                                == crate::app::LibraryViewMode::Flat
                                            {
                                                rgb(0x4e4e4e)
                                            } else {
                                                rgb(0x2d2d2d)
                                            })
                                            .cursor_pointer()
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(
                                                    |view, _: &MouseUpEvent, _window, cx| {
                                                        view.state.update(cx, |state, _cx| {
                                                            state.app.library_view_mode =
                                                                crate::app::LibraryViewMode::Flat
                                                        });
                                                        cx.notify();
                                                    },
                                                ),
                                            )
                                            .child("Flat"),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_1()
                                            .rounded_md()
                                            .bg(if library_view_mode
                                                == crate::app::LibraryViewMode::TreeView
                                            {
                                                rgb(0x4e4e4e)
                                            } else {
                                                rgb(0x2d2d2d)
                                            })
                                            .cursor_pointer()
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(
                                                    |view, _: &MouseUpEvent, _window, cx| {
                                                        view.state.update(cx, |state, _cx| {
                                                            state.app.library_view_mode =
                                                                crate::app::LibraryViewMode::TreeView;
                                                            state.app.rebuild_artist_tree();
                                                        });
                                                        cx.notify();
                                                    },
                                                ),
                                            )
                                            .child("Tree"),
                                    ),
                            )
                            .child(
                                div()
                                    .px_2()
                                    .py_1()
                                    .ml_2()
                                    .rounded_md()
                                    .bg(rgb(0x2d2d2d))
                                    .hover(|style| style.bg(rgb(0x3e3e3e)))
                                    .cursor_pointer()
                                    .id("scan_btn")
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                            view.state.update(cx, |state, _cx| {
                                                if let Err(e) = state.app.scan_library() {
                                                    log::error!("Scan failed: {}", e);
                                                }
                                            });
                                            cx.notify();
                                        }),
                                    )
                                    .child(if scan_in_progress { "Scanning..." } else { "Scan" }),
                            ),
                    ),
            )
            .child(content)
            .child(self.render_pagination_controls(cx))
    }

    pub(crate) fn render_pagination_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let current_page = state.app.library_page + 1; // Display as 1-indexed
        let total_pages = match state.app.library_view_mode {
            crate::app::LibraryViewMode::Flat => state.app.get_flat_total_pages(),
            crate::app::LibraryViewMode::TreeView => state.app.get_tree_total_pages(),
        };
        let items_per_page = state.app.library_items_per_page;

        div()
            .flex()
            .justify_between()
            .items_center()
            .p_3()
            .bg(rgb(0x1e1e1e))
            .border_t_1()
            .border_color(rgb(0x3e3e3e))
            .child(div().text_sm().text_color(rgb(0x999999)).child(format!(
                "Page {} of {} ({} items/page)",
                current_page, total_pages, items_per_page
            )))
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
                                div.bg(rgb(0x2d2d2d))
                                    .hover(|style| style.bg(rgb(0x3e3e3e)))
                                    .cursor_pointer()
                            })
                            .when(current_page == 1, |div| {
                                div.bg(rgb(0x1a1a1a)).text_color(rgb(0x666666))
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
                                div.bg(rgb(0x2d2d2d))
                                    .hover(|style| style.bg(rgb(0x3e3e3e)))
                                    .cursor_pointer()
                            })
                            .when(current_page == total_pages, |div| {
                                div.bg(rgb(0x1a1a1a)).text_color(rgb(0x666666))
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

    pub(crate) fn render_library_flat(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let albums = state.app.get_paginated_albums();

        div()
            .flex()
            .flex_col()
            .gap_2()
            .flex_1()
            .children(albums.iter().enumerate().map(|(idx, album)| {
                let is_selected = state.app.selected_album_index == idx;
                div()
                    .p_3()
                    .rounded_md()
                    .when(is_selected, |div| div.bg(rgb(0x007acc)))
                    .when(!is_selected, |div| div.bg(rgb(0x2d2d2d)))
                    .hover(|style| style.bg(rgb(0x3e3e3e)))
                    .cursor_pointer()
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                            view.state
                                .update(cx, |state, _cx| state.app.selected_album_index = idx);
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
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(album.title.clone()),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0x999999))
                                    .child(album.artist.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x666666))
                                    .child(format!("{} tracks", album.tracks.len())),
                            ),
                    )
            }))
    }

    pub(crate) fn render_library_tree(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let tree_items = state.app.get_paginated_tree_items();

        div()
            .flex()
            .flex_col()
            .gap_1()
            .flex_1()
            .children(tree_items.iter().enumerate().map(|(idx, item)| {
                let is_selected = state.app.selected_tree_index == idx;

                match item {
                    crate::app::TreeItem::Artist { name, expanded } => div()
                        .p_2()
                        .rounded_md()
                        .when(is_selected, |div| div.bg(rgb(0x007acc)))
                        .when(!is_selected, |div| div.bg(rgb(0x2d2d2d)))
                        .cursor_pointer()
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                view.state.update(cx, |state, _cx| {
                                    state.app.selected_tree_index = idx;
                                    state.app.toggle_artist_expansion();
                                });
                                cx.notify();
                            }),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(if *expanded { "▼" } else { "▶" })
                                .child(name.clone()),
                        ),
                    crate::app::TreeItem::Album { index } => {
                        let album = &state.app.library.albums[*index];
                        div()
                            .pl_8()
                            .p_2()
                            .rounded_md()
                            .when(is_selected, |div| div.bg(rgb(0x007acc)))
                            .when(!is_selected, |div| div.bg(rgb(0x252525)))
                            .cursor_pointer()
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                    view.state
                                        .update(cx, |state, _cx| state.app.selected_tree_index = idx);
                                    cx.notify();
                                }),
                            )
                            .child(album.title.clone())
                    }
                }
            }))
    }
}
