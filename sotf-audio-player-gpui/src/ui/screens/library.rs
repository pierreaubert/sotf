//! Library screen rendering functions

use crate::ui::PlayerView;
use crate::ui::components::album_card::{AlbumCard, AlbumCardMode};
use crate::ui::components::icon::{Icon, IconName, IconSize};
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Card, HStack, Input, InputSize, StackSpacing, Text,
    TextSize, TextWeight, VStack,
};
use std::sync::Arc;

impl PlayerView {
    pub(crate) fn render_library_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (
            albums_count,
            artists_count,
            tracks_count,
            composers_count,
            search_query,
            input_mode,
            sort_order,
            channel_filter,
            theme,
        ) = {
            let state = self.state.read(cx);

            // Count unique artists, tracks, and composers
            let mut artists: std::collections::HashSet<String> = std::collections::HashSet::new();
            let mut composers: std::collections::HashSet<String> = std::collections::HashSet::new();
            let mut total_tracks = 0usize;

            for album in &state.app.library.albums {
                for track in &album.tracks {
                    total_tracks += 1;
                    if let Some(artist) = &track.artist {
                        if !artist.is_empty() {
                            artists.insert(artist.to_lowercase());
                        }
                    }
                    if let Some(composer) = &track.composer {
                        if !composer.is_empty() {
                            composers.insert(composer.to_lowercase());
                        }
                    }
                }
            }

            (
                state.app.library.albums.len(),
                artists.len(),
                total_tracks,
                composers.len(),
                state.app.search_query.clone(),
                state.app.input_mode,
                state.app.library_sort_order,
                state.app.channel_filter,
                state.app.theme.clone(),
            )
        };

        let is_search_mode = input_mode == crate::app::InputMode::Search;

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            // Stats row with 4 centered boxes
            .child(
                div()
                    .flex()
                    .justify_center()
                    .gap_4()
                    .mb_4()
                    .child(self.render_stat_box("Albums", albums_count, IconName::Album, &theme))
                    .child(self.render_stat_box("Artists", artists_count, IconName::User, &theme))
                    .child(self.render_stat_box("Tracks", tracks_count, IconName::Music, &theme))
                    .child(self.render_stat_box("Composers", composers_count, IconName::PenTool, &theme)),
            )
            // Search and filter row
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .mb_4()
                    .child(
                        div()
                            .w_64()
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
                                Input::new("search-input")
                                    .value(SharedString::from(if search_query.is_empty() {
                                        if is_search_mode {
                                            "Type to search...".to_string()
                                        } else {
                                            "".to_string()
                                        }
                                    } else {
                                        format!(
                                            "{}{}",
                                            search_query,
                                            if is_search_mode { "|" } else { "" }
                                        )
                                    }))
                                    .placeholder("Click to search")
                                    .icon_left("🔍")
                                    .size(InputSize::Sm)
                                    .readonly(true),
                            ),
                    )
                    .child(
                        HStack::new()
                            .spacing(StackSpacing::Lg)
                            // Sort buttons
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Xs)
                                    .child(
                                        Text::new("Sort:")
                                            .size(TextSize::Xs)
                                            .color(theme.text_secondary),
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
                                    ))
                                    .build(),
                            )
                            // Filter buttons
                            .child(
                                HStack::new()
                                    .spacing(StackSpacing::Xs)
                                    .child(
                                        Text::new("Filter:")
                                            .size(TextSize::Xs)
                                            .color(theme.text_secondary),
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
                                    ))
                                    .build(),
                            )
                            .build(),
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

        let button_theme = theme.to_button_theme();
        let can_prev = current_page > 1;
        let can_next = current_page < total_pages;

        div()
            .flex()
            .justify_between()
            .items_center()
            .p_3()
            .bg(theme.background)
            .border_t_1()
            .border_color(theme.border)
            .child(
                Text::new(format!(
                    "Page {} of {} ({} items/page)",
                    current_page, total_pages, items_per_page
                ))
                .size(TextSize::Sm)
                .color(theme.text_secondary),
            )
            .child(
                HStack::new()
                    .spacing(StackSpacing::Sm)
                    .child({
                        let btn = Button::new("prev-page-btn", "← Prev")
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Sm)
                            .disabled(!can_prev)
                            .theme(button_theme.clone())
                            .build();
                        if can_prev {
                            btn.on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                    view.state.update(cx, |state, _cx| {
                                        state.app.prev_page();
                                    });
                                    cx.notify();
                                }),
                            )
                        } else {
                            btn
                        }
                    })
                    .child({
                        let btn = Button::new("next-page-btn", "Next →")
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Sm)
                            .disabled(!can_next)
                            .theme(button_theme)
                            .build();
                        if can_next {
                            btn.on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                    view.state.update(cx, |state, _cx| {
                                        state.app.next_page();
                                    });
                                    cx.notify();
                                }),
                            )
                        } else {
                            btn
                        }
                    })
                    .build(),
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

    /// Render a large stat box for the library header using Card component
    fn render_stat_box(
        &self,
        label: &str,
        count: usize,
        icon: IconName,
        theme: &crate::theme::Theme,
    ) -> impl IntoElement {
        // Clone values to avoid lifetime issues with closures
        let label_owned = label.to_string();
        let surface = theme.surface;
        let border = theme.border;
        let accent = theme.accent;
        let text_primary = theme.text_primary;
        let text_secondary = theme.text_secondary;

        Card::new()
            .content(
                HStack::new()
                    .spacing(StackSpacing::Md)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(accent)
                            .child(Icon::new(icon).size(IconSize::Xl)),
                    )
                    .child(
                        VStack::new()
                            .spacing(StackSpacing::None)
                            .child(
                                Text::new(format!("{}", count))
                                    .size(TextSize::Xxl)
                                    .weight(TextWeight::Bold)
                                    .color(text_primary),
                            )
                            .child(
                                Text::new(label_owned)
                                    .size(TextSize::Sm)
                                    .color(text_secondary),
                            )
                            .build(),
                    )
                    .build(),
            )
            .style(move |card| {
                card.min_w(px(160.0))
                    .bg(surface)
                    .border_color(border)
            })
    }
}
