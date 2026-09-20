use crate::app::actions::{AddDirectory, PlayPause};
use crate::app::i18n::{DesktopTranslations, HomeEntryTranslations};
use crate::app::{Screen, SettingsTab};
use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant, Text};

#[cfg(feature = "dev-api")]
use crate::app::dev_api::DevTrackExt;

macro_rules! tracked {
    ($element:expr, $id:expr) => {{
        #[cfg(feature = "dev-api")]
        {
            $element.dev_track($id)
        }
        #[cfg(not(feature = "dev-api"))]
        {
            $element
        }
    }};
}

impl PlayerView {
    pub(super) fn render_continue_listening(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let state = self.state.read(cx);
        if state.app.audio_device_state.playback_source != crate::app::types::PlaybackSource::File {
            return None;
        }
        let index = state.app.playback.current_queue_index?;
        let item = state.app.queue_state.get(index)?;
        let track = item.current_track()?;
        let d = Ds::from_cx(cx);
        let theme = state.app.ui_state.theme.clone();
        let labels = HomeEntryTranslations::for_language(state.app.ui_state.language);
        let desktop = DesktopTranslations::for_language(state.app.ui_state.language);
        let mut content = div().flex().flex_wrap().items_center().gap(d.section);
        if let Some(art) = item.album.album_art_path.clone() {
            content = content.child(
                img(art)
                    .w(rems(10.0))
                    .h(rems(10.0))
                    .object_fit(ObjectFit::Cover),
            );
        }
        let title = track
            .title
            .clone()
            .unwrap_or_else(|| item.album.title.clone());
        let artist = track
            .artist
            .clone()
            .unwrap_or_else(|| item.album.artist().to_string());
        let is_playing = state.app.playback.is_playing;
        content =
            content.child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .gap(d.gap)
                    .child(Text::caption(labels[0]))
                    .child(Text::section_header(title))
                    .child(Text::body(format!("{artist} · {}", item.album.title)))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(d.gap)
                            .child(tracked!(
                                Button::new(
                                    "home-resume",
                                    if is_playing { labels[4] } else { labels[1] }
                                )
                                .size(ButtonSize::Sm)
                                .variant(ButtonVariant::Primary)
                                .theme(theme.to_button_theme())
                                .on_click_event(cx.listener(|view, _, window, cx| {
                                    // Recheck current state so a stale button cannot pause playback.
                                    if !view.state.read(cx).app.playback.is_playing {
                                        view.toggle_playback(&PlayPause, window, cx);
                                    }
                                    view.state.update(cx, |state, _| {
                                        state.app.set_screen(Screen::NowPlaying, "HomeResume")
                                    });
                                    view.focus_handle.focus(window, cx);
                                    cx.notify();
                                })),
                                "home.resume"
                            ))
                            .child(tracked!(
                                Button::new("home-view-album", desktop.view_album)
                                    .size(ButtonSize::Sm)
                                    .variant(ButtonVariant::Secondary)
                                    .theme(theme.to_button_theme())
                                    .on_click_event(cx.listener(move |view, _, window, cx| {
                                        let album =
                                            view.state.read(cx).app.queue_state.get(index).map(
                                                |item| std::sync::Arc::new(item.album.clone()),
                                            );
                                        if let Some(album) = album {
                                            view.open_album_detail(album, window, cx);
                                        }
                                    })),
                                "home.view-album"
                            )),
                    ),
            );
        Some(content.into_any_element())
    }

    pub(crate) fn render_empty_library_actions(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = self.state.read(cx);
        let d = Ds::from_cx(cx);
        let theme = state.app.ui_state.theme.clone();
        let labels = HomeEntryTranslations::for_language(state.app.ui_state.language);
        let mut actions = div().flex().flex_wrap().gap(d.gap);
        #[cfg(not(target_os = "tvos"))]
        {
            actions = actions.child(tracked!(
                Button::new("home-add-folder", labels[2])
                    .size(ButtonSize::Sm)
                    .variant(ButtonVariant::Primary)
                    .theme(theme.to_button_theme())
                    .on_click_event(cx.listener(|view, _, window, cx| {
                        view.focus_handle.focus(window, cx);
                        window.dispatch_action(Box::new(AddDirectory), cx);
                    })),
                "library.empty.add-folder"
            ));
        }
        actions
            .child(tracked!(
                Button::new("home-connect-library", labels[3])
                    .size(ButtonSize::Sm)
                    .variant(ButtonVariant::Secondary)
                    .theme(theme.to_button_theme())
                    .on_click_event(cx.listener(|view, _, window, cx| {
                        view.state.update(cx, |state, _| {
                            state.app.ui_state.active_settings_tab = SettingsTab::Servers;
                            state.app.set_screen(Screen::Settings, "ConnectLibrary");
                        });
                        view.focus_handle.focus(window, cx);
                        cx.notify();
                    })),
                "library.empty.connect"
            ))
            .into_any_element()
    }
}
