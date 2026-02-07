//! Library screen rendering functions

use crate::components::home::album_card::{AlbumCard, AlbumCardMode};
use crate::components::icons::{Icon, IconName, IconSize};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Input, InputSize, Spinner, SpinnerSize, TabItem, TabVariant,
    Tabs, TabsTheme,
};
use std::sync::Arc;

/// Selection action types for the library filter UI (Genre only)
/// Note: Artist, Composer, Tracks now use filter bars like Year/Album
#[derive(Clone)]
enum SelectionAction {
    Genre(String),
}

impl PlayerView {
    pub(crate) fn render_library_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Ensure library stats are computed (cached - only recomputes when invalidated)
        self.state.update(cx, |state, _| {
            let _ = state.app.get_library_stats();
        });

        // Now read all values including cached stats
        let state = self.state.read(cx);
        let stats = &state.app.library_stats;

        let albums_count = state.app.library_state.library.albums.len();
        let artists_count = stats.artists_count;
        let tracks_count = stats.total_tracks;
        let composers_count = stats.composers_count;
        let search_query = state.app.library_state.search_query.clone();
        let input_mode = state.app.ui_state.input_mode;
        let sort_order = state.app.library_state.sort_order;
        let channel_filter = state.app.library_state.filter;
        let filter_menu_open = state.app.ui_state.filter_menu_open;
        let theme = state.app.ui_state.theme.clone();
        let translations = state.app.ui_state.translations.clone();
        let min_year = stats.min_year;
        let max_year = stats.max_year;
        let genres_count = stats.genres_count;
        let mono_count = stats.mono_count;
        let stereo_count = stats.stereo_count;
        let surround_count = stats.surround_count;
        let surround71_count = stats.surround71_count;
        let surround_plus_count = stats.surround_plus_count;

        // Selection filters and counts for each category
        let selected_genre = state.app.library_state.selected_genre.clone();
        let selected_decade = state.app.library_state.selected_decade;
        let selected_year = state.app.library_state.selected_year;
        let selected_artist_letter = state.app.library_state.selected_artist_letter;
        let selected_artist = state.app.library_state.selected_artist.clone();
        let selected_composer_letter = state.app.library_state.selected_composer_letter;
        let selected_composer = state.app.library_state.selected_composer.clone();
        let selected_album_letter = state.app.library_state.selected_album_letter;
        let selected_track_range = state.app.library_state.selected_track_range;

        let genre_counts = stats.genre_counts.clone();
        let year_counts = stats.year_counts.clone();
        let decade_counts = stats.decade_counts.clone();
        let artist_counts = stats.artist_counts.clone();
        let artist_letter_counts = stats.artist_letter_counts.clone();
        let composer_counts = stats.composer_counts.clone();
        let composer_letter_counts = stats.composer_letter_counts.clone();
        let album_letter_counts = stats.album_letter_counts.clone();
        let track_range_counts = stats.track_range_counts.clone();

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
            icon_selected: Some(theme.icon_on_accent),
            icon_unselected: None,
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
        // Icons use icon_with_color to receive the correct color at render time
        // based on the tab's selection state (selected vs unselected)
        let sort_tabs = vec![
            TabItem::new("year", translations.library_years)
                .icon_with_color(|color| {
                    Icon::new(IconName::Disc)
                        .size(IconSize::Lg)
                        .color(color)
                        .into_any_element()
                })
                .badge(year_badge),
            TabItem::new("genre", translations.library_genres)
                .icon_with_color(|color| {
                    Icon::new(IconName::Folder)
                        .size(IconSize::Lg)
                        .color(color)
                        .into_any_element()
                })
                .badge(genres_count.to_string()),
            TabItem::new("artist", translations.library_artists)
                .icon_with_color(|color| {
                    Icon::new(IconName::User)
                        .size(IconSize::Lg)
                        .color(color)
                        .into_any_element()
                })
                .badge(artists_count.to_string()),
            TabItem::new("album", translations.library_albums)
                .icon_with_color(|color| {
                    Icon::new(IconName::Album)
                        .size(IconSize::Lg)
                        .color(color)
                        .into_any_element()
                })
                .badge(albums_count.to_string()),
            TabItem::new("tracks", translations.library_tracks)
                .icon_with_color(|color| {
                    Icon::new(IconName::Music)
                        .size(IconSize::Lg)
                        .color(color)
                        .into_any_element()
                })
                .badge(tracks_count.to_string()),
            TabItem::new("composer", translations.library_composers)
                .icon_with_color(|color| {
                    Icon::new(IconName::PenTool)
                        .size(IconSize::Lg)
                        .color(color)
                        .into_any_element()
                })
                .badge(composers_count.to_string()),
            TabItem::new("filter", translations.library_stereo_multi)
                .icon_with_color(|color| {
                    Icon::new(IconName::AudioWaveform)
                        .size(IconSize::Lg)
                        .color(color)
                        .into_any_element()
                })
                .badge(format!(
                    "{}/{}/{}",
                    stereo_count,
                    surround_count,
                    surround71_count + surround_plus_count
                )),
            TabItem::new("search", translations.library_search)
                .icon_with_color(|color| {
                    Icon::new(IconName::Search)
                        .size(IconSize::Lg)
                        .color(color)
                        .into_any_element()
                })
                .badge(translations.library_albums),
        ];

        let state_for_tabs = self.state.clone();

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_2()
            .when(state.app.is_loading_initial_data, |el| {
                el.justify_center()
                    .items_center()
                    .child(div().child(Spinner::new().size(SpinnerSize::Xl)))
            })
            .when(!state.app.is_loading_initial_data, |el| {
                el.child(
                    div().flex().justify_center().mb_2().child(
                        Tabs::new("library-sort-tabs")
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
                                        state.app.ui_state.filter_menu_open = false;
                                        state.app.ui_state.input_mode =
                                            crate::app::InputMode::Normal;
                                    } else if index == 6 {
                                        // Filter tab
                                        state.app.ui_state.filter_menu_open =
                                            !state.app.ui_state.filter_menu_open;
                                        state.app.ui_state.input_mode =
                                            crate::app::InputMode::Normal;
                                    } else if index == 7 {
                                        // Search tab
                                        if state.app.ui_state.input_mode
                                            == crate::app::InputMode::Search
                                        {
                                            state.app.ui_state.input_mode =
                                                crate::app::InputMode::Normal;
                                            state.app.library_state.search_query.clear();
                                        } else {
                                            state.app.ui_state.input_mode =
                                                crate::app::InputMode::Search;
                                            state.app.ui_state.filter_menu_open = false;
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
                                crate::app::state::library::ChannelFilter::All,
                                channel_filter,
                                theme.clone(),
                                cx,
                            ))
                            .child(self.render_filter_button(
                                &format!("1.0 Mono ({})", mono_count),
                                crate::app::state::library::ChannelFilter::Mono,
                                channel_filter,
                                theme.clone(),
                                cx,
                            ))
                            .child(self.render_filter_button(
                                &format!("2.0 Stereo ({})", stereo_count),
                                crate::app::state::library::ChannelFilter::Stereo,
                                channel_filter,
                                theme.clone(),
                                cx,
                            ))
                            .child(self.render_filter_button(
                                &format!("5.x Surround ({})", surround_count),
                                crate::app::state::library::ChannelFilter::Surround,
                                channel_filter,
                                theme.clone(),
                                cx,
                            ))
                            .child(self.render_filter_button(
                                &format!("7.1 ({})", surround71_count),
                                crate::app::state::library::ChannelFilter::Surround71,
                                channel_filter,
                                theme.clone(),
                                cx,
                            ))
                            .child(self.render_filter_button(
                                &format!("7.1+ ({})", surround_plus_count),
                                crate::app::state::library::ChannelFilter::SurroundPlus,
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
                        div().flex().justify_center().mb_2().child(
                            div().w_96().child(
                                Input::new("search-input")
                                    .value(SharedString::from(search_query.clone()))
                                    .placeholder("Type to search albums, artists, tracks...")
                                    .icon_left("🔍")
                                    .size(InputSize::Md)
                                    .bg_color(theme.surface)
                                    .text_color(theme.text_primary)
                                    .placeholder_color(theme.text_muted)
                                    .focus_handle(self.search_focus_handle.clone())
                                    .on_text_change({
                                        let app_state = self.state.clone();
                                        let view_handle = cx.entity().clone();
                                        move |text, _window, cx| {
                                            app_state.update(cx, |state, _| {
                                                state.app.library_state.set_search_query(text);
                                                if state.app.ui_state.input_mode
                                                    != crate::app::InputMode::Search
                                                {
                                                    state.app.ui_state.input_mode =
                                                        crate::app::InputMode::Search;
                                                }
                                            });
                                            view_handle.update(cx, |_, cx| cx.notify());
                                        }
                                    })
                                    .on_change({
                                        let view_handle = cx.entity().clone();
                                        move |_text, _window, cx| {
                                            log::info!("Search confirmed");
                                            // Optionally we could trigger something here
                                            view_handle.update(cx, |_, cx| cx.notify());
                                        }
                                    }),
                            ),
                        ),
                    )
                })
                .child(
                    div()
                        .id("library-content-container")
                        .flex_1()
                        .min_h_0()
                        .overflow_hidden()
                        .child(self.render_library_content(
                            sort_order,
                            theme.clone(),
                            // Selection states
                            selected_genre,
                            selected_decade,
                            selected_year,
                            selected_artist_letter,
                            selected_artist,
                            selected_composer_letter,
                            selected_composer,
                            selected_album_letter,
                            selected_track_range,
                            // Count maps
                            genre_counts,
                            decade_counts,
                            year_counts,
                            artist_counts,
                            artist_letter_counts,
                            composer_counts,
                            composer_letter_counts,
                            album_letter_counts,
                            track_range_counts,
                            cx,
                        )),
                )
            })
    }

    /// Render library content - either selection UI or album grid based on sort order
    #[allow(clippy::too_many_arguments)]
    fn render_library_content(
        &self,
        sort_order: crate::app::LibrarySortOrder,
        theme: crate::theme::Theme,
        // Selection states
        selected_genre: Option<String>,
        selected_decade: Option<(i32, i32)>,
        selected_year: Option<i32>,
        selected_artist_letter: Option<char>,
        selected_artist: Option<String>,
        selected_composer_letter: Option<char>,
        selected_composer: Option<String>,
        selected_album_letter: Option<char>,
        selected_track_range: Option<(usize, usize)>,
        // Count maps
        genre_counts: std::collections::HashMap<String, usize>,
        decade_counts: Vec<(i32, i32, usize)>,
        year_counts: std::collections::HashMap<i32, usize>,
        artist_counts: std::collections::HashMap<String, usize>,
        artist_letter_counts: std::collections::HashMap<char, usize>,
        composer_counts: std::collections::HashMap<String, usize>,
        composer_letter_counts: std::collections::HashMap<char, usize>,
        album_letter_counts: std::collections::HashMap<char, usize>,
        track_range_counts: Vec<(usize, usize, usize)>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        use crate::app::LibrarySortOrder;

        // Year, Album, Artist, Composer, and Tracks tabs show filter bar + albums (always visible)
        // Genre shows selection UI first, then albums with back button
        match sort_order {
            LibrarySortOrder::Year => {
                // Year tab: filter bar (decades/years) + album grid with year dividers
                self.render_year_tab_content(
                    selected_decade,
                    selected_year,
                    decade_counts,
                    year_counts,
                    theme,
                    cx,
                )
                .into_any_element()
            }
            LibrarySortOrder::Album => {
                // Album tab: letter filter bar + album grid with letter dividers
                self.render_album_tab_content(selected_album_letter, album_letter_counts, theme, cx)
                    .into_any_element()
            }
            LibrarySortOrder::Artist => {
                // Artist tab: letter filter bar + artist names (top 20) + album grid with artist dividers
                self.render_artist_tab_content(
                    selected_artist_letter,
                    selected_artist.clone(),
                    artist_letter_counts,
                    artist_counts,
                    theme,
                    cx,
                )
                .into_any_element()
            }
            LibrarySortOrder::Composer => {
                // Composer tab: letter filter bar + composer names (top 20) + album grid with composer dividers
                self.render_composer_tab_content(
                    selected_composer_letter,
                    selected_composer.clone(),
                    composer_letter_counts,
                    composer_counts,
                    theme,
                    cx,
                )
                .into_any_element()
            }
            LibrarySortOrder::Tracks => {
                // Tracks tab: track range filter bar + album grid with track count dividers
                self.render_tracks_tab_content(selected_track_range, track_range_counts, theme, cx)
                    .into_any_element()
            }
            _ => {
                // Genre tab: selection UI first, then albums with back button
                self.render_selection_based_content(
                    sort_order,
                    theme,
                    selected_genre,
                    genre_counts,
                    cx,
                )
                .into_any_element()
            }
        }
    }

    /// Render Year tab with filter bar and album grid
    fn render_year_tab_content(
        &self,
        selected_decade: Option<(i32, i32)>,
        selected_year: Option<i32>,
        decade_counts: Vec<(i32, i32, usize)>,
        year_counts: std::collections::HashMap<i32, usize>,
        theme: crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                // Filter bar
                self.render_year_filter_bar(
                    selected_decade,
                    selected_year,
                    decade_counts,
                    year_counts,
                    theme.clone(),
                    cx,
                ),
            )
            .child(
                // Album grid
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(self.render_library_grid(cx)),
            )
    }

    /// Render year filter bar with decades and years
    fn render_year_filter_bar(
        &self,
        selected_decade: Option<(i32, i32)>,
        selected_year: Option<i32>,
        decade_counts: Vec<(i32, i32, usize)>,
        year_counts: std::collections::HashMap<i32, usize>,
        theme: crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .p_2()
            .bg(theme.surface)
            .border_b_1()
            .border_color(theme.border)
            .child(
                // Decade buttons row
                div().flex().flex_wrap().justify_center().gap_1().children(
                    decade_counts.into_iter().map(|(start, end, count)| {
                        let is_selected = selected_decade == Some((start, end));
                        let label = format!("{}s ({})", start, count);

                        Button::new(
                            SharedString::from(format!("decade-{}", start)),
                            SharedString::from(label),
                        )
                        .variant(if is_selected {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Secondary
                        })
                        .size(ButtonSize::Xs)
                        .theme(theme.to_button_theme())
                        .build()
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                view.state.update(cx, |state, _cx| {
                                    if state.app.library_state.selected_decade == Some((start, end))
                                    {
                                        // Deselect if already selected
                                        state.app.library_state.selected_decade = None;
                                        state.app.library_state.selected_year = None;
                                    } else {
                                        state.app.library_state.selected_decade =
                                            Some((start, end));
                                        state.app.library_state.selected_year = None;
                                    }
                                    state.app.library_state.selected_index = 0;
                                });
                                cx.notify();
                            }),
                        )
                    }),
                ),
            )
            .when_some(selected_decade, |el, (decade_start, decade_end)| {
                // Year buttons for selected decade
                let mut years_in_decade: Vec<(i32, usize)> = year_counts
                    .iter()
                    .filter(|(y, _)| **y >= decade_start && **y <= decade_end)
                    .map(|(y, c)| (*y, *c))
                    .collect();
                years_in_decade.sort_by(|a, b| b.0.cmp(&a.0));

                el.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .justify_center()
                        .gap_1()
                        .mt_1()
                        .children(years_in_decade.into_iter().map(|(year, count)| {
                            let is_selected = selected_year == Some(year);
                            let label = format!("{} ({})", year, count);

                            Button::new(
                                SharedString::from(format!("year-{}", year)),
                                SharedString::from(label),
                            )
                            .variant(if is_selected {
                                ButtonVariant::Primary
                            } else {
                                ButtonVariant::Secondary
                            })
                            .size(ButtonSize::Xs)
                            .theme(theme.to_button_theme())
                            .build()
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                    view.state.update(cx, |state, _cx| {
                                        if state.app.library_state.selected_year == Some(year) {
                                            state.app.library_state.selected_year = None;
                                        } else {
                                            state.app.library_state.selected_year = Some(year);
                                        }
                                        state.app.library_state.selected_index = 0;
                                    });
                                    cx.notify();
                                }),
                            )
                        })),
                )
            })
    }

    /// Render Album tab with letter filter bar and album grid
    fn render_album_tab_content(
        &self,
        selected_letter: Option<char>,
        letter_counts: std::collections::HashMap<char, usize>,
        theme: crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                // Letter filter bar
                self.render_album_letter_filter_bar(
                    selected_letter,
                    letter_counts,
                    theme.clone(),
                    cx,
                ),
            )
            .child(
                // Album grid
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(self.render_library_grid(cx)),
            )
    }

    /// Render album letter filter bar (A-Z, #)
    fn render_album_letter_filter_bar(
        &self,
        selected_letter: Option<char>,
        letter_counts: std::collections::HashMap<char, usize>,
        theme: crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // Sort alphabetically, # at the end
        let mut letters: Vec<char> = letter_counts.keys().copied().collect();
        letters.sort_by(|a, b| {
            if *a == '#' {
                std::cmp::Ordering::Greater
            } else if *b == '#' {
                std::cmp::Ordering::Less
            } else {
                a.cmp(b)
            }
        });

        div()
            .flex()
            .flex_wrap()
            .justify_center()
            .gap_1()
            .p_2()
            .bg(theme.surface)
            .border_b_1()
            .border_color(theme.border)
            .children(letters.into_iter().map(|letter| {
                let is_selected = selected_letter == Some(letter);

                Button::new(
                    SharedString::from(format!("letter-{}", letter)),
                    SharedString::from(letter.to_string()),
                )
                .variant(if is_selected {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Secondary
                })
                .size(ButtonSize::Xs)
                .theme(theme.to_button_theme())
                .build()
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                        view.state.update(cx, |state, _cx| {
                            if state.app.library_state.selected_album_letter == Some(letter) {
                                state.app.library_state.selected_album_letter = None;
                            } else {
                                state.app.library_state.selected_album_letter = Some(letter);
                            }
                            state.app.library_state.selected_index = 0;
                        });
                        cx.notify();
                    }),
                )
            }))
    }

    /// Render Artist tab with letter filter bar, artist names, and album grid
    fn render_artist_tab_content(
        &self,
        selected_letter: Option<char>,
        selected_artist: Option<String>,
        letter_counts: std::collections::HashMap<char, usize>,
        artist_counts: std::collections::HashMap<String, usize>,
        theme: crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                // Filter bar (letters + artist names)
                self.render_artist_filter_bar(
                    selected_letter,
                    selected_artist,
                    letter_counts,
                    artist_counts,
                    theme.clone(),
                    cx,
                ),
            )
            .child(
                // Album grid
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(self.render_library_grid(cx)),
            )
    }

    /// Render artist filter bar with letters and artist names (top 20)
    fn render_artist_filter_bar(
        &self,
        selected_letter: Option<char>,
        selected_artist: Option<String>,
        letter_counts: std::collections::HashMap<char, usize>,
        artist_counts: std::collections::HashMap<String, usize>,
        theme: crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // Sort letters alphabetically, # at the end
        let mut letters: Vec<char> = letter_counts.keys().copied().collect();
        letters.sort_by(|a, b| {
            if *a == '#' {
                std::cmp::Ordering::Greater
            } else if *b == '#' {
                std::cmp::Ordering::Less
            } else {
                a.cmp(b)
            }
        });

        // Get artists for selected letter (top 20, sorted alphabetically)
        let artists_for_letter: Vec<(String, usize)> = if let Some(letter) = selected_letter {
            let mut filtered: Vec<(String, usize)> = artist_counts
                .iter()
                .filter(|(name, _)| {
                    name.chars().next().map_or(false, |c| {
                        let first = c.to_ascii_uppercase();
                        if letter == '#' {
                            !first.is_ascii_alphabetic()
                        } else {
                            first == letter
                        }
                    })
                })
                .map(|(name, count)| (name.clone(), *count))
                .collect();
            filtered.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
            filtered.into_iter().take(20).collect()
        } else {
            Vec::new()
        };

        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .p_2()
            .bg(theme.surface)
            .border_b_1()
            .border_color(theme.border)
            .child(
                // Letter buttons row
                div().flex().flex_wrap().justify_center().gap_1().children(
                    letters.into_iter().map(|letter| {
                        let is_selected = selected_letter == Some(letter);

                        Button::new(
                            SharedString::from(format!("artist-letter-{}", letter)),
                            SharedString::from(letter.to_string()),
                        )
                        .variant(if is_selected {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Ghost
                        })
                        .size(ButtonSize::Xs)
                        .theme(theme.to_button_theme())
                        .build()
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                view.state.update(cx, |state, _cx| {
                                    if state.app.library_state.selected_artist_letter
                                        == Some(letter)
                                    {
                                        state.app.library_state.selected_artist_letter = None;
                                        state.app.library_state.selected_artist = None;
                                    } else {
                                        state.app.library_state.selected_artist_letter =
                                            Some(letter);
                                        state.app.library_state.selected_artist = None;
                                    }
                                    state.app.library_state.selected_index = 0;
                                });
                                cx.notify();
                            }),
                        )
                    }),
                ),
            )
            // Artist names row (when a letter is selected)
            .when(!artists_for_letter.is_empty(), |el| {
                let artists = artists_for_letter.clone();
                el.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .justify_center()
                        .gap_1()
                        .pt_1()
                        .children(artists.into_iter().map(|(artist, count)| {
                            let is_selected = selected_artist.as_ref() == Some(&artist);
                            let artist_clone = artist.clone();

                            Button::new(
                                SharedString::from(format!("artist-name-{}", artist.clone())),
                                SharedString::from(format!("{} ({})", artist, count)),
                            )
                            .variant(if is_selected {
                                ButtonVariant::Primary
                            } else {
                                ButtonVariant::Secondary
                            })
                            .size(ButtonSize::Xs)
                            .theme(theme.to_button_theme())
                            .build()
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                    let artist_to_set = artist_clone.clone();
                                    view.state.update(cx, |state, _cx| {
                                        if state.app.library_state.selected_artist.as_ref()
                                            == Some(&artist_to_set)
                                        {
                                            state.app.library_state.selected_artist = None;
                                        } else {
                                            state.app.library_state.selected_artist =
                                                Some(artist_to_set);
                                        }
                                        state.app.library_state.selected_index = 0;
                                    });
                                    cx.notify();
                                }),
                            )
                        })),
                )
            })
    }

    /// Render Composer tab with letter filter bar, composer names, and album grid
    fn render_composer_tab_content(
        &self,
        selected_letter: Option<char>,
        selected_composer: Option<String>,
        letter_counts: std::collections::HashMap<char, usize>,
        composer_counts: std::collections::HashMap<String, usize>,
        theme: crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                // Filter bar (letters + composer names)
                self.render_composer_filter_bar(
                    selected_letter,
                    selected_composer,
                    letter_counts,
                    composer_counts,
                    theme.clone(),
                    cx,
                ),
            )
            .child(
                // Album grid
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(self.render_library_grid(cx)),
            )
    }

    /// Render composer filter bar with letters and composer names (top 20)
    fn render_composer_filter_bar(
        &self,
        selected_letter: Option<char>,
        selected_composer: Option<String>,
        letter_counts: std::collections::HashMap<char, usize>,
        composer_counts: std::collections::HashMap<String, usize>,
        theme: crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // Sort letters alphabetically, # at the end
        let mut letters: Vec<char> = letter_counts.keys().copied().collect();
        letters.sort_by(|a, b| {
            if *a == '#' {
                std::cmp::Ordering::Greater
            } else if *b == '#' {
                std::cmp::Ordering::Less
            } else {
                a.cmp(b)
            }
        });

        // Get composers for selected letter (top 20, sorted alphabetically)
        let composers_for_letter: Vec<(String, usize)> = if let Some(letter) = selected_letter {
            let mut filtered: Vec<(String, usize)> = composer_counts
                .iter()
                .filter(|(name, _)| {
                    name.chars().next().map_or(false, |c| {
                        let first = c.to_ascii_uppercase();
                        if letter == '#' {
                            !first.is_ascii_alphabetic()
                        } else {
                            first == letter
                        }
                    })
                })
                .map(|(name, count)| (name.clone(), *count))
                .collect();
            filtered.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
            filtered.into_iter().take(20).collect()
        } else {
            Vec::new()
        };

        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .p_2()
            .bg(theme.surface)
            .border_b_1()
            .border_color(theme.border)
            .child(
                // Letter buttons row
                div().flex().flex_wrap().justify_center().gap_1().children(
                    letters.into_iter().map(|letter| {
                        let is_selected = selected_letter == Some(letter);

                        Button::new(
                            SharedString::from(format!("composer-letter-{}", letter)),
                            SharedString::from(letter.to_string()),
                        )
                        .variant(if is_selected {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Ghost
                        })
                        .size(ButtonSize::Xs)
                        .theme(theme.to_button_theme())
                        .build()
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                view.state.update(cx, |state, _cx| {
                                    if state.app.library_state.selected_composer_letter
                                        == Some(letter)
                                    {
                                        state.app.library_state.selected_composer_letter = None;
                                        state.app.library_state.selected_composer = None;
                                    } else {
                                        state.app.library_state.selected_composer_letter =
                                            Some(letter);
                                        state.app.library_state.selected_composer = None;
                                    }
                                    state.app.library_state.selected_index = 0;
                                });
                                cx.notify();
                            }),
                        )
                    }),
                ),
            )
            // Composer names row (when a letter is selected)
            .when(!composers_for_letter.is_empty(), |el| {
                let composers = composers_for_letter.clone();
                el.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .justify_center()
                        .gap_1()
                        .pt_1()
                        .children(composers.into_iter().map(|(composer, count)| {
                            let is_selected = selected_composer.as_ref() == Some(&composer);
                            let composer_clone = composer.clone();

                            Button::new(
                                SharedString::from(format!("composer-name-{}", composer.clone())),
                                SharedString::from(format!("{} ({})", composer, count)),
                            )
                            .variant(if is_selected {
                                ButtonVariant::Primary
                            } else {
                                ButtonVariant::Secondary
                            })
                            .size(ButtonSize::Xs)
                            .theme(theme.to_button_theme())
                            .build()
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                    let composer_to_set = composer_clone.clone();
                                    view.state.update(cx, |state, _cx| {
                                        if state.app.library_state.selected_composer.as_ref()
                                            == Some(&composer_to_set)
                                        {
                                            state.app.library_state.selected_composer = None;
                                        } else {
                                            state.app.library_state.selected_composer =
                                                Some(composer_to_set);
                                        }
                                        state.app.library_state.selected_index = 0;
                                    });
                                    cx.notify();
                                }),
                            )
                        })),
                )
            })
    }

    /// Render Tracks tab with track range filter bar and album grid
    fn render_tracks_tab_content(
        &self,
        selected_range: Option<(usize, usize)>,
        track_range_counts: Vec<(usize, usize, usize)>,
        theme: crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                // Filter bar (track ranges)
                self.render_tracks_filter_bar(
                    selected_range,
                    track_range_counts,
                    theme.clone(),
                    cx,
                ),
            )
            .child(
                // Album grid
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(self.render_library_grid(cx)),
            )
    }

    /// Render tracks filter bar with track count ranges
    fn render_tracks_filter_bar(
        &self,
        selected_range: Option<(usize, usize)>,
        track_range_counts: Vec<(usize, usize, usize)>,
        theme: crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_wrap()
            .justify_center()
            .items_center()
            .gap_1()
            .p_2()
            .bg(theme.surface)
            .border_b_1()
            .border_color(theme.border)
            .children(track_range_counts.into_iter().map(|(min, max, count)| {
                let is_selected = selected_range == Some((min, max));
                let label = if max == usize::MAX {
                    format!("{}+ ({})", min, count)
                } else if min == max {
                    format!("{} ({})", min, count)
                } else {
                    format!("{}-{} ({})", min, max, count)
                };

                Button::new(
                    SharedString::from(format!("tracks-range-{}-{}", min, max)),
                    SharedString::from(label),
                )
                .variant(if is_selected {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Ghost
                })
                .size(ButtonSize::Xs)
                .theme(theme.to_button_theme())
                .build()
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                        view.state.update(cx, |state, _cx| {
                            if state.app.library_state.selected_track_range == Some((min, max)) {
                                state.app.library_state.selected_track_range = None;
                            } else {
                                state.app.library_state.selected_track_range = Some((min, max));
                            }
                            state.app.library_state.selected_index = 0;
                        });
                        cx.notify();
                    }),
                )
            }))
    }

    /// Render content for Genre tab that uses selection UI
    fn render_selection_based_content(
        &self,
        _sort_order: crate::app::LibrarySortOrder,
        theme: crate::theme::Theme,
        selected_genre: Option<String>,
        genre_counts: std::collections::HashMap<String, usize>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // Check if we need to show selection UI
        let needs_selection = selected_genre.is_none();

        if needs_selection {
            self.render_genre_selection(genre_counts, theme, cx)
                .into_any_element()
        } else {
            // Show album grid with back button
            let mut content = div().size_full().flex().flex_col();

            if let Some(label) = selected_genre.clone() {
                content = content.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .p_2()
                        .bg(theme.surface)
                        .border_b_1()
                        .border_color(theme.border)
                        .child(
                            Button::new("back-to-selection", "← Back to Genres")
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Sm)
                                .theme(theme.to_button_theme())
                                .build()
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                        view.state.update(cx, |state, _cx| {
                                            state.app.library_state.selected_genre = None;
                                            state.app.library_state.selected_index = 0;
                                        });
                                        cx.notify();
                                    }),
                                ),
                        )
                        .child(
                            div()
                                .text_lg()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.text_primary)
                                .child(label),
                        ),
                );
            }

            content = content.child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(self.render_library_grid(cx)),
            );

            content.into_any_element()
        }
    }
    /// Render genre selection grid
    fn render_genre_selection(
        &self,
        genre_counts: std::collections::HashMap<String, usize>,
        theme: crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        if genre_counts.is_empty() {
            return self.render_empty_selection("No genres found in library", &theme);
        }

        let mut genres: Vec<(String, usize)> = genre_counts
            .into_iter()
            .filter(|(_, count)| *count >= 5)
            .collect();
        genres.sort_by(|a, b| b.1.cmp(&a.1));

        if genres.is_empty() {
            return self.render_empty_selection("No genres with 5+ albums found", &theme);
        }

        self.render_selection_grid(
            "Select a Genre",
            genres
                .into_iter()
                .map(|(genre, count)| {
                    let label = format!("{} ({})", genre, count);
                    let size = (80.0 + (count as f32).sqrt() * 20.0).min(250.0);
                    (
                        format!("genre-{}", genre.clone()),
                        label,
                        size,
                        SelectionAction::Genre(genre),
                    )
                })
                .collect(),
            theme,
            cx,
        )
    }

    /// Helper to render empty selection message
    fn render_empty_selection(&self, message: &str, theme: &crate::theme::Theme) -> AnyElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_color(theme.text_muted)
                    .child(message.to_string()),
            )
            .into_any_element()
    }

    /// Generic helper to render a selection grid
    fn render_selection_grid(
        &self,
        title: &str,
        items: Vec<(String, String, f32, SelectionAction)>,
        theme: crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id("selection-scroll")
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .overflow_y_scroll()
            .p_4()
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.text_primary)
                    .mb_4()
                    .child(title.to_string()),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .max_w(px(1000.0))
                    .justify_center()
                    .children(items.into_iter().map(|(id, label, size, action)| {
                        Button::new(SharedString::from(id), SharedString::from(label))
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Md)
                            .theme(theme.to_button_theme())
                            .build()
                            .w(px(size))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                    let action = action.clone();
                                    view.state.update(cx, |state, _cx| {
                                        match action {
                                            SelectionAction::Genre(g) => {
                                                state.app.library_state.selected_genre = Some(g)
                                            }
                                        }
                                        state.app.library_state.selected_index = 0;
                                    });
                                    cx.notify();
                                }),
                            )
                    })),
            )
            .into_any_element()
    }

    /// Render album grid view with thumbnails
    pub(crate) fn render_library_grid(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (albums, selected_album_index, theme, sort_order) = {
            let state = self.state.read(cx);
            (
                state.app.get_paginated_albums(),
                state.app.library_state.selected_index,
                state.app.ui_state.theme.clone(),
                state.app.library_state.sort_order,
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
                .on_click(cx.listener(move |view, event: &ClickEvent, _window, cx| {
                    view.state.update(cx, |state, _cx| {
                        state.app.library_state.selected_index = idx;
                    });
                    // Double-click adds to queue
                    if event.click_count() == 2 {
                        view.state.update(cx, |state, _cx| {
                            if let Some(path) = state.app.add_album_to_queue() {
                                Self::play_track(state, path);
                            }
                        });
                    }
                    cx.notify();
                }))
                .on_mouse_up(
                    MouseButton::Right,
                    cx.listener(move |view, event: &MouseUpEvent, _window, cx| {
                        view.state.update(cx, |state, _cx| {
                            state.app.library_state.selected_index = idx;
                            state.app.ui_state.context_menu = Some(crate::app::ContextMenuState {
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
                    AlbumCard::new(Arc::new((*album).clone()), idx, is_selected, card_theme)
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
        filter: crate::app::state::library::ChannelFilter,
        current_filter: crate::app::state::library::ChannelFilter,
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
