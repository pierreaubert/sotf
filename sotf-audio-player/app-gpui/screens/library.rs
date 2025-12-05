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
            (min_year, max_year, categories_count, stereo_count, multichannel_count),
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
                // New stats
                {
                    let mut min_year = 9999;
                    let mut max_year = 0;
                    let mut categories: std::collections::HashSet<String> = std::collections::HashSet::new();
                    let mut stereo_count = 0;
                    let mut multichannel_count = 0;

                    for album in &state.app.library.albums {
                        if let Some(y) = album.year {
                            if y > 0 {
                                if y < min_year { min_year = y; }
                                if y > max_year { max_year = y; }
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
                                    categories.insert(genre.to_lowercase());
                                }
                            }
                        }
                    }
                    if min_year == 9999 { min_year = 0; }
                    (min_year, max_year, categories.len(), stereo_count, multichannel_count)
                }
            )
        };

        let is_search_mode = input_mode == crate::app::InputMode::Search;

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_2()
            // Stats row with all boxes in one line
            .child(
                div()
                    .flex()
                    .flex_wrap() // Allow wrapping if window is too small
                    .justify_center()
                    .gap_2()
                    .mb_4()
                    // Box 1: Years
                    .child(self.render_stat_box(
                        "Years",
                        0, // Dummy value, we'll override the text in render_stat_box_custom
                        IconName::Disc,
                        &theme
                    ).with_value(format!("{} - {}", if min_year > 0 { min_year.to_string() } else { "??".to_string() }, if max_year > 0 { max_year.to_string() } else { "??".to_string() })))
                    // Box 2: Categories
                    .child(self.render_stat_box("Categories", categories_count, IconName::Folder, &theme))
                    // Box 3: Channels
                    .child(self.render_stat_box(
                        "Stereo / Multi",
                        0, // Dummy value
                        IconName::AudioWaveform,
                        &theme
                    ).with_value(format!("{} / {}", stereo_count, multichannel_count)))
                    // Existing boxes
                    .child(self.render_stat_box("Albums", albums_count, IconName::Album, &theme))
                    .child(self.render_stat_box("Artists", artists_count, IconName::User, &theme))
                    .child(self.render_stat_box("Tracks", tracks_count, IconName::Music, &theme))
                    .child(self.render_stat_box(
                        "Composers",
                        composers_count,
                        IconName::PenTool,
                        &theme,
                    )),
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
                                    .readonly(true)
                                    .bg_color(theme.surface)
                                    .text_color(theme.text_primary)
                                    .placeholder_color(theme.text_muted)
                                    .border_color(theme.border),
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
    ) -> StatBox {
        StatBox {
            label: label.to_string(),
            value: count.to_string(),
            icon,
            theme: theme.clone(),
        }
    }
}

struct StatBox {
    label: String,
    value: String,
    icon: IconName,
    theme: crate::theme::Theme,
}

impl StatBox {
    fn with_value(mut self, value: String) -> Self {
        self.value = value;
        self
    }
}

impl IntoElement for StatBox {
    type Element = gpui::AnyElement;

    fn into_element(self) -> Self::Element {
        let surface = self.theme.surface;
        let border = self.theme.border;
        let accent = self.theme.accent;
        let text_primary = self.theme.text_primary;
        let text_secondary = self.theme.text_secondary;

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
                            .child(Icon::new(self.icon).size(IconSize::Xxl).color(accent)),
                    )
                    .child(
                        VStack::new()
                            .spacing(StackSpacing::None)
                            .child(
                                Text::new(self.value)
                                    .size(TextSize::Lg)
                                    .weight(TextWeight::Bold)
                                    .color(text_primary),
                            )
                            .child(
                                Text::new(self.label)
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
                    .py_1()
                    .px_2()
            })
            .into_any_element()
    }
}
