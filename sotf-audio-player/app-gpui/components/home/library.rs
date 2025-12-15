//! Library screen rendering functions

use crate::components::home::album_card::{AlbumCard, AlbumCardMode};
use crate::components::icons::{Icon, IconName, IconSize};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Input, InputSize, TabItem, TabVariant, Tabs, TabsTheme,
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
            translations,
            (min_year, max_year, genres_count, stereo_count, multichannel_count),
        ) = {
            let state = self.state.read(cx);
            let translations = state.app.translations.clone();

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
                translations,
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
        let is_filter_mode = filter_menu_open;

        // Map sort order to tab index (Filter=6, Search=7 are special)
        let sort_tab_index = if is_search_mode {
            7 // Search tab
        } else if is_filter_mode {
            6 // Filter tab
        } else {
            match sort_order {
                crate::app::LibrarySortOrder::Year => 0,
                crate::app::LibrarySortOrder::Genre => 1,
                crate::app::LibrarySortOrder::Artist => 2,
                crate::app::LibrarySortOrder::Album => 3,
                crate::app::LibrarySortOrder::Tracks => 4,
                crate::app::LibrarySortOrder::Composer => 5,
                crate::app::LibrarySortOrder::Popularity => 3, // Default to Album tab
            }
        };

        // Build tabs theme from app theme
        let tabs_theme = TabsTheme {
            container_bg: theme.surface,
            container_border: theme.border,
            selected_bg: theme.surface_hover,
            selected_hover_bg: theme.surface_hover,
            hover_bg: theme.surface,
            accent: theme.accent,
            // Use text_on_accent for selected text since accent is used as background
            text_selected: theme.text_on_accent,
            text_unselected: theme.text_muted,
            text_hover: theme.text_secondary,
            badge_bg: theme.surface_hover,
            close_color: theme.text_muted,
            close_hover_color: theme.text_primary,
        };

        // Format year range for tab badge
        let year_badge = format!(
            "{}-{}",
            if min_year > 0 {
                min_year.to_string()
            } else {
                "?".to_string()
            },
            if max_year > 0 {
                max_year.to_string()
            } else {
                "?".to_string()
            }
        );

        // Build sort tabs with Filter and Search
        let sort_tabs = vec![
            TabItem::new("year", translations.library_years)
                .custom_icon(
                    Icon::new(IconName::Disc)
                        .size(IconSize::Lg)
                        .color(theme.accent),
                )
                .badge(year_badge),
            TabItem::new("genre", translations.library_genres)
                .custom_icon(
                    Icon::new(IconName::Folder)
                        .size(IconSize::Lg)
                        .color(theme.accent),
                )
                .badge(genres_count.to_string()),
            TabItem::new("artist", translations.library_artists)
                .custom_icon(
                    Icon::new(IconName::User)
                        .size(IconSize::Lg)
                        .color(theme.accent),
                )
                .badge(artists_count.to_string()),
            TabItem::new("album", translations.library_albums)
                .custom_icon(
                    Icon::new(IconName::Album)
                        .size(IconSize::Lg)
                        .color(theme.accent),
                )
                .badge(albums_count.to_string()),
            TabItem::new("tracks", translations.library_tracks)
                .custom_icon(
                    Icon::new(IconName::Music)
                        .size(IconSize::Lg)
                        .color(theme.accent),
                )
                .badge(tracks_count.to_string()),
            TabItem::new("composer", translations.library_composers)
                .custom_icon(
                    Icon::new(IconName::PenTool)
                        .size(IconSize::Lg)
                        .color(theme.accent),
                )
                .badge(composers_count.to_string()),
            TabItem::new("filter", translations.library_stereo_multi)
                .custom_icon(
                    Icon::new(IconName::AudioWaveform)
                        .size(IconSize::Lg)
                        .color(theme.accent),
                )
                .badge(format!("{}/{}", stereo_count, multichannel_count)),
            TabItem::new("search", translations.library_search)
                .custom_icon(
                    Icon::new(IconName::Search)
                        .size(IconSize::Lg)
                        .color(theme.accent),
                )
                .badge(translations.library_albums),
        ];

        let state_for_tabs = self.state.clone();

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_2()
            // Top row: Centered tabs (including Filter and Search)
            .child(
                div().flex().justify_center().mb_2().child(
                    Tabs::new()
                        .tabs(sort_tabs)
                        .selected_index(sort_tab_index)
                        .variant(TabVariant::VerticalCard)
                        .theme(tabs_theme.clone())
                        .on_change(move |index, _window, cx| {
                            state_for_tabs.update(cx, |state, _cx| {
                                // Handle sort order tabs (0-5)
                                if index <= 5 {
                                    let sort_order = match index {
                                        0 => crate::app::LibrarySortOrder::Year,
                                        1 => crate::app::LibrarySortOrder::Genre,
                                        2 => crate::app::LibrarySortOrder::Artist,
                                        3 => crate::app::LibrarySortOrder::Album,
                                        4 => crate::app::LibrarySortOrder::Tracks,
                                        5 => crate::app::LibrarySortOrder::Composer,
                                        _ => crate::app::LibrarySortOrder::Album,
                                    };
                                    state.app.set_library_sort_order(sort_order);
                                    // Close filter/search modes when selecting sort tab
                                    state.app.filter_menu_open = false;
                                    state.app.input_mode = crate::app::InputMode::Normal;
                                } else if index == 6 {
                                    // Filter tab
                                    state.app.filter_menu_open = !state.app.filter_menu_open;
                                    state.app.input_mode = crate::app::InputMode::Normal;
                                } else if index == 7 {
                                    // Search tab
                                    if state.app.input_mode == crate::app::InputMode::Search {
                                        state.app.input_mode = crate::app::InputMode::Normal;
                                        state.app.search_query.clear();
                                    } else {
                                        state.app.input_mode = crate::app::InputMode::Search;
                                        state.app.filter_menu_open = false;
                                    }
                                }
                            });
                        }),
                ),
            )
            // Filter options row (only visible when filter mode is active)
            .when(is_filter_mode, |el| {
                el.child(
                    div()
                        .flex()
                        .justify_center()
                        .gap_2()
                        .mb_2()
                        .py_2()
                        .px_4()
                        .bg(theme.surface)
                        .rounded_lg()
                        .child(self.render_filter_button(
                            "All",
                            crate::app::ChannelFilter::All,
                            channel_filter,
                            theme.clone(),
                            cx,
                        ))
                        .child(self.render_filter_button(
                            &format!("2.0 Stereo ({})", stereo_count),
                            crate::app::ChannelFilter::Stereo,
                            channel_filter,
                            theme.clone(),
                            cx,
                        ))
                        .child(self.render_filter_button(
                            &format!("5.1+ Multichannel ({})", multichannel_count),
                            crate::app::ChannelFilter::Multichannel,
                            channel_filter,
                            theme.clone(),
                            cx,
                        )),
                )
            })
            // Search bar row (only visible when in search mode)
            .when(is_search_mode, |el| {
                el.child(
                    div()
                        .flex()
                        .justify_center()
                        .mb_2()
                        .child(div().w_96().child({
                            let state = self.state.clone();
                            let state_for_end = state.clone();
                            Input::new("search-input")
                                .value(SharedString::from(search_query.clone()))
                                .edit_text(SharedString::from(search_query.clone()))
                                .placeholder("Type to search albums, artists, tracks...")
                                .icon_left("🔍")
                                .size(InputSize::Md)
                                .bg_color(theme.surface)
                                .text_color(theme.text_primary)
                                .placeholder_color(theme.text_muted)
                                .border_color(theme.accent)
                                .editing(true)
                                .on_text_change(move |text, _window, cx| {
                                    state.update(cx, |state, _| {
                                        state.app.search_query = text;
                                        state.app.selected_album_index = 0;
                                        state.app.reset_page();
                                    });
                                })
                                .on_edit_end(move |text_opt, _window, cx| {
                                    // Handle Enter or Escape - exit search mode
                                    if text_opt.is_some() {
                                        // Enter pressed - keep search results and exit
                                        state_for_end.update(cx, |state, _| {
                                            state.app.input_mode = crate::app::InputMode::Normal;
                                        });
                                    } else {
                                        // Escape pressed - clear search and exit
                                        state_for_end.update(cx, |state, _| {
                                            state.app.input_mode = crate::app::InputMode::Normal;
                                        });
                                    }
                                })
                        })),
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

    /// Render a filter button with active state styling
    fn render_filter_button(
        &self,
        label: &str,
        filter: crate::app::ChannelFilter,
        current_filter: crate::app::ChannelFilter,
        theme: crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_active = current_filter == filter;
        let label_owned = label.to_string();

        Button::new(
            SharedString::from(format!("filter-btn-{}", label)),
            SharedString::from(label_owned),
        )
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
