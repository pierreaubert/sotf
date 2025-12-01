//! Footer component rendering with transport controls, track info, and volume

use crate::ui::PlayerView;
use crate::ui::components::icon::{Icon, IconName, IconSize};
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    HStack, Potentiometer, StackAlign, StackJustify,
    StackSpacing, VStack,
};

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
                    .h(px(100.0))
                    .px_4(),
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
        let (title, album_name, artist, album_art_path) =
            if let Some(queue_idx) = state.app.current_queue_index {
                if let Some(item) = state.app.queue.get(queue_idx) {
                    let track_title = item
                        .current_track()
                        .and_then(|t| t.title.clone())
                        .unwrap_or_else(|| "Unknown Track".to_string());
                    (
                        track_title,
                        item.album.title.clone(),
                        item.album.artist(),
                        item.album.album_art_path.clone(),
                    )
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
                            .object_fit(gpui::ObjectFit::Cover),
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
                    ),
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

        // Get waveform data
        let waveform = if let Some(queue_idx) = state.app.current_queue_index {
            if let Some(item) = state.app.queue.get(queue_idx) {
                item.current_track().and_then(|t| t.waveform.clone())
            } else {
                None
            }
        } else {
            None
        };

        let text_muted = theme.text_muted;
        let progress_bar_bg = theme.progress_bar_bg;
        let progress_bar_fill = theme.progress_bar_fill;

        let theme_clone = {
            let state = self.state.read(cx);
            state.app.theme.clone()
        };

        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_0() // No gap - we use explicit margins
            .py_0() // Padding above/below
            .flex_1()
            .max_w(px(600.0))
            // Transport controls row (moved down 20px)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    // Previous track
                    .child(
                        div()
                            .id("transport-prev")
                            .p_2()
                            .rounded_full()
                            .cursor_pointer()
                            .hover(|s| s.bg(theme_clone.surface_hover))
                            .child(Icon::new(IconName::SkipBack).size(IconSize::Lg).color(theme_clone.text_primary))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _: &MouseUpEvent, window, cx| {
                                    view.prev_track(&crate::actions::PrevTrack, window, cx);
                                }),
                            ),
                    )
                    // Seek backward
                    .child(
                        div()
                            .id("transport-seek-back")
                            .p_2()
                            .rounded_full()
                            .cursor_pointer()
                            .hover(|s| s.bg(theme_clone.surface_hover))
                            .child(Icon::new(IconName::Rewind).size(IconSize::Lg).color(theme_clone.text_primary))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                    view.state.update(cx, |state, _cx| {
                                        state.app.position_secs =
                                            (state.app.position_secs - 10.0).max(0.0);
                                    });
                                    cx.notify();
                                }),
                            ),
                    )
                    // Play/Pause (large, accent background)
                    .child({
                        let play_icon = if is_playing { IconName::Pause } else { IconName::Play };
                        div()
                            .id("transport-play")
                            .p_3()
                            .rounded_full()
                            .cursor_pointer()
                            .bg(theme_clone.accent)
                            .hover(|s| s.bg(theme_clone.accent_hover))
                            .child(Icon::new(play_icon).size(IconSize::Lg).color(theme_clone.text_on_accent))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _: &MouseUpEvent, window, cx| {
                                    view.toggle_playback(&crate::actions::PlayPause, window, cx);
                                }),
                            )
                    })
                    // Seek forward
                    .child(
                        div()
                            .id("transport-seek-fwd")
                            .p_2()
                            .rounded_full()
                            .cursor_pointer()
                            .hover(|s| s.bg(theme_clone.surface_hover))
                            .child(Icon::new(IconName::FastForward).size(IconSize::Lg).color(theme_clone.text_primary))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                    view.state.update(cx, |state, _cx| {
                                        let max = state.app.duration_secs;
                                        state.app.position_secs =
                                            (state.app.position_secs + 10.0).min(max);
                                    });
                                    cx.notify();
                                }),
                            ),
                    )
                    // Next track
                    .child(
                        div()
                            .id("transport-next")
                            .p_2()
                            .rounded_full()
                            .cursor_pointer()
                            .hover(|s| s.bg(theme_clone.surface_hover))
                            .child(Icon::new(IconName::SkipForward).size(IconSize::Lg).color(theme_clone.text_primary))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _: &MouseUpEvent, window, cx| {
                                    view.next_track(&crate::actions::NextTrack, window, cx);
                                }),
                            ),
                    ),
            )
            // Waveform/progress row (moved up 20px relative to transport)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_0()
                    .w_full()
                    .mt(px(2.0)) // Move waveform up (closer to transport)
                    // Current position
                    .child(
                        div()
                            .text_xs()
                            .text_color(text_muted)
                            .min_w(px(40.0))
                            .child(position_str),
                    )
                    // Waveform visualization from track data
                    .child(
                        div()
                            .id("waveform-bar")
                            .flex_1()
                            .h(px(36.0)) // Taller waveform
                            .cursor_pointer()
                            .flex()
                            .items_center() // Center bars vertically for mirrored look
                            .children(
                                Self::render_waveform_bars(
                                    waveform.as_ref(),
                                    progress,
                                    progress_bar_fill,
                                    progress_bar_bg,
                                )
                            )
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
        let (
            volume,
            muted,
            show_device_popup,
            current_device,
            text_secondary,
            surface_hover,
            theme_clone,
        ) = {
            let state = self.state.read(cx);
            let theme = &state.app.theme;
            (
                state.app.volume,
                state.app.muted,
                state.app.show_device_popup,
                state
                    .app
                    .current_output_device_name
                    .clone()
                    .unwrap_or_else(|| "Default".to_string()),
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
            // Device selection button - icon on top, name below
            .child(
                div()
                    .id("device-selector")
                    .px_2()
                    .py_1()
                    .rounded_md()
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
                            .flex_col()
                            .items_center()
                            .gap_1()
                            // Speaker icon
                            .child(Icon::new(IconName::Speaker).size(IconSize::Lg).color(theme_clone.text_secondary))
                            // Device name below
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(text_secondary)
                                    .max_w(px(80.0))
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .whitespace_nowrap()
                                    .text_center()
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
        let (devices, selected_index, theme) = {
            let state = self.state.read(cx);
            (
                state.app.output_devices.clone(),
                state.app.selected_output_device_index,
                state.app.theme.clone(),
            )
        };

        div()
            .id("device-popup")
            .absolute()
            .bottom(px(40.0))
            .right_0()
            .w(px(250.0))
            .max_h(px(300.0))
            .bg(theme.surface)
            .border_1()
            .border_color(theme.border)
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
                    .text_color(theme.text_muted)
                    .border_b_1()
                    .border_color(theme.border)
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
                let theme = theme.clone();

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
                        d.bg(theme.surface_hover)
                            .text_color(theme.text_primary)
                            .font_weight(FontWeight::MEDIUM)
                    })
                    .when(!is_selected, |d| {
                        d.text_color(theme.text_secondary)
                            .hover(|style| style.bg(theme.surface_hover).text_color(theme.text_primary))
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

        div().absolute().inset_0().when(show_popup, |el| {
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

    /// Render waveform bars for the progress visualization
    /// Each bar represents one sample from the 128-sample waveform data
    /// Creates a mirrored waveform where bars extend up and down from center
    fn render_waveform_bars(
        waveform: Option<&Vec<u8>>,
        progress: f32,
        played_color: gpui::Rgba,
        unplayed_color: gpui::Rgba,
    ) -> Vec<gpui::Div> {
        const NUM_BARS: usize = 128;
        const MAX_HEIGHT: f32 = 16.0; // Half of total height (bars go up AND down)
        const MIN_HEIGHT: f32 = 2.0;
        const BAR_WIDTH: f32 = 4.0;
        const GAP: f32 = 0.0;

        // If no waveform data, create flat bars
        let default_waveform: Vec<u8> = vec![64; NUM_BARS];
        let samples = waveform.unwrap_or(&default_waveform);

        // Normalize to NUM_BARS samples if different length
        let bar_samples: Vec<u8> = if samples.len() == NUM_BARS {
            samples.clone()
        } else if samples.is_empty() {
            vec![64; NUM_BARS]
        } else {
            // Resample to NUM_BARS
            (0..NUM_BARS)
                .map(|i| {
                    let src_idx = (i * samples.len()) / NUM_BARS;
                    samples.get(src_idx).copied().unwrap_or(64)
                })
                .collect()
        };

        // Progress threshold for coloring (0.0 to 1.0 maps to bar index)
        let progress_bar_idx = (progress * NUM_BARS as f32) as usize;

        bar_samples
            .into_iter()
            .enumerate()
            .map(|(idx, amplitude)| {
                // Calculate height from amplitude (0-255 -> MIN_HEIGHT to MAX_HEIGHT)
                let height_ratio = amplitude as f32 / 255.0;
                let bar_height = MIN_HEIGHT + (MAX_HEIGHT - MIN_HEIGHT) * height_ratio;

                // Color based on whether we've played past this bar
                let bar_color = if idx < progress_bar_idx {
                    played_color
                } else {
                    unplayed_color
                };

                // Each bar is a column with top half and bottom half (mirrored)
                div()
                    .w(px(BAR_WIDTH))
                    .mr(px(GAP))
                    .h_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .child(
                        // Single bar that represents both halves visually
                        div()
                            .w(px(BAR_WIDTH))
                            .h(px(bar_height * 2.0)) // Total height (up + down from center)
                            .bg(bar_color)
                            .rounded_sm(),
                    )
            })
            .collect()
    }

    /// Render a round volume button with circular progress indicator
    /// Supports mouse scroll to change volume
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
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, event: &MouseDownEvent, _window, cx| {
                    // Start volume drag
                    view.state.update(cx, |state, _cx| {
                        state.app.is_dragging_volume = true;
                        state.app.volume_drag_start_y = Some(event.position.y.into());
                        state.app.volume_drag_start_value = state.app.volume;
                    });
                }),
            )
            .on_scroll_wheel(cx.listener(|view, event: &ScrollWheelEvent, _window, cx| {
                // Scroll up = increase volume, scroll down = decrease
                let delta: f32 = match event.delta {
                    gpui::ScrollDelta::Lines(lines) => lines.y * 0.05, // 5% per scroll line
                    gpui::ScrollDelta::Pixels(pixels) => {
                        let y_px: f32 = pixels.y.into();
                        y_px / 200.0 // Normalize pixel scroll
                    }
                };
                view.state.update(cx, |state, _cx| {
                    let new_volume = (state.app.volume + delta).clamp(0.0, 1.0);
                    state.app.volume = new_volume;
                    // Apply volume change to player
                    let _ = state.player.lock().set_volume(new_volume);
                });
                cx.notify();
            }))
            .child(
                Potentiometer::new()
                    .value(volume)
                    .label(format!("{}", volume_percent))
                    .size(px(72.0)) // 50% bigger (48 * 1.5 = 72)
                    .muted(muted)
                    .accent_color(accent_color)
                    .muted_color(muted_color)
                    .bg_color(bg_color)
                    .text_color(text_color),
            )
    }
}
