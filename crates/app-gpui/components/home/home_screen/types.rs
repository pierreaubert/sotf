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
#[cfg(feature = "dev-api")]
use crate::app::dev_api::DevTrackExt;
use crate::app::i18n::PhoneTranslations;
use crate::components::design::Ds;
use crate::components::home::album_card::{AlbumCard, AlbumCardMode};
use crate::components::icons::{Icon, IconName, IconSize};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::accessibility::{AccessibilityExt, AccessibilityNode, AriaProps, AriaRole};
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, Heading, IconButton, IconButtonSize, IconButtonVariant,
    Spinner, SpinnerSize, Text,
};
use sotf_audio_player::{Album, sotf_api_client::SotfApiAlbum};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;

const REMOTE_ALBUM_CARD_WIDTH_PX: f32 = 180.0;
const REMOTE_ALBUM_CARD_MIN_HEIGHT_PX: f32 = 132.0;

macro_rules! dev_track {
    ($element:expr, $selector:expr) => {{
        #[cfg(feature = "dev-api")]
        {
            $element.dev_track($selector)
        }
        #[cfg(not(feature = "dev-api"))]
        {
            $element
        }
    }};
}

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

#[derive(Clone)]
enum HomeAlbumTarget {
    Local(Arc<Album>),
    Remote { id: String, title: String },
}

#[derive(Clone)]
struct HomeAlbumEntry {
    shelf_id: String,
    album_index: usize,
    target: HomeAlbumTarget,
}

fn visible_home_album_entries(state: &crate::app::AppState) -> Vec<HomeAlbumEntry> {
    let app = &state.app;
    let ui = &app.ui_state;
    let collapsed_limit = collapsed_album_limit_for_width(ui.window_width);
    let expanded_limit = expanded_album_limit_for_dimensions(
        ui.window_width,
        ui.window_height,
        ui.font_scale,
        ui.min_font_size_px,
        ui.max_font_size_px,
    );

    if app.remote.server_store.selected_server_id.is_some() {
        let Some(page) = app.remote.current_album_page.as_ref() else {
            return Vec::new();
        };
        let mut entries = Vec::new();
        for shelf in build_remote_home_shelves(&page.albums) {
            let limit = if ui.expanded_home_sections.contains(&shelf.id) {
                expanded_limit
            } else {
                collapsed_limit
            };
            for (album_index, source_index) in
                shelf.album_indices.into_iter().take(limit).enumerate()
            {
                if let Some(album) = page.albums.get(source_index) {
                    entries.push(HomeAlbumEntry {
                        shelf_id: shelf.id.clone(),
                        album_index,
                        target: HomeAlbumTarget::Remote {
                            id: album.id.clone(),
                            title: album.title.clone(),
                        },
                    });
                }
            }
        }
        return entries;
    }

    let shelves = cached_home_shelves(
        &app.library_state.library.albums,
        app.library_state.content_generation(),
        collapsed_limit,
        expanded_limit,
    );
    let mut entries = Vec::new();
    for shelf in shelves {
        let limit = if ui.expanded_home_sections.contains(&shelf.id) {
            expanded_limit
        } else {
            collapsed_limit
        };
        for (album_index, album) in shelf.albums.into_iter().take(limit).enumerate() {
            entries.push(HomeAlbumEntry {
                shelf_id: shelf.id.clone(),
                album_index,
                target: HomeAlbumTarget::Local(album),
            });
        }
    }
    entries
}

fn selected_home_entry_index(
    entries: &[HomeAlbumEntry],
    selection: &crate::app::state::library::HomeAlbumSelection,
) -> Option<usize> {
    let shelf_id = selection.shelf_id.as_deref()?;
    entries
        .iter()
        .position(|entry| entry.shelf_id == shelf_id && entry.album_index == selection.album_index)
}

/// Move through exactly the album cards currently visible on the Home screen.
pub(crate) fn move_home_album_selection(state: &mut crate::app::AppState, forward: bool) {
    let entries = visible_home_album_entries(state);
    if entries.is_empty() {
        state.app.library_state.home_album_selection = Default::default();
        return;
    }

    let current =
        selected_home_entry_index(&entries, &state.app.library_state.home_album_selection);
    let next = match (current, forward) {
        (Some(index), true) => (index + 1) % entries.len(),
        (Some(0), false) => entries.len() - 1,
        (Some(index), false) => index - 1,
        (None, true) => 0,
        (None, false) => entries.len() - 1,
    };
    let entry = &entries[next];
    state.app.library_state.home_album_selection.shelf_id = Some(entry.shelf_id.clone());
    state.app.library_state.home_album_selection.album_index = entry.album_index;
}

/// Activate the selected Home album through the same queue path as its card.
pub(crate) fn activate_selected_home_album(state: &mut crate::app::AppState) {
    let entries = visible_home_album_entries(state);
    if entries.is_empty() {
        state.app.library_state.home_album_selection = Default::default();
        return;
    }

    let selected =
        selected_home_entry_index(&entries, &state.app.library_state.home_album_selection)
            .unwrap_or(0);
    let entry = entries[selected].clone();
    state.app.library_state.home_album_selection.shelf_id = Some(entry.shelf_id);
    state.app.library_state.home_album_selection.album_index = entry.album_index;

    match entry.target {
        HomeAlbumTarget::Local(album) => add_home_album_to_queue(state, &album, false),
        HomeAlbumTarget::Remote { id, title } => {
            state.app.start_remote_add_album_to_queue(id, title, false);
        }
    }
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
        let text = PhoneTranslations::for_language(self.state.read(cx).app.ui_state.language);
        let (theme, shelves, expanded_sections, is_loading) = {
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
                state.app.library_view.loading_initial_data,
            )
        };

        div()
            .id("home-screen")
            .flex()
            .flex_col()
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.home_scroll_handle)
            .bg(theme.background)
            .p(d.card)
            .gap(d.section_lg)
            .when(is_loading, |el| {
                el.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size_full()
                        .text_size(d.text_sm)
                        .text_color(theme.text_muted)
                        .child(Spinner::new().size(SpinnerSize::Lg)),
                )
            })
            .when(
                !is_loading && shelves.iter().all(|shelf| shelf.albums.is_empty()),
                |el| {
                    el.child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size_full()
                            .text_size(d.text_sm)
                            .text_color(theme.text_muted)
                            .child(text.home_empty),
                    )
                },
            )
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

    pub(crate) fn render_home_shelf_screen(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let (theme, text, album_label, active_shelf_id, shelf) = {
            let state = self.state.read(cx);
            let ui = &state.app.ui_state;
            let collapsed_limit = collapsed_album_limit_for_width(ui.window_width);
            let active_shelf_id = state
                .app
                .library_state
                .home_album_selection
                .shelf_id
                .clone();
            let shelf = active_shelf_id.as_deref().and_then(|active_id| {
                cached_home_shelves(
                    &state.app.library_state.library.albums,
                    state.app.library_state.content_generation(),
                    collapsed_limit,
                    usize::MAX,
                )
                .into_iter()
                .find(|candidate| candidate.id == active_id)
            });
            (
                ui.theme.clone(),
                PhoneTranslations::for_language(ui.language),
                ui.translations.library_albums.to_lowercase(),
                active_shelf_id,
                shelf,
            )
        };

        let state_for_back = self.state.clone();
        let navigation_id = active_shelf_id
            .as_deref()
            .map(|id| format!("home-shelf-navigation-{id}"))
            .unwrap_or_else(|| "home-shelf-navigation-missing".to_string());
        #[cfg(feature = "dev-api")]
        let navigation_selector = navigation_id.clone();

        let Some(shelf) = shelf else {
            return div()
                .id("home-shelf-screen")
                .flex()
                .flex_col()
                .size_full()
                .bg(theme.background)
                .p(d.card)
                .gap(d.section_lg)
                .child(
                    Button::new(navigation_id, text.back)
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Sm)
                        .theme(theme.to_button_theme())
                        .on_click_event(move |_event, _window, cx| {
                            state_for_back.update(cx, |state, cx| {
                                state
                                    .app
                                    .set_screen(crate::app::Screen::Home, "HomeShelfBack");
                                cx.notify();
                            });
                        }),
                )
                .child(Text::body(text.home_empty).muted(true))
                .into_any_element();
        };

        let shelf_id = shelf.id.clone();
        let selection = self
            .state
            .read(cx)
            .app
            .library_state
            .home_album_selection
            .clone();
        let count_label = format!("{} {}", shelf.total_count, album_label);
        cx.register_accessible(AccessibilityNode {
            element_id: "home-shelf-title".into(),
            label: shelf.title.clone().into(),
            props: AriaProps::with_role(AriaRole::Heading),
        });
        cx.register_accessible(AccessibilityNode {
            element_id: "home-shelf-count".into(),
            label: count_label.clone().into(),
            props: AriaProps::with_role(AriaRole::Status),
        });

        div()
            .id("home-shelf-screen")
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(d.gap_md)
                    .p(d.card)
                    .pb(d.gap)
                    .child(dev_track!(
                        dev_track!(
                            Button::new(navigation_id, text.back)
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Sm)
                                .theme(theme.to_button_theme())
                                .on_click_event(move |_event, _window, cx| {
                                    state_for_back.update(cx, |state, cx| {
                                        state
                                            .app
                                            .set_screen(crate::app::Screen::Home, "HomeShelfBack");
                                        cx.notify();
                                    });
                                }),
                            navigation_selector
                        ),
                        "home.shelf.back"
                    ))
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap(d.grid)
                            .child(Heading::h1(shelf.title.clone()))
                            .child(Text::caption(count_label).muted(true)),
                    ),
            )
            .child(dev_track!(
                div()
                    .id("home-shelf-grid")
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .flex_wrap()
                    .content_start()
                    .overflow_y_scroll()
                    .gap(d.gap_md)
                    .px(d.card)
                    .pb(d.card)
                    .children(shelf.albums.into_iter().enumerate().map(|(idx, album)| {
                        let is_selected = selection.shelf_id.as_deref() == Some(shelf_id.as_str())
                            && selection.album_index == idx;
                        #[cfg(feature = "dev-api")]
                        let album_selector = format!("home.shelf.album.{idx}");
                        dev_track!(
                            div().child(self.render_home_album_card(
                                &shelf_id,
                                album,
                                idx,
                                is_selected,
                                cx,
                            )),
                            album_selector
                        )
                        .into_any_element()
                    })),
                "home.shelf.grid"
            ))
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
        let text = PhoneTranslations::for_language(ui.language);
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
        #[cfg(feature = "dev-api")]
        let see_all_selector = format!("home.see_all.{shelf_id}");
        #[cfg(feature = "dev-api")]
        let navigation_selector = format!("home-shelf-navigation-{shelf_id}");
        let album_shelf_id = shelf.id.clone();
        let selection = state.app.library_state.home_album_selection.clone();
        let state_for_open = self.state.clone();

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
                        let button_theme = theme.clone();
                        el.child(dev_track!(
                            dev_track!(
                                Button::new(
                                    SharedString::from(format!("home-shelf-navigation-{shelf_id}")),
                                    text.see_all,
                                )
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Xs)
                                .theme(button_theme.to_button_theme())
                                .on_click_event(
                                    move |_event, _window, cx| {
                                        state_for_open.update(cx, |state, cx| {
                                            let selection =
                                                &mut state.app.library_state.home_album_selection;
                                            if selection.shelf_id.as_deref()
                                                != Some(shelf_id.as_str())
                                            {
                                                selection.album_index = 0;
                                            }
                                            selection.shelf_id = Some(shelf_id.clone());
                                            state.app.set_screen(
                                                crate::app::Screen::HomeShelf,
                                                "HomeSeeAll",
                                            );
                                            cx.notify();
                                        });
                                    }
                                ),
                                navigation_selector
                            ),
                            see_all_selector
                        ))
                    })
                    .child(div().flex_1()),
            )
            .child(
                div()
                    .flex()
                    .gap(d.gap_md)
                    .when(is_expanded, |el| el.flex_wrap())
                    .when(!is_expanded, |el| el.overflow_hidden())
                    .children(shelf.albums.into_iter().take(limit).enumerate().map(
                        |(idx, album)| {
                            let is_selected = selection.shelf_id.as_deref()
                                == Some(album_shelf_id.as_str())
                                && selection.album_index == idx;
                            self.render_home_album_card(
                                &album_shelf_id,
                                album,
                                idx,
                                is_selected,
                                cx,
                            )
                        },
                    )),
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
        let album_shelf_id = shelf.id.clone();
        let selection = state.app.library_state.home_album_selection.clone();
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
                                albums.get(album_idx).map(|album| {
                                    let is_selected = selection.shelf_id.as_deref()
                                        == Some(album_shelf_id.as_str())
                                        && selection.album_index == idx;
                                    self.render_remote_home_album_card(
                                        &album_shelf_id,
                                        album,
                                        idx,
                                        is_selected,
                                        cx,
                                    )
                                })
                            }),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn render_home_album_card(
        &self,
        shelf_id: &str,
        album: Arc<Album>,
        idx: usize,
        is_selected: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = self.state.read(cx).app.ui_state.theme.clone();
        let shelf_id_for_click = shelf_id.to_string();
        let shelf_id_for_menu = shelf_id.to_string();
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
                    state.app.library_state.home_album_selection.shelf_id =
                        Some(shelf_id_for_click.clone());
                    state.app.library_state.home_album_selection.album_index = idx;
                    add_home_album_to_queue(state, &album_for_click, event.click_count() >= 2);
                });
            }))
            .on_mouse_up(
                MouseButton::Right,
                cx.listener(move |view, event: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, _cx| {
                        state.app.library_state.home_album_selection.shelf_id =
                            Some(shelf_id_for_menu.clone());
                        state.app.library_state.home_album_selection.album_index = idx;
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
                AlbumCard::new(album, idx, is_selected, theme)
                    .mode(AlbumCardMode::Grid)
                    .state(state_entity),
            )
            .into_any_element()
    }

    pub(super) fn render_remote_home_album_card(
        &self,
        shelf_id: &str,
        album: &SotfApiAlbum,
        idx: usize,
        is_selected: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let d = Ds::from_cx(cx);
        let text = PhoneTranslations::for_language(self.state.read(cx).app.ui_state.language);
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
        let shelf_id_for_click = shelf_id.to_string();
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
            .border_color(if is_selected {
                theme.accent
            } else {
                theme.border
            })
            .bg(if is_selected {
                theme.surface_selected
            } else {
                theme.surface
            })
            .hover(move |style| style.bg(hover_bg))
            .on_click(cx.listener(move |view, event: &ClickEvent, _window, cx| {
                view.state.update(cx, |state, _cx| {
                    state.app.library_state.home_album_selection.shelf_id =
                        Some(shelf_id_for_click.clone());
                    state.app.library_state.home_album_selection.album_index = idx;
                    if event.click_count() >= 2 {
                        state.app.start_remote_add_album_to_queue(
                            album_id_for_click.clone(),
                            title_for_click.clone(),
                            true,
                        );
                    }
                });
                cx.notify();
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
                                    text.add,
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
                                    text.play,
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
