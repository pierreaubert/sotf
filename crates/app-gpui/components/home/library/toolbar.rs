use crate::app::i18n::{LibraryToolbarTranslations, PhoneTranslations};
use crate::app::{InputMode, LibrarySortOrder};
use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, IconButton, IconButtonSize, IconButtonVariant, Select,
    SelectOption, Text,
};
use sotf_audio_player::controllers::LibraryResultOrder;

#[cfg(feature = "dev-api")]
use crate::app::dev_api::DevTrackExt;

impl PlayerView {
    pub(super) fn render_library_toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = self.state.read(cx);
        let app = &state.app;
        let theme = app.ui_state.theme.clone();
        let t = &app.ui_state.translations;
        let text = LibraryToolbarTranslations::for_language(app.ui_state.language);
        let phone = PhoneTranslations::for_language(app.ui_state.language);
        let d = Ds::from_cx(cx);
        let mut views = div().flex().flex_wrap().items_center().gap(d.gap);
        for (id, order, label) in [
            ("album", LibrarySortOrder::Album, t.library_albums),
            ("artist", LibrarySortOrder::Artist, t.library_artists),
            ("tracks", LibrarySortOrder::Tracks, t.library_tracks),
        ] {
            let selected = if order == LibrarySortOrder::Album {
                !matches!(
                    app.library_state.sort_order,
                    LibrarySortOrder::Artist | LibrarySortOrder::Tracks
                )
            } else {
                app.library_state.sort_order == order
            };
            let button = Button::new(format!("library-view-{id}"), label)
                .size(ButtonSize::Sm)
                .selected(selected)
                .variant(if selected {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Ghost
                })
                .theme(theme.to_button_theme())
                .on_click_event(cx.listener(move |view, _, window, cx| {
                    view.state.update(cx, |state, _| {
                        state.app.set_library_sort_order(order);
                        state.app.ui_state.input_mode = InputMode::Normal;
                    });
                    view.focus_handle.focus(window, cx);
                    cx.notify();
                }));
            #[cfg(feature = "dev-api")]
            let button = button.dev_track(format!("library.tab.{id}"));
            views = views.child(button);
        }
        if app.remote.server_store.selected_server_id.is_none() {
            let toggle = cx.entity().downgrade();
            let change = cx.entity().downgrade();
            let highlight = cx.entity().downgrade();
            let selected = match app.library_state.result_order.unwrap_or_default() {
                LibraryResultOrder::Recent => "recent",
                LibraryResultOrder::Title => "title",
                LibraryResultOrder::Artist => "artist",
            };
            let select = Select::new("library-result-order")
                .label(text[6])
                .aria_label(text[6])
                .options(vec![
                    SelectOption::new("recent", text[7]),
                    SelectOption::new("title", text[8]),
                    SelectOption::new("artist", text[9]),
                ])
                .selected(selected)
                .is_open(app.library_state.sort_menu_open)
                .highlighted_index(app.library_state.sort_highlighted_index)
                .on_highlight(move |index, _, cx| {
                    let _ = highlight.update(cx, |view, cx| {
                        view.state.update(cx, |state, _| {
                            state.app.library_state.sort_highlighted_index = index
                        });
                        cx.notify();
                    });
                })
                .theme(theme.to_select_theme())
                .on_toggle(move |open, _, cx| {
                    let _ = toggle.update(cx, |view, cx| {
                        view.state.update(cx, |state, _| {
                            state.app.library_state.sort_menu_open = open;
                            state.app.library_state.sort_highlighted_index = open.then_some(
                                match state.app.library_state.result_order.unwrap_or_default() {
                                    LibraryResultOrder::Recent => 0,
                                    LibraryResultOrder::Title => 1,
                                    LibraryResultOrder::Artist => 2,
                                },
                            );
                        });
                        cx.notify();
                    });
                })
                .on_change(move |value: &SharedString, _, cx| {
                    let order = match value.as_ref() {
                        "recent" => LibraryResultOrder::Recent,
                        "title" => LibraryResultOrder::Title,
                        "artist" => LibraryResultOrder::Artist,
                        _ => return,
                    };
                    let _ = change.update(cx, |view, cx| {
                        view.state.update(cx, |state, _| {
                            state.app.library_state.set_result_order(order);
                            state.app.library_state.ensure_cache_valid();
                            state.app.library_state.ensure_selection_cache_valid();
                            state.app.library_state.sort_menu_open = false;
                        });
                        cx.notify();
                    });
                });
            #[cfg(feature = "dev-api")]
            let select = select.dev_track("library-result-order");
            views = views.child(div().w(rems(12.0)).child(select));
        }
        let toolbar_state = self.state.clone();
        let toolbar_view = cx.entity().clone();
        let mut utilities = div().flex().flex_wrap().items_center().gap(d.gap);
        if !matches!(
            app.library_state.sort_order,
            LibrarySortOrder::Artist | LibrarySortOrder::Tracks
        ) && app.remote.server_store.selected_server_id.is_none()
        {
            let is_list_view = app.library_state.album_list_view;
            let layout_state = toolbar_state.clone();
            let layout_view = toolbar_view.clone();
            let button = IconButton::with_child(
                "library-album-layout",
                Icon::new(if is_list_view {
                    IconName::List
                } else {
                    IconName::LayoutGrid
                })
                .small()
                .color(theme.text_primary),
            )
            .size(IconButtonSize::Sm)
            .variant(IconButtonVariant::Ghost)
            .theme(theme.to_icon_button_theme())
            .aria_label(if is_list_view { text[4] } else { text[5] })
            .on_click_event(move |_, _, cx| {
                layout_state.update(cx, |state, _| {
                    state.app.library_state.album_list_view =
                        !state.app.library_state.album_list_view;
                });
                layout_view.update(cx, |_view, cx| cx.notify());
            });
            #[cfg(feature = "dev-api")]
            let button = button.dev_track("library.layout.toggle");
            utilities = utilities.child(button);
        }
        for (id, label, selected) in [
            ("filter", text[1], app.ui_state.filter_menu_open),
            ("favorites", text[2], app.library_state.show_favorites_only),
        ] {
            let icon = match id {
                "filter" => IconName::Funnel,
                _ => {
                    if selected {
                        IconName::HeartFilled
                    } else {
                        IconName::Heart
                    }
                }
            };
            let tool_state = toolbar_state.clone();
            let tool_view = toolbar_view.clone();
            let button = IconButton::with_child(
                format!("library-tool-{id}"),
                Icon::new(icon).small().color(theme.text_primary),
            )
            .size(IconButtonSize::Sm)
            .selected(selected)
            .variant(if selected {
                IconButtonVariant::Filled
            } else {
                IconButtonVariant::Ghost
            })
            .theme(theme.to_icon_button_theme())
            .aria_label(label)
            .on_click_event(move |_, _, cx| {
                tool_state.update(cx, |state, _| match id {
                    "filter" => {
                        state.app.ui_state.filter_menu_open = !state.app.ui_state.filter_menu_open
                    }
                    "favorites" => state.app.toggle_favorites_filter(),
                    _ => {}
                });
                tool_view.update(cx, |_view, cx| cx.notify());
            });
            #[cfg(feature = "dev-api")]
            let button = button.dev_track(format!("library.tab.{id}"));
            utilities = utilities.child(button);
        }
        if app.library_state.has_active_filters() {
            utilities = utilities.child(
                Button::new("clear-filters", phone.reset)
                    .size(ButtonSize::Sm)
                    .variant(ButtonVariant::Ghost)
                    .theme(theme.to_button_theme())
                    .on_click_event(cx.listener(|view, _, _, cx| {
                        view.state.update(cx, |state, _| {
                            state.app.library_state.clear_all_filters();
                            state.app.library_state.ensure_cache_valid();
                            state.app.library_state.ensure_selection_cache_valid();
                            state.app.ui_state.input_mode = InputMode::Normal;
                        });
                        cx.notify();
                    })),
            );
        }
        let (count, count_label) = if app.remote.server_store.selected_server_id.is_some() {
            (
                app.remote
                    .current_album_page
                    .as_ref()
                    .map(|page| page.total)
                    .unwrap_or(0),
                t.library_albums,
            )
        } else if app.library_state.sort_order == LibrarySortOrder::Tracks {
            (
                app.library_state.selection_filtered_tracks().len(),
                t.library_tracks,
            )
        } else {
            (app.filtered_albums().len(), t.library_albums)
        };
        let mut toolbar = div()
            .flex()
            .flex_col()
            .gap(d.gap)
            .mb(d.gap)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(d.gap)
                    .child(Text::section_header(text[0]))
                    .child(Text::caption(format!("{count} {count_label}"))),
            )
            .child(views)
            .child(utilities);
        if app.ui_state.filter_menu_open {
            let mut advanced = div().flex().flex_wrap().gap(d.gap);
            for (id, order, label) in [
                ("year", LibrarySortOrder::Year, t.library_years),
                ("genre", LibrarySortOrder::Genre, t.library_genres),
                ("composer", LibrarySortOrder::Composer, t.library_composers),
            ] {
                let button = Button::new(format!("library-browse-{id}"), label)
                    .size(ButtonSize::Sm)
                    .variant(ButtonVariant::Ghost)
                    .selected(app.library_state.sort_order == order)
                    .theme(theme.to_button_theme())
                    .on_click_event(cx.listener(move |view, _, _, cx| {
                        view.state
                            .update(cx, |state, _| state.app.set_library_sort_order(order));
                        cx.notify();
                    }));
                #[cfg(feature = "dev-api")]
                let button = button.dev_track(format!("library.tab.{id}"));
                advanced = advanced.child(button);
            }
            toolbar = toolbar.child(advanced);
        }
        toolbar.into_any_element()
    }
}
