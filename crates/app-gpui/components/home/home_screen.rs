//! Home screen shelves for album discovery.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, IconButton, IconButtonSize, IconButtonVariant,
};
use sotf_audio_player::{Album, sotf_api_client::SotfApiAlbum};

use crate::components::design::Ds;
use crate::components::home::album_card::{AlbumCard, AlbumCardMode};
use crate::components::icons::{Icon, IconName, IconSize};
use crate::ui::PlayerView;

const COLLAPSED_ALBUM_LIMIT: usize = 8;
const EXPANDED_ALBUM_LIMIT: usize = 24;

#[derive(Clone)]
struct HomeShelf {
    id: String,
    title: String,
    albums: Vec<Album>,
}

#[derive(Clone)]
struct RemoteHomeShelf {
    id: String,
    title: String,
    albums: Vec<SotfApiAlbum>,
}

impl PlayerView {
    pub(crate) fn render_home_screen(&self, cx: &mut Context<Self>) -> AnyElement {
        if self
            .state
            .read(cx)
            .app
            .remote
            .server_store
            .selected_server_id
            .is_some()
        {
            return self.render_remote_home_screen(cx);
        }

        let d = Ds::from_cx(cx);
        let (theme, shelves, expanded_sections) = {
            let state = self.state.read(cx);
            let albums = state.app.library_state.library.albums.clone();
            (
                state.app.ui_state.theme.clone(),
                build_home_shelves(&albums),
                state.app.ui_state.expanded_home_sections.clone(),
            )
        };

        div()
            .id("home-screen")
            .flex()
            .flex_col()
            .size_full()
            .overflow_y_scroll()
            .bg(theme.background)
            .p(d.card)
            .gap(d.section_lg)
            .when(shelves.iter().all(|shelf| shelf.albums.is_empty()), |el| {
                el.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size_full()
                        .text_size(d.text_sm)
                        .text_color(theme.text_muted)
                        .child("Add albums to your library to build Home shelves."),
                )
            })
            .children(
                shelves
                    .into_iter()
                    .filter(|shelf| !shelf.albums.is_empty())
                    .map(|shelf| {
                        let is_expanded = expanded_sections.contains(&shelf.id);
                        self.render_home_shelf(shelf, is_expanded, cx)
                    }),
            )
            .into_any_element()
    }

    fn render_remote_home_screen(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let (theme, page, expanded_sections, is_loading, server_name) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state.app.remote.current_album_page.clone(),
                state.app.ui_state.expanded_home_sections.clone(),
                state.app.remote.cache_refresh_in_progress
                    || state.app.remote.refresh_requests.visible_album_page,
                state
                    .app
                    .remote
                    .server_store
                    .selected_server()
                    .map(|server| server.friendly_name.clone())
                    .unwrap_or_else(|| "Remote SOTF Player".to_string()),
            )
        };
        let shelves = page
            .as_ref()
            .map(|page| build_remote_home_shelves(&page.albums))
            .unwrap_or_default();

        div()
            .id("remote-home-screen")
            .flex()
            .flex_col()
            .size_full()
            .overflow_y_scroll()
            .bg(theme.background)
            .p(d.card)
            .gap(d.section_lg)
            .when(shelves.iter().all(|shelf| shelf.albums.is_empty()), |el| {
                el.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size_full()
                        .text_size(d.text_sm)
                        .text_color(theme.text_muted)
                        .child(if is_loading {
                            format!("Loading Home from {server_name}...")
                        } else {
                            format!("{server_name} has no albums to show.")
                        }),
                )
            })
            .children(
                shelves
                    .into_iter()
                    .filter(|shelf| !shelf.albums.is_empty())
                    .map(|shelf| {
                        let is_expanded = expanded_sections.contains(&shelf.id);
                        self.render_remote_home_shelf(shelf, is_expanded, cx)
                    }),
            )
            .into_any_element()
    }

    fn render_home_shelf(
        &self,
        shelf: HomeShelf,
        is_expanded: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let d = Ds::from_cx(cx);
        let theme = self.state.read(cx).app.ui_state.theme.clone();
        let limit = if is_expanded {
            EXPANDED_ALBUM_LIMIT
        } else {
            COLLAPSED_ALBUM_LIMIT
        };
        let can_expand = shelf.albums.len() > COLLAPSED_ALBUM_LIMIT;
        let shelf_id = shelf.id.clone();
        let state_for_toggle = self.state.clone();

        div()
            .id(SharedString::from(format!("home-shelf-{}", shelf.id)))
            .flex()
            .flex_col()
            .gap(d.gap)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(d.gap)
                    .child(
                        div()
                            .min_w_0()
                            .text_size(d.text_lg)
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.text_primary)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(shelf.title),
                    )
                    .when(can_expand, |el| {
                        let icon_name = if is_expanded {
                            IconName::ChevronUp
                        } else {
                            IconName::ChevronDown
                        };
                        let icon_label = if is_expanded {
                            "Collapse section"
                        } else {
                            "Expand section"
                        };
                        let icon_theme = theme.clone();
                        el.child(
                            IconButton::with_child(
                                SharedString::from(format!("home-shelf-toggle-{shelf_id}")),
                                Icon::new(icon_name)
                                    .size(IconSize::Xs)
                                    .color(icon_theme.text_primary),
                            )
                            .variant(IconButtonVariant::Filled)
                            .size(IconButtonSize::Sm)
                            .theme(icon_theme.to_icon_button_theme())
                            .aria_label(icon_label)
                            .on_click_event(
                                move |_event, _window, cx| {
                                    state_for_toggle.update(cx, |state, _cx| {
                                        let expanded =
                                            &mut state.app.ui_state.expanded_home_sections;
                                        if !expanded.insert(shelf_id.clone()) {
                                            expanded.remove(&shelf_id);
                                        }
                                    });
                                },
                            ),
                        )
                    })
                    .child(div().flex_1()),
            )
            .child(
                div()
                    .flex()
                    .gap(d.gap_md)
                    .when(is_expanded, |el| el.flex_wrap())
                    .when(!is_expanded, |el| el.overflow_hidden())
                    .children(
                        shelf
                            .albums
                            .into_iter()
                            .take(limit)
                            .enumerate()
                            .map(|(idx, album)| self.render_home_album_card(album, idx, cx)),
                    ),
            )
            .into_any_element()
    }

    fn render_remote_home_shelf(
        &self,
        shelf: RemoteHomeShelf,
        is_expanded: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let d = Ds::from_cx(cx);
        let theme = self.state.read(cx).app.ui_state.theme.clone();
        let limit = if is_expanded {
            EXPANDED_ALBUM_LIMIT
        } else {
            COLLAPSED_ALBUM_LIMIT
        };
        let can_expand = shelf.albums.len() > COLLAPSED_ALBUM_LIMIT;
        let shelf_id = shelf.id.clone();
        let state_for_toggle = self.state.clone();

        div()
            .id(SharedString::from(format!(
                "remote-home-shelf-{}",
                shelf.id
            )))
            .flex()
            .flex_col()
            .gap(d.gap)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(d.gap)
                    .child(
                        div()
                            .min_w_0()
                            .text_size(d.text_lg)
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.text_primary)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(shelf.title),
                    )
                    .when(can_expand, |el| {
                        let icon_name = if is_expanded {
                            IconName::ChevronUp
                        } else {
                            IconName::ChevronDown
                        };
                        let icon_label = if is_expanded {
                            "Collapse section"
                        } else {
                            "Expand section"
                        };
                        let icon_theme = theme.clone();
                        el.child(
                            IconButton::with_child(
                                SharedString::from(format!("remote-home-shelf-toggle-{shelf_id}")),
                                Icon::new(icon_name)
                                    .size(IconSize::Xs)
                                    .color(icon_theme.text_primary),
                            )
                            .variant(IconButtonVariant::Filled)
                            .size(IconButtonSize::Sm)
                            .theme(icon_theme.to_icon_button_theme())
                            .aria_label(icon_label)
                            .on_click_event(
                                move |_event, _window, cx| {
                                    state_for_toggle.update(cx, |state, _cx| {
                                        let expanded =
                                            &mut state.app.ui_state.expanded_home_sections;
                                        if !expanded.insert(shelf_id.clone()) {
                                            expanded.remove(&shelf_id);
                                        }
                                    });
                                },
                            ),
                        )
                    })
                    .child(div().flex_1()),
            )
            .child(
                div()
                    .flex()
                    .gap(d.gap_md)
                    .when(is_expanded, |el| el.flex_wrap())
                    .when(!is_expanded, |el| el.overflow_hidden())
                    .children(
                        shelf
                            .albums
                            .into_iter()
                            .take(limit)
                            .enumerate()
                            .map(|(idx, album)| self.render_remote_home_album_card(album, idx, cx)),
                    ),
            )
            .into_any_element()
    }

    fn render_home_album_card(
        &self,
        album: Album,
        idx: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = self.state.read(cx).app.ui_state.theme.clone();
        let album_for_click = album.clone();
        let state_entity = self.state.clone();

        div()
            .id(SharedString::from(format!(
                "home-album-{}-{}",
                album.id.unwrap_or(-1),
                idx
            )))
            .flex_none()
            .on_click(cx.listener(move |view, event: &ClickEvent, _window, cx| {
                view.state.update(cx, |state, _cx| {
                    add_home_album_to_queue(state, &album_for_click, event.click_count() >= 2);
                });
            }))
            .child(
                AlbumCard::new(Arc::new(album), idx, false, theme)
                    .mode(AlbumCardMode::Grid)
                    .state(state_entity),
            )
            .into_any_element()
    }

    fn render_remote_home_album_card(
        &self,
        album: SotfApiAlbum,
        idx: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let d = Ds::from_cx(cx);
        let theme = self.state.read(cx).app.ui_state.theme.clone();
        let album_id = album.id.clone();
        let title = album.title.clone();
        let title_for_click = title.clone();
        let title_for_add = title.clone();
        let title_for_play = title.clone();
        let album_id_for_click = album_id.clone();
        let album_id_for_add = album_id.clone();
        let album_id_for_play = album_id.clone();
        let year = album.year.map(|year| year.to_string()).unwrap_or_default();
        let dynamic_range = album.dynamic_range.map(|dr| format!("DR{dr}"));
        let hover_bg = theme.surface_hover;

        div()
            .id(SharedString::from(format!(
                "remote-home-album-{}-{}",
                album.id, idx
            )))
            .flex_none()
            .w(px(180.0))
            .min_h(px(132.0))
            .p(d.pad_y)
            .rounded(d.r_sm)
            .border_1()
            .border_color(theme.border)
            .bg(theme.surface)
            .hover(move |style| style.bg(hover_bg))
            .on_click(cx.listener(move |view, event: &ClickEvent, _window, cx| {
                if event.click_count() >= 2 {
                    view.state.update(cx, |state, _cx| {
                        state.app.start_remote_add_album_to_queue(
                            album_id_for_click.clone(),
                            title_for_click.clone(),
                            false,
                        );
                    });
                    cx.notify();
                }
            }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(d.grid)
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .text_color(theme.text_primary)
                            .font_weight(FontWeight::SEMIBOLD)
                            .line_height(relative(1.15))
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_secondary)
                            .child(album.artist),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(d.grid)
                            .text_size(d.text_xs)
                            .text_color(theme.text_muted)
                            .child(format!("{} tracks", album.track_count))
                            .when(!year.is_empty(), |el| el.child(year))
                            .when_some(dynamic_range, |el, dr| el.child(dr)),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(d.grid)
                            .mt(d.grid)
                            .child(
                                Button::new(
                                    SharedString::from(format!("remote-home-add-{album_id}-{idx}")),
                                    "Add",
                                )
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Xs)
                                .theme(theme.to_button_theme())
                                .on_click_event(cx.listener(
                                    move |view, _: &ClickEvent, _window, cx| {
                                        let album_id = album_id_for_add.clone();
                                        let title = title_for_add.clone();
                                        view.state.update(cx, |state, _cx| {
                                            state.app.start_remote_add_album_to_queue(
                                                album_id, title, false,
                                            );
                                        });
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                Button::new(
                                    SharedString::from(format!(
                                        "remote-home-play-{album_id}-{idx}"
                                    )),
                                    "Play",
                                )
                                .variant(ButtonVariant::Primary)
                                .size(ButtonSize::Xs)
                                .theme(theme.to_button_theme())
                                .on_click_event(cx.listener(
                                    move |view, _: &ClickEvent, _window, cx| {
                                        let album_id = album_id_for_play.clone();
                                        let title = title_for_play.clone();
                                        view.state.update(cx, |state, _cx| {
                                            state.app.start_remote_add_album_to_queue(
                                                album_id, title, true,
                                            );
                                        });
                                        cx.notify();
                                    },
                                )),
                            ),
                    ),
            )
            .into_any_element()
    }
}

fn add_home_album_to_queue(state: &mut crate::app::AppState, album: &Album, play_now: bool) {
    if let Some(id) = album.id
        && let Some(filtered_idx) = state
            .app
            .filtered_albums()
            .iter()
            .position(|candidate| candidate.id == Some(id))
    {
        state.app.library_state.selected_index = filtered_idx;
        let result = if play_now {
            state.app.play_album_now()
        } else {
            state.app.add_album_to_queue()
        };

        match result {
            Ok(Some(path)) => PlayerView::play_track(state, path),
            Ok(None) => {}
            Err(e) => {
                state.app.ui_state.toast_message = Some(crate::app::ToastMessage::error(e));
            }
        }
    }
}

fn build_home_shelves(albums: &[Album]) -> Vec<HomeShelf> {
    let favorite = prioritize_covers(sort_by_listening(
        albums
            .iter()
            .filter(|album| album.is_favorite)
            .cloned()
            .collect(),
    ));
    let top_listened = prioritize_covers(sort_by_listening(albums.to_vec()));
    let favorite_albums = if favorite.is_empty() {
        top_listened
    } else {
        favorite
    };
    let favorite_row = row_album_keys(&favorite_albums);
    let recommended = prioritize_covers(build_recommended(albums, &favorite_row));
    let mut first_two_rows = favorite_row.clone();
    first_two_rows.extend(row_album_keys(&recommended));
    let discover = prioritize_covers(build_discover(albums, &first_two_rows));

    let mut shelves = vec![
        HomeShelf {
            id: "favorite".to_string(),
            title: "Favorite".to_string(),
            albums: favorite_albums,
        },
        HomeShelf {
            id: "recommended".to_string(),
            title: "Recommended".to_string(),
            albums: recommended,
        },
        HomeShelf {
            id: "discover".to_string(),
            title: "Discover".to_string(),
            albums: discover,
        },
    ];

    shelves.extend(top_genre_shelves(albums));
    shelves
}

fn build_remote_home_shelves(albums: &[SotfApiAlbum]) -> Vec<RemoteHomeShelf> {
    let mut top = albums.to_vec();
    top.sort_by(|a, b| {
        b.play_count
            .cmp(&a.play_count)
            .then_with(|| a.artist.cmp(&b.artist))
            .then_with(|| a.title.cmp(&b.title))
    });

    let mut recent = albums.to_vec();
    recent.sort_by(|a, b| {
        b.year
            .unwrap_or(0)
            .cmp(&a.year.unwrap_or(0))
            .then_with(|| a.artist.cmp(&b.artist))
            .then_with(|| a.title.cmp(&b.title))
    });

    let favorites = top
        .iter()
        .filter(|album| album.is_favorite)
        .cloned()
        .collect::<Vec<_>>();

    let mut shelves = Vec::new();
    if !favorites.is_empty() {
        shelves.push(RemoteHomeShelf {
            id: "remote-favorites".to_string(),
            title: "Favorites".to_string(),
            albums: favorites,
        });
    }

    shelves.push(RemoteHomeShelf {
        id: "remote-albums".to_string(),
        title: if top.iter().any(|album| album.play_count > 0) {
            "Top Albums".to_string()
        } else {
            "Albums".to_string()
        },
        albums: top,
    });

    if recent.iter().any(|album| album.year.is_some()) {
        shelves.push(RemoteHomeShelf {
            id: "remote-recent".to_string(),
            title: "Recently Released".to_string(),
            albums: recent,
        });
    }

    shelves
}

fn sort_by_listening(mut albums: Vec<Album>) -> Vec<Album> {
    albums.sort_by(|a, b| {
        b.play_count
            .cmp(&a.play_count)
            .then_with(|| a.artist().cmp(&b.artist()))
            .then_with(|| a.title.cmp(&b.title))
    });
    albums
}

fn prioritize_covers(mut albums: Vec<Album>) -> Vec<Album> {
    albums.sort_by(|a, b| {
        b.has_cover()
            .cmp(&a.has_cover())
            .then_with(|| b.play_count.cmp(&a.play_count))
            .then_with(|| a.artist().cmp(&b.artist()))
            .then_with(|| a.title.cmp(&b.title))
    });
    albums
}

fn build_recommended(albums: &[Album], excluded: &BTreeSet<String>) -> Vec<Album> {
    let mut seed_genres = BTreeSet::new();
    let mut seed_artists = BTreeSet::new();

    for album in sort_by_listening(albums.to_vec()).into_iter().take(12) {
        if album.is_favorite || album.play_count > 0 {
            seed_artists.insert(album.artist().to_lowercase());
            for genre in album_genres(&album) {
                seed_genres.insert(genre.to_lowercase());
            }
        }
    }

    let mut scored = albums
        .iter()
        .filter(|album| !album.is_favorite && !excluded.contains(&album_key(album)))
        .cloned()
        .map(|album| {
            let genre_score = album_genres(&album)
                .iter()
                .filter(|genre| seed_genres.contains(&genre.to_lowercase()))
                .count();
            let artist_score = usize::from(seed_artists.contains(&album.artist().to_lowercase()));
            let score = genre_score * 4 + artist_score * 2 + album.play_count.min(4);
            (score, album)
        })
        .filter(|(score, _album)| *score > 0)
        .collect::<Vec<_>>();

    scored.sort_by(|(score_a, a), (score_b, b)| {
        score_b
            .cmp(score_a)
            .then_with(|| b.play_count.cmp(&a.play_count))
            .then_with(|| a.title.cmp(&b.title))
    });

    let recommended = scored
        .into_iter()
        .map(|(_score, album)| album)
        .collect::<Vec<_>>();
    if recommended.is_empty() {
        sort_by_listening(
            albums
                .iter()
                .filter(|album| !excluded.contains(&album_key(album)))
                .cloned()
                .collect(),
        )
    } else {
        recommended
    }
}

fn build_discover(albums: &[Album], excluded: &BTreeSet<String>) -> Vec<Album> {
    let mut albums = albums
        .iter()
        .filter(|album| !excluded.contains(&album_key(album)))
        .cloned()
        .collect::<Vec<_>>();
    albums.sort_by_key(stable_album_hash);
    albums
}

fn top_genre_shelves(albums: &[Album]) -> Vec<HomeShelf> {
    let mut by_genre: BTreeMap<String, Vec<Album>> = BTreeMap::new();
    for album in albums {
        for genre in album_genres(album) {
            by_genre.entry(genre).or_default().push(album.clone());
        }
    }

    let mut genres = by_genre.into_iter().collect::<Vec<_>>();
    genres.sort_by(|(genre_a, albums_a), (genre_b, albums_b)| {
        albums_b
            .len()
            .cmp(&albums_a.len())
            .then_with(|| genre_a.cmp(genre_b))
    });

    genres
        .into_iter()
        .take(3)
        .map(|(genre, albums)| HomeShelf {
            id: format!("genre-{}", slug(&genre)),
            title: genre,
            albums: prioritize_covers(sort_by_listening(albums)),
        })
        .collect()
}

fn row_album_keys(albums: &[Album]) -> BTreeSet<String> {
    albums
        .iter()
        .take(COLLAPSED_ALBUM_LIMIT)
        .map(album_key)
        .collect()
}

fn album_key(album: &Album) -> String {
    if let Some(id) = album.id {
        return format!("id:{id}");
    }
    if let Some(uuid) = album.uuid.as_ref() {
        return format!("uuid:{uuid}");
    }
    format!(
        "meta:{}:{}:{:?}",
        album.artist().to_lowercase(),
        album.title.to_lowercase(),
        album.year
    )
}

trait HomeAlbumExt {
    fn has_cover(&self) -> bool;
}

impl HomeAlbumExt for Album {
    fn has_cover(&self) -> bool {
        self.album_art_path.is_some() || self.album_art_thumbnail.is_some()
    }
}

fn album_genres(album: &Album) -> Vec<String> {
    let mut genres = BTreeSet::new();
    for track in &album.tracks {
        if let Some(genre) = track.genre.as_ref() {
            let trimmed = genre.trim();
            if !trimmed.is_empty() {
                genres.insert(trimmed.to_string());
            }
        }
    }
    genres.into_iter().collect()
}

fn stable_album_hash(album: &Album) -> u64 {
    let mut hash = 14_695_981_039_346_656_037u64;
    for byte in format!("{}:{}:{:?}", album.artist(), album.title, album.year).bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(1_099_511_628_211);
    }
    hash
}

fn slug(label: &str) -> String {
    label
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}
