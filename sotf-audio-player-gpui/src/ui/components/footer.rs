//! Footer component rendering with transport controls, track info, and volume

use crate::ui::components::potentiometer::render_potentiometer;
use crate::ui::PlayerView;
use gpui_ui_kit::{HStack, VStack, StackSpacing, StackAlign, StackJustify};
use gpui::prelude::*;
use gpui::*;

impl PlayerView {
    pub(crate) fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = &state.app.theme;

        let bg_surface = theme.surface;
        let border_color = theme.border;

        VStack::new()
            .spacing(StackSpacing::None)
            .child(
                // Main footer content: three-section layout
                HStack::new()
                    .spacing(StackSpacing::None)
                    .justify(StackJustify::SpaceBetween)
                    .align(StackAlign::Center)
                    // Left section: Track info
                    .child(self.render_footer_left(cx))
                    // Center section: Transport + waveform
                    .child(self.render_footer_center(cx))
                    // Right section: Device + Volume
                    .child(self.render_footer_right(cx))
                    .build()
                    .h(px(80.0))
                    .px_4()
            )
            .build()
            .bg(bg_surface)
            .border_t_1()
            .border_color(border_color)
    }

    /// Left section: Album artwork + track info
    fn render_footer_left(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = &state.app.theme;

        // Get current track info and album art from queue
        let (title, album_name, artist, album_art_path) = if let Some(queue_idx) = state.app.current_queue_index {
            if let Some(item) = state.app.queue.get(queue_idx) {
                let track_title = item
                    .current_track()
                    .and_then(|t| t.title.clone())
                    .unwrap_or_else(|| "Unknown Track".to_string());
                (track_title, item.album.title.clone(), item.album.artist(), item.album.album_art_path.clone())
            } else {
                (String::new(), String::new(), String::new(), None)
            }
        } else {
            (String::new(), String::new(), String::new(), None)
        };

        let text_primary = theme.text_primary;
        let text_secondary = theme.text_secondary;
        let text_muted = theme.text_muted;
        let surface_hover = theme.surface_hover;

        HStack::new()
            .spacing(StackSpacing::Md)
            .align(StackAlign::Center)
            // Album artwork (64x64)
            .child({
                let art_div = div()
                    .w(px(64.0))
                    .h(px(64.0))
                    .rounded_md()
                    .bg(surface_hover)
                    .overflow_hidden();

                if let Some(art_path) = album_art_path {
                    art_div.child(
                        img(art_path)
                            .w_full()
                            .h_full()
                            .object_fit(gpui::ObjectFit::Cover)
                    )
                } else {
                    art_div
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(text_muted)
                        .text_xl()
                        .child("♪")
                }
            })
            // Track info
            .child(
                VStack::new()
                    .spacing(StackSpacing::Xs)
                    .align(StackAlign::Start)
                    // Title (11px equivalent)
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(text_primary)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(if title.is_empty() {
                                "No track playing".to_string()
                            } else {
                                title
                            }),
                    )
                    // Album (9px equivalent)
                    .child(
                        div()
                            .text_xs()
                            .text_color(text_secondary)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(album_name),
                    )
                    // Artist (9px equivalent)
                    .child(
                        div()
                            .text_xs()
                            .text_color(text_muted)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(artist),
                    )
            )
            .build()
            .min_w(px(250.0))
            .max_w(px(350.0))
    }

    /// Center section: Transport controls + waveform + time
    fn render_footer_center(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = &state.app.theme;

        let position_secs = state.app.position_secs;
        let duration_secs = state.app.duration_secs;
        let is_playing = state.app.is_playing;

        // Format time as MM:SS
        let format_time = |secs: f64| -> String {
            let mins = (secs / 60.0) as u32;
            let s = (secs % 60.0) as u32;
            format!("{:02}:{:02}", mins, s)
        };

        let position_str = format_time(position_secs);
        let duration_str = format_time(duration_secs);

        // Calculate progress for waveform
        let progress = if duration_secs > 0.0 {
            (position_secs / duration_secs).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };

        let text_muted = theme.text_muted;
        let accent = theme.accent;
        let surface_hover: gpui::Hsla = theme.surface_hover.into();
        let progress_bar_bg = theme.progress_bar_bg;
        let progress_bar_fill = theme.progress_bar_fill;

        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_2()
            .flex_1()
            .max_w(px(600.0))
            // Transport controls row
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    // Previous track
                    .child(self.render_transport_button("⏮", "prev", surface_hover.clone(), cx))
                    // Seek backward
                    .child(self.render_transport_button("⏪", "seek-back", surface_hover.clone(), cx))
                    // Play/Stop (large)
                    .child(
                        div()
                            .id("transport-play")
                            .w(px(48.0))
                            .h(px(48.0))
                            .rounded_full()
                            .bg(accent)
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_xl()
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _: &MouseUpEvent, window, cx| {
                                    view.toggle_playback(&crate::actions::PlayPause, window, cx);
                                }),
                            )
                            .child(if is_playing { "⏹" } else { "▶" }),
                    )
                    // Seek forward
                    .child(self.render_transport_button("⏩", "seek-fwd", surface_hover.clone(), cx))
                    // Next track
                    .child(self.render_transport_button("⏭", "next", surface_hover, cx)),
            )
            // Waveform/progress row
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .w_full()
                    // Current position
                    .child(
                        div()
                            .text_xs()
                            .text_color(text_muted)
                            .min_w(px(40.0))
                            .child(position_str),
                    )
                    // Progress bar (waveform placeholder)
                    .child(
                        div()
                            .id("progress-bar")
                            .flex_1()
                            .h(px(4.0))
                            .bg(progress_bar_bg)
                            .rounded_full()
                            .overflow_hidden()
                            .cursor_pointer()
                            .child(
                                div()
                                    .h_full()
                                    .w(gpui::Length::Definite(gpui::DefiniteLength::Fraction(
                                        progress,
                                    )))
                                    .bg(progress_bar_fill)
                                    .rounded_full(),
                            ),
                    )
                    // Total duration
                    .child(
                        div()
                            .text_xs()
                            .text_color(text_muted)
                            .min_w(px(40.0))
                            .text_right()
                            .child(duration_str),
                    ),
            )
    }

    /// Right section: Device selection + Volume
    fn render_footer_right(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (volume, muted, show_device_popup, current_device, text_secondary, surface_hover, theme_clone) = {
            let state = self.state.read(cx);
            let theme = &state.app.theme;
            (
                state.app.volume,
                state.app.muted,
                state.app.show_device_popup,
                state.app.current_output_device_name.clone().unwrap_or_else(|| "Default".to_string()),
                theme.text_secondary,
                theme.surface_hover,
                theme.clone(),
            )
        };

        div()
            .flex()
            .items_center()
            .gap_3()
            .min_w(px(180.0))
            .justify_end()
            .relative()
            // Device selection button
            .child(
                div()
                    .id("device-selector")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .text_xs()
                    .text_color(text_secondary)
                    .cursor_pointer()
                    .hover(|style| style.bg(surface_hover))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                            view.state.update(cx, |state, _cx| {
                                state.app.show_device_popup = !state.app.show_device_popup;
                            });
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child("🔊")
                            .child(
                                div()
                                    .max_w(px(80.0))
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .whitespace_nowrap()
                                    .child(current_device),
                            ),
                    ),
            )
            // Device popup (renders above the button)
            .when(show_device_popup, |el| {
                el.child(self.render_device_popup(cx))
            })
            // Round volume button
            .child(self.render_volume_button(volume, muted, theme_clone, cx))
    }

    /// Render the device selection popup
    fn render_device_popup(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (devices, selected_index) = {
            let state = self.state.read(cx);
            (state.app.output_devices.clone(), state.app.selected_output_device_index)
        };

        div()
            .id("device-popup")
            .absolute()
            .bottom(px(40.0))
            .right_0()
            .w(px(250.0))
            .max_h(px(300.0))
            .bg(rgb(0x2a2a2a))
            .border_1()
            .border_color(rgb(0x444444))
            .rounded(px(4.0))
            .shadow_lg()
            .py_1()
            .overflow_y_scroll()
            // Header
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0x888888))
                    .border_b_1()
                    .border_color(rgb(0x3a3a3a))
                    .child("Output Devices"),
            )
            // Device list
            .children(devices.iter().enumerate().map(|(idx, device)| {
                let is_selected = idx == selected_index;
                let device_name = device.name.clone();
                let display_name = if device_name.len() > 30 {
                    format!("{}...", &device_name[..27])
                } else {
                    device_name.clone()
                };

                div()
                    .id(SharedString::from(format!("device-{}", idx)))
                    .px_3()
                    .py(px(6.0))
                    .mx_1()
                    .my(px(1.0))
                    .rounded(px(3.0))
                    .cursor_pointer()
                    .text_sm()
                    .when(is_selected, |d| {
                        d.bg(rgb(0x3a3a3a))
                            .text_color(rgb(0xffffff))
                            .font_weight(FontWeight::MEDIUM)
                    })
                    .when(!is_selected, |d| {
                        d.text_color(rgb(0xcccccc))
                            .hover(|style| style.bg(rgb(0x333333)).text_color(rgb(0xffffff)))
                    })
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                            view.state.update(cx, |state, _cx| {
                                state.app.selected_output_device_index = idx;
                                state.app.current_output_device_name = Some(device_name.clone());
                                state.app.show_device_popup = false;
                            });
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .when(is_selected, |d| d.child("✓"))
                            .child(display_name),
                    )
            }))
    }

    /// Render the device popup overlay (click outside to close)
    pub(crate) fn render_device_popup_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let show_popup = self.state.read(cx).app.show_device_popup;

        div()
            .absolute()
            .inset_0()
            .when(show_popup, |el| {
                el.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|view, _: &MouseDownEvent, _window, cx| {
                        view.state.update(cx, |state, _cx| {
                            state.app.show_device_popup = false;
                        });
                        cx.notify();
                    }),
                )
            })
    }

    /// Render a transport button
    fn render_transport_button(
        &self,
        icon: &'static str,
        id: &'static str,
        surface_hover: gpui::Hsla,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(SharedString::from(format!("transport-{}", id)))
            .w(px(32.0))
            .h(px(32.0))
            .rounded_full()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|style| style.bg(surface_hover))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |view, _: &MouseUpEvent, window, cx| {
                    match id {
                        "prev" => view.prev_track(&crate::actions::PrevTrack, window, cx),
                        "next" => view.next_track(&crate::actions::NextTrack, window, cx),
                        "seek-back" => {
                            // Seek backward 10 seconds
                            view.state.update(cx, |state, _cx| {
                                state.app.position_secs = (state.app.position_secs - 10.0).max(0.0);
                            });
                        }
                        "seek-fwd" => {
                            // Seek forward 10 seconds
                            view.state.update(cx, |state, _cx| {
                                let max = state.app.duration_secs;
                                state.app.position_secs = (state.app.position_secs + 10.0).min(max);
                            });
                        }
                        _ => {}
                    }
                    cx.notify();
                }),
            )
            .child(icon)
    }

    /// Render a round volume button with circular progress indicator
    fn render_volume_button(
        &self,
        volume: f32,
        muted: bool,
        theme: crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let volume_percent = (volume * 100.0) as u32;

        let accent_color: gpui::Hsla = theme.accent.into();
        let muted_color: gpui::Hsla = theme.text_muted.into();
        let bg_color: gpui::Hsla = theme.surface_hover.into();
        let text_color: gpui::Hsla = theme.text_primary.into();

        div()
            .id("volume-button")
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                    // Toggle mute on click
                    view.state.update(cx, |state, _cx| {
                        state.app.muted = !state.app.muted;
                    });
                    cx.notify();
                }),
            )
            .child(render_potentiometer(
                volume,
                format!("{}", volume_percent),
                48.0,
                muted,
                accent_color,
                muted_color,
                bg_color,
                text_color,
            ))
    }
}
