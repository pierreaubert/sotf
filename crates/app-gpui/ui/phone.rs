use gpui::StatefulInteractiveElement;

impl PlayerView {
    fn render_phone_shell(
        &mut self,
        current_screen: Screen,
        layout_mode: crate::app::LayoutMode,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = self.state.read(cx).app.ui_state.theme.clone();
        let show_mini_player = {
            let state = self.state.read(cx);
            state.app.playback.current_queue_index.is_some() && current_screen != Screen::NowPlaying
        };

        div()
            .id("phone-shell")
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .bg(theme.background)
            .child(self.render_phone_top_bar(current_screen, cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(self.render_current_screen_phone(current_screen, layout_mode, cx)),
            )
            .when(
                self.state.read(cx).app.federation.scan_progress.is_some(),
                |div| div.child(self.render_federation_scan_progress(cx)),
            )
            .child(self.render_scan_status_row(cx))
            .when(show_mini_player, |div| {
                div.child(self.render_phone_mini_player(cx))
            })
            .child(self.render_phone_tab_bar(current_screen, cx))
            .into_any_element()
    }

    fn render_current_screen_phone(
        &mut self,
        screen: Screen,
        _layout_mode: crate::app::LayoutMode,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match screen {
            Screen::NowPlaying => self.render_phone_now_playing(cx),
            Screen::Settings => self.render_settings_screen_phone(cx),
            Screen::SettingsDetail => self.render_settings_detail_phone(cx),
            Screen::StudioHub => self.render_studio_hub_phone(cx),
            Screen::EqCurve => self.render_phone_eq_curve(cx),
            Screen::Studio => self.render_phone_plugin_rack(cx),
            Screen::Queue => self.render_queue_screen_phone(cx),
            Screen::Library => self.render_library_screen_phone(cx),
            Screen::Home => self.render_home_screen_phone(cx),
            Screen::HomeShelf => self.render_home_shelf_screen_phone(cx),
            Screen::Playlists => self.render_phone_placeholder("Playlists", cx),
            Screen::Spectrum => self.render_phone_spectrum_screen(cx),
            Screen::Recording => {
                let content = self.render_recording_screen(cx).into_any_element();
                self.render_phone_tool_wrapper("Recording", "Full-screen capture flow", content, cx)
            }
            Screen::RoomEq => {
                let content = self.render_room_eq_screen(cx).into_any_element();
                self.render_phone_tool_wrapper("Room EQ", "Wizard", content, cx)
            }
            Screen::HeadphoneEq => {
                let content = self.render_headphone_eq_screen(cx).into_any_element();
                self.render_phone_tool_wrapper("Headphone EQ", "Wizard", content, cx)
            }
            Screen::Spinorama => {
                let content = self.render_spinorama_eq_screen(cx).into_any_element();
                self.render_phone_tool_wrapper("Spinorama", "Speaker EQ", content, cx)
            }
            Screen::PluginGraph => self.render_phone_plugin_graph_screen(cx),
            Screen::Streams => self.render_streams_screen_phone(cx),
        }
    }

    fn render_phone_top_bar(&self, current_screen: Screen, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let (theme, title, is_studio_tool) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                Self::phone_screen_title(current_screen).to_string(),
                current_screen.is_studio_tool()
                    || current_screen == Screen::SettingsDetail
                    || current_screen == Screen::HomeShelf,
            )
        };
        let state_for_back = self.state.clone();
        let state_for_search = self.state.clone();
        let state_for_settings = self.state.clone();

        div()
            .id("phone-top-bar")
            .flex()
            .items_center()
            .justify_between()
            .flex_none()
            .min_h(rems(3.25))
            .px(d.card)
            .bg(theme.surface)
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .id("phone-top-title")
                    .flex()
                    .items_center()
                    .gap(d.grid)
                    .min_w_0()
                    .when(is_studio_tool, |el| {
                        let back_screen = if current_screen == Screen::SettingsDetail {
                            Screen::Settings
                        } else if current_screen == Screen::HomeShelf {
                            Screen::Home
                        } else {
                            Screen::StudioHub
                        };
                        el.cursor_pointer()
                            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                                state_for_back.update(cx, |state, _cx| {
                                    state.app.set_screen(back_screen, "PhoneBack");
                                });
                            })
                            .child(
                                div()
                                    .size(rems(2.75))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(d.r_md)
                                    .child(
                                        Icon::new(IconName::ChevronLeft)
                                            .size(IconSize::Md)
                                            .color(theme.text_primary),
                                    ),
                            )
                    })
                    .child(
                        div()
                            .min_w_0()
                            .text_size(d.text_lg)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(if is_studio_tool {
                                title
                            } else {
                                "SOTF".to_string()
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(d.grid)
                    .child(
                        div()
                            .id("phone-search")
                            .size(rems(2.75))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(d.r_md)
                            .cursor_pointer()
                            .hover({
                                let theme = theme.clone();
                                move |s| s.bg(theme.surface_hover)
                            })
                            .child(
                                Icon::new(IconName::Search)
                                    .size(IconSize::Md)
                                    .color(theme.text_primary),
                            )
                            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                                state_for_search.update(cx, |state, _cx| {
                                    state.app.ui_state.input_mode = crate::app::InputMode::Search;
                                    state.app.set_screen(Screen::Library, "PhoneSearch");
                                });
                            }),
                    )
                    .child(
                        div()
                            .id("phone-settings")
                            .size(rems(2.75))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(d.r_md)
                            .cursor_pointer()
                            .hover({
                                let theme = theme.clone();
                                move |s| s.bg(theme.surface_hover)
                            })
                            .child(
                                Icon::new(IconName::Settings)
                                    .size(IconSize::Md)
                                    .color(theme.text_primary),
                            )
                            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                                state_for_settings.update(cx, |state, _cx| {
                                    state.app.set_screen(Screen::Settings, "PhoneSettings");
                                });
                            }),
                    ),
            )
            .into_any_element()
    }

    fn render_phone_tab_bar(&self, current_screen: Screen, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let theme = self.state.read(cx).app.ui_state.theme.clone();
        let tabs = [
            (Screen::Home, "Home", IconName::Home),
            (Screen::Library, "Library", IconName::Library),
            (Screen::NowPlaying, "Now", IconName::Play),
            (Screen::StudioHub, "Studio", IconName::SlidersHorizontal),
            (Screen::Settings, "Settings", IconName::Settings),
        ];

        div()
            .id("phone-tab-bar")
            .flex()
            .items_center()
            .justify_between()
            .flex_none()
            .min_h(rems(4.25))
            .px(d.pad_y)
            .py(d.grid)
            .bg(theme.surface)
            .border_t_1()
            .border_color(theme.border)
            .children(tabs.into_iter().map(|(screen, label, icon)| {
                let selected = current_screen == screen
                    || (screen == Screen::Home && current_screen == Screen::HomeShelf)
                    || (screen == Screen::StudioHub && current_screen.is_studio_tool())
                    || (screen == Screen::Settings && current_screen == Screen::SettingsDetail)
                    || (screen == Screen::NowPlaying && current_screen == Screen::Queue);
                self.render_phone_tab_item(screen, label, icon, selected, &theme, &d)
            }))
            .into_any_element()
    }

    fn render_phone_tab_item(
        &self,
        screen: Screen,
        label: &'static str,
        icon: IconName,
        selected: bool,
        theme: &crate::theme::Theme,
        d: &Ds,
    ) -> AnyElement {
        let state_entity = self.state.clone();
        let fg = if selected {
            theme.accent
        } else {
            theme.text_muted
        };

        div()
            .id(SharedString::from(format!("phone-tab-{screen:?}")))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(crate::app::constants::spacing::XS)
            .min_w(rems(3.5))
            .min_h(rems(3.5))
            .rounded(d.r_md)
            .text_color(fg)
            .cursor_pointer()
            .when(selected, |el| el.bg(theme.surface_selected))
            .when(!selected, |el| {
                let theme = theme.clone();
                el.hover(move |s| s.bg(theme.surface_hover))
            })
            .child(Icon::new(icon).size(IconSize::Md).color(fg))
            .child(
                div()
                    .text_size(d.text_xs)
                    .font_weight(if selected {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .child(label),
            )
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                state_entity.update(cx, |state, _cx| {
                    state.app.set_screen(screen, "PhoneTab");
                    state.app.ui_state.input_mode = crate::app::InputMode::Normal;
                });
            })
            .into_any_element()
    }

    fn render_home_screen_phone(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let albums = &state.app.library_state.library.albums;

        if albums.is_empty() {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .p(d.card)
                .bg(theme.background)
                .text_color(theme.text_muted)
                .child("Add albums to your library to build Home shelves.")
                .into_any_element();
        }

        let mut recently_played = albums.iter().collect::<Vec<_>>();
        recently_played.sort_by_key(|album| std::cmp::Reverse(album.play_count));

        let mut most_played = albums.iter().collect::<Vec<_>>();
        most_played.sort_by(|a, b| {
            b.play_count
                .cmp(&a.play_count)
                .then_with(|| a.title.cmp(&b.title))
        });

        let favorites = albums
            .iter()
            .filter(|album| album.is_favorite)
            .collect::<Vec<_>>();

        let mut recent_releases = albums.iter().collect::<Vec<_>>();
        recent_releases.sort_by_key(|album| std::cmp::Reverse(album.year));

        div()
            .id("phone-home")
            .size_full()
            .overflow_y_scroll()
            .bg(theme.background)
            .p(d.card)
            .flex()
            .flex_col()
            .gap(d.section_lg)
            .child(self.render_phone_home_shelf(
                crate::app::PhoneHomeShelf::RecentlyPlayed,
                recently_played.into_iter().take(12).collect(),
                &theme,
                &d,
            ))
            .child(self.render_phone_home_shelf(
                crate::app::PhoneHomeShelf::MostPlayed,
                most_played.into_iter().take(12).collect(),
                &theme,
                &d,
            ))
            .when(!favorites.is_empty(), |el| {
                el.child(self.render_phone_home_shelf(
                    crate::app::PhoneHomeShelf::Favorites,
                    favorites.into_iter().take(12).collect(),
                    &theme,
                    &d,
                ))
            })
            .child(self.render_phone_home_shelf(
                crate::app::PhoneHomeShelf::NewInLibrary,
                recent_releases.into_iter().take(12).collect(),
                &theme,
                &d,
            ))
            .into_any_element()
    }

    fn render_phone_home_shelf(
        &self,
        shelf: crate::app::PhoneHomeShelf,
        albums: Vec<&sotf_audio_player::Album>,
        theme: &crate::theme::Theme,
        d: &Ds,
    ) -> AnyElement {
        let state_for_see_all = self.state.clone();
        let title = shelf.title();

        div()
            .flex()
            .flex_col()
            .gap(d.grid)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(d.text_lg)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .child(title),
                    )
                    .child(
                        div()
                            .min_h(rems(2.75))
                            .px(d.pad_x)
                            .flex()
                            .items_center()
                            .rounded(d.r_md)
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.accent)
                            .cursor_pointer()
                            .child("See all")
                            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                                state_for_see_all.update(cx, |state, _cx| {
                                    state.app.ui_state.phone_home_shelf = shelf;
                                    state.app.set_screen(Screen::HomeShelf, "PhoneHomeSeeAll");
                                });
                            }),
                    ),
            )
            .child(
                div()
                    .id(SharedString::from(format!("phone-home-shelf-{title}")))
                    .flex()
                    .gap(d.gap_md)
                    .overflow_x_scroll()
                    .children(albums.into_iter().enumerate().map(|(idx, album)| {
                        self.render_phone_album_tile(
                            idx,
                            album,
                            Some(8.5),
                            "PhoneHomeAlbum",
                            theme,
                            d,
                        )
                    })),
            )
            .into_any_element()
    }

    fn render_home_shelf_screen_phone(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let shelf = state.app.ui_state.phone_home_shelf;
        let columns = if state.app.ui_state.window_width >= 430.0 {
            3
        } else {
            2
        };
        let mut albums = state
            .app
            .library_state
            .library
            .albums
            .iter()
            .filter(|album| shelf != crate::app::PhoneHomeShelf::Favorites || album.is_favorite)
            .collect::<Vec<_>>();
        match shelf {
            crate::app::PhoneHomeShelf::RecentlyPlayed | crate::app::PhoneHomeShelf::MostPlayed => {
                albums.sort_by(|a, b| {
                    b.play_count
                        .cmp(&a.play_count)
                        .then_with(|| a.title.cmp(&b.title))
                });
            }
            crate::app::PhoneHomeShelf::Favorites => {}
            crate::app::PhoneHomeShelf::NewInLibrary => {
                albums.sort_by(|a, b| b.year.cmp(&a.year).then_with(|| a.title.cmp(&b.title)));
            }
        }

        div()
            .id("phone-home-shelf-grid")
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .bg(theme.background)
            .child(
                div()
                    .flex_none()
                    .px(d.card)
                    .py(d.grid)
                    .text_size(d.text_sm)
                    .text_color(theme.text_muted)
                    .child(format!("{} albums", albums.len())),
            )
            .child(
                div()
                    .id("phone-home-shelf-grid-scroll")
                    .overflow_y_scroll()
                    .flex_1()
                    .min_h_0()
                    .p(d.card)
                    .pt(d.grid)
                    .child(div().grid().grid_cols(columns).gap(d.gap_md).children(
                        albums.into_iter().enumerate().map(|(idx, album)| {
                            self.render_phone_album_tile(
                                idx,
                                album,
                                None,
                                shelf.title(),
                                &theme,
                                &d,
                            )
                        }),
                    )),
            )
            .into_any_element()
    }

    fn render_library_screen_phone(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let columns = if state.app.ui_state.window_width >= 430.0 {
            3
        } else {
            2
        };
        let theme = state.app.ui_state.theme.clone();
        let search_query = state.app.library_state.search_query.clone();
        let filter_menu_open = state.app.ui_state.filter_menu_open;
        let albums = state.app.get_paginated_albums();
        let app_state = self.state.clone();
        let view_handle = cx.entity().clone();

        div()
            .id("phone-library")
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .bg(theme.background)
            .child(
                div().flex_none().p(d.card).pb(d.grid).child(
                    gpui_ui_kit::SearchBar::new("phone-library-search")
                        .value(search_query)
                        .placeholder("Search albums, artists, tracks")
                        .size(gpui_ui_kit::SearchBarSize::Sm)
                        .on_change(move |text, _window, cx| {
                            app_state.update(cx, |state, _| {
                                state.app.set_library_search_query(text.to_string());
                                state.app.ui_state.input_mode = crate::app::InputMode::Search;
                                if state.app.remote.server_store.selected_server_id.is_some() {
                                    state.app.remote.clear_remote_album_page();
                                    state.app.remote.refresh_requests.visible_album_page = true;
                                }
                            });
                            view_handle.update(cx, |_, cx| cx.notify());
                        }),
                ),
            )
            .child(self.render_phone_library_chips(filter_menu_open, &theme, &d))
            .child(
                div()
                    .id("phone-library-grid-scroll")
                    .overflow_y_scroll()
                    .flex_1()
                    .min_h_0()
                    .p(d.card)
                    .pt(d.grid)
                    .child(div().grid().grid_cols(columns).gap(d.gap_md).children(
                        albums.into_iter().enumerate().map(|(idx, album)| {
                            self.render_phone_album_tile(
                                idx,
                                album,
                                None,
                                "PhoneLibraryAlbum",
                                &theme,
                                &d,
                            )
                        }),
                    )),
            )
            .into_any_element()
    }

    fn render_phone_library_chips(
        &self,
        filter_menu_open: bool,
        theme: &crate::theme::Theme,
        d: &Ds,
    ) -> AnyElement {
        let chips = [
            (
                "Year",
                Some(sotf_audio_player::LibrarySortOrder::Year),
                None,
            ),
            (
                "Genre",
                Some(sotf_audio_player::LibrarySortOrder::Genre),
                None,
            ),
            (
                "Artist",
                Some(sotf_audio_player::LibrarySortOrder::Artist),
                None,
            ),
            (
                "Album",
                Some(sotf_audio_player::LibrarySortOrder::Album),
                None,
            ),
            ("More", None, None),
        ];
        let overflow_chips = [
            (
                "Tracks",
                Some(sotf_audio_player::LibrarySortOrder::Tracks),
                None,
            ),
            (
                "Composer",
                Some(sotf_audio_player::LibrarySortOrder::Composer),
                None,
            ),
            (
                "Stereo",
                None,
                Some(sotf_audio_player::ChannelFilter::Stereo),
            ),
            (
                "Multichannel",
                None,
                Some(sotf_audio_player::ChannelFilter::SurroundPlus),
            ),
            ("Reset", None, Some(sotf_audio_player::ChannelFilter::All)),
        ];

        div()
            .flex_none()
            .flex()
            .flex_col()
            .gap(d.grid)
            .px(d.card)
            .pb(d.grid)
            .child(
                div()
                    .id("phone-library-primary-chips")
                    .flex()
                    .gap(d.grid)
                    .overflow_x_scroll()
                    .children(chips.into_iter().map(|(label, sort, filter)| {
                        self.render_phone_library_chip(label, sort, filter, theme, d)
                    })),
            )
            .when(filter_menu_open, |el| {
                el.child(
                    div()
                        .id("phone-library-more-chips")
                        .flex()
                        .gap(d.grid)
                        .overflow_x_scroll()
                        .children(overflow_chips.into_iter().map(|(label, sort, filter)| {
                            self.render_phone_library_chip(label, sort, filter, theme, d)
                        })),
                )
            })
            .into_any_element()
    }

    fn render_phone_library_chip(
        &self,
        label: &'static str,
        sort: Option<sotf_audio_player::LibrarySortOrder>,
        filter: Option<sotf_audio_player::ChannelFilter>,
        theme: &crate::theme::Theme,
        d: &Ds,
    ) -> AnyElement {
        let state_entity = self.state.clone();

        div()
            .id(SharedString::from(format!("phone-library-chip-{label}")))
            .min_h(rems(2.75))
            .px(d.pad_x)
            .flex()
            .items_center()
            .rounded_full()
            .border_1()
            .border_color(theme.border)
            .bg(theme.surface)
            .text_size(d.text_sm)
            .text_color(theme.text_primary)
            .whitespace_nowrap()
            .cursor_pointer()
            .hover({
                let theme = theme.clone();
                move |s| s.bg(theme.surface_hover)
            })
            .child(label)
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                state_entity.update(cx, |state, _cx| {
                    if label == "More" {
                        state.app.ui_state.filter_menu_open = !state.app.ui_state.filter_menu_open;
                        return;
                    }
                    if label == "Reset" {
                        state
                            .app
                            .library_state
                            .set_sort_order(sotf_audio_player::LibrarySortOrder::Album);
                        state
                            .app
                            .library_state
                            .set_filter(sotf_audio_player::ChannelFilter::All);
                        state.app.library_state.show_favorites_only = false;
                    }
                    if let Some(sort) = sort {
                        state.app.library_state.set_sort_order(sort);
                    }
                    if let Some(filter) = filter {
                        state.app.library_state.set_filter(filter);
                    }
                });
            })
            .into_any_element()
    }

    fn render_queue_screen_phone(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let (theme, rows, current_index, editing) = {
            let state = self.state.read(cx);
            let rows =
                state
                    .app
                    .queue_state
                    .items
                    .iter()
                    .enumerate()
                    .flat_map(|(album_idx, item)| {
                        item.album.tracks.iter().cloned().enumerate().map(
                            move |(track_idx, track)| {
                                (album_idx, track_idx, item.album.title.clone(), track)
                            },
                        )
                    })
                    .collect::<Vec<_>>();
            (
                state.app.ui_state.theme.clone(),
                rows,
                state.app.queue_state.current_index(),
                state.app.ui_state.phone_queue_editing,
            )
        };
        let state_for_edit = self.state.clone();

        div()
            .id("phone-queue")
            .size_full()
            .bg(theme.background)
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(d.card)
                    .py(d.grid)
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .text_color(theme.text_muted)
                            .child(format!("{} tracks", rows.len())),
                    )
                    .child(
                        div()
                            .min_h(rems(2.75))
                            .px(d.pad_x)
                            .flex()
                            .items_center()
                            .rounded(d.r_md)
                            .bg(if editing {
                                theme.surface_selected
                            } else {
                                theme.surface
                            })
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(if editing {
                                theme.accent
                            } else {
                                theme.text_primary
                            })
                            .cursor_pointer()
                            .child(if editing { "Done" } else { "Edit" })
                            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                                state_for_edit.update(cx, |state, _cx| {
                                    state.app.ui_state.phone_queue_editing =
                                        !state.app.ui_state.phone_queue_editing;
                                });
                            }),
                    ),
            )
            .child(
                div()
                    .id("phone-queue-track-scroll")
                    .overflow_y_scroll()
                    .flex_1()
                    .min_h_0()
                    .p(d.card)
                    .flex()
                    .flex_col()
                    .gap(d.grid)
                    .when(rows.is_empty(), |el| {
                        el.child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .min_h(rems(12.0))
                                .text_color(theme.text_muted)
                                .child("Queue is empty."),
                        )
                    })
                    .children(rows.into_iter().map(
                        |(album_idx, track_idx, album_title, track)| {
                            self.render_phone_queue_track_row(
                                album_idx,
                                track_idx,
                                album_title,
                                track,
                                current_index == Some(album_idx),
                                editing,
                                &theme,
                                &d,
                            )
                        },
                    )),
            )
            .into_any_element()
    }

    fn render_phone_queue_track_row(
        &self,
        album_idx: usize,
        track_idx: usize,
        album_title: String,
        track: sotf_audio_player::Track,
        is_current_album: bool,
        editing: bool,
        theme: &crate::theme::Theme,
        d: &Ds,
    ) -> AnyElement {
        let state_for_play = self.state.clone();
        let state_for_remove = self.state.clone();
        let is_current = is_current_album;
        let title = track.title.clone().unwrap_or_else(|| {
            track
                .path
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("Untitled")
                .to_string()
        });
        let artist = track
            .artist
            .clone()
            .unwrap_or_else(|| "Unknown artist".to_string());
        let duration = track
            .duration_secs
            .map(|seconds| Self::format_phone_time(seconds as f64))
            .unwrap_or_else(|| "--:--".to_string());
        let source = track.audio_source();

        div()
            .id(SharedString::from(format!(
                "phone-queue-row-{album_idx}-{track_idx}"
            )))
            .flex()
            .items_center()
            .gap(d.gap_md)
            .min_h(rems(4.25))
            .p(d.pad_y)
            .rounded(d.r_md)
            .bg(if is_current {
                theme.surface_selected
            } else {
                theme.surface
            })
            .border_1()
            .border_color(if is_current {
                theme.accent
            } else {
                theme.border
            })
            .cursor_pointer()
            .child(
                div()
                    .size(rems(2.75))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(d.r_md)
                    .bg(theme.background_secondary)
                    .child(
                        Icon::new(if is_current {
                            IconName::Play
                        } else {
                            IconName::ListMusic
                        })
                        .size(IconSize::Sm)
                        .color(theme.accent),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_muted)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(format!("{} - {}", artist, album_title)),
                    ),
            )
            .child(
                div()
                    .text_size(d.text_xs)
                    .text_color(theme.text_muted)
                    .child(duration),
            )
            .when(editing, |el| {
                el.child(
                    div()
                        .size(rems(2.75))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(d.r_md)
                        .hover({
                            let theme = theme.clone();
                            move |s| s.bg(theme.surface_hover)
                        })
                        .child(
                            Icon::new(IconName::X)
                                .size(IconSize::Sm)
                                .color(theme.text_muted),
                        )
                        .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                            cx.stop_propagation();
                            state_for_remove.update(cx, |state, _cx| {
                                if album_idx >= state.app.queue_state.items.len() {
                                    return;
                                }
                                if track_idx
                                    < state.app.queue_state.items[album_idx].album.tracks.len()
                                {
                                    state.app.queue_state.items[album_idx]
                                        .album
                                        .tracks
                                        .remove(track_idx);
                                }
                                if state.app.queue_state.items[album_idx]
                                    .album
                                    .tracks
                                    .is_empty()
                                {
                                    match state.app.remove_from_queue(album_idx) {
                                        sotf_audio_player::QueuePlaybackEffect::Reload(source) => {
                                            PlayerView::play_track(state, source);
                                        }
                                        sotf_audio_player::QueuePlaybackEffect::Stop => {
                                            let _ = state.player.lock().stop();
                                        }
                                        _ => {}
                                    }
                                } else {
                                    let current_track_index =
                                        state.app.queue_state.items[album_idx].current_track_index;
                                    if current_track_index
                                        >= state.app.queue_state.items[album_idx].album.tracks.len()
                                    {
                                        state.app.queue_state.items[album_idx]
                                            .current_track_index = state.app.queue_state.items
                                            [album_idx]
                                            .album
                                            .tracks
                                            .len()
                                            - 1;
                                    }
                                }
                            });
                        }),
                )
            })
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                state_for_play.update(cx, |state, _cx| {
                    if album_idx < state.app.queue_state.items.len()
                        && track_idx < state.app.queue_state.items[album_idx].album.tracks.len()
                    {
                        state.app.queue_state.selected_index = album_idx;
                        state.app.queue_state.current_index = Some(album_idx);
                        state.app.queue_state.items[album_idx].current_track_index = track_idx;
                        state.app.playback.current_queue_index = Some(album_idx);
                        state.app.playback.is_playing = true;
                        PlayerView::play_track(state, source.clone());
                    }
                });
            })
            .into_any_element()
    }

    fn render_phone_album_tile(
        &self,
        idx: usize,
        album: &sotf_audio_player::Album,
        width_rems: Option<f32>,
        trigger: &'static str,
        theme: &crate::theme::Theme,
        d: &Ds,
    ) -> AnyElement {
        let state_entity = self.state.clone();
        let title = album.title.clone();
        let artist = album.artist();
        let album_id = album.id;
        let art_path = album.album_art_path.clone();

        div()
            .id(SharedString::from(format!("phone-album-{trigger}-{idx}")))
            .when_some(width_rems, |el, width| el.w(rems(width)))
            .when(width_rems.is_none(), |el| el.w_full())
            .flex_none()
            .flex()
            .flex_col()
            .gap(d.grid)
            .cursor_pointer()
            .child(self.render_phone_album_art(art_path, rems(8.5), theme, d))
            .child(
                div()
                    .text_size(d.text_sm)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text_primary)
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(title.clone()),
            )
            .child(
                div()
                    .text_size(d.text_xs)
                    .text_color(theme.text_muted)
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(artist.clone()),
            )
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                let selected_title = title.clone();
                let selected_artist = artist.clone();
                state_entity.update(cx, |state, _cx| {
                    let resolved_idx = state
                        .app
                        .filtered_albums()
                        .iter()
                        .position(|candidate| {
                            album_id
                                .zip(candidate.id)
                                .is_some_and(|(lhs, rhs)| lhs == rhs)
                                || (candidate.title == selected_title
                                    && candidate.artist() == selected_artist)
                        })
                        .unwrap_or(idx);
                    state.app.library_state.selected_index = resolved_idx;
                    match state.app.play_album_now() {
                        Ok(Some(source)) => PlayerView::play_track(state, source),
                        Err(e) => {
                            state.app.ui_state.toast_message =
                                Some(crate::app::ToastMessage::error(e));
                        }
                        _ => {}
                    }
                    state.app.set_screen(Screen::Queue, trigger);
                });
            })
            .into_any_element()
    }

    fn render_phone_album_art(
        &self,
        art_path: Option<std::path::PathBuf>,
        size: gpui::Rems,
        theme: &crate::theme::Theme,
        d: &Ds,
    ) -> AnyElement {
        let art = div()
            .w(size)
            .h(size)
            .rounded(d.r_md)
            .overflow_hidden()
            .bg(theme.background_secondary)
            .flex()
            .items_center()
            .justify_center()
            .text_color(theme.text_muted);

        if let Some(path) = art_path {
            art.child(img(path).w_full().h_full().object_fit(ObjectFit::Cover))
                .into_any_element()
        } else {
            art.child(
                Icon::new(IconName::Music)
                    .size(IconSize::Lg)
                    .color(theme.accent),
            )
            .into_any_element()
        }
    }

    fn render_phone_mini_player(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let (theme, title, artist, art_path, is_playing) = {
            let state = self.state.read(cx);
            let item = state
                .app
                .playback
                .current_queue_index
                .and_then(|idx| state.app.queue_state.get(idx));
            let track = item.and_then(|item| item.current_track());
            (
                state.app.ui_state.theme.clone(),
                track
                    .and_then(|track| track.title.clone())
                    .unwrap_or_else(|| "Now Playing".to_string()),
                track
                    .and_then(|track| track.artist.clone())
                    .unwrap_or_else(|| "SOTF".to_string()),
                item.and_then(|item| item.album.album_art_path.clone()),
                state.app.playback.is_playing,
            )
        };
        let state_for_expand = self.state.clone();

        div()
            .id("phone-mini-player")
            .flex()
            .items_center()
            .gap(d.gap_md)
            .min_h(rems(3.5))
            .px(d.card)
            .bg(theme.surface)
            .border_t_1()
            .border_color(theme.border)
            .cursor_pointer()
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                state_for_expand.update(cx, |state, _cx| {
                    state.app.set_screen(Screen::NowPlaying, "PhoneMiniPlayer");
                });
            })
            .child(self.render_phone_album_art(art_path, rems(2.25), &theme, &d))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text_primary)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_muted)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(artist),
                    ),
            )
            .child(self.render_phone_transport_button(
                "phone-mini-next",
                IconName::SkipForward,
                "PhoneMiniNext",
                &theme,
                cx,
            ))
            .child(self.render_phone_play_button("phone-mini-play", is_playing, &theme, cx))
            .into_any_element()
    }

    fn render_phone_now_playing(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let (
            theme,
            title,
            artist,
            album,
            art_path,
            is_playing,
            position,
            duration,
            shuffle_enabled,
            repeat_enabled,
        ) = {
            let state = self.state.read(cx);
            let item = state
                .app
                .playback
                .current_queue_index
                .and_then(|idx| state.app.queue_state.get(idx));
            let track = item.and_then(|item| item.current_track());
            (
                state.app.ui_state.theme.clone(),
                track
                    .and_then(|track| track.title.clone())
                    .unwrap_or_else(|| "Nothing playing".to_string()),
                track
                    .and_then(|track| track.artist.clone())
                    .unwrap_or_else(|| "Choose music from Library".to_string()),
                item.map(|item| item.album.title.clone())
                    .unwrap_or_else(|| "SOTF".to_string()),
                item.and_then(|item| item.album.album_art_path.clone()),
                state.app.playback.is_playing,
                state.app.playback.position_secs,
                state.app.playback.duration_secs,
                state.app.ui_state.phone_shuffle_enabled,
                state.app.ui_state.phone_repeat_enabled,
            )
        };
        let progress = if duration > 0.0 {
            (position / duration).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        div()
            .id("phone-now-playing")
            .flex()
            .flex_col()
            .items_center()
            .size_full()
            .overflow_y_scroll()
            .p(d.card)
            .gap(d.section_lg)
            .bg(theme.background)
            .child(
                div()
                    .w_full()
                    .max_w(rems(22.0))
                    .child(self.render_phone_album_art(art_path, rems(22.0), &theme, &d)),
            )
            .child(
                div()
                    .w_full()
                    .max_w(rems(26.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(d.gap_md)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(d.text_lg)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text_primary)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .whitespace_nowrap()
                                    .child(title),
                            )
                            .child(
                                div()
                                    .text_size(d.text_sm)
                                    .text_color(theme.text_secondary)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .whitespace_nowrap()
                                    .child(artist),
                            )
                            .child(
                                div()
                                    .text_size(d.text_xs)
                                    .text_color(theme.text_muted)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .whitespace_nowrap()
                                    .child(album),
                            ),
                    )
                    .child(
                        Icon::new(IconName::Heart)
                            .size(IconSize::Lg)
                            .color(theme.text_muted),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .max_w(rems(26.0))
                    .flex()
                    .flex_col()
                    .gap(d.grid)
                    .child(
                        div()
                            .id("phone-now-scrubber")
                            .w_full()
                            .h(rems(0.375))
                            .rounded_full()
                            .bg(theme.feedback.progress_bar_bg)
                            .overflow_hidden()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, {
                                let state_entity = self.state.clone();
                                move |event, window, cx| {
                                    state_entity.update(cx, |state, _cx| {
                                        let duration = state.app.playback.duration_secs;
                                        if duration <= 0.0 {
                                            return;
                                        }
                                        let width: f32 = window.bounds().size.width.into();
                                        let x: f32 = event.position.x.into();
                                        let ratio = if width > 0.0 {
                                            (x / width).clamp(0.0, 1.0)
                                        } else {
                                            0.0
                                        };
                                        let new_position = duration * ratio as f64;
                                        state.app.playback.position_secs = new_position;
                                        if let Err(e) = state.player.lock().seek(new_position) {
                                            log::error!(
                                                "Failed to seek from phone scrubber: {}",
                                                e
                                            );
                                        }
                                    });
                                }
                            })
                            .child(
                                div()
                                    .h_full()
                                    .w(relative(progress))
                                    .rounded_full()
                                    .bg(theme.feedback.progress_bar_fill),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .text_size(d.text_xs)
                            .text_color(theme.text_muted)
                            .child(Self::format_phone_time(position))
                            .child(Self::format_phone_time(duration)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(d.gap_md)
                    .child(self.render_phone_transport_button(
                        "phone-now-prev",
                        IconName::SkipBack,
                        "PhoneNowPrev",
                        &theme,
                        cx,
                    ))
                    .child(self.render_phone_transport_button(
                        "phone-now-rewind",
                        IconName::Rewind,
                        "PhoneNowRewind",
                        &theme,
                        cx,
                    ))
                    .child(self.render_phone_play_button("phone-now-play", is_playing, &theme, cx))
                    .child(self.render_phone_transport_button(
                        "phone-now-forward",
                        IconName::FastForward,
                        "PhoneNowForward",
                        &theme,
                        cx,
                    ))
                    .child(self.render_phone_transport_button(
                        "phone-now-next",
                        IconName::SkipForward,
                        "PhoneNowNext",
                        &theme,
                        cx,
                    )),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(d.gap_md)
                    .child(self.render_phone_icon_button(
                        "phone-shuffle",
                        IconName::Shuffle,
                        &theme,
                        None,
                        Some(shuffle_enabled),
                    ))
                    .child(self.render_phone_icon_button(
                        "phone-repeat",
                        IconName::Repeat,
                        &theme,
                        None,
                        Some(repeat_enabled),
                    ))
                    .child(self.render_phone_icon_button(
                        "phone-output",
                        IconName::Speaker,
                        &theme,
                        Some(Screen::SettingsDetail),
                        None,
                    ))
                    .child(
                        div()
                            .size(rems(2.75))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(d.r_md)
                            .cursor_pointer()
                            .child(
                                Icon::new(IconName::ListMusic)
                                    .size(IconSize::Lg)
                                    .color(theme.text_muted),
                            )
                            .on_mouse_up(MouseButton::Left, {
                                let state_entity = self.state.clone();
                                move |_event, _window, cx| {
                                    state_entity.update(cx, |state, _cx| {
                                        state.app.set_screen(Screen::Queue, "PhoneQueueButton");
                                    });
                                }
                            }),
                    ),
            )
            .child(self.render_phone_now_playing_drawer(&theme, &d, cx))
            .into_any_element()
    }

    fn render_phone_icon_button(
        &self,
        id: &'static str,
        icon: IconName,
        theme: &crate::theme::Theme,
        target_screen: Option<Screen>,
        selected: Option<bool>,
    ) -> AnyElement {
        let state_entity = self.state.clone();
        let is_selected = selected.unwrap_or(false);

        div()
            .id(id)
            .size(rems(2.75))
            .flex()
            .items_center()
            .justify_center()
            .rounded(rems(0.5))
            .when(is_selected, |el| el.bg(theme.surface_selected))
            .cursor_pointer()
            .hover({
                let theme = theme.clone();
                move |s| s.bg(theme.surface_hover)
            })
            .child(Icon::new(icon).size(IconSize::Lg).color(if is_selected {
                theme.accent
            } else {
                theme.text_muted
            }))
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                state_entity.update(cx, |state, _cx| {
                    match id {
                        "phone-shuffle" => {
                            state.app.ui_state.phone_shuffle_enabled =
                                !state.app.ui_state.phone_shuffle_enabled;
                        }
                        "phone-repeat" => {
                            state.app.ui_state.phone_repeat_enabled =
                                !state.app.ui_state.phone_repeat_enabled;
                        }
                        _ => {}
                    }
                    if let Some(screen) = target_screen {
                        if screen == Screen::SettingsDetail {
                            state.app.ui_state.active_settings_tab =
                                crate::app::SettingsTab::AudioDevice;
                        }
                        state.app.set_screen(screen, "PhoneIconButton");
                    }
                });
            })
            .into_any_element()
    }

    fn render_phone_now_playing_drawer(
        &self,
        theme: &crate::theme::Theme,
        d: &Ds,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (queue_rows, plugins) = {
            let state = self.state.read(cx);
            let start = state
                .app
                .queue_state
                .current_index()
                .map(|idx| idx + 1)
                .unwrap_or(0);
            (
                state
                    .app
                    .queue_state
                    .iter()
                    .skip(start)
                    .take(3)
                    .cloned()
                    .collect::<Vec<_>>(),
                state
                    .app
                    .plugin_state
                    .graph
                    .plugins()
                    .into_iter()
                    .take(4)
                    .map(|plugin| {
                        format!(
                            "{} {}",
                            if plugin.enabled { "On" } else { "Off" },
                            plugin.display_name()
                        )
                    })
                    .collect::<Vec<_>>(),
            )
        };

        div()
            .w_full()
            .max_w(rems(26.0))
            .flex()
            .flex_col()
            .gap(d.grid)
            .pt(d.grid)
            .child(
                div().flex().justify_center().child(
                    div()
                        .w(rems(2.75))
                        .h(rems(0.25))
                        .rounded_full()
                        .bg(theme.border),
                ),
            )
            .child(
                div()
                    .text_size(d.text_sm)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text_primary)
                    .child("Up Next"),
            )
            .children(queue_rows.into_iter().map(|item| {
                let title = item
                    .current_track()
                    .and_then(|track| track.title.clone())
                    .unwrap_or_else(|| item.album.title.clone());
                div()
                    .flex()
                    .items_center()
                    .gap(d.grid)
                    .min_h(rems(2.75))
                    .px(d.pad_x)
                    .rounded(d.r_md)
                    .bg(theme.surface)
                    .child(
                        Icon::new(IconName::ListMusic)
                            .size(IconSize::Sm)
                            .color(theme.text_muted),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .text_size(d.text_sm)
                            .text_color(theme.text_secondary)
                            .child(title),
                    )
            }))
            .child(
                div()
                    .text_size(d.text_sm)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text_primary)
                    .mt(d.grid)
                    .child("Plugin Chain"),
            )
            .children(plugins.into_iter().map(|label| {
                div()
                    .min_h(rems(2.5))
                    .px(d.pad_x)
                    .flex()
                    .items_center()
                    .rounded(d.r_md)
                    .bg(theme.surface)
                    .text_size(d.text_sm)
                    .text_color(theme.text_secondary)
                    .child(label)
            }))
            .into_any_element()
    }

    fn render_studio_hub_phone(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let (theme, translations, release_channel) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state.app.ui_state.translations.clone(),
                state.app.ui_state.release_channel,
            )
        };
        let tools = [
            (Screen::Studio, "Plugin Rack", IconName::SlidersHorizontal),
            (Screen::Spectrum, "Spectrum", IconName::AudioWaveform),
            (Screen::EqCurve, "EQ", IconName::AudioWaveform),
            (Screen::RoomEq, translations.screen_room_eq, IconName::Brain),
            (
                Screen::HeadphoneEq,
                translations.screen_headphone_eq,
                IconName::Headphones,
            ),
            (
                Screen::Recording,
                translations.screen_recording,
                IconName::Disc,
            ),
            (Screen::Streams, "Streams", IconName::ListMusic),
            (Screen::PluginGraph, "Plug Graph", IconName::Plug),
            (
                Screen::Spinorama,
                translations.screen_spinorama,
                IconName::Speaker,
            ),
        ];

        div()
            .id("phone-studio-hub")
            .size_full()
            .overflow_y_scroll()
            .bg(theme.background)
            .p(d.card)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(d.gap_md)
                    .children(tools.into_iter().filter_map(|(screen, label, icon)| {
                        if release_channel.allows(screen.maturity()) {
                            Some(self.render_studio_hub_card(screen, label, icon, &theme, &d))
                        } else {
                            None
                        }
                    })),
            )
            .into_any_element()
    }

    fn render_studio_hub_card(
        &self,
        screen: Screen,
        label: &'static str,
        icon: IconName,
        theme: &crate::theme::Theme,
        d: &Ds,
    ) -> AnyElement {
        let state_entity = self.state.clone();

        div()
            .id(SharedString::from(format!("phone-studio-{screen:?}")))
            .w(relative(0.48))
            .min_h(rems(7.25))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(d.grid)
            .rounded(d.r_md)
            .bg(theme.surface)
            .border_1()
            .border_color(theme.border)
            .cursor_pointer()
            .hover({
                let theme = theme.clone();
                move |s| s.bg(theme.surface_hover)
            })
            .child(Icon::new(icon).size(IconSize::Xl).color(theme.accent))
            .child(
                div()
                    .text_size(d.text_sm)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text_primary)
                    .text_center()
                    .child(label),
            )
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                state_entity.update(cx, |state, _cx| {
                    state.app.set_screen(screen, "PhoneStudioHub");
                });
            })
            .into_any_element()
    }

    fn render_phone_plugin_rack(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let (theme, plugins, editing_idx, add_open, release_channel, rack_editing) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state
                    .app
                    .plugin_state
                    .graph
                    .plugins()
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>(),
                state.app.plugin_state.editing_plugin_index,
                state.app.ui_state.active_menu == crate::app::ActiveMenu::AddPlugin,
                state.app.ui_state.release_channel,
                state.app.ui_state.phone_plugin_rack_editing,
            )
        };

        if editing_idx.is_some() {
            return self.render_phone_plugin_parameter_sheet(cx);
        }

        let state_for_add = self.state.clone();
        let state_for_edit = self.state.clone();

        div()
            .id("phone-plugin-rack")
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .bg(theme.background)
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .p(d.card)
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .text_color(theme.text_muted)
                            .child(format!("{} plugins", plugins.len())),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(d.grid)
                            .child(
                                div()
                                    .min_h(rems(2.75))
                                    .px(d.pad_x)
                                    .flex()
                                    .items_center()
                                    .gap(d.grid)
                                    .rounded(d.r_md)
                                    .bg(theme.background_secondary)
                                    .text_color(theme.text_primary)
                                    .cursor_pointer()
                                    .child(if rack_editing { "Done" } else { "Edit" })
                                    .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                                        state_for_edit.update(cx, |state, _cx| {
                                            state.app.ui_state.phone_plugin_rack_editing =
                                                !state.app.ui_state.phone_plugin_rack_editing;
                                        });
                                    }),
                            )
                            .child(
                                div()
                                    .min_h(rems(2.75))
                                    .px(d.pad_x)
                                    .flex()
                                    .items_center()
                                    .gap(d.grid)
                                    .rounded(d.r_md)
                                    .bg(theme.accent)
                                    .text_color(theme.text_on_accent)
                                    .cursor_pointer()
                                    .child(Icon::new(IconName::Plus).size(IconSize::Sm))
                                    .child("Add")
                                    .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                                        state_for_add.update(cx, |state, _cx| {
                                            state.app.ui_state.active_menu =
                                                crate::app::ActiveMenu::AddPlugin;
                                        });
                                    }),
                            ),
                    ),
            )
            .when(add_open, |el| {
                el.child(self.render_phone_plugin_picker(release_channel, &theme, &d))
            })
            .child(
                div()
                    .id("phone-plugin-list-scroll")
                    .overflow_y_scroll()
                    .flex_1()
                    .min_h_0()
                    .p(d.card)
                    .flex()
                    .flex_col()
                    .gap(d.grid)
                    .children(plugins.into_iter().enumerate().map(|(idx, plugin)| {
                        self.render_phone_plugin_card(idx, plugin, rack_editing, &theme, &d)
                    })),
            )
            .into_any_element()
    }

    fn render_phone_plugin_picker(
        &self,
        release_channel: sotf_audio_player::ReleaseChannel,
        theme: &crate::theme::Theme,
        d: &Ds,
    ) -> AnyElement {
        let choices = sotf_audio_player::PluginType::all()
            .into_iter()
            .filter(|plugin_type| release_channel.allows(plugin_type.maturity()))
            .collect::<Vec<_>>();

        div()
            .flex_none()
            .px(d.card)
            .pb(d.grid)
            .child(
                div()
                    .id("phone-plugin-picker-scroll")
                    .flex()
                    .gap(d.grid)
                    .overflow_x_scroll()
                    .children(choices.into_iter().map(|plugin_type| {
                        let state_entity = self.state.clone();
                        let label = plugin_type.name();
                        div()
                            .min_h(rems(2.75))
                            .px(d.pad_x)
                            .flex()
                            .items_center()
                            .rounded_full()
                            .bg(theme.surface)
                            .border_1()
                            .border_color(theme.border)
                            .text_size(d.text_sm)
                            .text_color(theme.text_primary)
                            .whitespace_nowrap()
                            .cursor_pointer()
                            .child(label)
                            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                                state_entity.update(cx, |state, _cx| {
                                    state.app.add_plugin(&plugin_type);
                                    state.app.ui_state.active_menu = crate::app::ActiveMenu::None;
                                });
                            })
                    })),
            )
            .into_any_element()
    }

    fn render_phone_plugin_card(
        &self,
        idx: usize,
        plugin: sotf_audio_player::Plugin,
        rack_editing: bool,
        theme: &crate::theme::Theme,
        d: &Ds,
    ) -> AnyElement {
        let state_for_open = self.state.clone();
        let state_for_bypass = self.state.clone();
        let state_for_move_up = self.state.clone();
        let state_for_move_down = self.state.clone();
        let name = plugin.display_name();
        let plugin_type = plugin.plugin_type().name().to_string();
        let summary = Self::phone_plugin_card_summary(&plugin);

        div()
            .id(SharedString::from(format!("phone-plugin-card-{idx}")))
            .flex()
            .items_center()
            .gap(d.gap_md)
            .min_h(rems(4.5))
            .p(d.pad_x)
            .rounded(d.r_md)
            .bg(theme.surface)
            .border_1()
            .border_color(theme.border)
            .cursor_pointer()
            .child(
                div()
                    .size(rems(2.75))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(d.r_md)
                    .bg(theme.background_secondary)
                    .child(
                        Icon::new(IconName::SlidersHorizontal)
                            .size(IconSize::Md)
                            .color(theme.accent),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(name),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_muted)
                            .child(plugin_type),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_secondary)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(summary),
                    ),
            )
            .child(
                div()
                    .min_w(rems(4.5))
                    .min_h(rems(2.75))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(if plugin.enabled {
                        theme.surface_selected
                    } else {
                        theme.background_secondary
                    })
                    .text_size(d.text_xs)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if plugin.enabled {
                        theme.accent
                    } else {
                        theme.text_muted
                    })
                    .child(if plugin.enabled { "On" } else { "Bypass" })
                    .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                        cx.stop_propagation();
                        state_for_bypass.update(cx, |state, _cx| {
                            state.app.toggle_plugin(idx);
                        });
                    }),
            )
            .when(rack_editing, |el| {
                el.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(d.grid)
                        .child(
                            div()
                                .size(rems(2.5))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(d.r_md)
                                .bg(theme.background_secondary)
                                .child(Icon::new(IconName::ChevronUp).size(IconSize::Sm))
                                .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                                    cx.stop_propagation();
                                    if idx > 0 {
                                        state_for_move_up.update(cx, |state, _cx| {
                                            state.app.move_plugin_up(idx);
                                        });
                                    }
                                }),
                        )
                        .child(
                            div()
                                .size(rems(2.5))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(d.r_md)
                                .bg(theme.background_secondary)
                                .child(Icon::new(IconName::ChevronDown).size(IconSize::Sm))
                                .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                                    cx.stop_propagation();
                                    state_for_move_down.update(cx, |state, _cx| {
                                        state.app.move_plugin_down(idx);
                                    });
                                }),
                        ),
                )
            })
            .child(
                Icon::new(IconName::ChevronRight)
                    .size(IconSize::Sm)
                    .color(theme.text_muted),
            )
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                state_for_open.update(cx, |state, _cx| {
                    state.app.plugin_state.selected_plugin_index = idx;
                    state.app.plugin_state.editing_plugin_index = Some(idx);
                    state.app.plugin_state.plugin_ui_state.plugin_ui_view =
                        crate::app::state::plugin::PluginUiView::Simple;
                });
            })
            .into_any_element()
    }

    fn render_phone_plugin_parameter_sheet(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let (theme, title, enabled, selected_idx, settings) = {
            let state = self.state.read(cx);
            let plugin = state.app.plugin_state.get_editing_plugin().cloned();
            (
                state.app.ui_state.theme.clone(),
                plugin
                    .as_ref()
                    .map(|plugin| plugin.display_name())
                    .unwrap_or_else(|| "Plugin".to_string()),
                plugin.as_ref().is_none_or(|plugin| plugin.enabled),
                state.app.plugin_state.selected_plugin_index,
                plugin.map(|plugin| plugin.settings),
            )
        };
        let state_for_close = self.state.clone();
        let state_for_bypass = self.state.clone();

        div()
            .id("phone-plugin-parameter-sheet")
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .p(d.card)
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .size(rems(2.75))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(d.r_md)
                            .cursor_pointer()
                            .child(Icon::new(IconName::ChevronLeft).size(IconSize::Md))
                            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                                state_for_close.update(cx, |state, _cx| {
                                    state.app.plugin_state.editing_plugin_index = None;
                                });
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_center()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(title),
                    )
                    .child(
                        div()
                            .min_w(rems(4.5))
                            .min_h(rems(2.75))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .bg(if enabled {
                                theme.surface_selected
                            } else {
                                theme.background_secondary
                            })
                            .text_size(d.text_xs)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(if enabled {
                                theme.accent
                            } else {
                                theme.text_muted
                            })
                            .child(if enabled { "On" } else { "Bypass" })
                            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                                state_for_bypass.update(cx, |state, _cx| {
                                    state.app.toggle_plugin(selected_idx);
                                });
                            }),
                    ),
            )
            .child(
                div()
                    .id("phone-plugin-sheet-scroll")
                    .overflow_y_scroll()
                    .flex_1()
                    .min_h_0()
                    .p(d.card)
                    .child(match settings {
                        Some(settings) if settings.eq_global_filters().is_some() => {
                            self.render_phone_eq_parameter_sheet(selected_idx, settings, &theme, &d)
                        }
                        Some(settings) => self.render_phone_generic_parameter_sheet(
                            selected_idx,
                            settings,
                            &theme,
                            &d,
                        ),
                        None => div()
                            .min_h(rems(10.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(theme.text_muted)
                            .child("No plugin selected.")
                            .into_any_element(),
                    }),
            )
            .into_any_element()
    }

    fn phone_plugin_card_summary(plugin: &sotf_audio_player::Plugin) -> String {
        if plugin.suspended {
            return "Suspended for channel layout".to_string();
        }

        match &plugin.settings {
            sotf_audio_player::PluginSettings::EQ { filters, .. }
            | sotf_audio_player::PluginSettings::LinearPhaseEq { filters, .. }
            | sotf_audio_player::PluginSettings::FirDesigner { filters, .. } => filters
                .first()
                .map(|filter| {
                    format!(
                        "{}  {:.0} Hz  {:+.1} dB  Q {:.2}",
                        Self::phone_filter_type_label(filter.filter_type),
                        filter.frequency,
                        filter.gain_db,
                        filter.q
                    )
                })
                .unwrap_or_else(|| "No filters".to_string()),
            sotf_audio_player::PluginSettings::Gain {
                gain_db,
                smoothing_ms,
                ..
            } => {
                format!("{gain_db:+.1} dB  {smoothing_ms:.0} ms")
            }
            sotf_audio_player::PluginSettings::Compressor {
                threshold_db,
                ratio,
                ..
            } => format!("Thresh {threshold_db:+.1} dB  Ratio {ratio:.1}:1"),
            sotf_audio_player::PluginSettings::Limiter {
                threshold_db,
                release_ms,
                ..
            } => format!("Ceiling {threshold_db:+.1} dB  Rel {release_ms:.0} ms"),
            sotf_audio_player::PluginSettings::Gate {
                threshold_db,
                ratio,
                ..
            } => format!("Thresh {threshold_db:+.1} dB  Ratio {ratio:.1}:1"),
            sotf_audio_player::PluginSettings::Upmixer { speaker_config, .. }
            | sotf_audio_player::PluginSettings::AAE { speaker_config, .. } => {
                format!("{speaker_config} output")
            }
            _ => {
                let param_count = sotf_audio_player::get_param_count(&plugin.settings);
                if param_count == 1 {
                    "1 parameter".to_string()
                } else {
                    format!("{param_count} parameters")
                }
            }
        }
    }

    fn phone_filter_type_label(filter_type: sotf_audio_player::BiquadFilterType) -> &'static str {
        match filter_type {
            sotf_audio_player::BiquadFilterType::Peak => "Peak",
            sotf_audio_player::BiquadFilterType::Lowshelf => "LowShelf",
            sotf_audio_player::BiquadFilterType::Highshelf => "HighShelf",
            sotf_audio_player::BiquadFilterType::Lowpass => "LowPass",
            sotf_audio_player::BiquadFilterType::Highpass => "HighPass",
            sotf_audio_player::BiquadFilterType::Bandpass => "BandPass",
            sotf_audio_player::BiquadFilterType::Notch => "Notch",
            _ => "Filter",
        }
    }

    fn render_phone_eq_parameter_sheet(
        &self,
        plugin_idx: usize,
        settings: sotf_audio_player::PluginSettings,
        theme: &crate::theme::Theme,
        d: &Ds,
    ) -> AnyElement {
        let filters = settings.eq_global_filters().cloned().unwrap_or_default();
        let state_for_add = self.state.clone();

        div()
            .id("phone-eq-parameter-sheet")
            .flex()
            .flex_col()
            .gap(d.section)
            .children(filters.iter().enumerate().map(|(band_idx, filter)| {
                self.render_phone_eq_band(plugin_idx, band_idx, filter.clone(), theme, d)
            }))
            .child(
                div()
                    .min_h(rems(2.75))
                    .px(d.pad_x)
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(d.r_md)
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.surface)
                    .text_size(d.text_sm)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.accent)
                    .cursor_pointer()
                    .child("+ Add Filter")
                    .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                        state_for_add.update(cx, |state, _cx| {
                            if let Err(err) = state.app.add_eq_band() {
                                state.app.ui_state.toast_message =
                                    Some(crate::app::ToastMessage::error(err));
                            }
                        });
                    }),
            )
            .into_any_element()
    }

    fn render_phone_eq_band(
        &self,
        plugin_idx: usize,
        band_idx: usize,
        filter: sotf_audio_player::EQFilter,
        theme: &crate::theme::Theme,
        d: &Ds,
    ) -> AnyElement {
        let state_for_reset = self.state.clone();
        let state_for_delete = self.state.clone();
        let base_param = band_idx * 4;

        div()
            .id(SharedString::from(format!("phone-eq-band-{band_idx}")))
            .flex()
            .flex_col()
            .gap(d.grid)
            .p(d.card)
            .rounded(d.r_md)
            .bg(theme.surface)
            .border_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(d.grid)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(d.text_sm)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text_primary)
                                    .child(format!(
                                        "Filter {}: {}",
                                        band_idx + 1,
                                        Self::phone_filter_type_label(filter.filter_type)
                                    )),
                            )
                            .child(
                                div()
                                    .text_size(d.text_xs)
                                    .text_color(theme.text_muted)
                                    .child(format!(
                                        "{:.0} Hz  {:+.1} dB  Q {:.2}",
                                        filter.frequency, filter.gain_db, filter.q
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .min_h(rems(2.5))
                            .px(d.grid)
                            .flex()
                            .items_center()
                            .rounded(d.r_md)
                            .bg(theme.background_secondary)
                            .text_size(d.text_xs)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(if filter.muted {
                                theme.text_muted
                            } else {
                                theme.accent
                            })
                            .child(if filter.muted { "Muted" } else { "Active" }),
                    ),
            )
            .child(self.render_phone_param_slider(
                "Frequency",
                format!("{:.0} Hz", filter.frequency),
                filter.frequency,
                20.0,
                20_000.0,
                plugin_idx,
                base_param + 1,
                theme,
                d,
            ))
            .child(self.render_phone_param_slider(
                "Gain",
                format!("{:+.1} dB", filter.gain_db),
                filter.gain_db,
                -24.0,
                24.0,
                plugin_idx,
                base_param + 3,
                theme,
                d,
            ))
            .child(self.render_phone_param_slider(
                "Q",
                format!("{:.2}", filter.q),
                filter.q,
                0.1,
                10.0,
                plugin_idx,
                base_param + 2,
                theme,
                d,
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(d.grid)
                    .child(
                        div()
                            .flex_1()
                            .min_h(rems(2.75))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(d.r_md)
                            .bg(theme.background_secondary)
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .cursor_pointer()
                            .child("Reset")
                            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                                state_for_reset.update(cx, |state, _cx| {
                                    state
                                        .app
                                        .set_plugin_param(plugin_idx, base_param + 1, 1000.0);
                                    state.app.set_plugin_param(plugin_idx, base_param + 2, 1.0);
                                    state.app.set_plugin_param(plugin_idx, base_param + 3, 0.0);
                                });
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h(rems(2.75))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(d.r_md)
                            .bg(theme.background_secondary)
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.error)
                            .cursor_pointer()
                            .child("Delete Filter")
                            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                                state_for_delete.update(cx, |state, _cx| {
                                    if let Err(err) = state.app.remove_eq_band(band_idx) {
                                        state.app.ui_state.toast_message =
                                            Some(crate::app::ToastMessage::error(err));
                                    }
                                });
                            }),
                    ),
            )
            .into_any_element()
    }

    fn render_phone_generic_parameter_sheet(
        &self,
        plugin_idx: usize,
        settings: sotf_audio_player::PluginSettings,
        theme: &crate::theme::Theme,
        d: &Ds,
    ) -> AnyElement {
        let rows = Self::phone_generic_parameter_rows(&settings);

        div()
            .id("phone-generic-parameter-sheet")
            .flex()
            .flex_col()
            .gap(d.grid)
            .when(rows.is_empty(), |el| {
                el.child(
                    div()
                        .min_h(rems(10.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(theme.text_muted)
                        .child("No touch-editable parameters for this plugin yet."),
                )
            })
            .children(rows.into_iter().map(|(label, value, min, max, param_idx)| {
                self.render_phone_param_slider(
                    label,
                    format!("{value:.2}"),
                    value,
                    min,
                    max,
                    plugin_idx,
                    param_idx,
                    theme,
                    d,
                )
            }))
            .into_any_element()
    }

    fn phone_generic_parameter_rows(
        settings: &sotf_audio_player::PluginSettings,
    ) -> Vec<(&'static str, f64, f64, f64, usize)> {
        match settings {
            sotf_audio_player::PluginSettings::Gain { gain_db, .. } => {
                vec![("Gain", *gain_db, -24.0, 24.0, 0)]
            }
            sotf_audio_player::PluginSettings::Compressor {
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                makeup_gain_db,
                mix,
                ..
            } => vec![
                ("Threshold", *threshold_db, -60.0, 0.0, 0),
                ("Ratio", *ratio, 1.0, 20.0, 1),
                ("Attack", *attack_ms, 0.1, 200.0, 2),
                ("Release", *release_ms, 5.0, 2000.0, 3),
                ("Makeup", *makeup_gain_db, -24.0, 24.0, 5),
                ("Mix", *mix, 0.0, 1.0, 6),
            ],
            sotf_audio_player::PluginSettings::Limiter {
                threshold_db,
                release_ms,
                mix,
                ..
            } => vec![
                ("Ceiling", *threshold_db, -24.0, 0.0, 0),
                ("Release", *release_ms, 5.0, 2000.0, 1),
                ("Mix", *mix, 0.0, 1.0, 7),
            ],
            sotf_audio_player::PluginSettings::Gate {
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                mix,
                ..
            } => vec![
                ("Threshold", *threshold_db, -80.0, 0.0, 0),
                ("Ratio", *ratio, 1.0, 20.0, 1),
                ("Attack", *attack_ms, 0.1, 200.0, 2),
                ("Release", *release_ms, 5.0, 2000.0, 3),
                ("Mix", *mix, 0.0, 1.0, 5),
            ],
            _ => Vec::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_phone_param_slider(
        &self,
        label: &'static str,
        value_text: impl Into<String>,
        value: f64,
        min: f64,
        max: f64,
        plugin_idx: usize,
        param_idx: usize,
        theme: &crate::theme::Theme,
        d: &Ds,
    ) -> AnyElement {
        let ratio = if max > min {
            ((value - min) / (max - min)).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        let state_for_value = self.state.clone();
        let state_for_minus = self.state.clone();
        let state_for_plus = self.state.clone();
        let value_text = value_text.into();
        let step = ((max - min) / 32.0).max(0.1);
        let decimals = if value_text.contains("Hz") {
            0
        } else if value_text.contains("dB") {
            1
        } else {
            2
        };
        let unit = if value_text.contains("Hz") {
            "Hz"
        } else if value_text.contains("dB") {
            "dB"
        } else {
            ""
        };

        div()
            .flex()
            .flex_col()
            .gap(d.grid)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(d.grid)
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text_primary)
                            .child(label),
                    )
                    .child(
                        NumberInput::new(SharedString::from(format!(
                            "phone-param-{plugin_idx}-{param_idx}"
                        )))
                        .value(value)
                        .min(min)
                        .max(max)
                        .step(step)
                        .decimals(decimals)
                        .unit(unit)
                        .size(NumberInputSize::Sm)
                        .width(104.0)
                        .on_change(move |next, _window, cx| {
                            state_for_value.update(cx, |state, _cx| {
                                state.app.set_plugin_param(
                                    plugin_idx,
                                    param_idx,
                                    next.clamp(min, max),
                                );
                            });
                        }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(d.grid)
                    .child(
                        div()
                            .size(rems(2.75))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(d.r_md)
                            .bg(theme.background_secondary)
                            .cursor_pointer()
                            .child(Icon::new(IconName::Minus).size(IconSize::Sm))
                            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                                state_for_minus.update(cx, |state, _cx| {
                                    state.app.set_plugin_param(
                                        plugin_idx,
                                        param_idx,
                                        (value - step).clamp(min, max),
                                    );
                                });
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .h(rems(0.5))
                            .rounded_full()
                            .bg(theme.background_secondary)
                            .child(
                                div()
                                    .h_full()
                                    .w(relative(ratio))
                                    .rounded_full()
                                    .bg(theme.accent),
                            ),
                    )
                    .child(
                        div()
                            .size(rems(2.75))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(d.r_md)
                            .bg(theme.background_secondary)
                            .cursor_pointer()
                            .child(Icon::new(IconName::Plus).size(IconSize::Sm))
                            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                                state_for_plus.update(cx, |state, _cx| {
                                    state.app.set_plugin_param(
                                        plugin_idx,
                                        param_idx,
                                        (value + step).clamp(min, max),
                                    );
                                });
                            }),
                    ),
            )
            .into_any_element()
    }

    fn render_phone_eq_curve(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let (theme, eq_index, filters) = {
            let state = self.state.read(cx);
            let eq_index = state
                .app
                .plugin_state
                .graph
                .plugins()
                .into_iter()
                .position(|plugin| {
                    matches!(plugin.plugin_type(), sotf_audio_player::PluginType::EQ)
                });
            let filters = eq_index
                .and_then(|idx| state.app.plugin_state.graph.get_plugin(idx))
                .and_then(|plugin| match &plugin.settings {
                    sotf_audio_player::PluginSettings::EQ { filters, .. } => Some(filters.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            (state.app.ui_state.theme.clone(), eq_index, filters)
        };
        let state_for_edit = self.state.clone();
        let filter_count = filters.len();

        div()
            .id("phone-eq-curve")
            .size_full()
            .overflow_y_scroll()
            .bg(theme.background)
            .p(d.card)
            .flex()
            .flex_col()
            .gap(d.section)
            .child(
                div()
                    .w_full()
                    .h(rems(14.0))
                    .rounded(d.r_md)
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.border)
                    .p(d.card)
                    .flex()
                    .items_end()
                    .gap(crate::app::constants::spacing::XS)
                    .children((0..48).map(|i| {
                        let norm = i as f64 / 47.0;
                        let freq = 20.0_f64 * (1000.0_f64).powf(norm);
                        let gain = crate::components::plugins::ui_eq::calculate_response_at_freq(
                            &filters, freq,
                        )
                        .clamp(-12.0, 12.0);
                        let height = 50.0 + (gain as f32 / 12.0) * 42.0;
                        div()
                            .flex_1()
                            .h(relative((height / 100.0).clamp(0.08, 0.96)))
                            .rounded_full()
                            .bg(theme.accent)
                    })),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .text_color(theme.text_muted)
                            .child(format!("{filter_count} filters")),
                    )
                    .child(
                        div()
                            .min_h(rems(2.75))
                            .px(d.pad_x)
                            .flex()
                            .items_center()
                            .rounded(d.r_md)
                            .bg(theme.accent)
                            .text_color(theme.text_on_accent)
                            .cursor_pointer()
                            .child("Edit")
                            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                                state_for_edit.update(cx, |state, _cx| {
                                    if let Some(idx) = eq_index {
                                        state.app.plugin_state.selected_plugin_index = idx;
                                        state.app.plugin_state.editing_plugin_index = Some(idx);
                                    }
                                    state.app.set_screen(Screen::Studio, "PhoneEqEdit");
                                });
                            }),
                    ),
            )
            .child(
                div()
                    .rounded(d.r_md)
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.border)
                    .overflow_hidden()
                    .child(
                        div()
                            .min_h(rems(2.75))
                            .px(d.card)
                            .flex()
                            .items_center()
                            .justify_between()
                            .border_b_1()
                            .border_color(theme.border)
                            .child(
                                div()
                                    .text_size(d.text_sm)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text_primary)
                                    .child("Filters"),
                            )
                            .child(
                                Icon::new(IconName::ChevronUp)
                                    .size(IconSize::Sm)
                                    .color(theme.text_muted),
                            ),
                    )
                    .children(filters.iter().take(4).enumerate().map(|(idx, filter)| {
                        div()
                            .min_h(rems(3.0))
                            .px(d.card)
                            .flex()
                            .items_center()
                            .justify_between()
                            .border_b_1()
                            .border_color(theme.border)
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .text_size(d.text_sm)
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme.text_primary)
                                            .child(format!(
                                                "Filter {}: {:?}",
                                                idx + 1,
                                                filter.filter_type
                                            )),
                                    )
                                    .child(
                                        div()
                                            .text_size(d.text_xs)
                                            .text_color(theme.text_muted)
                                            .child(format!(
                                                "{:.0} Hz  {:+.1} dB  Q {:.2}",
                                                filter.frequency, filter.gain_db, filter.q
                                            )),
                                    ),
                            )
                    })),
            )
            .into_any_element()
    }

    fn render_phone_spectrum_screen(&self, cx: &mut Context<Self>) -> AnyElement {
        let (hold, smooth) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.phone_spectrum_hold,
                state.app.ui_state.phone_spectrum_smoothed,
            )
        };
        let content = self.render_spectrum_screen(cx).into_any_element();

        self.render_phone_tool_wrapper(
            "Spectrum",
            if hold {
                "Held analyzer frame"
            } else if smooth {
                "Smoothed live analyzer"
            } else {
                "Live analyzer"
            },
            content,
            cx,
        )
    }

    fn render_phone_plugin_graph_screen(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let (theme, show_list, actions_open, plugins, release_channel) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state.app.ui_state.phone_plugin_graph_list,
                state.app.ui_state.phone_plugin_graph_actions_open,
                state
                    .app
                    .plugin_state
                    .graph
                    .plugins()
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>(),
                state.app.ui_state.release_channel,
            )
        };
        let state_for_open_rack = self.state.clone();
        let action_plugins = plugins.clone();

        let content = if show_list {
            div()
                .id("phone-graph-list-scroll")
                .size_full()
                .flex()
                .flex_col()
                .overflow_y_scroll()
                .p(d.card)
                .gap(d.grid)
                .when(actions_open, |el| {
                    el.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(d.grid)
                            .p(d.card)
                            .rounded(d.r_md)
                            .bg(theme.surface)
                            .border_1()
                            .border_color(theme.border)
                            .child(
                                div()
                                    .min_h(rems(2.75))
                                    .px(d.pad_x)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(d.r_md)
                                    .bg(theme.accent)
                                    .text_color(theme.text_on_accent)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .cursor_pointer()
                                    .child("Open Rack Editor")
                                    .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                                        state_for_open_rack.update(cx, |state, _cx| {
                                            state.app.set_screen(Screen::Studio, "PhoneGraphRack");
                                        });
                                    }),
                            )
                            .child(self.render_phone_plugin_picker(release_channel, &theme, &d))
                            .children(action_plugins.iter().enumerate().map(|(idx, plugin)| {
                                let state_for_remove = self.state.clone();
                                div()
                                    .min_h(rems(2.75))
                                    .px(d.pad_x)
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap(d.grid)
                                    .rounded(d.r_md)
                                    .bg(theme.background_secondary)
                                    .child(
                                        div()
                                            .min_w_0()
                                            .text_size(d.text_xs)
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme.text_primary)
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .whitespace_nowrap()
                                            .child(plugin.display_name()),
                                    )
                                    .child(
                                        div()
                                            .min_h(rems(2.25))
                                            .px(d.grid)
                                            .flex()
                                            .items_center()
                                            .rounded(d.r_md)
                                            .bg(theme.surface)
                                            .text_size(d.text_xs)
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme.error)
                                            .cursor_pointer()
                                            .child("Remove")
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                move |_event, _window, cx| {
                                                    state_for_remove.update(cx, |state, _cx| {
                                                        state.app.remove_plugin(idx);
                                                        state
                                                            .app
                                                            .ui_state
                                                            .phone_plugin_graph_actions_open =
                                                            false;
                                                    });
                                                },
                                            ),
                                    )
                            })),
                    )
                })
                .children(plugins.into_iter().enumerate().map(|(idx, plugin)| {
                    let state_for_select = self.state.clone();
                    div()
                        .id(SharedString::from(format!("phone-graph-node-{idx}")))
                        .flex()
                        .items_center()
                        .gap(d.gap_md)
                        .min_h(rems(3.75))
                        .p(d.pad_x)
                        .rounded(d.r_md)
                        .bg(theme.surface)
                        .border_1()
                        .border_color(theme.border)
                        .cursor_pointer()
                        .child(
                            Icon::new(IconName::Plug)
                                .size(IconSize::Md)
                                .color(theme.accent),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    div()
                                        .text_size(d.text_sm)
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(theme.text_primary)
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .whitespace_nowrap()
                                        .child(plugin.display_name()),
                                )
                                .child(
                                    div()
                                        .text_size(d.text_xs)
                                        .text_color(theme.text_muted)
                                        .child(Self::phone_plugin_card_summary(&plugin)),
                                ),
                        )
                        .child(
                            div()
                                .min_w(rems(3.5))
                                .text_size(d.text_xs)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(if plugin.enabled {
                                    theme.accent
                                } else {
                                    theme.text_muted
                                })
                                .child(if plugin.enabled { "On" } else { "Off" }),
                        )
                        .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                            state_for_select.update(cx, |state, _cx| {
                                state.app.plugin_state.selected_plugin_index = idx;
                                state.app.plugin_state.editing_plugin_index = Some(idx);
                                state.app.set_screen(Screen::Studio, "PhoneGraphNode");
                            });
                        })
                }))
                .into_any_element()
        } else {
            self.render_plugin_graph_screen(cx).into_any_element()
        };

        self.render_phone_tool_wrapper("Plug Graph", "Signal flow", content, cx)
    }

    fn render_streams_screen_phone(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let (theme, show_sources, streams, last_error, last_status) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state.app.ui_state.phone_stream_sources_open,
                state.app.stream_state.store.streams.clone(),
                state.app.stream_state.last_error.clone(),
                state.app.stream_state.last_status.clone(),
            )
        };

        let content = if show_sources {
            self.render_streams_screen(cx).into_any_element()
        } else {
            div()
                .id("phone-stream-list")
                .size_full()
                .overflow_y_scroll()
                .p(d.card)
                .flex()
                .flex_col()
                .gap(d.grid)
                .when_some(last_error, |el, err| {
                    el.child(
                        div()
                            .p(d.pad_y)
                            .rounded(d.r_md)
                            .bg(theme.feedback.toast_error_bg)
                            .text_size(d.text_sm)
                            .text_color(theme.error)
                            .child(err),
                    )
                })
                .when_some(last_status, |el, status| {
                    el.child(
                        div()
                            .p(d.pad_y)
                            .rounded(d.r_md)
                            .bg(theme.feedback.toast_success_bg)
                            .text_size(d.text_sm)
                            .text_color(theme.success)
                            .child(status),
                    )
                })
                .when(streams.is_empty(), |el| {
                    el.child(
                        div()
                            .min_h(rems(12.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(d.r_md)
                            .border_1()
                            .border_color(theme.border)
                            .text_color(theme.text_muted)
                            .child("No saved streams"),
                    )
                })
                .children(
                    streams
                        .into_iter()
                        .enumerate()
                        .map(|(idx, stream)| self.render_phone_stream_row(idx, stream, &theme, &d)),
                )
                .into_any_element()
        };

        self.render_phone_tool_wrapper("Streams", "Remote and internet sources", content, cx)
    }

    fn render_phone_stream_row(
        &self,
        idx: usize,
        stream: sotf_audio_player::SavedStream,
        theme: &crate::theme::Theme,
        d: &Ds,
    ) -> AnyElement {
        let state_for_detail = self.state.clone();
        let state_for_play = self.state.clone();
        let play_stream = stream.clone();

        div()
            .id(SharedString::from(format!("phone-stream-row-{idx}")))
            .flex()
            .items_center()
            .gap(d.gap_md)
            .min_h(rems(4.25))
            .p(d.pad_x)
            .rounded(d.r_md)
            .bg(theme.surface)
            .border_1()
            .border_color(theme.border)
            .cursor_pointer()
            .child(
                Icon::new(IconName::ListMusic)
                    .size(IconSize::Md)
                    .color(theme.accent),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(stream.name.clone()),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_muted)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(stream.url.clone()),
                    ),
            )
            .child(
                div()
                    .min_h(rems(2.75))
                    .px(d.pad_x)
                    .flex()
                    .items_center()
                    .rounded(d.r_md)
                    .bg(theme.accent)
                    .text_color(theme.text_on_accent)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Play")
                    .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                        cx.stop_propagation();
                        state_for_play.update(cx, |state, _cx| {
                            match state.app.play_stream_now(play_stream.clone()) {
                                Ok(Some(source)) => PlayerView::play_track(state, source),
                                Ok(None) => {}
                                Err(err) => state.app.record_stream_error(err),
                            }
                        });
                    }),
            )
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                state_for_detail.update(cx, |state, _cx| {
                    state.app.set_stream_inputs_from_selected(idx);
                    state.app.ui_state.phone_stream_sources_open = true;
                });
            })
            .into_any_element()
    }

    fn render_phone_tool_wrapper(
        &self,
        title: &'static str,
        subtitle: &'static str,
        content: AnyElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let d = Ds::from_cx(cx);
        let (
            theme,
            subtitle_text,
            progress,
            wizard_kind,
            spectrum_hold,
            spectrum_smoothed,
            graph_list,
            streams_open,
        ) = {
            let state = self.state.read(cx);
            let wizard_status = match title {
                "Recording" => {
                    let step = state.app.measurement_state.recording_state.step;
                    let steps = crate::app::types::RecordingStep::all();
                    let index = steps
                        .iter()
                        .position(|candidate| *candidate == step)
                        .unwrap_or(0);
                    Some((
                        format!("Step {} of {}: {}", index + 1, steps.len(), step.label()),
                        (index + 1) as f32 / steps.len() as f32,
                    ))
                }
                "Room EQ" => {
                    let step = state.app.measurement_state.room_eq_state.step;
                    let steps = crate::app::types::RoomEqStep::all();
                    Some((
                        format!(
                            "Step {} of {}: {}",
                            step.index() + 1,
                            steps.len(),
                            step.label()
                        ),
                        (step.index() + 1) as f32 / steps.len() as f32,
                    ))
                }
                "Headphone EQ" => {
                    let step = state.app.measurement_state.headphone_eq_state.step;
                    let steps = crate::app::types::HeadphoneEqStep::all();
                    Some((
                        format!(
                            "Step {} of {}: {}",
                            step.index() + 1,
                            steps.len(),
                            step.label()
                        ),
                        (step.index() + 1) as f32 / steps.len() as f32,
                    ))
                }
                "Spinorama" => {
                    let step = state.app.measurement_state.spinorama_eq_state.step;
                    let steps = sotf_audio_player::spinorama_eq_types::SpinoramaStep::all();
                    Some((
                        format!(
                            "Step {} of {}: {}",
                            step.index() + 1,
                            steps.len(),
                            step.label()
                        ),
                        (step.index() + 1) as f32 / steps.len() as f32,
                    ))
                }
                _ => None,
            };
            (
                state.app.ui_state.theme.clone(),
                wizard_status
                    .as_ref()
                    .map(|(label, _progress)| label.clone())
                    .unwrap_or_else(|| subtitle.to_string()),
                wizard_status.map(|(_label, progress)| progress),
                match title {
                    "Recording" => Some("recording"),
                    "Room EQ" => Some("room_eq"),
                    "Headphone EQ" => Some("headphone_eq"),
                    "Spinorama" => Some("spinorama"),
                    _ => None,
                },
                state.app.ui_state.phone_spectrum_hold,
                state.app.ui_state.phone_spectrum_smoothed,
                state.app.ui_state.phone_plugin_graph_list,
                state.app.ui_state.phone_stream_sources_open,
            )
        };
        let wizard_back = self.state.clone();
        let wizard_next = self.state.clone();

        div()
            .id(SharedString::from(format!("phone-tool-{title}")))
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .bg(theme.background)
            .child(
                div()
                    .flex_none()
                    .px(d.card)
                    .py(d.grid)
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(d.grid)
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .text_size(d.text_xs)
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme.accent)
                                            .child(subtitle_text),
                                    )
                                    .child(
                                        div()
                                            .text_size(d.text_lg)
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme.text_primary)
                                            .child(title),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(d.grid)
                                    .when_some(wizard_kind, |el, kind| {
                                        el.child(
                                            div()
                                                .min_h(rems(2.25))
                                                .px(d.grid)
                                                .flex()
                                                .items_center()
                                                .rounded(d.r_md)
                                                .bg(theme.background_secondary)
                                                .text_size(d.text_xs)
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(theme.text_primary)
                                                .cursor_pointer()
                                                .child("Back")
                                                .on_mouse_up(MouseButton::Left, {
                                                    let wizard_back = wizard_back.clone();
                                                    move |_event, _window, cx| {
                                                        wizard_back.update(cx, |state, _cx| {
                                                            Self::move_phone_wizard_step(
                                                                state, kind, false,
                                                            );
                                                        });
                                                    }
                                                }),
                                        )
                                        .child(
                                            div()
                                                .min_h(rems(2.25))
                                                .px(d.grid)
                                                .flex()
                                                .items_center()
                                                .rounded(d.r_md)
                                                .bg(theme.accent)
                                                .text_size(d.text_xs)
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(theme.text_on_accent)
                                                .cursor_pointer()
                                                .child("Next")
                                                .on_mouse_up(MouseButton::Left, {
                                                    let wizard_next = wizard_next.clone();
                                                    move |_event, _window, cx| {
                                                        wizard_next.update(cx, |state, _cx| {
                                                            Self::move_phone_wizard_step(
                                                                state, kind, true,
                                                            );
                                                        });
                                                    }
                                                }),
                                        )
                                    })
                                    .when(title == "Spectrum", |el| {
                                        el.child(self.render_phone_tool_toggle(
                                            "Hold",
                                            IconName::Pause,
                                            spectrum_hold,
                                            "spectrum_hold",
                                            &theme,
                                            &d,
                                        ))
                                        .child(
                                            self.render_phone_tool_toggle(
                                                "Smooth",
                                                IconName::AudioWaveform,
                                                spectrum_smoothed,
                                                "spectrum_smoothed",
                                                &theme,
                                                &d,
                                            ),
                                        )
                                    })
                                    .when(title == "Plug Graph", |el| {
                                        el.child(self.render_phone_tool_toggle(
                                            if graph_list { "Graph" } else { "List" },
                                            IconName::ListMusic,
                                            graph_list,
                                            "plugin_graph_list",
                                            &theme,
                                            &d,
                                        ))
                                        .child(
                                            self.render_phone_tool_toggle(
                                                "Actions",
                                                IconName::Settings,
                                                false,
                                                "plugin_graph_actions",
                                                &theme,
                                                &d,
                                            ),
                                        )
                                    })
                                    .when(title == "Streams", |el| {
                                        el.child(self.render_phone_tool_toggle(
                                            "Sources",
                                            IconName::ListMusic,
                                            streams_open,
                                            "stream_sources",
                                            &theme,
                                            &d,
                                        ))
                                    }),
                            ),
                    )
                    .when_some(progress, |el, progress| {
                        el.child(
                            div()
                                .mt(d.grid)
                                .h(rems(0.25))
                                .w_full()
                                .rounded_full()
                                .bg(theme.background_secondary)
                                .child(
                                    div()
                                        .h_full()
                                        .w(relative(progress.clamp(0.05, 1.0)))
                                        .rounded_full()
                                        .bg(theme.accent),
                                ),
                        )
                    }),
            )
            .child(div().flex_1().min_h_0().overflow_hidden().child(content))
            .into_any_element()
    }

    fn render_phone_tool_toggle(
        &self,
        label: &'static str,
        icon: IconName,
        selected: bool,
        action: &'static str,
        theme: &crate::theme::Theme,
        d: &Ds,
    ) -> AnyElement {
        let state_entity = self.state.clone();

        div()
            .min_h(rems(2.25))
            .px(d.grid)
            .flex()
            .items_center()
            .gap(d.grid)
            .rounded(d.r_md)
            .bg(if selected {
                theme.surface_selected
            } else {
                theme.background_secondary
            })
            .text_color(if selected {
                theme.accent
            } else {
                theme.text_primary
            })
            .text_size(d.text_xs)
            .font_weight(FontWeight::SEMIBOLD)
            .cursor_pointer()
            .child(Icon::new(icon).size(IconSize::Sm))
            .child(label)
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                state_entity.update(cx, |state, _cx| match action {
                    "spectrum_hold" => {
                        let next_hold = !state.app.ui_state.phone_spectrum_hold;
                        state.app.ui_state.phone_spectrum_hold = next_hold;
                        state.app.ui_state.phone_spectrum_hold_magnitudes = if next_hold {
                            state
                                .app
                                .playback
                                .spectrum_info
                                .as_ref()
                                .map(|info| info.magnitudes.as_ref().as_slice().to_vec())
                        } else {
                            None
                        };
                    }
                    "spectrum_smoothed" => {
                        state.app.ui_state.phone_spectrum_smoothed =
                            !state.app.ui_state.phone_spectrum_smoothed;
                    }
                    "plugin_graph_list" => {
                        state.app.ui_state.phone_plugin_graph_list =
                            !state.app.ui_state.phone_plugin_graph_list;
                    }
                    "plugin_graph_actions" => {
                        state.app.ui_state.phone_plugin_graph_actions_open =
                            !state.app.ui_state.phone_plugin_graph_actions_open;
                    }
                    "stream_sources" => {
                        state.app.ui_state.phone_stream_sources_open =
                            !state.app.ui_state.phone_stream_sources_open;
                    }
                    _ => {}
                });
            })
            .into_any_element()
    }

    fn move_phone_wizard_step(state: &mut crate::app::AppState, kind: &str, forward: bool) {
        match kind {
            "recording" => {
                let step = state.app.measurement_state.recording_state.step;
                let next = if forward {
                    step.next()
                } else {
                    step.previous()
                };
                if let Some(next) = next {
                    state.app.measurement_state.recording_state.step = next;
                }
            }
            "room_eq" => {
                let step = state.app.measurement_state.room_eq_state.step;
                let next = if forward {
                    step.next()
                } else {
                    step.previous()
                };
                if let Some(next) = next {
                    state.app.measurement_state.room_eq_state.step = next;
                }
            }
            "headphone_eq" => {
                let step = state.app.measurement_state.headphone_eq_state.step;
                let next = if forward {
                    step.next()
                } else {
                    step.previous()
                };
                if let Some(next) = next {
                    state.app.measurement_state.headphone_eq_state.step = next;
                }
            }
            "spinorama" => {
                let step = state.app.measurement_state.spinorama_eq_state.step;
                let next = if forward {
                    step.next()
                } else {
                    step.previous()
                };
                if let Some(next) = next {
                    state.app.measurement_state.spinorama_eq_state.step = next;
                }
            }
            _ => {}
        }
    }

    fn render_settings_screen_phone(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let (theme, translations, visible_tabs) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state.app.ui_state.translations.clone(),
                crate::app::SettingsTab::visible_tabs(),
            )
        };

        div()
            .id("phone-settings-screen")
            .size_full()
            .overflow_y_scroll()
            .bg(theme.background)
            .p(d.card)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .rounded(d.r_md)
                    .border_1()
                    .border_color(theme.border)
                    .overflow_hidden()
                    .children(
                        visible_tabs.into_iter().map(|tab| {
                            self.render_phone_settings_row(tab, &translations, &theme, &d)
                        }),
                    ),
            )
            .into_any_element()
    }

    fn render_settings_detail_phone(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let (theme, active_tab, visible_tabs) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state.app.ui_state.active_settings_tab,
                crate::app::SettingsTab::visible_tabs(),
            )
        };
        let active_tab = if visible_tabs.contains(&active_tab) {
            active_tab
        } else {
            crate::app::SettingsTab::fallback_for_platform()
        };
        let content = match active_tab {
            crate::app::SettingsTab::Library => {
                self.render_library_settings_content(cx).into_any_element()
            }
            crate::app::SettingsTab::Theme => {
                self.render_theme_settings_content(cx).into_any_element()
            }
            crate::app::SettingsTab::Language => {
                self.render_language_settings_content(cx).into_any_element()
            }
            crate::app::SettingsTab::Keybindings => self.render_phone_keybindings_settings(cx),
            crate::app::SettingsTab::AudioDevice => self
                .render_audio_device_settings_content(cx)
                .into_any_element(),
            crate::app::SettingsTab::Misc => {
                self.render_plugins_settings_content(cx).into_any_element()
            }
            crate::app::SettingsTab::Federation => self
                .render_federation_settings_content(cx)
                .into_any_element(),
            crate::app::SettingsTab::Servers => {
                self.render_servers_settings_content(cx).into_any_element()
            }
            crate::app::SettingsTab::Metadata => {
                self.render_metadata_settings_content(cx).into_any_element()
            }
            crate::app::SettingsTab::ReleaseChannel => self
                .render_release_channel_settings_content(cx)
                .into_any_element(),
        };

        div()
            .id("phone-settings-detail")
            .size_full()
            .overflow_y_scroll()
            .bg(theme.background)
            .p(d.card)
            .child(content)
            .into_any_element()
    }

    fn render_phone_keybindings_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let (theme, query, preset) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state.app.ui_state.phone_keybindings_query.clone(),
                state.app.ui_state.keymap_preset,
            )
        };
        let query_lc = query.to_lowercase();
        let rows = crate::app::keybindings::get_documented_keybindings(preset)
            .into_iter()
            .filter(|binding| {
                query_lc.is_empty()
                    || binding.key.to_lowercase().contains(&query_lc)
                    || binding.description.to_lowercase().contains(&query_lc)
                    || binding.category.name().to_lowercase().contains(&query_lc)
            })
            .collect::<Vec<_>>();
        let state_for_search = self.state.clone();

        div()
            .id("phone-keybindings-settings")
            .size_full()
            .flex()
            .flex_col()
            .gap(d.grid)
            .child(
                gpui_ui_kit::SearchBar::new("phone-keybindings-search")
                    .value(query)
                    .placeholder("Search shortcuts")
                    .size(gpui_ui_kit::SearchBarSize::Sm)
                    .on_change(move |text, _window, cx| {
                        state_for_search.update(cx, |state, _cx| {
                            state.app.ui_state.phone_keybindings_query = text.to_string();
                        });
                    }),
            )
            .child(
                div()
                    .id("phone-keybindings-scroll")
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    .gap(d.grid)
                    .children(rows.into_iter().map(|binding| {
                        div()
                            .flex()
                            .items_center()
                            .gap(d.gap_md)
                            .min_h(rems(3.5))
                            .p(d.pad_x)
                            .rounded(d.r_md)
                            .bg(theme.surface)
                            .border_1()
                            .border_color(theme.border)
                            .child(
                                div()
                                    .min_w(rems(4.75))
                                    .px(d.grid)
                                    .py(d.grid)
                                    .rounded(d.r_md)
                                    .bg(theme.background_secondary)
                                    .text_size(d.text_xs)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.accent)
                                    .text_center()
                                    .child(binding.key),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .text_size(d.text_sm)
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme.text_primary)
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .whitespace_nowrap()
                                            .child(binding.description),
                                    )
                                    .child(
                                        div()
                                            .text_size(d.text_xs)
                                            .text_color(theme.text_muted)
                                            .child(binding.category.name()),
                                    ),
                            )
                    })),
            )
            .into_any_element()
    }

    fn render_phone_settings_row(
        &self,
        tab: crate::app::SettingsTab,
        translations: &crate::i18n::Translations,
        theme: &crate::theme::Theme,
        d: &Ds,
    ) -> AnyElement {
        let state_entity = self.state.clone();
        let label = crate::components::settings_tab_label(tab, translations);
        let icon = crate::components::settings_tab_icon_name(tab);

        div()
            .id(SharedString::from(format!("phone-settings-{tab:?}")))
            .flex()
            .items_center()
            .gap(d.gap_md)
            .min_h(rems(3.5))
            .px(d.card)
            .bg(theme.surface)
            .border_b_1()
            .border_color(theme.border)
            .cursor_pointer()
            .hover({
                let theme = theme.clone();
                move |s| s.bg(theme.surface_hover)
            })
            .child(Icon::new(icon).size(IconSize::Md).color(theme.text_muted))
            .child(
                div()
                    .flex_1()
                    .text_size(d.text_sm)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text_primary)
                    .child(label),
            )
            .child(
                Icon::new(IconName::ChevronRight)
                    .size(IconSize::Sm)
                    .color(theme.text_muted),
            )
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                state_entity.update(cx, |state, _cx| {
                    state.app.ui_state.active_settings_tab = tab;
                    state
                        .app
                        .set_screen(Screen::SettingsDetail, "PhoneSettingsRow");
                });
            })
            .into_any_element()
    }

    fn render_phone_placeholder(&self, title: &'static str, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let theme = self.state.read(cx).app.ui_state.theme.clone();

        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .p(d.card)
            .bg(theme.background)
            .child(
                div()
                    .text_size(d.text_sm)
                    .text_color(theme.text_muted)
                    .child(format!("{title} is not available yet.")),
            )
            .into_any_element()
    }

    fn render_phone_transport_button(
        &self,
        id: &'static str,
        icon: IconName,
        trigger: &'static str,
        theme: &crate::theme::Theme,
        _cx: &mut Context<Self>,
    ) -> AnyElement {
        let state_entity = self.state.clone();

        div()
            .id(id)
            .size(rems(2.75))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .cursor_pointer()
            .hover({
                let theme = theme.clone();
                move |s| s.bg(theme.surface_hover)
            })
            .child(Icon::new(icon).size(IconSize::Md).color(theme.text_primary))
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                state_entity.update(cx, |state, _cx| match trigger {
                    "PhoneMiniNext" | "PhoneNowNext" => {
                        if let Some(path) = state.app.next_track() {
                            PlayerView::play_track(state, path);
                        }
                    }
                    "PhoneNowPrev" => {
                        if let Some(path) = state.app.previous_track() {
                            PlayerView::play_track(state, path);
                        }
                    }
                    "PhoneNowRewind" => {
                        let new_position = (state.app.playback.position_secs - 30.0).max(0.0);
                        state.app.playback.position_secs = new_position;
                        if let Err(e) = state.player.lock().seek(new_position) {
                            log::error!("Failed to seek backward: {}", e);
                        }
                    }
                    "PhoneNowForward" => {
                        let max = state.app.playback.duration_secs;
                        let new_position = (state.app.playback.position_secs + 30.0).min(max);
                        state.app.playback.position_secs = new_position;
                        if let Err(e) = state.player.lock().seek(new_position) {
                            log::error!("Failed to seek forward: {}", e);
                        }
                    }
                    _ => {}
                });
            })
            .into_any_element()
    }

    fn render_phone_play_button(
        &self,
        id: &'static str,
        is_playing: bool,
        theme: &crate::theme::Theme,
        _cx: &mut Context<Self>,
    ) -> AnyElement {
        let state_entity = self.state.clone();
        let icon = if is_playing {
            IconName::Pause
        } else {
            IconName::Play
        };

        div()
            .id(id)
            .size(rems(3.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(theme.accent)
            .cursor_pointer()
            .child(
                Icon::new(icon)
                    .size(IconSize::Md)
                    .color(theme.text_on_accent),
            )
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                state_entity.update(cx, |state, _cx| {
                    if state.app.playback.current_queue_index.is_none() {
                        if let Some(source) = state.app.start_queue() {
                            PlayerView::play_track(state, source);
                        }
                    } else if state.app.playback.is_playing {
                        if let Err(e) = state.player.lock().pause() {
                            log::error!("Failed to pause: {}", e);
                        }
                        state.app.playback.is_playing = false;
                    } else {
                        if let Err(e) = state.player.lock().resume() {
                            log::error!("Failed to play: {}", e);
                        }
                        state.app.playback.is_playing = true;
                    }
                });
            })
            .into_any_element()
    }

    fn phone_screen_title(screen: Screen) -> &'static str {
        match screen {
            Screen::Home => "Home",
            Screen::HomeShelf => "Home",
            Screen::NowPlaying => "Now Playing",
            Screen::Library => "Library",
            Screen::Streams => "Streams",
            Screen::Queue => "Queue",
            Screen::Playlists => "Playlists",
            Screen::Spectrum => "Spectrum",
            Screen::Settings => "Settings",
            Screen::SettingsDetail => "Settings",
            Screen::StudioHub => "Studio",
            Screen::EqCurve => "EQ",
            Screen::Studio => "Rack",
            Screen::Recording => "Recording",
            Screen::RoomEq => "Room EQ",
            Screen::HeadphoneEq => "Headphone EQ",
            Screen::Spinorama => "Spinorama",
            Screen::PluginGraph => "Plugin Graph",
        }
    }

    fn format_phone_time(seconds: f64) -> String {
        let total = seconds.max(0.0) as u64;
        let minutes = total / 60;
        let seconds = total % 60;
        format!("{minutes}:{seconds:02}")
    }
}
