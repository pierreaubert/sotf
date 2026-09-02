use gpui::{StatefulInteractiveElement, deferred};

use crate::app::i18n::{
    ContextMenuTranslations, FooterTranslations, PhoneLibraryTranslations, PhoneToolTranslations,
    PhoneTranslations,
};

macro_rules! phone_dev_track {
    ($element:expr, $selector:expr) => {{
        #[cfg(feature = "dev-api")]
        {
            use crate::app::dev_api::DevTrackExt;
            ($element).dev_track($selector).into_any_element()
        }
        #[cfg(not(feature = "dev-api"))]
        {
            ($element).into_any_element()
        }
    }};
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PhoneTool {
    Recording,
    RoomEq,
    HeadphoneEq,
    Spinorama,
    ListeningTest,
    Spectrum,
    PluginGraph,
    Streams,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PhoneLibraryChipAction {
    ToggleMore,
    Reset,
    Apply,
}

impl PlayerView {
    fn phone_translations(&self, cx: &Context<Self>) -> PhoneTranslations {
        PhoneTranslations::for_language(self.state.read(cx).app.ui_state.language)
    }

    fn phone_tool_translations(&self, cx: &Context<Self>) -> PhoneToolTranslations {
        PhoneToolTranslations::for_language(self.state.read(cx).app.ui_state.language)
    }

    fn phone_library_translations(&self, cx: &Context<Self>) -> PhoneLibraryTranslations {
        PhoneLibraryTranslations::for_language(self.state.read(cx).app.ui_state.language)
    }

    fn render_phone_shell(
        &mut self,
        current_screen: Screen,
        layout_mode: crate::app::LayoutMode,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        reset_interactive_focus_order();
        let theme = self.state.read(cx).app.ui_state.theme.clone();
        let show_mini_player = {
            let state = self.state.read(cx);
            state.app.playback.current_queue_index.is_some() && current_screen != Screen::NowPlaying
        };
        let tab_bar = self.render_phone_tab_bar_collapsible(current_screen, cx);
        let tab_bar = if self.suppress_geometry_sync {
            // Off-screen visual QA captures a fresh scene into a cleared
            // texture. Paint the footer after route-specific deferred content
            // so every tab is materialized in deterministic screenshots.
            deferred(tab_bar).with_priority(100).into_any_element()
        } else {
            deferred(tab_bar).with_priority(1).into_any_element()
        };

        div()
            .id("phone-shell")
            .on_key_down(|event: &KeyDownEvent, window, cx| {
                if event.keystroke.key.as_str() == "tab"
                    && focus_interactive_relative(window, cx, event.keystroke.modifiers.shift)
                {
                    cx.stop_propagation();
                }
            })
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .min_w_0()
            .overflow_hidden()
            .bg(theme.background)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .w_full()
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
            .child(tab_bar)
            .into_any_element()
    }

    fn render_current_screen_phone(
        &mut self,
        screen: Screen,
        _layout_mode: crate::app::LayoutMode,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let translations = self.state.read(cx).app.ui_state.translations.clone();
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
            Screen::Playlists => self.render_playlists_screen(cx).into_any_element(),
            Screen::Spectrum => self.render_phone_spectrum_screen(cx),
            Screen::Recording => {
                let content = self.render_recording_screen(cx).into_any_element();
                self.render_phone_tool_wrapper(
                    PhoneTool::Recording,
                    translations.screen_recording,
                    translations.recording_capture_desc,
                    content,
                    cx,
                )
            }
            Screen::RoomEq => {
                let content = self.render_room_eq_screen(cx).into_any_element();
                self.render_phone_tool_wrapper(
                    PhoneTool::RoomEq,
                    translations.screen_room_eq,
                    self.phone_translations(cx).wizard,
                    content,
                    cx,
                )
            }
            Screen::HeadphoneEq => {
                let content = self.render_headphone_eq_screen(cx).into_any_element();
                self.render_phone_tool_wrapper(
                    PhoneTool::HeadphoneEq,
                    translations.screen_headphone_eq,
                    self.phone_translations(cx).wizard,
                    content,
                    cx,
                )
            }
            Screen::Spinorama => {
                let content = self.render_spinorama_eq_screen(cx).into_any_element();
                self.render_phone_tool_wrapper(
                    PhoneTool::Spinorama,
                    translations.screen_spinorama,
                    translations.spinorama_generate_speaker_eq,
                    content,
                    cx,
                )
            }
            Screen::PluginGraph => self.render_phone_plugin_graph_screen(cx),
            Screen::ListeningTest => {
                let content = self.render_listening_test_screen(cx).into_any_element();
                self.render_phone_tool_wrapper(
                    PhoneTool::ListeningTest,
                    translations.screen_listening_test,
                    translations.listening_test.trial.title,
                    content,
                    cx,
                )
            }
            Screen::Streams => self.render_streams_screen_phone(cx),
        }
    }

    fn render_phone_tab_bar(&self, current_screen: Screen, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let (theme, translations) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state.app.ui_state.translations.clone(),
            )
        };
        let tabs = [
            (Screen::Home, translations.screen_home, IconName::Home),
            (
                Screen::Library,
                translations.screen_library,
                IconName::Library,
            ),
            (
                Screen::NowPlaying,
                translations.screen_now_playing,
                IconName::Play,
            ),
            (
                Screen::StudioHub,
                translations.screen_studio,
                IconName::SlidersHorizontal,
            ),
            (
                Screen::Settings,
                translations.screen_settings,
                IconName::Settings,
            ),
        ];

        div()
            .id("phone-tab-bar")
            .flex()
            .items_center()
            .justify_between()
            .flex_none()
            .min_w_0()
            .w_full()
            .min_h(rems(4.25))
            .px(d.pad_y)
            .py(d.grid)
            .bg(theme.surface)
            .children(tabs.into_iter().map(|(screen, label, icon)| {
                let selected = current_screen == screen
                    || (screen == Screen::Home && current_screen == Screen::HomeShelf)
                    || (screen == Screen::StudioHub && current_screen.is_studio_tool())
                    || (screen == Screen::Settings && current_screen == Screen::SettingsDetail)
                    || (screen == Screen::NowPlaying && current_screen == Screen::Queue);
                self.render_phone_tab_item(screen, label, icon, selected, &theme, &d, cx)
            }))
            .into_any_element()
    }

    fn render_phone_tab_bar_collapsible(
        &self,
        current_screen: Screen,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let d = Ds::from_cx(cx);
        let theme = self.state.read(cx).app.ui_state.theme.clone();
        let hidden = self.state.read(cx).app.ui_state.phone_tab_bar_hidden;
        let state_for_toggle = self.state.clone();
        let text = self.phone_translations(cx);
        let handle_label = if hidden {
            text.show_navigation
        } else {
            text.hide_navigation
        };
        let handle_element_id: ElementId = "phone-tab-bar-handle".into();
        let handle_focus = interactive_focus_handle(&handle_element_id, cx);
        let accessibility_props =
            AriaProps::with_role(AriaRole::Button).state(AriaState::Expanded(!hidden));
        cx.register_accessible(AccessibilityNode {
            element_id: handle_element_id.clone(),
            label: handle_label.into(),
            props: accessibility_props.clone(),
        });
        let toggle = std::rc::Rc::new(move |cx: &mut App| {
            state_for_toggle.update(cx, |state, cx| {
                state.app.ui_state.phone_tab_bar_hidden = !state.app.ui_state.phone_tab_bar_hidden;
                cx.notify();
            });
        });
        let mouse_toggle = toggle.clone();
        let key_toggle = toggle;

        let current_tab_icon = [
            (Screen::Home, IconName::Home),
            (Screen::Library, IconName::Library),
            (Screen::NowPlaying, IconName::Play),
            (Screen::StudioHub, IconName::SlidersHorizontal),
            (Screen::Settings, IconName::Settings),
        ]
        .into_iter()
        .find_map(|(screen, icon)| {
            let selected = current_screen == screen
                || (screen == Screen::Home && current_screen == Screen::HomeShelf)
                || (screen == Screen::StudioHub && current_screen.is_studio_tool())
                || (screen == Screen::Settings && current_screen == Screen::SettingsDetail)
                || (screen == Screen::NowPlaying && current_screen == Screen::Queue);
            if selected { Some(icon) } else { None }
        })
        .unwrap_or(IconName::Home);

        let handle_icon = if hidden {
            IconName::ChevronUp
        } else {
            IconName::ChevronDown
        };

        let handle = div()
            .id(handle_element_id)
            .track_focus(&handle_focus)
            .track_focus_element(&handle_focus)
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .flex_none()
            .min_h(rems(1.25))
            .bg(theme.surface)
            .border_t_1()
            .border_color(theme.border)
            .cursor_pointer()
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.accent)
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(d.grid)
                    .child(
                        Icon::new(current_tab_icon)
                            .size(IconSize::Xs)
                            .color(theme.text_muted),
                    )
                    .child(
                        Icon::new(handle_icon)
                            .size(IconSize::Xs)
                            .color(theme.text_muted),
                    ),
            )
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                mouse_toggle(cx);
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    key_toggle(cx);
                    cx.stop_propagation();
                }
            });

        #[cfg(feature = "dev-api")]
        let handle = {
            use crate::app::dev_api::DevTrackExt;
            handle
                .dev_track_with_state(
                    "phone.tab-bar-handle",
                    crate::app::dev_api::DevElementState::default().expanded(!hidden),
                )
                .into_any_element()
        };
        #[cfg(not(feature = "dev-api"))]
        let handle = handle.into_any_element();

        if hidden {
            return handle;
        }

        div()
            .id("phone-tab-bar-collapsible")
            .flex()
            .flex_col()
            .flex_none()
            .min_w_0()
            .w_full()
            .overflow_hidden()
            .child(handle)
            .child(self.render_phone_tab_bar(current_screen, cx))
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
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state_entity = self.state.clone();
        let element_id: ElementId = format!("phone-tab-{screen:?}").into();
        let focus_handle = interactive_focus_handle(&element_id, cx);
        let accessibility_props =
            AriaProps::with_role(AriaRole::Tab).state(AriaState::Selected(selected));
        cx.register_accessible(AccessibilityNode {
            element_id: element_id.clone(),
            label: label.into(),
            props: accessibility_props.clone(),
        });
        let activate = std::rc::Rc::new(move |cx: &mut App| {
            state_entity.update(cx, |state, cx| {
                state.app.set_screen(screen, "PhoneTab");
                state.app.ui_state.input_mode = crate::app::InputMode::Normal;
                cx.notify();
            });
        });
        let mouse_activate = activate.clone();
        let key_activate = activate;
        let mouse_focus = focus_handle.clone();
        let fg = if selected {
            theme.accent
        } else {
            theme.text_muted
        };

        let element = div()
            .id(element_id)
            .track_focus(&focus_handle)
            .track_focus_element(&focus_handle)
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(d.half_grid)
            .flex_1()
            .min_w_0()
            .min_h(rems(3.5))
            .rounded(d.r_md)
            .text_color(fg)
            .cursor_pointer()
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.accent)
            })
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
            .on_mouse_up(MouseButton::Left, move |_event, window, cx| {
                mouse_activate(cx);
                window.focus(&mouse_focus, cx);
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    key_activate(cx);
                    cx.stop_propagation();
                }
            });

        #[cfg(feature = "dev-api")]
        {
            use crate::app::dev_api::DevTrackExt;
            element
                .dev_track_with_state(
                    format!("phone.tab.{screen:?}"),
                    crate::app::dev_api::DevElementState::default().selected(selected),
                )
                .into_any_element()
        }
        #[cfg(not(feature = "dev-api"))]
        {
            element.into_any_element()
        }
    }

    fn render_phone_home_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let text = self.phone_translations(cx);
        let theme = self.state.read(cx).app.ui_state.theme.clone();
        let state_for_search = self.state.clone();
        let state_for_search_key = self.state.clone();
        let search_element_id: ElementId = "phone-home-search".into();
        let search_focus = interactive_focus_handle(&search_element_id, cx);
        let search_mouse_focus = search_focus.clone();
        let search_props = AriaProps::with_role(AriaRole::Button);
        cx.register_accessible(AccessibilityNode {
            element_id: search_element_id.clone(),
            label: text.search_library.into(),
            props: search_props.clone(),
        });

        div()
            .id("phone-home-header")
            .flex()
            .items_center()
            .justify_between()
            .flex_none()
            .min_h(rems(3.25))
            .px(d.card)
            .py(d.grid)
            .bg(theme.background)
            .child(
                div()
                    .text_size(d.text_lg)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text_primary)
                    .child("SOTF"),
            )
            .child(phone_dev_track!(
                div()
                    .id(search_element_id)
                    .track_focus(&search_focus)
                    .track_focus_element(&search_focus)
                    .size(rems(2.75))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(d.r_md)
                    .cursor_pointer()
                    .focus_visible({
                        let theme = theme.clone();
                        move |style| style.border_2().border_color(theme.accent)
                    })
                    .hover({
                        let theme = theme.clone();
                        move |s| s.bg(theme.surface_hover)
                    })
                    .child(
                        Icon::new(IconName::Search)
                            .size(IconSize::Md)
                            .color(theme.text_primary),
                    )
                    .on_mouse_up(MouseButton::Left, move |_event, window, cx| {
                        state_for_search.update(cx, |state, cx| {
                            state.app.ui_state.input_mode = crate::app::InputMode::Search;
                            state.app.set_screen(Screen::Library, "PhoneHomeSearch");
                            cx.notify();
                        });
                        window.focus(&search_mouse_focus, cx);
                    })
                    .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                        let key = event.keystroke.key.as_str();
                        if key == "enter" || key == "space" {
                            state_for_search_key.update(cx, |state, cx| {
                                state.app.ui_state.input_mode = crate::app::InputMode::Search;
                                state.app.set_screen(Screen::Library, "PhoneHomeSearch");
                                cx.notify();
                            });
                            cx.stop_propagation();
                        }
                    }),
                "phone.home.search"
            ))
            .into_any_element()
    }

    fn render_phone_screen_header(
        &self,
        title: &'static str,
        back_screen: Screen,
        return_focus_element_id: ElementId,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let d = Ds::from_cx(cx);
        let text = self.phone_translations(cx);
        let theme = self.state.read(cx).app.ui_state.theme.clone();
        let state_for_back = self.state.clone();
        let state_for_back_key = self.state.clone();
        let back_element_id: ElementId = "phone-screen-back".into();
        let back_focus = interactive_focus_handle(&back_element_id, cx);
        let return_focus = interactive_focus_handle(&return_focus_element_id, cx);
        let return_focus_key = return_focus.clone();
        let back_props = AriaProps::with_role(AriaRole::Button);
        cx.register_accessible(AccessibilityNode {
            element_id: back_element_id.clone(),
            label: text.back.into(),
            props: back_props.clone(),
        });

        div()
            .id("phone-screen-header")
            .flex()
            .items_center()
            .gap(d.grid)
            .flex_none()
            .min_h(rems(3.25))
            .px(d.card)
            .py(d.grid)
            .bg(theme.surface)
            .border_b_1()
            .border_color(theme.border)
            .child(phone_dev_track!(
                div()
                    .id(back_element_id)
                    .track_focus(&back_focus)
                    .track_focus_element(&back_focus)
                    .size(rems(2.75))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(d.r_md)
                    .cursor_pointer()
                    .focus_visible({
                        let theme = theme.clone();
                        move |style| style.border_2().border_color(theme.accent)
                    })
                    .hover({
                        let theme = theme.clone();
                        move |s| s.bg(theme.surface_hover)
                    })
                    .child(
                        Icon::new(IconName::ChevronLeft)
                            .size(IconSize::Md)
                            .color(theme.text_primary),
                    )
                    .on_mouse_up(MouseButton::Left, move |_event, window, cx| {
                        state_for_back.update(cx, |state, cx| {
                            state.app.set_screen(back_screen, "PhoneBack");
                            cx.notify();
                        });
                        window.focus(&return_focus, cx);
                    })
                    .on_key_down(move |event: &KeyDownEvent, window, cx| {
                        let key = event.keystroke.key.as_str();
                        if key == "enter" || key == "space" {
                            state_for_back_key.update(cx, |state, cx| {
                                state.app.set_screen(back_screen, "PhoneBack");
                                cx.notify();
                            });
                            window.focus(&return_focus_key, cx);
                            cx.stop_propagation();
                        }
                    }),
                "phone.screen.back"
            ))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(d.text_lg)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text_primary)
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(title),
            )
            .into_any_element()
    }

    fn render_home_screen_phone(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let text = self.phone_translations(cx);
        let theme = self.state.read(cx).app.ui_state.theme.clone();
        let albums: Vec<sotf_audio_player::Album> =
            self.state.read(cx).app.library_state.library.albums.clone();

        if albums.is_empty() {
            return div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .p(d.card)
                .bg(theme.background)
                .text_color(theme.text_muted)
                .child(self.render_phone_home_header(cx))
                .child(text.home_empty)
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
            .child(self.render_phone_home_header(cx))
            .child(self.render_phone_home_shelf(
                crate::app::PhoneHomeShelf::RecentlyPlayed,
                recently_played.into_iter().take(12).collect(),
                text,
                &theme,
                &d,
                cx,
            ))
            .child(self.render_phone_home_shelf(
                crate::app::PhoneHomeShelf::MostPlayed,
                most_played.into_iter().take(12).collect(),
                text,
                &theme,
                &d,
                cx,
            ))
            .when(!favorites.is_empty(), |el| {
                el.child(self.render_phone_home_shelf(
                    crate::app::PhoneHomeShelf::Favorites,
                    favorites.into_iter().take(12).collect(),
                    text,
                    &theme,
                    &d,
                    cx,
                ))
            })
            .child(self.render_phone_home_shelf(
                crate::app::PhoneHomeShelf::NewInLibrary,
                recent_releases.into_iter().take(12).collect(),
                text,
                &theme,
                &d,
                cx,
            ))
            .into_any_element()
    }

    fn render_phone_home_shelf(
        &self,
        shelf: crate::app::PhoneHomeShelf,
        albums: Vec<&sotf_audio_player::Album>,
        text: PhoneTranslations,
        theme: &crate::theme::Theme,
        d: &Ds,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state_for_see_all = self.state.clone();
        let state_for_see_all_key = self.state.clone();
        let title = shelf.title();
        let see_all_element_id: ElementId = format!("phone-home-see-all-{shelf:?}").into();
        let see_all_focus = interactive_focus_handle(&see_all_element_id, cx);
        let see_all_mouse_focus = see_all_focus.clone();
        let see_all_label = format!("{} {title}", text.see_all);
        cx.register_accessible(AccessibilityNode {
            element_id: see_all_element_id.clone(),
            label: see_all_label.into(),
            props: AriaProps::with_role(AriaRole::Button),
        });

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
                    .child(phone_dev_track!(
                        div()
                            .id(see_all_element_id)
                            .track_focus(&see_all_focus)
                            .track_focus_element(&see_all_focus)
                            .min_h(rems(2.75))
                            .px(d.pad_x)
                            .flex()
                            .items_center()
                            .rounded(d.r_md)
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.accent)
                            .cursor_pointer()
                            .focus_visible({
                                let theme = theme.clone();
                                move |style| style.border_2().border_color(theme.accent)
                            })
                            .child(text.see_all)
                            .on_mouse_up(MouseButton::Left, move |_event, window, cx| {
                                state_for_see_all.update(cx, |state, cx| {
                                    state.app.ui_state.phone_home_shelf = shelf;
                                    state.app.set_screen(Screen::HomeShelf, "PhoneHomeSeeAll");
                                    cx.notify();
                                });
                                window.focus(&see_all_mouse_focus, cx);
                            })
                            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                                let key = event.keystroke.key.as_str();
                                if key == "enter" || key == "space" {
                                    state_for_see_all_key.update(cx, |state, cx| {
                                        state.app.ui_state.phone_home_shelf = shelf;
                                        state.app.set_screen(Screen::HomeShelf, "PhoneHomeSeeAll");
                                        cx.notify();
                                    });
                                    cx.stop_propagation();
                                }
                            }),
                        format!("phone.home.see-all.{shelf:?}")
                    )),
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
                            cx,
                        )
                    })),
            )
            .into_any_element()
    }

    fn render_home_shelf_screen_phone(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let (theme, shelf, columns, albums, album_count) = {
            let state = self.state.read(cx);
            let theme = state.app.ui_state.theme.clone();
            let shelf = state.app.ui_state.phone_home_shelf;
            let columns = if state.app.ui_state.window_width >= 430.0 {
                3
            } else {
                2
            };
            let mut albums: Vec<sotf_audio_player::Album> = state
                .app
                .library_state
                .library
                .albums
                .iter()
                .filter(|album| shelf != crate::app::PhoneHomeShelf::Favorites || album.is_favorite)
                .cloned()
                .collect();
            match shelf {
                crate::app::PhoneHomeShelf::RecentlyPlayed
                | crate::app::PhoneHomeShelf::MostPlayed => {
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
            let album_count = albums.len();
            (theme, shelf, columns, albums, album_count)
        };

        div()
            .id("phone-home-shelf-grid")
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .bg(theme.background)
            .child(self.render_phone_screen_header(
                shelf.title(),
                Screen::Home,
                format!("phone-home-see-all-{shelf:?}").into(),
                cx,
            ))
            .child(
                div()
                    .flex_none()
                    .px(d.card)
                    .py(d.grid)
                    .text_size(d.text_sm)
                    .text_color(theme.text_muted)
                    .child(format!("{} albums", album_count)),
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
                        albums.iter().enumerate().map(|(idx, album)| {
                            self.render_phone_album_tile(
                                idx,
                                album,
                                None,
                                shelf.title(),
                                &theme,
                                &d,
                                cx,
                            )
                        }),
                    )),
            )
            .into_any_element()
    }

    fn render_library_screen_phone(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let text = self.phone_translations(cx);
        let state = self.state.read(cx);
        let columns = if state.app.ui_state.window_width >= 430.0 {
            3
        } else {
            2
        };
        let theme = state.app.ui_state.theme.clone();
        let search_query = state.app.library_state.search_query.clone();
        let filter_menu_open = state.app.ui_state.filter_menu_open;
        let albums = state
            .app
            .get_paginated_albums()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
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
                        .placeholder(text.search_library)
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
            .child(self.render_phone_library_chips(filter_menu_open, &theme, &d, cx))
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
                                &album,
                                None,
                                "PhoneLibraryAlbum",
                                &theme,
                                &d,
                                cx,
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
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let text = self.phone_tool_translations(cx);
        let library_text = self.phone_library_translations(cx);
        let chips = [
            (
                library_text.year,
                Some(sotf_audio_player::LibrarySortOrder::Year),
                None,
                PhoneLibraryChipAction::Apply,
            ),
            (
                library_text.genre,
                Some(sotf_audio_player::LibrarySortOrder::Genre),
                None,
                PhoneLibraryChipAction::Apply,
            ),
            (
                library_text.artist,
                Some(sotf_audio_player::LibrarySortOrder::Artist),
                None,
                PhoneLibraryChipAction::Apply,
            ),
            (
                library_text.album,
                Some(sotf_audio_player::LibrarySortOrder::Album),
                None,
                PhoneLibraryChipAction::Apply,
            ),
            (text.more, None, None, PhoneLibraryChipAction::ToggleMore),
        ];
        let overflow_chips = [
            (
                library_text.tracks,
                Some(sotf_audio_player::LibrarySortOrder::Tracks),
                None,
                PhoneLibraryChipAction::Apply,
            ),
            (
                library_text.composer,
                Some(sotf_audio_player::LibrarySortOrder::Composer),
                None,
                PhoneLibraryChipAction::Apply,
            ),
            (
                library_text.stereo,
                None,
                Some(sotf_audio_player::ChannelFilter::Stereo),
                PhoneLibraryChipAction::Apply,
            ),
            (
                library_text.multichannel,
                None,
                Some(sotf_audio_player::ChannelFilter::SurroundPlus),
                PhoneLibraryChipAction::Apply,
            ),
            (
                self.phone_translations(cx).reset,
                None,
                Some(sotf_audio_player::ChannelFilter::All),
                PhoneLibraryChipAction::Reset,
            ),
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
                        .children(chips.into_iter().map(|(label, sort, filter, action)| {
                            self.render_phone_library_chip(
                                label, sort, filter, action, theme, d, cx,
                            )
                        })),
            )
            .when(filter_menu_open, |el| {
                el.child(
                    div()
                        .id("phone-library-more-chips")
                        .flex()
                        .gap(d.grid)
                        .overflow_x_scroll()
                        .children(overflow_chips.into_iter().map(
                            |(label, sort, filter, action)| {
                                self.render_phone_library_chip(
                                    label, sort, filter, action, theme, d, cx,
                                )
                            },
                        )),
                )
            })
            .into_any_element()
    }

    fn render_phone_library_chip(
        &self,
        label: &'static str,
        sort: Option<sotf_audio_player::LibrarySortOrder>,
        filter: Option<sotf_audio_player::ChannelFilter>,
        action: PhoneLibraryChipAction,
        theme: &crate::theme::Theme,
        d: &Ds,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state_entity = self.state.clone();
        let selector = match (action, sort, filter) {
            (PhoneLibraryChipAction::ToggleMore, _, _) => "phone.library-chip.more".to_string(),
            (PhoneLibraryChipAction::Reset, _, _) => "phone.library-chip.reset".to_string(),
            (_, Some(sort), _) => format!("phone.library-chip.sort.{sort:?}"),
            (_, _, Some(filter)) => format!("phone.library-chip.filter.{filter:?}"),
            _ => "phone.library-chip.action".to_string(),
        };
        let element_id: ElementId = selector.clone().into();
        let focus_handle = interactive_focus_handle(&element_id, cx);
        let accessibility_props = AriaProps::with_role(AriaRole::Button);
        cx.register_accessible(AccessibilityNode {
            element_id: element_id.clone(),
            label: label.into(),
            props: accessibility_props.clone(),
        });
        let activate = std::rc::Rc::new(move |cx: &mut App| {
            state_entity.update(cx, |state, _cx| {
                if action == PhoneLibraryChipAction::ToggleMore {
                    state.app.ui_state.filter_menu_open = !state.app.ui_state.filter_menu_open;
                    return;
                }
                if action == PhoneLibraryChipAction::Reset {
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
        });
        let mouse_activate = activate.clone();
        let key_activate = activate;

        let element = div()
            .id(element_id)
            .track_focus(&focus_handle)
            .track_focus_element(&focus_handle)
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
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.accent)
            })
            .hover({
                let theme = theme.clone();
                move |s| s.bg(theme.surface_hover)
            })
            .child(label)
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                mouse_activate(cx);
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    key_activate(cx);
                    cx.stop_propagation();
                }
            });
        let element = gpui_ui_kit::accessibility::apply_native_accessibility(
            element,
            label,
            &accessibility_props,
        );
        phone_dev_track!(element, selector)
    }

    fn render_queue_screen_phone(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let text = self.phone_translations(cx);
        let menu_text =
            ContextMenuTranslations::for_language(self.state.read(cx).app.ui_state.language);
        let (theme, rows, current_position, editing) = {
            let state = self.state.read(cx);
            let rows = state
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
            let current_position = state.app.queue_state.current_index().and_then(|album_idx| {
                state
                    .app
                    .queue_state
                    .items
                    .get(album_idx)
                    .map(|item| (album_idx, item.current_track_index))
            });
            (
                state.app.ui_state.theme.clone(),
                rows,
                current_position,
                state.app.ui_state.phone_queue_editing,
            )
        };

        let edit_label = if editing { text.done() } else { text.edit };
        let edit_id: ElementId = "phone-queue-edit".into();
        let edit_focus = interactive_focus_handle(&edit_id, cx);
        let edit_props =
            AriaProps::with_role(AriaRole::Button).state(AriaState::Pressed(editing));
        cx.register_accessible(AccessibilityNode {
            element_id: edit_id.clone(),
            label: edit_label.into(),
            props: edit_props.clone(),
        });
        let state_for_edit = self.state.clone();
        let view_for_edit = cx.entity().clone();
        let edit_activate = std::rc::Rc::new(move |cx: &mut App| {
            state_for_edit.update(cx, |state, _cx| {
                state.app.ui_state.phone_queue_editing =
                    !state.app.ui_state.phone_queue_editing;
            });
            view_for_edit.update(cx, |_view, cx| cx.notify());
        });
        let edit_mouse = edit_activate.clone();
        let edit_key = edit_activate;
        let edit_element = div()
            .id(edit_id)
            .track_focus(&edit_focus)
            .track_focus_element(&edit_focus)
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
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.accent)
            })
            .hover({
                let theme = theme.clone();
                move |style| style.bg(theme.surface_hover)
            })
            .child(edit_label)
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                edit_mouse(cx);
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    edit_key(cx);
                    cx.stop_propagation();
                }
            });
        let edit_element = gpui_ui_kit::accessibility::apply_native_accessibility(
            edit_element,
            edit_label,
            &edit_props,
        );
        let edit_element = phone_dev_track!(edit_element, "phone.queue.edit");

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
                            .child(text.tracks(rows.len())),
                    )
                    .child(edit_element),
            )
            .child(
                div()
                    .id("phone-queue-track-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
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
                                .child(text.queue_empty),
                        )
                    })
                    .children(rows.into_iter().map(
                        |(album_idx, track_idx, album_title, track)| {
                            self.render_phone_queue_track_row(
                                album_idx,
                                track_idx,
                                album_title,
                                track,
                                current_position == Some((album_idx, track_idx)),
                                editing,
                                text.untitled(),
                                text.unknown_artist(),
                                menu_text.remove_from_queue,
                                &theme,
                                &d,
                                cx,
                            )
                        },
                    )),
            )
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn render_phone_queue_track_row(
        &self,
        album_idx: usize,
        track_idx: usize,
        album_title: String,
        track: sotf_audio_player::Track,
        is_current: bool,
        editing: bool,
        untitled_label: &'static str,
        unknown_artist_label: &'static str,
        remove_label: &'static str,
        theme: &crate::theme::Theme,
        d: &Ds,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let title = track.title.clone().unwrap_or_else(|| {
            track
                .path
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or(untitled_label)
                .to_string()
        });
        let artist = track
            .artist
            .clone()
            .unwrap_or_else(|| unknown_artist_label.to_string());
        let duration = track
            .duration_secs
            .map(|seconds| Self::format_phone_time(seconds as f64))
            .unwrap_or_else(|| "--:--".to_string());
        let source = track.audio_source();

        let play_label = format!("{title}, {artist}, {album_title}, {duration}");
        let play_id: ElementId =
            format!("phone-queue-row-{album_idx}-{track_idx}").into();
        let play_focus = interactive_focus_handle(&play_id, cx);
        let play_props = AriaProps::with_role(AriaRole::Button);
        cx.register_accessible(AccessibilityNode {
            element_id: play_id.clone(),
            label: play_label.clone().into(),
            props: play_props.clone(),
        });
        let state_for_play = self.state.clone();
        let view_for_play = cx.entity().clone();
        let play_activate = std::rc::Rc::new(move |cx: &mut App| {
            state_for_play.update(cx, |state, _cx| {
                if album_idx < state.app.queue_state.items.len()
                    && track_idx
                        < state.app.queue_state.items[album_idx].album.tracks.len()
                {
                    state.app.queue_state.selected_index = album_idx;
                    state.app.queue_state.current_index = Some(album_idx);
                    state.app.queue_state.items[album_idx].current_track_index = track_idx;
                    state.app.playback.current_queue_index = Some(album_idx);
                    state.app.playback.is_playing = true;
                    PlayerView::play_track(state, source.clone());
                }
            });
            view_for_play.update(cx, |_view, cx| cx.notify());
        });
        let play_mouse = play_activate.clone();
        let play_key = play_activate;

        let play_element = div()
            .id(play_id)
            .track_focus(&play_focus)
            .track_focus_element(&play_focus)
            .flex()
            .flex_1()
            .min_w_0()
            .items_center()
            .gap(d.gap_md)
            .min_h(rems(2.75))
            .rounded(d.r_md)
            .cursor_pointer()
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.accent)
            })
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
                            .child(title.clone()),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_muted)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(format!("{artist} • {album_title}")),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_muted)
                            .child(duration),
                    ),
            )
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                play_mouse(cx);
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    play_key(cx);
                    cx.stop_propagation();
                }
            });
        let play_element = gpui_ui_kit::accessibility::apply_native_accessibility(
            play_element,
            play_label,
            &play_props,
        );
        let play_element = phone_dev_track!(
            play_element,
            format!("phone.queue.row.{album_idx}.{track_idx}")
        );

        let remove_element = editing.then(|| {
            let remove_accessible_label = format!("{remove_label}: {title}");
            let remove_id: ElementId =
                format!("phone-queue-remove-{album_idx}-{track_idx}").into();
            let remove_focus = interactive_focus_handle(&remove_id, cx);
            let remove_props = AriaProps::with_role(AriaRole::Button);
            cx.register_accessible(AccessibilityNode {
                element_id: remove_id.clone(),
                label: remove_accessible_label.clone().into(),
                props: remove_props.clone(),
            });
            let state_for_remove = self.state.clone();
            let view_for_remove = cx.entity().clone();
            let remove_activate = std::rc::Rc::new(move |cx: &mut App| {
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
                                    let _ = state.player.stop();
                                }
                                _ => {}
                            }
                        } else {
                            let current_track_index =
                                state.app.queue_state.items[album_idx].current_track_index;
                            if current_track_index
                                >= state.app.queue_state.items[album_idx].album.tracks.len()
                            {
                                state.app.queue_state.items[album_idx].current_track_index =
                                    state.app.queue_state.items[album_idx].album.tracks.len() - 1;
                            }
                        }
                    }
                });
                view_for_remove.update(cx, |_view, cx| cx.notify());
            });
            let remove_mouse = remove_activate.clone();
            let remove_key = remove_activate;
            let element = div()
                .id(remove_id)
                .track_focus(&remove_focus)
                .track_focus_element(&remove_focus)
                .size(rems(2.75))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(d.r_md)
                .cursor_pointer()
                .focus_visible({
                    let theme = theme.clone();
                    move |style| style.border_2().border_color(theme.accent)
                })
                .hover({
                    let theme = theme.clone();
                    move |style| style.bg(theme.surface_hover)
                })
                .child(
                    Icon::new(IconName::X)
                        .size(IconSize::Sm)
                        .color(theme.text_muted),
                )
                .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                    remove_mouse(cx);
                })
                .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                    let key = event.keystroke.key.as_str();
                    if key == "enter" || key == "space" {
                        remove_key(cx);
                        cx.stop_propagation();
                    }
                });
            let element = gpui_ui_kit::accessibility::apply_native_accessibility(
                element,
                remove_accessible_label,
                &remove_props,
            );
            phone_dev_track!(
                element,
                format!("phone.queue.remove.{album_idx}.{track_idx}")
            )
        });

        div()
            .id(SharedString::from(format!(
                "phone-queue-row-shell-{album_idx}-{track_idx}"
            )))
            .w_full()
            .flex()
            .items_center()
            .gap(d.grid)
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
            .child(play_element)
            .children(remove_element)
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
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state_entity = self.state.clone();
        let title = album.title.clone();
        let artist = album.artist();
        let album_id = album.id;
        let art_path = album.album_art_path.clone();
        let selected_title_for_activate = title.clone();
        let selected_artist_for_activate = artist.clone();
        let label = if artist.trim().is_empty() {
            title.clone()
        } else {
            format!("{title} — {artist}")
        };
        let selector = format!("phone.album.{trigger}.{idx}");
        let element_id: ElementId = selector.clone().into();
        let focus_handle = interactive_focus_handle(&element_id, cx);
        let accessibility_props = AriaProps::with_role(AriaRole::Button);
        cx.register_accessible(AccessibilityNode {
            element_id: element_id.clone(),
            label: label.clone().into(),
            props: accessibility_props.clone(),
        });
        let activate = std::rc::Rc::new(move |cx: &mut App| {
            let selected_title = selected_title_for_activate.clone();
            let selected_artist = selected_artist_for_activate.clone();
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
        });
        let mouse_activate = activate.clone();
        let key_activate = activate;

        let element = div()
            .id(element_id)
            .track_focus(&focus_handle)
            .track_focus_element(&focus_handle)
            .when_some(width_rems, |el, width| el.w(rems(width)))
            .when(width_rems.is_none(), |el| el.w_full())
            .flex_none()
            .flex()
            .flex_col()
            .gap(d.grid)
            .cursor_pointer()
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.accent)
            })
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
                mouse_activate(cx);
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    key_activate(cx);
                    cx.stop_propagation();
                }
            });
        let element = gpui_ui_kit::accessibility::apply_native_accessibility(
            element,
            &label,
            &accessibility_props,
        );
        phone_dev_track!(element, selector)
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
        let unknown_artist = self.phone_translations(cx).unknown_artist();
        let (theme, title, artist, art_path, is_playing, now_playing_label) = {
            let state = self.state.read(cx);
            let item = state
                .app
                .playback
                .current_queue_index
                .and_then(|idx| state.app.queue_state.get(idx));
            let track = item.and_then(|item| item.current_track());
            let now_playing_label = state.app.ui_state.translations.screen_now_playing;
            (
                state.app.ui_state.theme.clone(),
                track
                    .and_then(|track| track.title.clone())
                    .unwrap_or_else(|| now_playing_label.to_string()),
                track
                    .and_then(|track| track.artist.clone())
                    .unwrap_or_else(|| unknown_artist.to_string()),
                item.and_then(|item| item.album.album_art_path.clone()),
                state.app.playback.is_playing,
                now_playing_label,
            )
        };

        let open_label = format!("{now_playing_label}: {title}, {artist}");
        let open_id: ElementId = "phone-mini-player-open".into();
        let open_focus = interactive_focus_handle(&open_id, cx);
        let open_props = AriaProps::with_role(AriaRole::Button);
        cx.register_accessible(AccessibilityNode {
            element_id: open_id.clone(),
            label: open_label.clone().into(),
            props: open_props.clone(),
        });
        let state_for_expand = self.state.clone();
        let view_for_expand = cx.entity().clone();
        let open_activate = std::rc::Rc::new(move |cx: &mut App| {
            state_for_expand.update(cx, |state, _cx| {
                state.app.set_screen(Screen::NowPlaying, "PhoneMiniPlayer");
            });
            view_for_expand.update(cx, |_view, cx| cx.notify());
        });
        let open_mouse = open_activate.clone();
        let open_key = open_activate;
        let open_mouse_focus = open_focus.clone();

        let open_element = div()
            .id(open_id)
            .track_focus(&open_focus)
            .track_focus_element(&open_focus)
            .flex()
            .flex_1()
            .min_w_0()
            .items_center()
            .gap(d.gap_md)
            .rounded(d.r_md)
            .cursor_pointer()
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.accent)
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
            .on_mouse_up(MouseButton::Left, move |_event, window, cx| {
                open_mouse(cx);
                window.focus(&open_mouse_focus, cx);
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    open_key(cx);
                    cx.stop_propagation();
                }
            });
        let open_element = gpui_ui_kit::accessibility::apply_native_accessibility(
            open_element,
            open_label,
            &open_props,
        );
        let open_element = phone_dev_track!(open_element, "phone.mini-player.open");

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
            .child(open_element)
            .child(self.render_phone_transport_button(
                "phone-mini-next",
                IconName::SkipForward,
                "PhoneMiniNext",
                &theme,
                cx,
            ))
            .child(self.render_phone_play_button(
                "phone-mini-play",
                is_playing,
                &theme,
                cx,
            ))
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
            queue_label,
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
                state.app.ui_state.translations.queue_title,
            )
        };
        let progress = if duration > 0.0 {
            (position / duration).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        let language = self.state.read(cx).app.ui_state.language;
        let footer_text = crate::app::i18n::FooterTranslations::for_language(language);
        let output_label = self
            .state
            .read(cx)
            .app
            .ui_state
            .translations
            .devices_title;
        let queue_element_id: ElementId = "phone-now-queue".into();
        let queue_focus = interactive_focus_handle(&queue_element_id, cx);
        let queue_mouse_focus = queue_focus.clone();
        cx.register_accessible(AccessibilityNode {
            element_id: queue_element_id.clone(),
            label: queue_label.into(),
            props: AriaProps::with_role(AriaRole::Button),
        });
        let state_for_queue = self.state.clone();
        let state_for_queue_key = self.state.clone();
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
                                        if let Err(e) = state.player.seek(new_position) {
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
                            footer_text.shuffle,
                            IconName::Shuffle,
                            &theme,
                            None,
                            Some(shuffle_enabled),
                            cx,
                        ))
                        .child(self.render_phone_icon_button(
                            "phone-repeat",
                            footer_text.repeat,
                            IconName::Repeat,
                            &theme,
                            None,
                            Some(repeat_enabled),
                            cx,
                        ))
                        .child(self.render_phone_icon_button(
                            "phone-output",
                            output_label,
                            IconName::Speaker,
                            &theme,
                            Some(Screen::SettingsDetail),
                            None,
                            cx,
                        ))
                    .child(phone_dev_track!(
                        div()
                            .id(queue_element_id)
                            .track_focus(&queue_focus)
                            .track_focus_element(&queue_focus)
                            .size(rems(2.75))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(d.r_md)
                            .cursor_pointer()
                            .focus_visible({
                                let theme = theme.clone();
                                move |style| style.border_2().border_color(theme.accent)
                            })
                            .child(
                                Icon::new(IconName::ListMusic)
                                    .size(IconSize::Lg)
                                    .color(theme.text_muted),
                            )
                            .on_mouse_up(MouseButton::Left, move |_event, window, cx| {
                                state_for_queue.update(cx, |state, cx| {
                                    state.app.set_screen(Screen::Queue, "PhoneQueueButton");
                                    cx.notify();
                                });
                                window.focus(&queue_mouse_focus, cx);
                            })
                            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                                let key = event.keystroke.key.as_str();
                                if key == "enter" || key == "space" {
                                    state_for_queue_key.update(cx, |state, cx| {
                                        state.app.set_screen(Screen::Queue, "PhoneQueueButton");
                                        cx.notify();
                                    });
                                    cx.stop_propagation();
                                }
                            }),
                        "phone.now.queue"
                    )),
            )
            .child(self.render_phone_now_playing_drawer(&theme, &d, cx))
            .into_any_element()
    }

    fn render_phone_icon_button(
        &self,
        id: &'static str,
        label: &'static str,
        icon: IconName,
        theme: &crate::theme::Theme,
        target_screen: Option<Screen>,
        selected: Option<bool>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state_entity = self.state.clone();
        let is_selected = selected.unwrap_or(false);
        let element_id: ElementId = id.into();
        let focus_handle = interactive_focus_handle(&element_id, cx);
        let accessibility_props = AriaProps::with_role(AriaRole::Button)
            .maybe_state(selected.is_some(), AriaState::Pressed(is_selected));
        cx.register_accessible(AccessibilityNode {
            element_id: element_id.clone(),
            label: label.into(),
            props: accessibility_props.clone(),
        });
        let activate = std::rc::Rc::new(move |cx: &mut App| {
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
        });
        let mouse_activate = activate.clone();
        let key_activate = activate;

        let element = div()
            .id(element_id)
            .track_focus(&focus_handle)
            .track_focus_element(&focus_handle)
            .size(rems(2.75))
            .flex()
            .items_center()
            .justify_center()
            .rounded(rems(0.5))
            .when(is_selected, |el| el.bg(theme.surface_selected))
            .cursor_pointer()
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.accent)
            })
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
                mouse_activate(cx);
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    key_activate(cx);
                    cx.stop_propagation();
                }
            });
        let element = gpui_ui_kit::accessibility::apply_native_accessibility(
            element,
            label,
            &accessibility_props,
        );
        phone_dev_track!(element, format!("phone.icon.{id}"))
    }

    fn render_phone_now_playing_drawer(
        &self,
        theme: &crate::theme::Theme,
        d: &Ds,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let text = self.phone_translations(cx);
        let tool_text = self.phone_tool_translations(cx);
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
                            if plugin.enabled {
                                tool_text.on
                            } else {
                                tool_text.off
                            },
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
                    .child(text.up_next),
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
                    .child(text.plugin_chain),
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
        let tool_text = self.phone_tool_translations(cx);
        let (theme, translations, release_channel) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state.app.ui_state.translations.clone(),
                state.app.ui_state.release_channel,
            )
        };
        let tools = [
            (
                Screen::Studio,
                tool_text.plugin_rack,
                IconName::SlidersHorizontal,
            ),
            (
                Screen::Spectrum,
                tool_text.spectrum,
                IconName::AudioWaveform,
            ),
            (Screen::EqCurve, tool_text.eq, IconName::AudioWaveform),
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
            (
                Screen::Streams,
                translations.screen_streams,
                IconName::ListMusic,
            ),
            (Screen::PluginGraph, tool_text.plugin_graph, IconName::Plug),
            (
                Screen::ListeningTest,
                translations.screen_listening_test,
                IconName::Headphones,
            ),
            (
                Screen::Spinorama,
                translations.screen_spinorama,
                IconName::Speaker,
            ),
        ];

        let element =
            div()
                .id("phone-studio-hub")
                .size_full()
                .overflow_y_scroll()
                .bg(theme.background)
                .p(d.card)
                .child(div().flex().flex_wrap().gap(d.gap_md).children(
                    tools.into_iter().filter_map(|(screen, label, icon)| {
                        if release_channel.allows(screen.maturity()) {
                            Some(self.render_studio_hub_card(screen, label, icon, &theme, &d, cx))
                        } else {
                            None
                        }
                    }),
                ));

        phone_dev_track!(element, "phone.studio.screen")
    }

    fn render_studio_hub_card(
        &self,
        screen: Screen,
        label: &'static str,
        icon: IconName,
        theme: &crate::theme::Theme,
        d: &Ds,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state_entity = self.state.clone();
        let state_for_key = self.state.clone();
        let element_id: ElementId = format!("phone-studio-{screen:?}").into();
        let focus_handle = interactive_focus_handle(&element_id, cx);
        let mouse_focus = focus_handle.clone();
        cx.register_accessible(AccessibilityNode {
            element_id: element_id.clone(),
            label: label.into(),
            props: AriaProps::with_role(AriaRole::Button),
        });

        let element = div()
            .id(element_id)
            .track_focus(&focus_handle)
            .track_focus_element(&focus_handle)
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
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.accent)
            })
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.accent)
            })
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
            .on_mouse_up(MouseButton::Left, move |_event, window, cx| {
                state_entity.update(cx, |state, cx| {
                    state.app.set_screen(screen, "PhoneStudioHub");
                    cx.notify();
                });
                window.focus(&mouse_focus, cx);
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    state_for_key.update(cx, |state, cx| {
                        state.app.set_screen(screen, "PhoneStudioHub");
                        cx.notify();
                    });
                    cx.stop_propagation();
                }
            });

        phone_dev_track!(element, format!("phone.studio.{screen:?}"))
    }

    fn render_phone_plugin_rack(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let text = self.phone_translations(cx);
        let tool_text = self.phone_tool_translations(cx);
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
        let state_for_back = self.state.clone();
        let back_id: ElementId = "phone-plugin-rack-back".into();
        let back_focus = interactive_focus_handle(&back_id, cx);
        let back_props = AriaProps::with_role(AriaRole::Button);
        cx.register_accessible(AccessibilityNode {
            element_id: back_id.clone(),
            label: text.back.into(),
            props: back_props.clone(),
        });
        let back_activate = std::rc::Rc::new(move |cx: &mut App| {
            state_for_back.update(cx, |state, _cx| {
                state.app.set_screen(Screen::StudioHub, "PhoneRackBack");
            });
        });
        let back_mouse = back_activate.clone();
        let back_key = back_activate;
        let back_element = div()
            .id(back_id)
            .track_focus(&back_focus)
            .track_focus_element(&back_focus)
            .size(rems(2.75))
            .flex()
            .items_center()
            .justify_center()
            .rounded(d.r_md)
            .cursor_pointer()
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.accent)
            })
            .hover({
                let theme = theme.clone();
                move |style| style.bg(theme.surface_hover)
            })
            .child(
                Icon::new(IconName::ChevronLeft)
                    .size(IconSize::Md)
                    .color(theme.text_primary),
            )
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                back_mouse(cx);
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    back_key(cx);
                    cx.stop_propagation();
                }
            });
        let back_element = gpui_ui_kit::accessibility::apply_native_accessibility(
            back_element,
            text.back,
            &back_props,
        );
        let back_element = phone_dev_track!(back_element, "phone.plugin-rack.back");

        let edit_label = if rack_editing { "Done" } else { text.edit };
        let edit_id: ElementId = "phone-plugin-rack-edit".into();
        let edit_focus = interactive_focus_handle(&edit_id, cx);
        let edit_props =
            AriaProps::with_role(AriaRole::Button).state(AriaState::Pressed(rack_editing));
        cx.register_accessible(AccessibilityNode {
            element_id: edit_id.clone(),
            label: edit_label.into(),
            props: edit_props.clone(),
        });
        let view_for_edit = cx.entity().clone();
        let edit_activate = std::rc::Rc::new(move |cx: &mut App| {
            state_for_edit.update(cx, |state, _cx| {
                state.app.ui_state.phone_plugin_rack_editing =
                    !state.app.ui_state.phone_plugin_rack_editing;
            });
            view_for_edit.update(cx, |_view, cx| cx.notify());
        });
        let edit_mouse = edit_activate.clone();
        let edit_key = edit_activate;
        let edit_element = div()
            .id(edit_id)
            .track_focus(&edit_focus)
            .track_focus_element(&edit_focus)
            .min_h(rems(2.75))
            .px(d.pad_x)
            .flex()
            .items_center()
            .gap(d.grid)
            .rounded(d.r_md)
            .bg(theme.background_secondary)
            .text_color(theme.text_primary)
            .cursor_pointer()
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.accent)
            })
            .child(edit_label)
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                edit_mouse(cx);
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    edit_key(cx);
                    cx.stop_propagation();
                }
            });
        let edit_element = gpui_ui_kit::accessibility::apply_native_accessibility(
            edit_element,
            edit_label,
            &edit_props,
        );
        let edit_element = phone_dev_track!(edit_element, "phone.plugin-rack.edit");

        let add_id: ElementId = "phone-plugin-rack-add".into();
        let add_focus = interactive_focus_handle(&add_id, cx);
        let add_props = AriaProps::with_role(AriaRole::Button);
        cx.register_accessible(AccessibilityNode {
            element_id: add_id.clone(),
            label: text.add.into(),
            props: add_props.clone(),
        });
        let add_activate = std::rc::Rc::new(move |cx: &mut App| {
            state_for_add.update(cx, |state, _cx| {
                state.app.ui_state.active_menu = crate::app::ActiveMenu::AddPlugin;
            });
        });
        let add_mouse = add_activate.clone();
        let add_key = add_activate;
        let add_element = div()
            .id(add_id)
            .track_focus(&add_focus)
            .track_focus_element(&add_focus)
            .min_h(rems(2.75))
            .px(d.pad_x)
            .flex()
            .items_center()
            .gap(d.grid)
            .rounded(d.r_md)
            .bg(theme.accent)
            .text_color(theme.text_on_accent)
            .cursor_pointer()
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.text_on_accent)
            })
            .child(Icon::new(IconName::Plus).size(IconSize::Sm))
            .child(text.add)
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                add_mouse(cx);
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    add_key(cx);
                    cx.stop_propagation();
                }
            });
        let add_element = gpui_ui_kit::accessibility::apply_native_accessibility(
            add_element,
            text.add,
            &add_props,
        );
        let add_element = phone_dev_track!(add_element, "phone.plugin-rack.add");

        let menu_text =
            ContextMenuTranslations::for_language(self.state.read(cx).app.ui_state.language);
        let plugin_count = plugins.len();

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
                            .flex()
                            .items_center()
                            .gap(d.grid)
                            .child(back_element)
                            .child(
                                div()
                                    .text_size(d.text_sm)
                                    .text_color(theme.text_muted)
                                    .child(format!("{} plugins", plugins.len())),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(d.grid)
                            .child(edit_element)
                            .child(add_element),
                    ),
            )
            .when(add_open, |el| {
                el.child(self.render_phone_plugin_picker(release_channel, &theme, &d, cx))
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
                        self.render_phone_plugin_card(
                            idx,
                            plugin,
                                rack_editing,
                                &theme,
                                &d,
                                tool_text,
                                menu_text,
                                plugin_count,
                                cx,
                            )
                    })),
            )
            .into_any_element()
    }

    fn render_phone_plugin_picker(
        &self,
        release_channel: sotf_audio_player::ReleaseChannel,
        theme: &crate::theme::Theme,
        d: &Ds,
        cx: &mut Context<Self>,
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
                            let element_id: ElementId =
                                format!("phone-plugin-picker-{label}").into();
                            let focus_handle = interactive_focus_handle(&element_id, cx);
                            let accessibility_props = AriaProps::with_role(AriaRole::Button);
                            cx.register_accessible(AccessibilityNode {
                                element_id: element_id.clone(),
                                label: label.into(),
                                props: accessibility_props.clone(),
                            });
                            let activate = std::rc::Rc::new(move |cx: &mut App| {
                                state_entity.update(cx, |state, _cx| {
                                    state.app.add_plugin(&plugin_type);
                                    state.app.ui_state.active_menu = crate::app::ActiveMenu::None;
                                });
                            });
                            let mouse_activate = activate.clone();
                            let key_activate = activate;
                            let element = div()
                                .id(element_id)
                                .track_focus(&focus_handle)
                                .track_focus_element(&focus_handle)
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
                                .focus_visible({
                                    let theme = theme.clone();
                                    move |style| style.border_2().border_color(theme.accent)
                                })
                                .child(label)
                                .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                                    mouse_activate(cx);
                                })
                                .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                                    let key = event.keystroke.key.as_str();
                                    if key == "enter" || key == "space" {
                                        key_activate(cx);
                                        cx.stop_propagation();
                                    }
                                });
                            let element = gpui_ui_kit::accessibility::apply_native_accessibility(
                                element,
                                label,
                                &accessibility_props,
                            );
                            phone_dev_track!(element, format!("phone.plugin-picker.{label}"))
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
        tool_text: PhoneToolTranslations,
        menu_text: ContextMenuTranslations,
        plugin_count: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state_for_open = self.state.clone();
        let state_for_bypass = self.state.clone();
        let name = plugin.display_name();
        let plugin_type = plugin.plugin_type().name().to_string();
        let summary = Self::phone_plugin_card_summary(&plugin);
        let card_label = format!("{name}, {plugin_type}");
        let card_id: ElementId = format!("phone-plugin-card-{idx}").into();
        let card_focus = interactive_focus_handle(&card_id, cx);
        let card_props = AriaProps::with_role(AriaRole::Button);
        cx.register_accessible(AccessibilityNode {
            element_id: card_id.clone(),
            label: card_label.clone().into(),
            props: card_props.clone(),
        });
        let view_for_open = cx.entity().clone();
        let open_activate = std::rc::Rc::new(move |cx: &mut App| {
            state_for_open.update(cx, |state, _cx| {
                state.app.plugin_state.selected_plugin_index = idx;
                state.app.plugin_state.editing_plugin_index = Some(idx);
                state.app.plugin_state.plugin_ui_state.plugin_ui_view =
                    crate::app::state::plugin::PluginUiView::Simple;
            });
            view_for_open.update(cx, |_view, cx| cx.notify());
        });
        let open_mouse = open_activate.clone();
        let open_key = open_activate;
        let plugin_enabled = plugin.enabled;
        let bypass_label = format!(
            "{name}: {}",
            if plugin_enabled {
                tool_text.on
            } else {
                tool_text.bypass
            }
        );
        let bypass_id: ElementId = format!("phone-plugin-{idx}-bypass").into();
        let bypass_focus = interactive_focus_handle(&bypass_id, cx);
        let bypass_props =
            AriaProps::with_role(AriaRole::Button).state(AriaState::Pressed(plugin_enabled));
        cx.register_accessible(AccessibilityNode {
            element_id: bypass_id.clone(),
            label: bypass_label.clone().into(),
            props: bypass_props.clone(),
        });
        let bypass_activate = std::rc::Rc::new(move |cx: &mut App| {
            state_for_bypass.update(cx, |state, _cx| {
                state.app.toggle_plugin(idx);
            });
        });
        let bypass_mouse = bypass_activate.clone();
        let bypass_key = bypass_activate;
        let bypass_element = div()
            .id(bypass_id)
            .track_focus(&bypass_focus)
            .track_focus_element(&bypass_focus)
            .min_w(rems(4.5))
            .min_h(rems(2.75))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(if plugin_enabled {
                theme.surface_selected
            } else {
                theme.background_secondary
            })
            .text_size(d.text_xs)
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(if plugin_enabled {
                theme.accent
            } else {
                theme.text_muted
            })
            .cursor_pointer()
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.accent)
            })
            .child(if plugin_enabled {
                tool_text.on
            } else {
                tool_text.bypass
            })
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                cx.stop_propagation();
                bypass_mouse(cx);
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    bypass_key(cx);
                    cx.stop_propagation();
                }
            });
        let bypass_element = gpui_ui_kit::accessibility::apply_native_accessibility(
            bypass_element,
            bypass_label,
            &bypass_props,
        );
        let bypass_element =
            phone_dev_track!(bypass_element, format!("phone.plugin.{idx}.bypass"));
        let move_controls = rack_editing.then(|| {
            let move_up = self.render_phone_plugin_move_button(
                idx,
                true,
                idx > 0,
                format!("{}: {name}", menu_text.move_up),
                theme,
                d,
                cx,
            );
            let move_down = self.render_phone_plugin_move_button(
                idx,
                false,
                idx + 1 < plugin_count,
                format!("{}: {name}", menu_text.move_down),
                theme,
                d,
                cx,
            );
            div()
                .flex()
                .flex_col()
                .flex_none()
                .items_center()
                .gap(d.grid)
                .child(move_up)
                .child(move_down)
                .into_any_element()
        });

        let card = div()
            .id(card_id)
            .track_focus(&card_focus)
            .track_focus_element(&card_focus)
            .flex()
            .items_center()
            .gap(if rack_editing { d.grid } else { d.gap_md })
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
                    .flex_none()
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
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
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
            .child(bypass_element)
            .children(move_controls)
            .when(!rack_editing, |card| {
                card.child(
                    Icon::new(IconName::ChevronRight)
                        .size(IconSize::Sm)
                        .color(theme.text_muted),
                )
            })
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                open_mouse(cx);
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    open_key(cx);
                    cx.stop_propagation();
                }
            });
        let card = gpui_ui_kit::accessibility::apply_native_accessibility(
            card,
            card_label,
            &card_props,
        );
        phone_dev_track!(card, format!("phone.plugin.{idx}"))
    }

    fn render_phone_plugin_move_button(
        &self,
        idx: usize,
        move_up: bool,
        enabled: bool,
        label: String,
        theme: &crate::theme::Theme,
        d: &Ds,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let suffix = if move_up { "move-up" } else { "move-down" };
        let element_id: ElementId = format!("phone-plugin-{idx}-{suffix}").into();
        let focus_handle = interactive_focus_handle(&element_id, cx);
        let accessibility_props = AriaProps::with_role(AriaRole::Button);
        cx.register_accessible(AccessibilityNode {
            element_id: element_id.clone(),
            label: label.clone().into(),
            props: accessibility_props.clone(),
        });
        let state_entity = self.state.clone();
        let activate = std::rc::Rc::new(move |cx: &mut App| {
            if !enabled {
                return;
            }
            state_entity.update(cx, |state, _cx| {
                if move_up {
                    state.app.move_plugin_up(idx);
                } else {
                    state.app.move_plugin_down(idx);
                }
            });
        });
        let mouse_activate = activate.clone();
        let key_activate = activate;
        let element = div()
            .id(element_id)
            .track_focus(&focus_handle)
            .track_focus_element(&focus_handle)
            .size(rems(2.5))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(d.r_md)
            .bg(theme.background_secondary)
            .when(enabled, |element| element.cursor_pointer())
            .when(!enabled, |element| element.opacity(0.45))
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.accent)
            })
            .child(
                Icon::new(if move_up {
                    IconName::ChevronUp
                } else {
                    IconName::ChevronDown
                })
                .size(IconSize::Sm)
                .color(theme.text_primary),
            )
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                cx.stop_propagation();
                mouse_activate(cx);
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    key_activate(cx);
                    cx.stop_propagation();
                }
            });
        let element = gpui_ui_kit::accessibility::apply_native_accessibility(
            element,
            label,
            &accessibility_props,
        );
        phone_dev_track!(element, format!("phone.plugin.{idx}.{suffix}"))
    }

    fn render_phone_plugin_parameter_sheet(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let text = self.phone_translations(cx);
        let tool_text = self.phone_tool_translations(cx);
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
        let back_id: ElementId = "phone-plugin-sheet-back".into();
        let back_focus = interactive_focus_handle(&back_id, cx);
        let back_props = AriaProps::with_role(AriaRole::Button);
        cx.register_accessible(AccessibilityNode {
            element_id: back_id.clone(),
            label: text.back.into(),
            props: back_props.clone(),
        });
        let state_for_close = self.state.clone();
        let view_for_close = cx.entity().clone();
        let close_activate = std::rc::Rc::new(move |cx: &mut App| {
            state_for_close.update(cx, |state, _cx| {
                state.app.plugin_state.editing_plugin_index = None;
            });
            view_for_close.update(cx, |_view, cx| cx.notify());
        });
        let close_mouse = close_activate.clone();
        let close_key = close_activate;
        let back_element = div()
            .id(back_id)
            .track_focus(&back_focus)
            .track_focus_element(&back_focus)
            .size(rems(2.75))
            .flex()
            .items_center()
            .justify_center()
            .rounded(d.r_md)
            .cursor_pointer()
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.accent)
            })
            .hover({
                let theme = theme.clone();
                move |style| style.bg(theme.surface_hover)
            })
            .child(
                Icon::new(IconName::ChevronLeft)
                    .size(IconSize::Md)
                    .color(theme.text_primary),
            )
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| close_mouse(cx))
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    close_key(cx);
                    cx.stop_propagation();
                }
            });
        let back_element = gpui_ui_kit::accessibility::apply_native_accessibility(
            back_element,
            text.back,
            &back_props,
        );
        let back_element = phone_dev_track!(back_element, "phone.plugin-sheet.back");

        let bypass_label = format!(
            "{title}: {}",
            if enabled { tool_text.on } else { tool_text.bypass }
        );
        let bypass_id: ElementId = "phone-plugin-sheet-bypass".into();
        let bypass_focus = interactive_focus_handle(&bypass_id, cx);
        let bypass_props =
            AriaProps::with_role(AriaRole::Button).state(AriaState::Pressed(enabled));
        cx.register_accessible(AccessibilityNode {
            element_id: bypass_id.clone(),
            label: bypass_label.clone().into(),
            props: bypass_props.clone(),
        });
        let state_for_bypass = self.state.clone();
        let view_for_bypass = cx.entity().clone();
        let bypass_activate = std::rc::Rc::new(move |cx: &mut App| {
            state_for_bypass.update(cx, |state, _cx| {
                state.app.toggle_plugin(selected_idx);
            });
            view_for_bypass.update(cx, |_view, cx| cx.notify());
        });
        let bypass_mouse = bypass_activate.clone();
        let bypass_key = bypass_activate;
        let bypass_element = div()
            .id(bypass_id)
            .track_focus(&bypass_focus)
            .track_focus_element(&bypass_focus)
            .min_w(rems(4.5))
            .min_h(rems(2.75))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(if enabled { theme.surface_selected } else { theme.background_secondary })
            .text_size(d.text_xs)
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(if enabled { theme.accent } else { theme.text_muted })
            .cursor_pointer()
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.accent)
            })
            .child(if enabled { tool_text.on } else { tool_text.bypass })
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| bypass_mouse(cx))
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    bypass_key(cx);
                    cx.stop_propagation();
                }
            });
        let bypass_element = gpui_ui_kit::accessibility::apply_native_accessibility(
            bypass_element,
            bypass_label,
            &bypass_props,
        );
        let bypass_element = phone_dev_track!(bypass_element, "phone.plugin-sheet.bypass");

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
                    .child(back_element)
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
                    .child(bypass_element),
            )
            .child(
                div()
                    .id("phone-plugin-sheet-scroll")
                    .overflow_y_scroll()
                    .flex_1()
                    .min_h_0()
                    .p(d.card)
                    .child(match settings {
                        Some(settings) if settings.eq_global_filters().is_some() => self
                            .render_phone_eq_parameter_sheet(
                                selected_idx,
                                settings,
                                text,
                                &theme,
                                &d,
                                cx,
                            ),
                        Some(settings) => self.render_phone_generic_parameter_sheet(
                            selected_idx,
                            settings,
                            text,
                            &theme,
                            &d,
                            cx,
                        ),
                        None => div()
                            .min_h(rems(10.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(theme.text_muted)
                            .child(text.no_plugin_selected)
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
            | sotf_audio_player::PluginSettings::LinearPhaseEq { filters, .. } => filters
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

    fn render_phone_sheet_action_button(
        &self,
        label: String,
        selector: String,
        danger: bool,
        activate: std::rc::Rc<dyn Fn(&mut App)>,
        theme: &crate::theme::Theme,
        d: &Ds,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let element_id: ElementId = selector.replace('.', "-").into();
        let focus_handle = interactive_focus_handle(&element_id, cx);
        let accessibility_props = AriaProps::with_role(AriaRole::Button);
        cx.register_accessible(AccessibilityNode {
            element_id: element_id.clone(),
            label: label.clone().into(),
            props: accessibility_props.clone(),
        });
        let mouse_activate = activate.clone();
        let key_activate = activate;
        let mouse_focus = focus_handle.clone();
        let element = div()
            .id(element_id)
            .track_focus(&focus_handle)
            .track_focus_element(&focus_handle)
            .flex_1()
            .min_h(rems(2.75))
            .px(d.pad_x)
            .flex()
            .items_center()
            .justify_center()
            .rounded(d.r_md)
            .bg(theme.background_secondary)
            .text_size(d.text_sm)
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(if danger { theme.error } else { theme.text_primary })
            .cursor_pointer()
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.accent)
            })
            .child(label.clone())
            .on_mouse_up(MouseButton::Left, move |_event, window, cx| {
                mouse_activate(cx);
                window.focus(&mouse_focus, cx);
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    key_activate(cx);
                    cx.stop_propagation();
                }
            });
        let element = gpui_ui_kit::accessibility::apply_native_accessibility(
            element,
            label,
            &accessibility_props,
        );
        phone_dev_track!(element, selector)
    }

    fn render_phone_eq_parameter_sheet(
        &self,
        plugin_idx: usize,
        settings: sotf_audio_player::PluginSettings,
        text: PhoneTranslations,
        theme: &crate::theme::Theme,
        d: &Ds,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let linear_phase =
            matches!(settings, sotf_audio_player::PluginSettings::LinearPhaseEq { .. });
        let filters = settings.eq_global_filters().cloned().unwrap_or_default();
        let state_for_add = self.state.clone();
        let view_for_add = cx.entity().clone();
        let add_filter = std::rc::Rc::new(move |cx: &mut App| {
            state_for_add.update(cx, |state, _cx| {
                if let Err(err) = state.app.add_eq_band() {
                    state.app.ui_state.toast_message =
                        Some(crate::app::ToastMessage::error(err));
                }
            });
            view_for_add.update(cx, |_view, cx| cx.notify());
        });

        div()
            .id("phone-eq-parameter-sheet")
            .flex()
            .flex_col()
            .gap(d.section)
            .children(filters.iter().enumerate().map(|(band_idx, filter)| {
                self.render_phone_eq_band(
                    plugin_idx,
                    band_idx,
                    filter.clone(),
                    linear_phase,
                    text,
                    theme,
                    d,
                    cx,
                )
            }))
            .child(self.render_phone_sheet_action_button(
                text.add_filter.to_string(),
                "phone.plugin-sheet.add-filter".to_string(),
                false,
                add_filter,
                theme,
                d,
                cx,
            ))
            .into_any_element()
    }

    fn render_phone_eq_band(
        &self,
        plugin_idx: usize,
        band_idx: usize,
        filter: sotf_audio_player::EQFilter,
        linear_phase: bool,
        text: PhoneTranslations,
        theme: &crate::theme::Theme,
        d: &Ds,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (param_stride, frequency_offset, q_offset, gain_offset) = if linear_phase {
            (5, 1, 2, 3)
        } else {
            (4, 0, 1, 2)
        };
        let base_param = band_idx * param_stride;
        let state_for_reset = self.state.clone();
        let view_for_reset = cx.entity().clone();
        let reset = std::rc::Rc::new(move |cx: &mut App| {
            state_for_reset.update(cx, |state, _cx| {
                state
                    .app
                    .set_plugin_param(plugin_idx, base_param + frequency_offset, 1000.0);
                state
                    .app
                    .set_plugin_param(plugin_idx, base_param + q_offset, 1.0);
                state
                    .app
                    .set_plugin_param(plugin_idx, base_param + gain_offset, 0.0);
            });
            view_for_reset.update(cx, |_view, cx| cx.notify());
        });
        let state_for_delete = self.state.clone();
        let view_for_delete = cx.entity().clone();
        let delete = std::rc::Rc::new(move |cx: &mut App| {
            state_for_delete.update(cx, |state, _cx| {
                if let Err(err) = state.app.remove_eq_band(band_idx) {
                    state.app.ui_state.toast_message =
                        Some(crate::app::ToastMessage::error(err));
                }
            });
            view_for_delete.update(cx, |_view, cx| cx.notify());
        });

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
                    .child(if filter.muted {
                        text.muted()
                    } else {
                        text.active()
                    }),
                    ),
            )
            .child(self.render_phone_param_slider(
                "Frequency",
                format!("{:.0} Hz", filter.frequency),
                filter.frequency,
                20.0,
                20_000.0,
                plugin_idx,
                base_param + frequency_offset,
                theme,
                d,
                cx,
            ))
            .child(self.render_phone_param_slider(
                "Gain",
                format!("{:+.1} dB", filter.gain_db),
                filter.gain_db,
                -24.0,
                24.0,
                plugin_idx,
                base_param + gain_offset,
                theme,
                d,
                cx,
            ))
            .child(self.render_phone_param_slider(
                "Q",
                format!("{:.2}", filter.q),
                filter.q,
                0.1,
                10.0,
                plugin_idx,
                base_param + q_offset,
                theme,
                d,
                cx,
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(d.grid)
                    .child(self.render_phone_sheet_action_button(
                        text.reset.to_string(),
                        format!("phone.plugin-sheet.band.{band_idx}.reset"),
                        false,
                        reset,
                        theme,
                        d,
                        cx,
                    ))
                    .child(self.render_phone_sheet_action_button(
                        text.delete_filter.to_string(),
                        format!("phone.plugin-sheet.band.{band_idx}.delete"),
                        true,
                        delete,
                        theme,
                        d,
                        cx,
                    )),
            )
            .into_any_element()
    }

    fn render_phone_generic_parameter_sheet(
        &self,
        plugin_idx: usize,
        settings: sotf_audio_player::PluginSettings,
        text: PhoneTranslations,
        theme: &crate::theme::Theme,
        d: &Ds,
        cx: &mut Context<Self>,
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
                        .child(text.no_touch_parameters),
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
                    cx,
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
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let ratio = if max > min {
            ((value - min) / (max - min)).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
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

        let text = self.phone_translations(cx);
        let decrease_label = text.decrease(label);
        let decrease_id: ElementId =
            format!("phone-param-{plugin_idx}-{param_idx}-decrease").into();
        let decrease_focus = interactive_focus_handle(&decrease_id, cx);
        let decrease_props = AriaProps::with_role(AriaRole::Button);
        cx.register_accessible(AccessibilityNode {
            element_id: decrease_id.clone(),
            label: decrease_label.clone().into(),
            props: decrease_props.clone(),
        });
        let state_for_minus = self.state.clone();
        let view_for_minus = cx.entity().clone();
        let decrease_activate = std::rc::Rc::new(move |cx: &mut App| {
            state_for_minus.update(cx, |state, _cx| {
                state.app.set_plugin_param(
                    plugin_idx,
                    param_idx,
                    (value - step).clamp(min, max),
                );
            });
            view_for_minus.update(cx, |_view, cx| cx.notify());
        });
        let decrease_mouse = decrease_activate.clone();
        let decrease_key = decrease_activate;
        let decrease_element = div()
            .id(decrease_id)
            .track_focus(&decrease_focus)
            .track_focus_element(&decrease_focus)
            .size(rems(2.75))
            .flex()
            .items_center()
            .justify_center()
            .rounded(d.r_md)
            .bg(theme.background_secondary)
            .cursor_pointer()
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.accent)
            })
            .child(
                Icon::new(IconName::Minus)
                    .size(IconSize::Sm)
                    .color(theme.text_primary),
            )
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                decrease_mouse(cx);
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    decrease_key(cx);
                    cx.stop_propagation();
                }
            });
        let decrease_element = gpui_ui_kit::accessibility::apply_native_accessibility(
            decrease_element,
            decrease_label,
            &decrease_props,
        );
        let decrease_element = phone_dev_track!(
            decrease_element,
            format!("phone.plugin-param.{plugin_idx}.{param_idx}.decrease")
        );

        let increase_label = text.increase(label);
        let increase_id: ElementId =
            format!("phone-param-{plugin_idx}-{param_idx}-increase").into();
        let increase_focus = interactive_focus_handle(&increase_id, cx);
        let increase_props = AriaProps::with_role(AriaRole::Button);
        cx.register_accessible(AccessibilityNode {
            element_id: increase_id.clone(),
            label: increase_label.clone().into(),
            props: increase_props.clone(),
        });
        let state_for_plus = self.state.clone();
        let view_for_plus = cx.entity().clone();
        let increase_activate = std::rc::Rc::new(move |cx: &mut App| {
            state_for_plus.update(cx, |state, _cx| {
                state.app.set_plugin_param(
                    plugin_idx,
                    param_idx,
                    (value + step).clamp(min, max),
                );
            });
            view_for_plus.update(cx, |_view, cx| cx.notify());
        });
        let increase_mouse = increase_activate.clone();
        let increase_key = increase_activate;
        let increase_element = div()
            .id(increase_id)
            .track_focus(&increase_focus)
            .track_focus_element(&increase_focus)
            .size(rems(2.75))
            .flex()
            .items_center()
            .justify_center()
            .rounded(d.r_md)
            .bg(theme.background_secondary)
            .cursor_pointer()
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.accent)
            })
            .child(
                Icon::new(IconName::Plus)
                    .size(IconSize::Sm)
                    .color(theme.text_primary),
            )
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                increase_mouse(cx);
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    increase_key(cx);
                    cx.stop_propagation();
                }
            });
        let increase_element = gpui_ui_kit::accessibility::apply_native_accessibility(
            increase_element,
            increase_label,
            &increase_props,
        );
        let increase_element = phone_dev_track!(
            increase_element,
            format!("phone.plugin-param.{plugin_idx}.{param_idx}.increase")
        );

        let state_for_value = self.state.clone();
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
                        .label(label)
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
                    .child(decrease_element)
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
                    .child(increase_element),
            )
            .into_any_element()
    }
    fn render_phone_eq_curve(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let text = self.phone_translations(cx);
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
            .child(self.render_phone_screen_header(
                "EQ Curve",
                Screen::StudioHub,
                "phone-studio-EqCurve".into(),
                cx,
            ))
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
                    .gap(d.half_grid)
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
                            .child(text.edit)
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
                                    .child(text.filters),
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
        let text = self.phone_tool_translations(cx);
        let (hold, smooth) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.phone_spectrum_hold,
                state.app.ui_state.phone_spectrum_smoothed,
            )
        };
        let content = self.render_spectrum_screen(cx).into_any_element();

        self.render_phone_tool_wrapper(
            PhoneTool::Spectrum,
            text.spectrum,
            if hold {
                text.held_analyzer_frame
            } else if smooth {
                text.smoothed_live_analyzer
            } else {
                text.live_analyzer
            },
            content,
            cx,
        )
    }

    fn render_phone_plugin_graph_remove_button(
        &self,
        idx: usize,
        plugin_name: String,
        remove_label: &str,
        theme: &crate::theme::Theme,
        d: &Ds,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let label = format!("{remove_label} {plugin_name}");
        let element_id: ElementId = format!("phone-plugin-graph-remove-{idx}").into();
        let focus_handle = interactive_focus_handle(&element_id, cx);
        let accessibility_props = AriaProps::with_role(AriaRole::Button);
        cx.register_accessible(AccessibilityNode {
            element_id: element_id.clone(),
            label: label.clone().into(),
            props: accessibility_props.clone(),
        });

        let state_entity = self.state.clone();
        let activate = std::rc::Rc::new(move |cx: &mut App| {
            state_entity.update(cx, |state, _cx| {
                state.app.remove_plugin(idx);
                state.app.ui_state.phone_plugin_graph_actions_open = false;
            });
        });
        let mouse_activate = activate.clone();
        let key_activate = activate;

        let element = div()
            .id(element_id)
            .track_focus(&focus_handle)
            .track_focus_element(&focus_handle)
            .min_h(rems(2.75))
            .px(d.pad_x)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .rounded(d.r_md)
            .bg(theme.error)
            .text_size(d.text_xs)
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme.text_on_accent)
            .cursor_pointer()
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.text_primary)
            })
            .child(remove_label.to_string())
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                cx.stop_propagation();
                mouse_activate(cx);
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    key_activate(cx);
                    cx.stop_propagation();
                }
            });
        let element = gpui_ui_kit::accessibility::apply_native_accessibility(
            element,
            label,
            &accessibility_props,
        );
        phone_dev_track!(element, format!("phone.plugin-graph.remove.{idx}"))
    }

    fn render_phone_plugin_graph_node(
        &self,
        idx: usize,
        plugin: sotf_audio_player::Plugin,
        on_label: &str,
        off_label: &str,
        theme: &crate::theme::Theme,
        d: &Ds,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let name = plugin.display_name();
        let plugin_type = plugin.plugin_type().name().to_string();
        let summary = Self::phone_plugin_card_summary(&plugin);
        let status = if plugin.enabled { on_label } else { off_label };
        let label = format!("{name}, {plugin_type}, {status}");
        let element_id: ElementId = format!("phone-plugin-graph-node-{idx}").into();
        let focus_handle = interactive_focus_handle(&element_id, cx);
        let accessibility_props = AriaProps::with_role(AriaRole::Button);
        cx.register_accessible(AccessibilityNode {
            element_id: element_id.clone(),
            label: label.clone().into(),
            props: accessibility_props.clone(),
        });

        let state_entity = self.state.clone();
        let activate = std::rc::Rc::new(move |cx: &mut App| {
            state_entity.update(cx, |state, _cx| {
                state.app.plugin_state.selected_plugin_index = idx;
                state.app.plugin_state.editing_plugin_index = Some(idx);
                state.app.set_screen(Screen::Studio, "PhoneGraphNode");
            });
        });
        let mouse_activate = activate.clone();
        let key_activate = activate;

        let element = div()
            .id(element_id)
            .track_focus(&focus_handle)
            .track_focus_element(&focus_handle)
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
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.accent)
            })
            .hover({
                let theme = theme.clone();
                move |style| style.bg(theme.surface_hover)
            })
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
                            .child(name),
                    )
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_muted)
                            .child(summary),
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
                    .child(status.to_string()),
            )
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                mouse_activate(cx);
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    key_activate(cx);
                    cx.stop_propagation();
                }
            });
        let element = gpui_ui_kit::accessibility::apply_native_accessibility(
            element,
            label,
            &accessibility_props,
        );
        phone_dev_track!(element, format!("phone.plugin-graph.node.{idx}"))
    }

    fn render_phone_plugin_graph_screen(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let text = self.phone_translations(cx);
        let tool_text = self.phone_tool_translations(cx);
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
        let open_rack_id: ElementId = "phone-plugin-graph-open-rack".into();
        let open_rack_focus = interactive_focus_handle(&open_rack_id, cx);
        let open_rack_props = AriaProps::with_role(AriaRole::Button);
        cx.register_accessible(AccessibilityNode {
            element_id: open_rack_id.clone(),
            label: text.open_rack_editor.into(),
            props: open_rack_props.clone(),
        });
        let open_rack_activate = std::rc::Rc::new(move |cx: &mut App| {
            state_for_open_rack.update(cx, |state, _cx| {
                state.app.set_screen(Screen::Studio, "PhoneGraphRack");
            });
        });
        let open_rack_mouse = open_rack_activate.clone();
        let open_rack_key = open_rack_activate;
        let open_rack = div()
            .id(open_rack_id)
            .track_focus(&open_rack_focus)
            .track_focus_element(&open_rack_focus)
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
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.text_on_accent)
            })
            .child(text.open_rack_editor)
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                open_rack_mouse(cx);
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    open_rack_key(cx);
                    cx.stop_propagation();
                }
            });
        let open_rack = gpui_ui_kit::accessibility::apply_native_accessibility(
            open_rack,
            text.open_rack_editor,
            &open_rack_props,
        );
        let open_rack = phone_dev_track!(open_rack, "phone.plugin-graph.open-rack");

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
                            .child(open_rack)
                            .child(self.render_phone_plugin_picker(release_channel, &theme, &d, cx))
                            .children(action_plugins.iter().enumerate().map(|(idx, plugin)| {
                                let remove_button = self.render_phone_plugin_graph_remove_button(
                                    idx,
                                    plugin.display_name(),
                                    text.remove,
                                    &theme,
                                    &d,
                                    cx,
                                );
                                div()
                                    .w_full()
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
                                            .flex_1()
                                            .min_w_0()
                                            .text_size(d.text_xs)
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme.text_primary)
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .whitespace_nowrap()
                                            .child(plugin.display_name()),
                                    )
                                    .child(remove_button)
                            })),
                    )
                })
                .children(plugins.into_iter().enumerate().map(|(idx, plugin)| {
                    self.render_phone_plugin_graph_node(
                        idx,
                        plugin,
                        tool_text.on,
                        tool_text.off,
                        &theme,
                        &d,
                        cx,
                    )
                }))
                .into_any_element()
        } else {
            self.render_plugin_graph_screen(cx).into_any_element()
        };

        self.render_phone_tool_wrapper(
            PhoneTool::PluginGraph,
            tool_text.plugin_graph,
            tool_text.signal_flow,
            content,
            cx,
        )
    }

    fn render_streams_screen_phone(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let text = self.phone_translations(cx);
        let tool_text = self.phone_tool_translations(cx);
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
                            .child(text.no_saved_streams),
                    )
                })
                .children(streams.into_iter().enumerate().map(|(idx, stream)| {
                    self.render_phone_stream_row(idx, stream, text, &theme, &d, cx)
                }))
                .into_any_element()
        };

        self.render_phone_tool_wrapper(
            PhoneTool::Streams,
            self.state.read(cx).app.ui_state.translations.screen_streams,
            tool_text.remote_sources,
            content,
            cx,
        )
    }

    fn render_phone_stream_row(
        &self,
        idx: usize,
        stream: sotf_audio_player::SavedStream,
        text: PhoneTranslations,
        theme: &crate::theme::Theme,
        d: &Ds,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state_for_detail = self.state.clone();
        let state_for_play = self.state.clone();
        let play_stream = stream.clone();
        let row_label = if stream.url.trim().is_empty() {
            stream.name.clone()
        } else {
            format!("{} — {}", stream.name, stream.url)
        };
        let row_element_id: ElementId = format!("phone-stream-row-{idx}").into();
        let row_focus = interactive_focus_handle(&row_element_id, cx);
        let row_accessibility = AriaProps::with_role(AriaRole::Button);
        cx.register_accessible(AccessibilityNode {
            element_id: row_element_id.clone(),
            label: row_label.clone().into(),
            props: row_accessibility.clone(),
        });
        let detail_activate = std::rc::Rc::new(move |cx: &mut App| {
            state_for_detail.update(cx, |state, _cx| {
                state.app.set_stream_inputs_from_selected(idx);
                state.app.ui_state.phone_stream_sources_open = true;
            });
        });
        let detail_mouse_activate = detail_activate.clone();
        let detail_key_activate = detail_activate;

        let play_element_id: ElementId = format!("phone-stream-play-{idx}").into();
        let play_focus = interactive_focus_handle(&play_element_id, cx);
        let play_accessibility = AriaProps::with_role(AriaRole::Button);
        cx.register_accessible(AccessibilityNode {
            element_id: play_element_id.clone(),
            label: text.play.into(),
            props: play_accessibility.clone(),
        });
        let play_activate = std::rc::Rc::new(move |cx: &mut App| {
            let stream = play_stream.clone();
            state_for_play.update(cx, |state, _cx| {
                match state.app.play_stream_now(stream) {
                    Ok(Some(source)) => PlayerView::play_track(state, source),
                    Ok(None) => {}
                    Err(err) => state.app.record_stream_error(err),
                }
            });
        });
        let play_mouse_activate = play_activate.clone();
        let play_key_activate = play_activate;
        let play_element = div()
            .id(play_element_id)
            .track_focus(&play_focus)
            .track_focus_element(&play_focus)
            .min_h(rems(2.75))
            .px(d.pad_x)
            .flex()
            .items_center()
            .rounded(d.r_md)
            .bg(theme.accent)
            .text_color(theme.text_on_accent)
            .font_weight(FontWeight::SEMIBOLD)
            .cursor_pointer()
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.text_primary)
            })
            .child(text.play)
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                cx.stop_propagation();
                play_mouse_activate(cx);
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    play_key_activate(cx);
                    cx.stop_propagation();
                }
            });
        let play_element = gpui_ui_kit::accessibility::apply_native_accessibility(
            play_element,
            text.play,
            &play_accessibility,
        );
        let play_element = phone_dev_track!(play_element, format!("phone.stream.{idx}.play"));

        let row = div()
            .id(row_element_id)
            .track_focus(&row_focus)
            .track_focus_element(&row_focus)
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
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.accent)
            })
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
            .child(play_element)
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                detail_mouse_activate(cx);
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    detail_key_activate(cx);
                    cx.stop_propagation();
                }
            });
        let row = gpui_ui_kit::accessibility::apply_native_accessibility(
            row,
            &row_label,
            &row_accessibility,
        );
        phone_dev_track!(row, format!("phone.stream.{idx}"))
    }

    fn render_phone_wizard_button(
        &self,
        label: String,
        selector: &'static str,
        primary: bool,
        activate: std::rc::Rc<dyn Fn(&mut App)>,
        theme: &crate::theme::Theme,
        d: &Ds,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let element_id: ElementId = selector.replace('.', "-").into();
        let focus_handle = interactive_focus_handle(&element_id, cx);
        let accessibility_props = AriaProps::with_role(AriaRole::Button);
        cx.register_accessible(AccessibilityNode {
            element_id: element_id.clone(),
            label: label.clone().into(),
            props: accessibility_props.clone(),
        });
        let mouse_activate = activate.clone();
        let key_activate = activate;
        let mouse_focus = focus_handle.clone();
        let element = div()
            .id(element_id)
            .track_focus(&focus_handle)
            .track_focus_element(&focus_handle)
            .min_h(rems(2.25))
            .px(d.grid)
            .flex()
            .items_center()
            .rounded(d.r_md)
            .bg(if primary {
                theme.accent
            } else {
                theme.background_secondary
            })
            .text_size(d.text_xs)
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(if primary {
                theme.text_on_accent
            } else {
                theme.text_primary
            })
            .cursor_pointer()
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.accent)
            })
            .child(label.clone())
            .on_mouse_up(MouseButton::Left, move |_event, window, cx| {
                mouse_activate(cx);
                window.focus(&mouse_focus, cx);
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    key_activate(cx);
                    cx.stop_propagation();
                }
            });
        let element = gpui_ui_kit::accessibility::apply_native_accessibility(
            element,
            label,
            &accessibility_props,
        );
        phone_dev_track!(element, selector)
    }

    fn render_phone_tool_wrapper(
        &self,
        tool: PhoneTool,
        title: &'static str,
        subtitle: &'static str,
        content: AnyElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let d = Ds::from_cx(cx);
        let text = self.phone_translations(cx);
        let tool_text = self.phone_tool_translations(cx);
        let (
            theme,
            subtitle_text,
            progress,
            wizard_kind,
            spectrum_hold,
            spectrum_smoothed,
            graph_list,
            graph_actions_open,
            streams_open,
        ) = {
            let state = self.state.read(cx);
            let wizard_status = match tool {
                PhoneTool::Recording => {
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
                PhoneTool::RoomEq => {
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
                PhoneTool::HeadphoneEq => {
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
                PhoneTool::Spinorama => {
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
                match tool {
                    PhoneTool::Recording => Some("recording"),
                    PhoneTool::RoomEq => Some("room_eq"),
                    PhoneTool::HeadphoneEq => Some("headphone_eq"),
                    PhoneTool::Spinorama => Some("spinorama"),
                    _ => None,
                },
                state.app.ui_state.phone_spectrum_hold,
                state.app.ui_state.phone_spectrum_smoothed,
                state.app.ui_state.phone_plugin_graph_list,
                state.app.ui_state.phone_plugin_graph_actions_open,
                state.app.ui_state.phone_stream_sources_open,
            )
        };
        let wizard_back = self.state.clone();
        let wizard_next = self.state.clone();
        let state_for_back = self.state.clone();

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
                                    .id("phone-tool-back")
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
                                        Icon::new(IconName::ChevronLeft)
                                            .size(IconSize::Md)
                                            .color(theme.text_primary),
                                    )
                                    .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                                        state_for_back.update(cx, |state, _cx| {
                                            state
                                                .app
                                                .set_screen(Screen::StudioHub, "PhoneToolBack");
                                        });
                                    }),
                            )
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
                                        let state_for_wizard_back = wizard_back.clone();
                                        let view_for_wizard_back = cx.entity().clone();
                                        let back = std::rc::Rc::new(move |cx: &mut App| {
                                            state_for_wizard_back.update(cx, |state, _cx| {
                                                Self::move_phone_wizard_step(state, kind, false);
                                            });
                                            view_for_wizard_back.update(cx, |_view, cx| cx.notify());
                                        });
                                        let state_for_wizard_next = wizard_next.clone();
                                        let view_for_wizard_next = cx.entity().clone();
                                        let next = std::rc::Rc::new(move |cx: &mut App| {
                                            state_for_wizard_next.update(cx, |state, _cx| {
                                                Self::move_phone_wizard_step(state, kind, true);
                                            });
                                            view_for_wizard_next.update(cx, |_view, cx| cx.notify());
                                        });
                                        el.child(self.render_phone_wizard_button(
                                            text.back.to_string(),
                                            "phone.wizard.back",
                                            false,
                                            back,
                                            &theme,
                                            &d,
                                            cx,
                                        ))
                                        .child(self.render_phone_wizard_button(
                                            text.next.to_string(),
                                            "phone.wizard.next",
                                            true,
                                            next,
                                            &theme,
                                            &d,
                                            cx,
                                        ))
                                    })
                                    .when(tool == PhoneTool::Spectrum, |el| {
                                        el.child(self.render_phone_tool_toggle(
                                            tool_text.hold,
                                            IconName::Pause,
                                            spectrum_hold,
                                            "spectrum_hold",
                                            &theme,
                                            &d,
                                            cx,
                                        ))
                                        .child(
                                            self.render_phone_tool_toggle(
                                                tool_text.smooth,
                                                IconName::AudioWaveform,
                                                spectrum_smoothed,
                                                "spectrum_smoothed",
                                                &theme,
                                                &d,
                                                cx,
                                            ),
                                        )
                                    })
                                    .when(tool == PhoneTool::PluginGraph, |el| {
                                        el.child(self.render_phone_tool_toggle(
                                            if graph_list {
                                                tool_text.graph
                                            } else {
                                                tool_text.list
                                            },
                                            IconName::ListMusic,
                                            graph_list,
                                            "plugin_graph_list",
                                            &theme,
                                            &d,
                                            cx,
                                        ))
                                        .child(
                                            self.render_phone_tool_toggle(
                                                tool_text.actions,
                                                IconName::Settings,
                                                graph_actions_open,
                                                "plugin_graph_actions",
                                                &theme,
                                                &d,
                                                cx,
                                            ),
                                        )
                                    })
                                    .when(tool == PhoneTool::Streams, |el| {
                                        el.child(self.render_phone_tool_toggle(
                                            tool_text.sources,
                                            IconName::ListMusic,
                                            streams_open,
                                            "stream_sources",
                                            &theme,
                                            &d,
                                            cx,
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
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let element_id: ElementId = format!("phone-tool-toggle-{action}").into();
        let focus_handle = interactive_focus_handle(&element_id, cx);
        let accessibility_props =
            AriaProps::with_role(AriaRole::Button).state(AriaState::Pressed(selected));
        cx.register_accessible(AccessibilityNode {
            element_id: element_id.clone(),
            label: label.into(),
            props: accessibility_props.clone(),
        });

        let state_entity = self.state.clone();
        let view_for_toggle = cx.entity().clone();
        let activate = std::rc::Rc::new(move |cx: &mut App| {
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
                                            .map(|info| info.magnitudes.as_ref().to_vec())
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
            view_for_toggle.update(cx, |_view, cx| cx.notify());
        });
        let mouse_activate = activate.clone();
        let key_activate = activate;

        let element = div()
            .id(element_id)
            .track_focus(&focus_handle)
            .track_focus_element(&focus_handle)
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
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.accent)
            })
            .hover({
                let theme = theme.clone();
                move |style| style.bg(theme.surface_hover)
            })
            .child(Icon::new(icon).size(IconSize::Sm))
            .child(label)
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                mouse_activate(cx);
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    key_activate(cx);
                    cx.stop_propagation();
                }
            });
        let element = gpui_ui_kit::accessibility::apply_native_accessibility(
            element,
            label,
            &accessibility_props,
        );
        phone_dev_track!(element, format!("phone.tool-toggle.{action}"))
    }
    fn move_phone_wizard_step(state: &mut crate::app::AppState, _kind: &str, forward: bool) {
        state.app.move_workflow_step(forward);
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

        let element = div()
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
                    .children(visible_tabs.into_iter().map(|tab| {
                        self.render_phone_settings_row(tab, &translations, &theme, &d, cx)
                    })),
            );

        phone_dev_track!(element, "phone.settings.screen")
    }

    fn render_settings_detail_phone(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let (theme, active_tab, visible_tabs, translations) = {
            let state = self.state.read(cx);
            (
                state.app.ui_state.theme.clone(),
                state.app.ui_state.active_settings_tab,
                crate::app::SettingsTab::visible_tabs(),
                state.app.ui_state.translations.clone(),
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

        let title = crate::components::settings_tab_label(active_tab, &translations);

        let element = div()
            .id("phone-settings-detail")
            .size_full()
            .overflow_y_scroll()
            .bg(theme.background)
            .flex()
            .flex_col()
            .gap(d.grid)
            .child(self.render_phone_screen_header(
                title,
                Screen::Settings,
                format!("phone-settings-{active_tab:?}").into(),
                cx,
            ))
            .child(div().p(d.card).child(content));

        phone_dev_track!(element, "phone.settings.detail")
    }

    fn render_phone_keybindings_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let text = self.phone_translations(cx);
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
                    .placeholder(text.search_shortcuts)
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
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state_entity = self.state.clone();
        let state_for_key = self.state.clone();
        let label = crate::components::settings_tab_label(tab, translations);
        let icon = crate::components::settings_tab_icon_name(tab);
        let element_id: ElementId = format!("phone-settings-{tab:?}").into();
        let focus_handle = interactive_focus_handle(&element_id, cx);
        let mouse_focus = focus_handle.clone();
        cx.register_accessible(AccessibilityNode {
            element_id: element_id.clone(),
            label: label.into(),
            props: AriaProps::with_role(AriaRole::Button),
        });

        let element = div()
            .id(element_id)
            .track_focus(&focus_handle)
            .track_focus_element(&focus_handle)
            .flex()
            .items_center()
            .gap(d.gap_md)
            .min_h(rems(3.5))
            .px(d.card)
            .bg(theme.surface)
            .border_b_1()
            .border_color(theme.border)
            .cursor_pointer()
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.accent)
            })
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
            .on_mouse_up(MouseButton::Left, move |_event, window, cx| {
                state_entity.update(cx, |state, cx| {
                    state.app.ui_state.active_settings_tab = tab;
                    state
                        .app
                        .set_screen(Screen::SettingsDetail, "PhoneSettingsRow");
                    cx.notify();
                });
                window.focus(&mouse_focus, cx);
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    state_for_key.update(cx, |state, cx| {
                        state.app.ui_state.active_settings_tab = tab;
                        state
                            .app
                            .set_screen(Screen::SettingsDetail, "PhoneSettingsRow");
                        cx.notify();
                    });
                    cx.stop_propagation();
                }
            });

        phone_dev_track!(element, format!("phone.settings.{tab:?}"))
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
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state_entity = self.state.clone();
        let footer = FooterTranslations::for_language(self.state.read(cx).app.ui_state.language);
        let label = match trigger {
            "PhoneMiniNext" | "PhoneNowNext" => footer.next_track,
            "PhoneNowPrev" => footer.previous_track,
            "PhoneNowRewind" => footer.seek_back_30s,
            "PhoneNowForward" => footer.seek_forward_30s,
            _ => trigger,
        };
        let element_id: ElementId = id.into();
        let focus_handle = interactive_focus_handle(&element_id, cx);
        let accessibility_props = AriaProps::with_role(AriaRole::Button);
        cx.register_accessible(AccessibilityNode {
            element_id: element_id.clone(),
            label: label.into(),
            props: accessibility_props.clone(),
        });

        let activate = std::rc::Rc::new(move |cx: &mut App| {
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
                    if let Err(e) = state.player.seek(new_position) {
                        log::error!("Failed to seek backward: {}", e);
                    }
                }
                "PhoneNowForward" => {
                    let max = state.app.playback.duration_secs;
                    let new_position = (state.app.playback.position_secs + 30.0).min(max);
                    state.app.playback.position_secs = new_position;
                    if let Err(e) = state.player.seek(new_position) {
                        log::error!("Failed to seek forward: {}", e);
                    }
                }
                _ => {}
            });
        });
        let mouse_activate = activate.clone();
        let key_activate = activate;

        let element = div()
            .id(element_id)
            .track_focus(&focus_handle)
            .track_focus_element(&focus_handle)
            .size(rems(2.75))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .cursor_pointer()
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.accent)
            })
            .hover({
                let theme = theme.clone();
                move |s| s.bg(theme.surface_hover)
            })
            .child(Icon::new(icon).size(IconSize::Md).color(theme.text_primary))
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                mouse_activate(cx);
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    key_activate(cx);
                    cx.stop_propagation();
                }
            });
        let element = gpui_ui_kit::accessibility::apply_native_accessibility(
            element,
            label,
            &accessibility_props,
        );
        phone_dev_track!(element, format!("phone.transport.{id}"))
    }

    fn render_phone_play_button(
        &self,
        id: &'static str,
        is_playing: bool,
        theme: &crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state_entity = self.state.clone();
        let footer = FooterTranslations::for_language(self.state.read(cx).app.ui_state.language);
        let label = if is_playing { footer.pause } else { footer.play };
        let icon = if is_playing {
            IconName::Pause
        } else {
            IconName::Play
        };
        let element_id: ElementId = id.into();
        let focus_handle = interactive_focus_handle(&element_id, cx);
        let accessibility_props =
            AriaProps::with_role(AriaRole::Button).state(AriaState::Pressed(is_playing));
        cx.register_accessible(AccessibilityNode {
            element_id: element_id.clone(),
            label: label.into(),
            props: accessibility_props.clone(),
        });

        let activate = std::rc::Rc::new(move |cx: &mut App| {
            state_entity.update(cx, |state, _cx| {
                if state.app.playback.current_queue_index.is_none() {
                    if let Some(source) = state.app.start_queue() {
                        PlayerView::play_track(state, source);
                    }
                } else if state.app.playback.is_playing {
                    if let Err(e) = state.player.pause() {
                        log::error!("Failed to pause: {}", e);
                    }
                    state.app.playback.is_playing = false;
                } else {
                    if let Err(e) = state.player.resume() {
                        log::error!("Failed to play: {}", e);
                    }
                    state.app.playback.is_playing = true;
                }
            });
        });
        let mouse_activate = activate.clone();
        let key_activate = activate;

        let element = div()
            .id(element_id)
            .track_focus(&focus_handle)
            .track_focus_element(&focus_handle)
            .size(rems(3.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(theme.accent)
            .cursor_pointer()
            .focus_visible({
                let theme = theme.clone();
                move |style| style.border_2().border_color(theme.text_on_accent)
            })
            .child(
                Icon::new(icon)
                    .size(IconSize::Md)
                    .color(theme.text_on_accent),
            )
            .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                mouse_activate(cx);
            })
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    key_activate(cx);
                    cx.stop_propagation();
                }
            });
        let element = gpui_ui_kit::accessibility::apply_native_accessibility(
            element,
            label,
            &accessibility_props,
        );
        phone_dev_track!(element, format!("phone.transport.{id}"))
    }

    fn format_phone_time(seconds: f64) -> String {
        let total = seconds.max(0.0) as u64;
        let minutes = total / 60;
        let seconds = total % 60;
        format!("{minutes}:{seconds:02}")
    }
}
