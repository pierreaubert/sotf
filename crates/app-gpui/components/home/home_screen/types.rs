use super::album::album_genres;
use super::build::build_home_shelves;
use super::build::build_remote_home_shelves;
use super::misc::add_home_album_to_queue;
use super::misc::arc_album_refs;
use super::misc::collapsed_album_limit_for_width;
use super::misc::expanded_album_limit_for_dimensions;
use super::misc::prioritize_cover_refs;
use super::misc::slug;
use super::misc::sort_album_refs_by_listening;
use crate::components::design::Ds;
use crate::components::home::album_card::{AlbumCard, AlbumCardMode};
use crate::components::icons::{Icon, IconName, IconSize};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, IconButton, IconButtonSize, IconButtonVariant,
};
use sotf_audio_player::{Album, sotf_api_client::SotfApiAlbum};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;

const REMOTE_ALBUM_CARD_WIDTH_PX: f32 = 180.0;
const REMOTE_ALBUM_CARD_MIN_HEIGHT_PX: f32 = 132.0;

#[derive(Clone)]
pub(super) struct HomeShelf {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) total_count: usize,
    pub(super) albums: Vec<Arc<Album>>,
}

#[derive(Clone)]
pub(super) struct RemoteHomeShelf {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) album_indices: Vec<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HomeShelvesCacheKey {
    content_generation: u64,
    album_count: usize,
    album_storage: usize,
    collapsed_limit: usize,
    expanded_limit: usize,
}

#[derive(Clone)]
struct HomeShelvesCacheEntry {
    key: HomeShelvesCacheKey,
    shelves: Vec<HomeShelf>,
}

std::thread_local! {
    static HOME_SHELVES_CACHE: RefCell<Option<HomeShelvesCacheEntry>> = const { RefCell::new(None) };
}

fn cached_home_shelves(
    albums: &[Album],
    content_generation: u64,
    collapsed_limit: usize,
    expanded_limit: usize,
) -> Vec<HomeShelf> {
    let key = HomeShelvesCacheKey {
        content_generation,
        album_count: albums.len(),
        album_storage: albums.as_ptr() as usize,
        collapsed_limit,
        expanded_limit,
    };

    HOME_SHELVES_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(entry) = cache.as_ref()
            && entry.key == key
        {
            return entry.shelves.clone();
        }

        let shelves = build_home_shelves(albums, collapsed_limit, expanded_limit);
        *cache = Some(HomeShelvesCacheEntry {
            key,
            shelves: shelves.clone(),
        });
        shelves
    })
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
            let ui = &state.app.ui_state;
            let collapsed_limit = collapsed_album_limit_for_width(ui.window_width);
            let expanded_limit = expanded_album_limit_for_dimensions(
                ui.window_width,
                ui.window_height,
                ui.font_scale,
                ui.min_font_size_px,
                ui.max_font_size_px,
            );
            (
                ui.theme.clone(),
                cached_home_shelves(
                    &state.app.library_state.library.albums,
                    state.app.library_state.content_generation(),
                    collapsed_limit,
                    expanded_limit,
                ),
                ui.expanded_home_sections.clone(),
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

    pub(super) fn render_remote_home_screen(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let (theme, remote_albums, expanded_sections, is_loading, server_name) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state
                    .app
                    .remote
                    .current_album_page
                    .as_ref()
                    .map(|page| page.albums.clone())
                    .unwrap_or_default(),
                state.app.ui_state.expanded_home_sections.clone(),
                state
                    .app
                    .remote
                    .cache_refresh_requests_in_progress
                    .visible_album_page
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
        let shelves = build_remote_home_shelves(&remote_albums);

        div()
            .id("remote-home-screen")
            .flex()
            .flex_col()
            .size_full()
            .overflow_y_scroll()
            .bg(theme.background)
            .p(d.card)
            .gap(d.section_lg)
            .when(
                shelves.iter().all(|shelf| shelf.album_indices.is_empty()),
                |el| {
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
                },
            )
            .children(
                shelves
                    .into_iter()
                    .filter(|shelf| !shelf.album_indices.is_empty())
                    .map(|shelf| {
                        let is_expanded = expanded_sections.contains(&shelf.id);
                        self.render_remote_home_shelf(shelf, &remote_albums, is_expanded, cx)
                    }),
            )
            .into_any_element()
    }

    pub(super) fn render_home_shelf(
        &self,
        shelf: HomeShelf,
        is_expanded: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let ui = &state.app.ui_state;
        let theme = ui.theme.clone();
        let collapsed_limit = collapsed_album_limit_for_width(ui.window_width);
        let expanded_limit = expanded_album_limit_for_dimensions(
            ui.window_width,
            ui.window_height,
            ui.font_scale,
            ui.min_font_size_px,
            ui.max_font_size_px,
        );
        let limit = if is_expanded {
            expanded_limit
        } else {
            collapsed_limit
        };
        let can_expand = shelf.total_count > collapsed_limit;
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

    pub(super) fn render_remote_home_shelf(
        &self,
        shelf: RemoteHomeShelf,
        albums: &[SotfApiAlbum],
        is_expanded: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let ui = &state.app.ui_state;
        let theme = ui.theme.clone();
        let collapsed_limit = collapsed_album_limit_for_width(ui.window_width);
        let expanded_limit = expanded_album_limit_for_dimensions(
            ui.window_width,
            ui.window_height,
            ui.font_scale,
            ui.min_font_size_px,
            ui.max_font_size_px,
        );
        let limit = if is_expanded {
            expanded_limit
        } else {
            collapsed_limit
        };
        let can_expand = shelf.album_indices.len() > collapsed_limit;
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
                            .album_indices
                            .into_iter()
                            .take(limit)
                            .enumerate()
                            .filter_map(|(idx, album_idx)| {
                                albums
                                    .get(album_idx)
                                    .map(|album| self.render_remote_home_album_card(album, idx, cx))
                            }),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn render_home_album_card(
        &self,
        album: Arc<Album>,
        idx: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = self.state.read(cx).app.ui_state.theme.clone();
        let album_for_click = Arc::clone(&album);
        let album_for_menu = Arc::clone(&album);
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
            .on_mouse_up(
                MouseButton::Right,
                cx.listener(move |view, event: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, _cx| {
                        if let Some(id) = album_for_menu.id
                            && let Some(filtered_idx) = state
                                .app
                                .filtered_albums()
                                .iter()
                                .position(|candidate| candidate.id == Some(id))
                        {
                            state.app.library_state.selected_index = filtered_idx;
                            state.app.ui_state.input_mode = crate::app::InputMode::ContextMenu;
                            state.app.ui_state.context_menu = Some(crate::app::ContextMenuState {
                                menu_type: crate::app::ContextMenuType::Album,
                                position_x: event.position.x.into(),
                                position_y: event.position.y.into(),
                                item_index: filtered_idx,
                            });
                        }
                    });
                    cx.notify();
                }),
            )
            .child(
                AlbumCard::new(album, idx, false, theme)
                    .mode(AlbumCardMode::Grid)
                    .state(state_entity),
            )
            .into_any_element()
    }

    pub(super) fn render_remote_home_album_card(
        &self,
        album: &SotfApiAlbum,
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
        let artist = album.artist.clone();
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
            .w(px(REMOTE_ALBUM_CARD_WIDTH_PX))
            .min_h(px(REMOTE_ALBUM_CARD_MIN_HEIGHT_PX))
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
                            .child(artist),
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

pub(super) fn top_genre_shelves(albums: &[Album], display_limit: usize) -> Vec<HomeShelf> {
    let mut by_genre: BTreeMap<String, Vec<&Album>> = BTreeMap::new();
    for album in albums {
        for genre in album_genres(album) {
            by_genre.entry(genre).or_default().push(album);
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
        .map(|(genre, albums)| {
            let albums = prioritize_cover_refs(sort_album_refs_by_listening(albums));
            HomeShelf {
                id: format!("genre-{}", slug(&genre)),
                title: genre,
                total_count: albums.len(),
                albums: arc_album_refs(&albums, display_limit),
            }
        })
        .collect()
}
