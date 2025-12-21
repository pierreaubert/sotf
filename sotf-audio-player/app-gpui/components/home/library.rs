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
        // Ensure library stats are computed (cached - only recomputes when invalidated)
        self.state.update(cx, |state, _| {
            let _ = state.app.get_library_stats();
        });

        // Now read all values including cached stats
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
            min_year,
            max_year,
            genres_count,
            mono_count,
            stereo_count,
            surround_count,
            surround71_count,
            surround_plus_count,
        ) = {
            let state = self.state.read(cx);
            let stats = &state.app.library_stats;
            (
                state.app.library.albums.len(),
                stats.artists_count,
                stats.total_tracks,
                stats.composers_count,
                state.app.search_query.clone(),
                state.app.input_mode,
                state.app.library_sort_order,
                state.app.channel_filter,
                state.app.filter_menu_open,
                state.app.theme.clone(),
                state.app.translations.clone(),
                stats.min_year,
                stats.max_year,
                stats.genres_count,
                stats.mono_count,
                stats.stereo_count,
                stats.surround_count,
                stats.surround71_count,
                stats.surround_plus_count,
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
                .badge(format!(
                    "{}/{}/{}",
                    stereo_count,
                    surround_count,
                    surround71_count + surround_plus_count
                )),
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
                        .flex_wrap()
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
                            &format!("1.0 Mono ({})", mono_count),
                            crate::app::ChannelFilter::Mono,
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
                            &format!("5.x Surround ({})", surround_count),
                            crate::app::ChannelFilter::Surround,
                            channel_filter,
                            theme.clone(),
                            cx,
                        ))
                        .child(self.render_filter_button(
                            &format!("7.1 ({})", surround71_count),
                            crate::app::ChannelFilter::Surround71,
                            channel_filter,
                            theme.clone(),
                            cx,
                        ))
                        .child(self.render_filter_button(
                            &format!("7.1+ ({})", surround_plus_count),
                            crate::app::ChannelFilter::SurroundPlus,
                            channel_filter,
                            theme.clone(),
                            cx,
                        )),
                )
            })
            // Search bar row (only visible when in search mode)
            // Keyboard input is handled at the parent level in ui.rs via handle_search_input
            .when(is_search_mode, |el| {
                el.child(
                    div()
                        .flex()
                        .justify_center()
                        .mb_2()
                        .child(div().w_96().child(
                            Input::new("search-input")
                                .value(SharedString::from(search_query.clone()))
                                .placeholder("Type to search albums, artists, tracks...")
                                .icon_left("🔍")
                                .size(InputSize::Md)
                                .bg_color(theme.surface)
                                .text_color(theme.text_primary)
                                .placeholder_color(theme.text_muted)
                        ))
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
        let (albums, selected_album_index, theme, sort_order) = {
            let state = self.state.read(cx);
            (
                state.app.get_paginated_albums(),
                state.app.selected_album_index,
                state.app.theme.clone(),
                state.app.library_sort_order,
            )
        };

        // Build elements with dividers based on sort order
        let mut elements: Vec<AnyElement> = Vec::new();
        let mut previous_group: Option<String> = None;

        for (idx, album) in albums.iter().enumerate() {
            // Determine group key based on sort order
            let group_key = match sort_order {
                crate::app::LibrarySortOrder::Year => {
                    let year = album.year.unwrap_or(0);
                    if year == 0 {
                        "Unknown Year".to_string()
                    } else {
                        year.to_string()
                    }
                }
                crate::app::LibrarySortOrder::Genre => album
                    .tracks
                    .first()
                    .and_then(|t| t.genre.as_ref())
                    .cloned()
                    .unwrap_or_else(|| "Unknown Genre".to_string()),
                crate::app::LibrarySortOrder::Artist => {
                    let artist = album.artist();
                    artist
                        .chars()
                        .next()
                        .map(|c| c.to_uppercase().to_string())
                        .unwrap_or_else(|| "#".to_string())
                }
                crate::app::LibrarySortOrder::Composer => {
                    let composer = album
                        .tracks
                        .first()
                        .and_then(|t| t.composer.as_ref())
                        .cloned()
                        .unwrap_or_default();
                    if composer.is_empty() {
                        "Unknown Composer".to_string()
                    } else {
                        composer
                            .chars()
                            .next()
                            .map(|c| c.to_uppercase().to_string())
                            .unwrap_or_else(|| "#".to_string())
                    }
                }
                // No dividers for Album, Tracks, Popularity sort orders
                _ => String::new(),
            };

            // Add divider when group changes (only for sort orders that use grouping)
            if !group_key.is_empty() && previous_group.as_ref() != Some(&group_key) {
                elements.push(self.render_section_divider(&group_key, idx, &theme));
                previous_group = Some(group_key);
            }

            let is_selected = selected_album_index == idx;
            let card_theme = theme.clone();

            let album_card = div()
                .id(("album-wrapper", idx))
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
                    AlbumCard::new(Arc::new(album.clone()), idx, is_selected, card_theme)
                        .mode(AlbumCardMode::Grid),
                );

            elements.push(album_card.into_any_element());
        }

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
            .children(elements)
    }

    /// Render a section divider with label and horizontal line
    fn render_section_divider(
        &self,
        label: &str,
        index: usize,
        theme: &crate::theme::Theme,
    ) -> AnyElement {
        div()
            .id(("section-divider", index))
            .flex_basis(relative(1.0)) // Full width to force new row
            .pt_4()
            .pb_2()
            .flex()
            .items_center()
            .gap_3()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.text_primary)
                    .child(label.to_string()),
            )
            .child(div().flex_1().h(px(1.0)).bg(theme.border))
            .into_any_element()
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
