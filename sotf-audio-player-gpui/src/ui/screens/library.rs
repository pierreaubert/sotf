//! Library screen rendering functions

use crate::ui::components::album_card::{AlbumCard, AlbumCardMode};
use crate::ui::PlayerView;
use gpui_ui_kit::{Button, ButtonVariant, ButtonSize};
use gpui::prelude::*;
use gpui::{img, uniform_list, *};
use std::sync::Arc;

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
            theme,
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
                state.app.theme.clone(),
            )
        };

        let is_search_mode = input_mode == crate::app::InputMode::Search;

        let content = match library_view_mode {
            crate::app::LibraryViewMode::TreeView => self.render_library_tree(cx).into_any_element(),
            crate::app::LibraryViewMode::Grid => self.render_library_grid(cx).into_any_element(),
            crate::app::LibraryViewMode::Flat => self.render_library_flat(cx).into_any_element(),
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
                                            .text_sm()
                                            .bg(if library_view_mode
                                                == crate::app::LibraryViewMode::Flat
                                            {
                                                theme.surface_selected
                                            } else {
                                                theme.surface
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
                                            .child("List"),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_1()
                                            .rounded_md()
                                            .text_sm()
                                            .bg(if library_view_mode
                                                == crate::app::LibraryViewMode::TreeView
                                            {
                                                theme.surface_selected
                                            } else {
                                                theme.surface
                                            })
                                            .cursor_pointer()
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(
                                                    |view, _: &MouseUpEvent, _window, cx| {
                                                        view.state.update(cx, |state, _cx| {
                                                            state.app.library_view_mode =
                                                                crate::app::LibraryViewMode::TreeView;
                                                            state.app.rebuild_letter_tree();
                                                        });
                                                        cx.notify();
                                                    },
                                                ),
                                            )
                                            .child("Tree"),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_1()
                                            .rounded_md()
                                            .text_sm()
                                            .bg(if library_view_mode
                                                == crate::app::LibraryViewMode::Grid
                                            {
                                                theme.surface_selected
                                            } else {
                                                theme.surface
                                            })
                                            .cursor_pointer()
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(
                                                    |view, _: &MouseUpEvent, _window, cx| {
                                                        view.state.update(cx, |state, _cx| {
                                                            state.app.library_view_mode =
                                                                crate::app::LibraryViewMode::Grid
                                                        });
                                                        cx.notify();
                                                    },
                                                ),
                                            )
                                            .child("Grid"),
                                    ),
                            )
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(content),
            )
            .child(self.render_pagination_controls(cx))
    }

    pub(crate) fn render_pagination_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (current_page, total_pages, items_per_page, theme) = {
            let state = self.state.read(cx);
            let current_page = state.app.library_page + 1;
            let total_pages = match state.app.library_view_mode {
                crate::app::LibraryViewMode::Flat | crate::app::LibraryViewMode::Grid => {
                    state.app.get_flat_total_pages()
                }
                crate::app::LibraryViewMode::TreeView => state.app.get_tree_total_pages(),
            };
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
            .child(div().text_sm().text_color(theme.text_secondary).child(format!(
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

    pub(crate) fn render_library_flat(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Get all filtered albums (not paginated - uniform_list handles virtualization)
        let (albums, selected_album_index, theme) = {
            let state = self.state.read(cx);
            let filtered = state.app.filtered_albums();
            // Convert to Arc for efficient cloning in render callback
            let albums: Vec<Arc<sotf_audio_player::Album>> =
                filtered.into_iter().cloned().map(Arc::new).collect();
            (albums, state.app.selected_album_index, state.app.theme.clone())
        };

        let album_count = albums.len();

        // Capture state entity for click handlers
        let state_entity = self.state.clone();

        div()
            .id("library-flat-view")
            .flex()
            .flex_col()
            .flex_1()
            .p_2()
            .child(
                uniform_list(
                    "album-list-flat",
                    album_count,
                    {
                        let albums = albums.clone();
                        let theme = theme.clone();
                        let state_entity = state_entity.clone();
                        move |range, _window, cx| {
                            range
                                .map(|idx| {
                                    let album = albums[idx].clone();
                                    let is_selected = selected_album_index == idx;
                                    let theme = theme.clone();
                                    let state_entity = state_entity.clone();

                                    // Wrap AlbumCard in a container with click handlers
                                    div()
                                        .id(SharedString::from(format!("album-flat-{}", idx)))
                                        .mb_2()
                                        .on_mouse_up(MouseButton::Left, {
                                            let state_entity = state_entity.clone();
                                            move |_event, _window, cx| {
                                                state_entity.update(cx, |state, cx| {
                                                    state.app.selected_album_index = idx;
                                                    cx.notify();
                                                });
                                            }
                                        })
                                        .on_mouse_up(MouseButton::Right, {
                                            let state_entity = state_entity.clone();
                                            move |event: &MouseUpEvent, _window, cx| {
                                                state_entity.update(cx, |state, cx| {
                                                    state.app.selected_album_index = idx;
                                                    state.app.context_menu =
                                                        Some(crate::app::ContextMenuState {
                                                            menu_type: crate::app::ContextMenuType::Album,
                                                            position_x: event.position.x.into(),
                                                            position_y: event.position.y.into(),
                                                            item_index: idx,
                                                        });
                                                    cx.notify();
                                                });
                                            }
                                        })
                                        .child(AlbumCard::new(album, idx, is_selected, theme).mode(AlbumCardMode::List))
                                })
                                .collect()
                        }
                    },
                )
                .track_scroll(self.library_scroll_handle.clone())
                .size_full()
                .with_sizing_behavior(ListSizingBehavior::Infer),
            )
    }

    pub(crate) fn render_library_tree(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (tree_items, albums, selected_tree_index, theme) = {
            let state = self.state.read(cx);
            (
                state.app.get_paginated_tree_items(),
                state.app.library.albums.clone(),
                state.app.selected_tree_index,
                state.app.theme.clone(),
            )
        };

        div()
            .id("library-tree-view")
            .flex()
            .flex_col()
            .gap_1()
            .flex_1()
            .overflow_y_scroll()
            .p_2()
            .children(tree_items.iter().enumerate().map(|(idx, item)| {
                let is_selected = selected_tree_index == idx;
                let theme = theme.clone();

                match item {
                    crate::app::TreeItem::Letter { letter, expanded } => div()
                        .p_2()
                        .rounded_md()
                        .when(is_selected, |d| d.bg(theme.accent))
                        .when(!is_selected, |d| d.bg(theme.surface))
                        .cursor_pointer()
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                view.state.update(cx, |state, _cx| {
                                    state.app.selected_tree_index = idx;
                                    state.app.toggle_letter_expansion();
                                });
                                cx.notify();
                            }),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(if *expanded { "▼" } else { "▶" })
                                .child(letter.to_string())
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.text_muted)
                                        .child("-"),
                                ),
                        ),
                    crate::app::TreeItem::Album { index } => {
                        let album = &albums[*index];
                        let track_count = album.tracks.len();
                        div()
                            .pl_8()
                            .p_2()
                            .rounded_md()
                            .when(is_selected, |d| d.bg(theme.accent))
                            .when(!is_selected, |d| d.bg(theme.background_secondary))
                            .cursor_pointer()
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                    view.state
                                        .update(cx, |state, _cx| state.app.selected_tree_index = idx);
                                    cx.notify();
                                }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .w_full()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .child(album.title.clone())
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(theme.text_muted)
                                                    .child(album.artist()),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.text_secondary)
                                            .child(format!("#{}", track_count)),
                                    ),
                            )
                    }
                }
            }))
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

        let thumbnail_size = 120.0;
        let card_width = 150.0;

        let grid = div()
            .id("album-grid")
            .flex()
            .flex_wrap()
            .gap_4()
            .p_2()
            .flex_1()
            .overflow_y_scroll()
            .children(albums.iter().enumerate().map(|(idx, album)| {
            let is_selected = selected_album_index == idx;
            let theme = theme.clone();
            let has_thumbnail = album.album_art_thumbnail.is_some();

            div()
                .id(SharedString::from(format!("album-card-{}", idx)))
                .w(px(card_width))
                .flex()
                .flex_col()
                .items_center()
                .p_2()
                .rounded_lg()
                .when(is_selected, |d| d.bg(theme.accent))
                .when(!is_selected, |d| d.bg(theme.surface))
                .hover(|style| style.bg(theme.surface_hover))
                .cursor_pointer()
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
                // Album art thumbnail or placeholder
                .child(
                    div()
                        .w(px(thumbnail_size))
                        .h(px(thumbnail_size))
                        .rounded_md()
                        .overflow_hidden()
                        .bg(theme.background_secondary)
                        .border_1()
                        .border_color(theme.border)
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(has_thumbnail, |d| {
                            // Use album art path if available
                            if let Some(ref path) = album.album_art_path {
                                d.child(
                                    img(path.clone())
                                        .w(px(thumbnail_size))
                                        .h(px(thumbnail_size))
                                        .object_fit(gpui::ObjectFit::Cover)
                                )
                            } else {
                                // Fallback to placeholder even if has_thumbnail is true
                                d.child(
                                    div()
                                        .text_3xl()
                                        .text_color(theme.text_muted)
                                        .child("♪")
                                )
                            }
                        })
                        .when(!has_thumbnail, |d| {
                            d.child(
                                div()
                                    .text_3xl()
                                    .text_color(theme.text_muted)
                                    .child("♪")
                            )
                        })
                )
                // Album title
                .child(
                    div()
                        .w_full()
                        .mt_2()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_primary)
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .child(album.title.clone())
                )
                // Artist name
                .child(
                    div()
                        .w_full()
                        .text_xs()
                        .text_color(theme.text_secondary)
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .child(album.artist())
                )
                // Track count
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.text_muted)
                        .child(format!("{} tracks", album.tracks.len()))
                )
        }));

        grid
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
            .variant(if is_active { ButtonVariant::Primary } else { ButtonVariant::Secondary })
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
            .variant(if is_active { ButtonVariant::Primary } else { ButtonVariant::Secondary })
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
