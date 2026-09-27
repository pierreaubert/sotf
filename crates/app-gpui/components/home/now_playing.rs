#[cfg(feature = "dev-api")]
use crate::app::dev_api::DevTrackExt;
use crate::app::types::{Screen, SettingsTab};
use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, IconButton, IconButtonSize, IconButtonVariant, Text,
};

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
    pub(crate) fn render_now_playing_screen(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let text = state.app.ui_state.translations.clone();
        let item = state
            .app
            .playback
            .current_queue_index
            .and_then(|index| state.app.queue_state.get(index));
        let track = item.and_then(|item| item.current_track());
        let labels =
            crate::app::i18n::NowPlayingTranslations::for_language(state.app.ui_state.language);
        let stream_seekable = track.and_then(|track| match track.source.as_ref() {
            Some(sotf_audio::decoder::AudioSource::Url { url, seekable, .. })
                if !*seekable
                    || state
                        .app
                        .stream_state
                        .store
                        .streams
                        .iter()
                        .any(|station| station.url == *url) =>
            {
                Some(*seekable)
            }
            _ => None,
        });
        let system_input =
            state.app.audio_device_state.playback_source != crate::app::types::PlaybackSource::File;
        let stream_seekable = if system_input { None } else { stream_seekable };
        let mut title = track
            .and_then(|track| track.title.clone())
            .unwrap_or_else(|| text.playback_no_track.into());
        let mut artist = track
            .and_then(|track| track.artist.clone())
            .unwrap_or_default();
        let mut album = item
            .map(|item| item.album.title.clone())
            .unwrap_or_default();
        let art = (!system_input && stream_seekable.is_none())
            .then(|| item.and_then(|item| item.album.album_art_path.clone()))
            .flatten();
        if system_input {
            title = crate::app::i18n::AudioDeviceTranslations::for_language(
                state.app.ui_state.language,
            )
            .hal_device
            .into();
            artist = state
                .app
                .audio_device_state
                .current_input_device_name
                .clone()
                .unwrap_or_default();
            album.clear();
        }
        if let Some(seekable) = stream_seekable {
            artist = if seekable { labels[4] } else { labels[3] }.into();
            album.clear();
        }
        let mut identity = div().flex().flex_wrap().items_start().gap(d.section);
        if let Some(art) = art {
            identity = identity.child(
                img(art)
                    .w(rems(14.0))
                    .h(rems(14.0))
                    .object_fit(ObjectFit::Cover),
            );
        }
        identity = identity.child(
            div()
                .flex()
                .flex_col()
                .min_w_0()
                .gap(d.gap)
                .child(Text::section_header(title))
                .child(Text::body(artist))
                .child(Text::caption(album)),
        );
        let mut body = div()
            .id("now-playing-screen")
            .size_full()
            .min_h_0()
            .overflow_y_scroll()
            .p(d.card)
            .child(identity);
        let mut actions = div().flex().flex_wrap().gap(d.gap).py(d.card);
        if !system_input
            && stream_seekable.is_none()
            && let Some(index) = state.app.playback.current_queue_index
        {
            let label =
                crate::app::i18n::DesktopTranslations::for_language(state.app.ui_state.language)
                    .view_album;
            let button = Button::new("now-view-album", label)
                .size(ButtonSize::Sm)
                .variant(ButtonVariant::Secondary)
                .theme(theme.to_button_theme())
                .on_click_event(cx.listener(move |view, _, window, cx| {
                    let album = view
                        .state
                        .read(cx)
                        .app
                        .queue_state
                        .get(index)
                        .map(|item| std::sync::Arc::new(item.album.clone()));
                    if let Some(album) = album {
                        view.open_album_detail(album, window, cx);
                    }
                }));
            #[cfg(feature = "dev-api")]
            let button = button.dev_track("now-playing.view-album");
            actions = actions.child(button);
        }
        if stream_seekable.is_some() {
            actions = actions.child(tracked!(
                Button::new("now-view-station", labels[2])
                    .size(ButtonSize::Sm)
                    .variant(ButtonVariant::Secondary)
                    .theme(theme.to_button_theme())
                    .on_click_event(cx.listener(|view, _, window, cx| {
                        view.state.update(cx, |state, _| {
                            state.app.set_screen(Screen::Streams, "NowPlayingStation")
                        });
                        view.focus_handle.focus(window, cx);
                        cx.notify();
                    })),
                "now-playing.view-station"
            ));
        }
        for (id, label, screen) in [
            ("now-library", text.screen_library, Screen::Library),
            ("now-queue", text.queue_title, Screen::Queue),
            (
                "now-spectrum",
                crate::app::i18n::SpectrumTranslations::for_language(state.app.ui_state.language)
                    .analyzer,
                Screen::Spectrum,
            ),
            ("now-processing", labels[5], Screen::Studio),
        ] {
            let entity = self.state.clone();
            actions = actions.child(
                Button::new(id, label)
                    .size(ButtonSize::Sm)
                    .variant(ButtonVariant::Secondary)
                    .theme(theme.to_button_theme())
                    .on_click_event(move |_, _, cx| {
                        entity.update(cx, |state, cx| {
                            state.app.set_screen(screen, "NowPlaying");
                            cx.notify();
                        });
                    }),
            );
        }
        body = body.child(actions);
        body = body.child(self.render_now_playing_signal_path(cx));
        let details_open = state.app.playback.track_information_open;
        let details_state = self.state.clone();
        let details_view = cx.entity().clone();
        body = body.child(
            div()
                .flex()
                .items_center()
                .gap(d.gap)
                .child(tracked!(
                    IconButton::with_child(
                        "now-track-information",
                        Icon::new(if details_open {
                            IconName::ChevronDown
                        } else {
                            IconName::ChevronRight
                        })
                        .small()
                        .color(theme.text_primary),
                    )
                    .size(IconButtonSize::Sm)
                    .variant(IconButtonVariant::Ghost)
                    .selected(details_open)
                    .theme(theme.to_icon_button_theme())
                    .aria_label(labels[1].to_string())
                    .on_click_event(move |_, _, cx| {
                        details_state.update(cx, |state, _| {
                            state.app.playback.track_information_open =
                                !state.app.playback.track_information_open;
                        });
                        details_view.update(cx, |_view, cx| cx.notify());
                    }),
                    "now-playing.details-toggle"
                ))
                .child(Text::body(labels[1].to_string())),
        );
        if details_open {
            let mut details = div().flex().flex_col().gap(d.gap).py(d.pad_y);
            if let Some(track) = track.filter(|_| !system_input) {
                if let Some(item) = item {
                    details = details.child(Text::body(item.album.title.clone()));
                    if let Some(year) = item.album.year {
                        details = details.child(Text::caption(year.to_string()));
                    }
                }
                if let Some(duration) = track.duration_secs {
                    details = details.child(Text::caption(format!(
                        "{}:{:02}",
                        duration / 60,
                        duration % 60
                    )));
                }
                if let Some(channels) = track.channels {
                    details = details.child(Text::caption(format!("{}: {channels}", labels[10])));
                }
                if let Some(bits) = track.bit_depth {
                    details = details.child(Text::caption(format!("{}: {bits}", labels[11])));
                }
                if let Some(rate) = track.sample_rate {
                    details = details.child(Text::caption(format!("{rate} Hz")));
                }
            } else {
                details = details.child(Text::body(labels[6]));
            }
            body = body.child(tracked!(details, "now-playing.track-information"));
        }
        let entity = self.state.clone();
        body.child(
            div().flex().child(
                Button::new("now-output", text.devices_title)
                    .size(ButtonSize::Sm)
                    .variant(ButtonVariant::Ghost)
                    .theme(theme.to_button_theme())
                    .on_click_event(move |_, _, cx| {
                        entity.update(cx, |state, cx| {
                            state.app.ui_state.active_settings_tab = SettingsTab::AudioDevice;
                            state.app.set_screen(Screen::Settings, "NowPlayingOutput");
                            cx.notify();
                        });
                    }),
            ),
        )
        .into_any_element()
    }
}

impl PlayerView {
    fn render_now_playing_signal_path(&self, cx: &Context<Self>) -> AnyElement {
        let state = self.state.read(cx);
        let d = Ds::from_cx(cx);
        let theme = state.app.ui_state.theme.clone();
        let labels =
            crate::app::i18n::NowPlayingTranslations::for_language(state.app.ui_state.language);
        let signal_path_open = state.app.playback.signal_path_open;
        let path_state = self.state.clone();
        let path_view = cx.entity().clone();
        let panel = div().flex().flex_col().gap(d.gap).py(d.card).child(
            div()
                .flex()
                .items_center()
                .gap(d.gap)
                .child(tracked!(
                    IconButton::with_child(
                        "now-signal-path",
                        Icon::new(if signal_path_open {
                            IconName::ChevronDown
                        } else {
                            IconName::ChevronRight
                        })
                        .small()
                        .color(theme.text_primary),
                    )
                    .size(IconButtonSize::Sm)
                    .variant(IconButtonVariant::Ghost)
                    .selected(signal_path_open)
                    .theme(theme.to_icon_button_theme())
                    .aria_label(labels[0].to_string())
                    .on_click_event(move |_, _, cx| {
                        path_state.update(cx, |state, _| {
                            state.app.playback.signal_path_open =
                                !state.app.playback.signal_path_open;
                        });
                        path_view.update(cx, |_view, cx| cx.notify());
                    }),
                    "now-playing.signal-path-toggle"
                ))
                .child(Text::body(labels[0].to_string())),
        );
        if !signal_path_open {
            return panel.into_any_element();
        }
        let mut details = div().flex().flex_col().gap(d.gap);
        if let Some(path) = state.app.playback.signal_path.as_ref() {
            let source = path
                .source
                .as_ref()
                .map(|source| {
                    format!(
                        "{} · {} Hz · {}: {} · {}: {}",
                        source.format,
                        source.sample_rate_hz,
                        labels[10],
                        source.channels,
                        labels[11],
                        source.bits_per_sample
                    )
                })
                .unwrap_or_else(|| labels[6].into());
            details = details.child(Text::body(format!("{}: {source}", labels[12])));
            if let Some(resampling) = &path.processing.resampling {
                details = details.child(Text::caption(format!(
                    "{}: {} → {} Hz",
                    labels[8], resampling.from_hz, resampling.to_hz
                )));
            }
            if path.processing.bypassed {
                details = details.child(Text::body(labels[7]));
            } else if !path.plugin_chain.is_empty() {
                details = details.child(Text::body(
                    path.plugin_chain
                        .iter()
                        .map(|plugin| {
                            sotf_audio_player::PluginType::from_wire_name(&plugin.plugin_type)
                                .map(|kind| kind.name())
                                .unwrap_or(plugin.plugin_type.as_str())
                        })
                        .collect::<Vec<_>>()
                        .join(" → "),
                ));
            }
            let output_format = if path.output.sample_rate_hz > 0 && path.output.channels > 0 {
                format!(
                    "{} Hz · {}: {}",
                    path.output.sample_rate_hz, labels[10], path.output.channels
                )
            } else {
                labels[6].to_string()
            };
            details = details
                .child(Text::caption(format!(
                    "{}: {}",
                    labels[9], path.processing.latency_samples
                )))
                .child(Text::body(format!(
                    "{}: {} · {}",
                    labels[13],
                    path.output.device.as_deref().unwrap_or(labels[6]),
                    output_format
                )));
        } else {
            details = details.child(Text::body(labels[6]));
        }
        panel
            .child(tracked!(details, "now-playing.signal-path"))
            .into_any_element()
    }
}
