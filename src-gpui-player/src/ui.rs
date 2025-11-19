use crate::app::{App, AppState, Screen};
use gpui::*;
use std::sync::Arc;

// Actions for keyboard shortcuts
actions!(
    player_ui,
    [
        PlayPause,
        Stop,
        NextTrack,
        PrevTrack,
        VolumeUp,
        VolumeDown,
        SwitchToLibrary,
        SwitchToQueue,
        SwitchToPlugins,
        SwitchToDevices,
        ToggleSearch,
    ]
);

pub struct PlayerView {
    state: Model<AppState>,
    focus_handle: FocusHandle,
}

impl PlayerView {
    pub fn new(state: Model<AppState>, cx: &mut ViewContext<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        // Set up keyboard shortcuts
        cx.on_action(|view: &mut Self, _: &PlayPause, cx| {
            view.toggle_playback(cx);
        });

        cx.on_action(|view: &mut Self, _: &Stop, cx| {
            view.stop_playback(cx);
        });

        cx.on_action(|view: &mut Self, _: &NextTrack, cx| {
            view.next_track(cx);
        });

        cx.on_action(|view: &mut Self, _: &VolumeUp, cx| {
            view.adjust_volume(0.05, cx);
        });

        cx.on_action(|view: &mut Self, _: &VolumeDown, cx| {
            view.adjust_volume(-0.05, cx);
        });

        cx.on_action(|view: &mut Self, _: &SwitchToLibrary, cx| {
            view.switch_screen(Screen::Library, cx);
        });

        cx.on_action(|view: &mut Self, _: &SwitchToQueue, cx| {
            view.switch_screen(Screen::Queue, cx);
        });

        cx.on_action(|view: &mut Self, _: &SwitchToPlugins, cx| {
            view.switch_screen(Screen::Plugins, cx);
        });

        cx.on_action(|view: &mut Self, _: &SwitchToDevices, cx| {
            view.switch_screen(Screen::Devices, cx);
        });

        // Set up periodic update timer for playback position and loudness
        cx.spawn(|view, mut cx| async move {
            loop {
                smol::Timer::after(std::time::Duration::from_millis(100)).await;
                let _ = view.update(&mut cx, |view, cx| {
                    view.update_playback_state(cx);
                    cx.notify();
                });
            }
        })
        .detach();

        Self {
            state,
            focus_handle,
        }
    }

    fn toggle_playback(&mut self, cx: &mut ViewContext<Self>) {
        self.state.update(cx, |state, cx| {
            if state.app.is_playing {
                let _ = state.player.pause();
                state.app.is_playing = false;
            } else {
                let _ = state.player.resume();
                state.app.is_playing = true;
            }
        });
        cx.notify();
    }

    fn stop_playback(&mut self, cx: &mut ViewContext<Self>) {
        self.state.update(cx, |state, cx| {
            let _ = state.player.stop();
            state.app.is_playing = false;
            state.app.current_queue_index = None;
        });
        cx.notify();
    }

    fn next_track(&mut self, cx: &mut ViewContext<Self>) {
        self.state.update(cx, |state, cx| {
            if let Some(path) = state.app.next_track() {
                let sample_rate = 48000.0;
                let plugins = state.app.plugin_chain.to_plugin_configs(sample_rate);
                let output_channels = state.app.plugin_chain.output_channels();

                if let Err(e) = state.player.load_and_play(
                    path,
                    plugins,
                    output_channels,
                    state.app.current_output_device_name.clone(),
                ) {
                    log::error!("Failed to play next track: {}", e);
                    state.app.is_playing = false;
                }
            } else {
                state.app.is_playing = false;
            }
        });
        cx.notify();
    }

    fn adjust_volume(&mut self, delta: f32, cx: &mut ViewContext<Self>) {
        self.state.update(cx, |state, cx| {
            state.app.volume = (state.app.volume + delta).clamp(0.0, 1.0);
            let _ = state.player.set_volume(state.app.volume);
        });
        cx.notify();
    }

    fn switch_screen(&mut self, screen: Screen, cx: &mut ViewContext<Self>) {
        self.state.update(cx, |state, cx| {
            state.app.current_screen = screen;
        });
        cx.notify();
    }

    fn update_playback_state(&mut self, cx: &mut ViewContext<Self>) {
        self.state.update(cx, |state, cx| {
            let playback_state = state.player.get_playback_state(state.app.spectrum_visible);

            state.app.position_secs = playback_state.position_secs;
            state.app.loudness_info = playback_state.loudness;

            if state.app.spectrum_visible {
                state.app.spectrum_info = playback_state.spectrum;
            }

            // Check if playback ended and auto-advance
            if state.app.is_playing
                && !playback_state.is_playing
                && state.app.current_queue_index.is_some()
            {
                if let Some(path) = state.app.next_track() {
                    let sample_rate = 48000.0;
                    let plugins = state.app.plugin_chain.to_plugin_configs(sample_rate);
                    let output_channels = state.app.plugin_chain.output_channels();

                    if let Err(e) = state.player.load_and_play(
                        path,
                        plugins,
                        output_channels,
                        state.app.current_output_device_name.clone(),
                    ) {
                        log::error!("Failed to auto-advance: {}", e);
                        state.app.is_playing = false;
                    }
                } else {
                    state.app.is_playing = false;
                }
            }
        });
    }

    fn play_album_at_index(&mut self, index: usize, cx: &mut ViewContext<Self>) {
        self.state.update(cx, |state, cx| {
            // Add album to queue and start playing
            let albums = state.app.filtered_albums();
            if let Some(album) = albums.get(index).cloned() {
                state.app.queue.push(crate::app::QueueItem::new(album));
                state.app.expanded_queue_items.push(false);

                if let Some(path) = state.app.start_queue() {
                    let sample_rate = 48000.0;
                    let plugins = state.app.plugin_chain.to_plugin_configs(sample_rate);
                    let output_channels = state.app.plugin_chain.output_channels();

                    if let Err(e) = state.player.load_and_play(
                        path,
                        plugins,
                        output_channels,
                        state.app.current_output_device_name.clone(),
                    ) {
                        log::error!("Failed to play album: {}", e);
                        state.app.is_playing = false;
                    }
                }
            }
        });
        cx.notify();
    }
}

impl Render for PlayerView {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        div()
            .key_context("PlayerView")
            .track_focus(&self.focus_handle)
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x1e1e1e))
            .text_color(rgb(0xcccccc))
            .child(self.render_header(cx))
            .child(div().flex().flex_1().child(match state.app.current_screen {
                Screen::Library => self.render_library_screen(cx),
                Screen::Queue => self.render_queue_screen(cx),
                Screen::Plugins => self.render_plugins_screen(cx),
                Screen::Devices => self.render_devices_screen(cx),
                Screen::Spectrum => self.render_spectrum_screen(cx),
                Screen::DirectoryManager => self.render_directory_screen(cx),
            }))
            .child(self.render_footer(cx))
    }
}

impl PlayerView {
    fn render_header(&self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        div()
            .flex()
            .items_center()
            .justify_between()
            .p_4()
            .bg(rgb(0x2d2d2d))
            .border_b_1()
            .border_color(rgb(0x3e3e3e))
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::BOLD)
                    .child("SOTF Audio Player"),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.render_tab_button("Library", Screen::Library, cx))
                    .child(self.render_tab_button("Queue", Screen::Queue, cx))
                    .child(self.render_tab_button("Plugins", Screen::Plugins, cx))
                    .child(self.render_tab_button("Devices", Screen::Devices, cx)),
            )
    }

    fn render_tab_button(
        &self,
        label: &str,
        screen: Screen,
        cx: &mut ViewContext<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let is_active = state.app.current_screen == screen;

        let button = div()
            .px_4()
            .py_2()
            .rounded_md()
            .cursor_pointer()
            .child(label);

        if is_active {
            button.bg(rgb(0x007acc)).text_color(rgb(0xffffff))
        } else {
            button
                .bg(rgb(0x3e3e3e))
                .hover(|style| style.bg(rgb(0x505050)))
        }
    }

    fn render_library_screen(&self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let albums = state.app.filtered_albums();

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .mb_4()
                    .child(format!("Library ({} albums)", albums.len())),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .overflow_y_scroll()
                    .flex_1()
                    .children(albums.iter().enumerate().map(|(idx, album)| {
                        let album_clone = (*album).clone();
                        div()
                            .p_3()
                            .rounded_md()
                            .bg(rgb(0x2d2d2d))
                            .hover(|style| style.bg(rgb(0x3e3e3e)))
                            .cursor_pointer()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div().font_weight(FontWeight::SEMIBOLD).child(&album.title),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(0x999999))
                                            .child(&album.artist),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(0x666666))
                                            .child(format!("{} tracks", album.tracks.len())),
                                    ),
                            )
                    })),
            )
    }

    fn render_queue_screen(&self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .mb_4()
                    .child(format!("Queue ({} albums)", state.app.queue.len())),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .overflow_y_scroll()
                    .flex_1()
                    .children(state.app.queue.iter().enumerate().map(|(idx, item)| {
                        let is_current = state.app.current_queue_index == Some(idx);
                        div()
                            .p_3()
                            .rounded_md()
                            .when(is_current, |div| div.bg(rgb(0x007acc)))
                            .when(!is_current, |div| div.bg(rgb(0x2d2d2d)))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(&item.album.title),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(0x999999))
                                            .child(&item.album.artist),
                                    )
                                    .child(div().text_xs().text_color(rgb(0x666666)).child(
                                        format!(
                                            "Track {}/{}",
                                            item.current_track_index + 1,
                                            item.album.tracks.len()
                                        ),
                                    )),
                            )
                    })),
            )
    }

    fn render_plugins_screen(&self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .mb_4()
                    .child("Audio Plugins"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().text_sm().child("Plugin chain:"))
                    .child(div().text_sm().text_color(rgb(0x999999)).child(format!(
                        "{} plugins, {} output channels",
                        state.app.plugin_chain.plugins.len(),
                        state.app.plugin_chain.output_channels()
                    ))),
            )
    }

    fn render_devices_screen(&self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .mb_4()
                    .child("Audio Devices"),
            )
            .child(
                div().flex().flex_col().gap_2().children(
                    state
                        .app
                        .output_devices
                        .iter()
                        .enumerate()
                        .map(|(idx, device)| {
                            let is_selected = state.app.selected_output_device_index == idx;
                            div()
                                .p_3()
                                .rounded_md()
                                .when(is_selected, |div| div.bg(rgb(0x007acc)))
                                .when(!is_selected, |div| div.bg(rgb(0x2d2d2d)))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .child(
                                            div()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(&device.name),
                                        )
                                        .child(div().text_sm().text_color(rgb(0x999999)).child(
                                            format!(
                                                    "{} channels",
                                                    device
                                                        .default_config
                                                        .as_ref()
                                                        .map(|c| c.channels)
                                                        .unwrap_or(0)
                                                ),
                                        )),
                                )
                        }),
                ),
            )
    }

    fn render_spectrum_screen(&self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .mb_4()
                    .child("Spectrum Analyzer"),
            )
            .child(
                div()
                    .text_sm()
                    .child("Spectrum visualization coming soon..."),
            )
    }

    fn render_directory_screen(&self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .mb_4()
                    .child("Directory Manager"),
            )
            .child(div().flex().flex_col().gap_2().children(
                state.app.library.directories.iter().map(|dir| {
                    div()
                        .p_3()
                        .rounded_md()
                        .bg(rgb(0x2d2d2d))
                        .child(div().text_sm().child(dir.display().to_string()))
                }),
            ))
    }

    fn render_footer(&self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        div()
            .flex()
            .items_center()
            .justify_between()
            .p_4()
            .bg(rgb(0x2d2d2d))
            .border_t_1()
            .border_color(rgb(0x3e3e3e))
            .child(
                div()
                    .flex()
                    .gap_4()
                    .child(div().text_sm().child(if state.app.is_playing {
                        "Playing"
                    } else {
                        "Stopped"
                    }))
                    .child(
                        div()
                            .text_sm()
                            .child(format!("Volume: {:.0}%", state.app.volume * 100.0)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x999999))
                            .child("Space: Play/Pause"),
                    )
                    .child(div().text_xs().text_color(rgb(0x999999)).child("N: Next"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x999999))
                            .child("+/-: Volume"),
                    ),
            )
    }
}
