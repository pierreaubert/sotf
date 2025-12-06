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
            filter_menu_open,
            theme,
            (min_year, max_year, genres_count, stereo_count, multichannel_count),
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
                state.app.filter_menu_open,
                state.app.theme.clone(),
                // New stats
                {
                    let mut min_year = 9999;
                    let mut max_year = 0;
                    let mut genres: std::collections::HashSet<String> =
                        std::collections::HashSet::new();
                    let mut stereo_count = 0;
                    let mut multichannel_count = 0;

                    for album in &state.app.library.albums {
                        if let Some(y) = album.year {
                            if y > 0 {
                                if y < min_year {
                                    min_year = y;
                                }
                                if y > max_year {
                                    max_year = y;
                                }
                            }
                        }

                        if let Some(channels) = album.uniform_channel_count() {
                            if channels == 2 {
                                stereo_count += 1;
                            } else if channels > 2 {
                                multichannel_count += 1;
                            }
                        }

                        for track in &album.tracks {
                            if let Some(genre) = &track.genre {
                                if !genre.is_empty() {
                                    genres.insert(genre.to_lowercase());
                                }
                            }
                        }
                    }
                    if min_year == 9999 {
                        min_year = 0;
                    }
                    (
                        min_year,
                        max_year,
                        genres.len(),
                        stereo_count,
                        multichannel_count,
                    )
                },
            )
        };

        let is_search_mode = input_mode == crate::app::InputMode::Search;
        let is_filter_open = filter_menu_open && !is_search_mode; // Close filter if search opens (logic handled in click handlers too)

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_2()
            // Top row: Sortable stat boxes + Channel filter box + Search box
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .justify_center()
                    .gap_2()
                    .mb_2()
                    // Box 1: Years (clickable, sorts by Year)
                    .child(self.render_sortable_stat_box(
                        "Years",
                        format!(
                            "{} - {}",
                            if min_year > 0 {
                                min_year.to_string()
                            } else {
                                "??".to_string()
                            },
                            if max_year > 0 {
                                max_year.to_string()
                            } else {
                                "??".to_string()
                            }
                        ),
                        IconName::Disc,
                        crate::app::LibrarySortOrder::Year,
                        sort_order,
                        &theme,
                        cx,
                    ))
                    // Box 2: Genres (clickable, sorts by Genre)
                    .child(self.render_sortable_stat_box(
                        "Genres",
                        genres_count.to_string(),
                        IconName::Folder,
                        crate::app::LibrarySortOrder::Genre,
                        sort_order,
                        &theme,
                        cx,
                    ))
                    // Box 3: Artists (clickable, sorts by Artist)
                    .child(self.render_sortable_stat_box(
                        "Artists",
                        artists_count.to_string(),
                        IconName::User,
                        crate::app::LibrarySortOrder::Artist,
                        sort_order,
                        &theme,
                        cx,
                    ))
                    // Box 4: Albums (clickable, sorts by Album title)
                    .child(self.render_sortable_stat_box(
                        "Albums",
                        albums_count.to_string(),
                        IconName::Album,
                        crate::app::LibrarySortOrder::Album,
                        sort_order,
                        &theme,
                        cx,
                    ))
                    // Box 5: Tracks (clickable, sorts by track count)
                    .child(self.render_sortable_stat_box(
                        "Tracks",
                        tracks_count.to_string(),
                        IconName::Music,
                        crate::app::LibrarySortOrder::Tracks,
                        sort_order,
                        &theme,
                        cx,
                    ))
                    // Box 6: Composers (clickable, sorts by Composer)
                    .child(self.render_sortable_stat_box(
                        "Composers",
                        composers_count.to_string(),
                        IconName::PenTool,
                        crate::app::LibrarySortOrder::Composer,
                        sort_order,
                        &theme,
                        cx,
                    ))
                    // Box 7: Channels with filter buttons inside
                    .child(self.render_channel_filter_box(
                        stereo_count,
                        multichannel_count,
                        channel_filter,
                        &theme,
                        cx,
                    ))
                    // Box 8: Search icon box
                    .child(self.render_search_box(&theme, is_search_mode, cx)),
            )
            // Search bar (only visible when in search mode)
            .when(is_search_mode, |el| {
                el.child(
                    div().flex().justify_center().mb_2().child(
                        div().w_96().child(
                            Input::new("search-input")
                                .value(SharedString::from(if search_query.is_empty() {
                                    "".to_string()
                                } else {
                                    format!("{}|", search_query)
                                }))
                                .placeholder("Type to search albums, artists, tracks...")
                                .icon_left("🔍")
                                .size(InputSize::Md)
                                .readonly(true)
                                .bg_color(theme.surface)
                                .text_color(theme.text_primary)
                                .placeholder_color(theme.text_muted)
                                .border_color(theme.accent),
                        ),
                    ),
                )
            })
            // Filter options (only visible when filter menu is open)
            .when(is_filter_open, |el| {
                el.child(
                    div().flex().justify_center().mb_2().child(
                        HStack::new()
                            .spacing(StackSpacing::Sm)
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
                                crate::app::ChannelFilter::Specific(8),
                                channel_filter,
                                theme.clone(),
                                cx,
                            ))
                            .build(),
                    ),
                )
            })
            .child(
                div()
                    .id("library-content-container")
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(self.render_library_grid(cx)),
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
            .content_start()
            .gap_4()
            .p_2()
            .size_full()
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

    /// Render a sortable stat box that acts as a sort button
    fn render_sortable_stat_box(
        &self,
        label: &str,
        value: String,
        icon: IconName,
        sort_order: crate::app::LibrarySortOrder,
        current_sort: crate::app::LibrarySortOrder,
        theme: &crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_active = current_sort == sort_order;
        let surface = if is_active {
            theme.surface_selected
        } else {
            theme.surface
        };
        let border = if is_active {
            theme.accent
        } else {
            theme.border
        };
        let accent = theme.accent;
        let text_primary = theme.text_primary;
        let text_secondary = theme.text_secondary;

        div()
            .id(SharedString::from(format!("stat-box-{}", label)))
            .cursor_pointer()
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, _cx| {
                        state.app.set_library_sort_order(sort_order);
                    });
                    cx.notify();
                }),
            )
            .child(
                Card::new()
                    .content(
                        HStack::new()
                            .spacing(StackSpacing::Sm)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(accent)
                                    .child(Icon::new(icon).size(IconSize::Xl).color(accent)),
                            )
                            .child(
                                VStack::new()
                                    .spacing(StackSpacing::None)
                                    .child(
                                        Text::new(value)
                                            .size(TextSize::Md)
                                            .weight(TextWeight::Bold)
                                            .color(text_primary),
                                    )
                                    .child(
                                        Text::new(label.to_string())
                                            .size(TextSize::Xs)
                                            .color(text_secondary),
                                    )
                                    .build(),
                            )
                            .build(),
                    )
                    .style(move |card| {
                        card.min_w(px(100.0))
                            .bg(surface)
                            .border_color(border)
                            .border_1()
                            .py_1()
                            .px_2()
                    }),
            )
    }

    /// Render the channel filter box (button style)
    fn render_channel_filter_box(
        &self,
        stereo_count: usize,
        multichannel_count: usize,
        _current_filter: crate::app::ChannelFilter,
        theme: &crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_open = self.state.read(cx).app.filter_menu_open;
        let surface = if is_open {
            theme.surface_selected
        } else {
            theme.surface
        };
        let border = if is_open { theme.accent } else { theme.border };
        let accent = theme.accent;
        let text_primary = theme.text_primary;
        let text_secondary = theme.text_secondary;

        div()
            .id("channel-filter-box")
            .cursor_pointer()
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, _cx| {
                        state.app.filter_menu_open = !state.app.filter_menu_open;
                        // Close search if opening filter
                        if state.app.filter_menu_open
                            && state.app.input_mode == crate::app::InputMode::Search
                        {
                            state.app.input_mode = crate::app::InputMode::Normal;
                        }
                    });
                    cx.notify();
                }),
            )
            .child(
                Card::new()
                    .content(
                        HStack::new()
                            .spacing(StackSpacing::Sm)
                            .child(
                                div().flex().items_center().justify_center().child(
                                    Icon::new(IconName::AudioWaveform)
                                        .size(IconSize::Xl)
                                        .color(accent),
                                ),
                            )
                            .child(
                                VStack::new()
                                    .spacing(StackSpacing::None)
                                    .child(
                                        Text::new(format!(
                                            "{} / {}",
                                            stereo_count, multichannel_count
                                        ))
                                        .size(TextSize::Md)
                                        .weight(TextWeight::Bold)
                                        .color(text_primary),
                                    )
                                    .child(
                                        Text::new("Stereo / Multi")
                                            .size(TextSize::Xs)
                                            .color(text_secondary),
                                    )
                                    .build(),
                            )
                            .build(),
                    )
                    .style(move |card| {
                        card.min_w(px(120.0))
                            .bg(surface)
                            .border_color(border)
                            .border_1()
                            .py_1()
                            .px_2()
                    }),
            )
    }

    /// Render the search icon box
    fn render_search_box(
        &self,
        theme: &crate::theme::Theme,
        is_active: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let surface = if is_active {
            theme.surface_selected
        } else {
            theme.surface
        };
        let border = if is_active {
            theme.accent
        } else {
            theme.border
        };
        let accent = theme.accent;
        let text_secondary = theme.text_secondary;

        div()
            .id("search-box")
            .cursor_pointer()
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, _cx| {
                        if state.app.input_mode == crate::app::InputMode::Search {
                            state.app.input_mode = crate::app::InputMode::Normal;
                            state.app.search_query.clear();
                        } else {
                            state.app.input_mode = crate::app::InputMode::Search;
                        }
                    });
                    cx.notify();
                }),
            )
            .child(
                Card::new()
                    .content(
                        HStack::new()
                            .spacing(StackSpacing::Sm)
                            .child(div().flex().items_center().justify_center().child(
                                Icon::new(IconName::Search).size(IconSize::Xl).color(accent),
                            ))
                            .child(
                                VStack::new()
                                    .spacing(StackSpacing::None)
                                    .child(
                                        Text::new("Search")
                                            .size(TextSize::Md)
                                            .weight(TextWeight::Bold)
                                            .color(text_secondary),
                                    )
                                    .child(
                                        Text::new("Albums")
                                            .size(TextSize::Xs)
                                            .color(text_secondary),
                                    )
                                    .build(),
                            )
                            .build(),
                    )
                    .style(move |card| {
                        card.min_w(px(70.0))
                            .bg(surface)
                            .border_color(border)
                            .border_1()
                            .py_1()
                            .px_2()
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
