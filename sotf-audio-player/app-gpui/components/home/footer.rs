//! Footer component rendering with transport controls, track info, and volume

use crate::components::icons::{Icon, IconName, IconSize};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    HStack, IconButton, IconButtonSize, IconButtonVariant, Menu, MenuItem, StackAlign,
    StackJustify, StackSpacing, VStack, VolumeKnob,
};

use std::cell::RefCell;
use std::rc::Rc;

/// Custom element to capture waveform bounds and render bars
struct WaveformElement {
    waveform: Option<Vec<u8>>,
    progress: f32,
    played_color: gpui::Rgba,
    unplayed_color: gpui::Rgba,
    bounds_ref: Rc<RefCell<Option<Bounds<Pixels>>>>,
}

impl WaveformElement {
    fn new(
        waveform: Option<Vec<u8>>,
        progress: f32,
        played_color: gpui::Rgba,
        unplayed_color: gpui::Rgba,
        bounds_ref: Rc<RefCell<Option<Bounds<Pixels>>>>,
    ) -> Self {
        Self {
            waveform,
            progress,
            played_color,
            unplayed_color,
            bounds_ref,
        }
    }
}

impl IntoElement for WaveformElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for WaveformElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        Some(std::panic::Location::caller())
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let layout_id = window.request_layout(Style::default(), [], cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
        ()
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        _cx: &mut App,
    ) {
        // Capture bounds
        *self.bounds_ref.borrow_mut() = Some(bounds);

        // Render logic from render_waveform_bars
        const NUM_BARS: usize = 128;
        const MAX_HEIGHT: f32 = 12.0;
        const MIN_HEIGHT: f32 = 0.0;
        const BAR_WIDTH: f32 = 4.0;

        let default_waveform: Vec<u8> = vec![64; NUM_BARS];
        let samples = self.waveform.as_ref().unwrap_or(&default_waveform);

        let bar_samples: Vec<u8> = if samples.len() == NUM_BARS {
            samples.clone()
        } else if samples.is_empty() {
            vec![64; NUM_BARS]
        } else {
            (0..NUM_BARS)
                .map(|i| {
                    let src_idx = (i * samples.len()) / NUM_BARS;
                    samples.get(src_idx).copied().unwrap_or(64)
                })
                .collect()
        };

        let progress_bar_idx = (self.progress * NUM_BARS as f32) as usize;

        // Calculate total width to center the bars
        let total_width = NUM_BARS as f32 * BAR_WIDTH;
        let start_x = bounds.origin.x + (bounds.size.width - px(total_width)) / 2.0;
        let center_y = bounds.origin.y + bounds.size.height / 2.0;

        for (idx, amplitude) in bar_samples.iter().enumerate() {
            let height_ratio = *amplitude as f32 / 255.0;
            let bar_height = MIN_HEIGHT + (MAX_HEIGHT - MIN_HEIGHT) * height_ratio;
            let bar_color = if idx < progress_bar_idx {
                self.played_color
            } else {
                self.unplayed_color
            };

            let x = start_x + px(idx as f32 * BAR_WIDTH);

            // Draw top half
            window.paint_quad(PaintQuad {
                bounds: Bounds {
                    origin: point(x, center_y - px(bar_height)),
                    size: size(px(BAR_WIDTH - 1.0), px(bar_height * 2.0)),
                },
                corner_radii: Corners::all(px(1.0)),
                background: bar_color.into(),
                border_widths: Edges::default(),
                border_color: Hsla::transparent_black(),
                border_style: Default::default(),
            });
        }
    }
}

/// Responsive breakpoints for footer layout
const BREAKPOINT_HIDE_WAVEFORM: f32 = 700.0;
const BREAKPOINT_HIDE_TRACK_INFO: f32 = 550.0;
const BREAKPOINT_HIDE_STUDIO_DEVICE: f32 = 400.0;

impl PlayerView {
    pub(crate) fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = &state.app.theme;
        let translations = state.app.translations.clone();
        let window_width = state.app.window_width;

        let bg_surface = theme.surface;
        let border_color = theme.border;

        // Determine what to show based on width
        let show_waveform = window_width >= BREAKPOINT_HIDE_WAVEFORM;
        let show_track_info = window_width >= BREAKPOINT_HIDE_TRACK_INFO;
        let show_studio_device = window_width >= BREAKPOINT_HIDE_STUDIO_DEVICE;

        VStack::new()
            .spacing(StackSpacing::None)
            .child(
                // Main footer content: responsive three-section layout
                HStack::new()
                    .spacing(StackSpacing::None)
                    .justify(if show_track_info {
                        StackJustify::SpaceBetween
                    } else {
                        StackJustify::Center
                    })
                    .align(StackAlign::Center)
                    // Left section: Track info (hidden on narrow screens)
                    .when(show_track_info, |el| {
                        el.child(self.render_footer_left(&translations, cx))
                    })
                    // Center section: Transport + waveform
                    .child(self.render_footer_center(show_waveform, cx))
                    // Right section: Device + Volume (partially hidden on narrow screens)
                    .child(self.render_footer_right(&translations, show_studio_device, cx))
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
    fn render_footer_left(
        &self,
        translations: &crate::i18n::Translations,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = &state.app.theme;
        let no_track_label = translations.playback_no_track;

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
                    .w(px(96.0))
                    .h(px(96.0))
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
            .child({
                let title_text = if title.is_empty() {
                    no_track_label.to_string()
                } else {
                    title.clone()
                };
                let title_len = title_text.chars().count();

                // Truncate album name if too long (max 35 characters)
                let album_text = if album_name.chars().count() > 35 {
                    album_name.chars().take(35).collect::<String>() + "..."
                } else {
                    album_name.clone()
                };

                // Truncate artist name if too long (max 35 characters)
                let artist_text = if artist.chars().count() > 35 {
                    artist.chars().take(35).collect::<String>() + "..."
                } else {
                    artist.clone()
                };

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
                            // If title is longer than 40 characters, allow wrapping
                            .when(title_len <= 40, |d| d.text_ellipsis().whitespace_nowrap())
                            // If title is longer than 40 characters, wrap and justify
                            .when(title_len > 40, |d| d.text_align(gpui::TextAlign::Left))
                            .child(title_text),
                    )
                    // Album (9px equivalent)
                    .child(
                        div()
                            .text_xs()
                            .text_color(text_secondary)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(album_text),
                    )
                    // Artist (9px equivalent)
                    .child(
                        div()
                            .text_xs()
                            .text_color(text_muted)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(artist_text),
                    )
            })
            .build()
            .min_w(px(250.0))
            .max_w(px(350.0))
    }

    /// Center section: Transport controls + waveform + time
    fn render_footer_center(
        &self,
        show_waveform: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
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

        let bounds_ref = Rc::new(RefCell::new(None::<Bounds<Pixels>>));
        let bounds_ref_clone = bounds_ref.clone();

        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_0() // No gap - we use explicit margins
            .pt(px(8.0)) // Top padding to push transport down from border
            .flex_1()
            .max_w(px(600.0))
            // Transport controls row
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    // Previous track
                    .child(
                        div()
                            .id("transport-prev-wrapper")
                            .on_click(cx.listener(|view, _event: &ClickEvent, window, cx| {
                                view.prev_track(&crate::app::actions::PrevTrack, window, cx);
                            }))
                            .child(
                                IconButton::with_child(
                                    "transport-prev",
                                    Icon::new(IconName::SkipBack)
                                        .size(IconSize::Lg)
                                        .color(theme_clone.text_primary),
                                )
                                .variant(IconButtonVariant::Ghost)
                                .size(IconButtonSize::Lg)
                                .rounded_full()
                                .theme(theme_clone.to_icon_button_theme()),
                            ),
                    )
                    // Seek backward
                    .child(
                        div()
                            .id("transport-seek-back-wrapper")
                            .on_click(cx.listener(|view, _event: &ClickEvent, _window, cx| {
                                view.state.update(cx, |state, _cx| {
                                    let new_position = (state.app.position_secs - 30.0).max(0.0);
                                    state.app.position_secs = new_position;
                                    if let Err(e) = state.player.lock().seek(new_position) {
                                        log::error!("Failed to seek backward: {}", e);
                                    }
                                });
                                cx.notify();
                            }))
                            .child(
                                IconButton::with_child(
                                    "transport-seek-back",
                                    Icon::new(IconName::Rewind)
                                        .size(IconSize::Lg)
                                        .color(theme_clone.text_primary),
                                )
                                .variant(IconButtonVariant::Ghost)
                                .size(IconButtonSize::Lg)
                                .rounded_full()
                                .theme(theme_clone.to_icon_button_theme()),
                            ),
                    )
                    // Play/Pause (large, accent background)
                    .child({
                        let play_icon = if is_playing {
                            IconName::Pause
                        } else {
                            IconName::Play
                        };
                        div()
                            .id("transport-play-wrapper")
                            .on_click(cx.listener(|view, _event: &ClickEvent, window, cx| {
                                view.toggle_playback(&crate::app::actions::PlayPause, window, cx);
                            }))
                            .child(
                                IconButton::with_child(
                                    "transport-play",
                                    Icon::new(play_icon)
                                        .size(IconSize::Lg)
                                        .color(theme_clone.text_on_accent),
                                )
                                .variant(IconButtonVariant::Filled)
                                .size(IconButtonSize::Xl)
                                .rounded_full()
                                .selected(true) // Use selected state for accent background
                                .theme(theme_clone.to_icon_button_theme()),
                            )
                    })
                    // Seek forward
                    .child(
                        div()
                            .id("transport-seek-fwd-wrapper")
                            .on_click(cx.listener(|view, _event: &ClickEvent, _window, cx| {
                                view.state.update(cx, |state, _cx| {
                                    let max = state.app.duration_secs;
                                    let new_position = (state.app.position_secs + 30.0).min(max);
                                    state.app.position_secs = new_position;
                                    if let Err(e) = state.player.lock().seek(new_position) {
                                        log::error!("Failed to seek forward: {}", e);
                                    }
                                });
                                cx.notify();
                            }))
                            .child(
                                IconButton::with_child(
                                    "transport-seek-fwd",
                                    Icon::new(IconName::FastForward)
                                        .size(IconSize::Lg)
                                        .color(theme_clone.text_primary),
                                )
                                .variant(IconButtonVariant::Ghost)
                                .size(IconButtonSize::Lg)
                                .rounded_full()
                                .theme(theme_clone.to_icon_button_theme()),
                            ),
                    )
                    // Next track
                    .child(
                        div()
                            .id("transport-next-wrapper")
                            .on_click(cx.listener(|view, _event: &ClickEvent, window, cx| {
                                view.next_track(&crate::app::actions::NextTrack, window, cx);
                            }))
                            .child(
                                IconButton::with_child(
                                    "transport-next",
                                    Icon::new(IconName::SkipForward)
                                        .size(IconSize::Lg)
                                        .color(theme_clone.text_primary),
                                )
                                .variant(IconButtonVariant::Ghost)
                                .size(IconButtonSize::Lg)
                                .rounded_full()
                                .theme(theme_clone.to_icon_button_theme()),
                            ),
                    ),
            )
            // Waveform/progress row - conditionally shown based on screen width
            .when(show_waveform, |el| {
                el.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .w_full()
                        .mt(px(12.0)) // Space between transport and waveform row
                        // Current position - vertically centered with waveform
                        .child(
                            div()
                                .text_xs()
                                .text_color(text_muted)
                                .min_w(px(40.0))
                                .mb(px(24.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(position_str.clone()),
                        )
                        // Waveform visualization from track data
                        .child(
                            div()
                                .id("waveform-bar")
                                .flex_1()
                                .gap_0()
                                .h(px(24.0)) // Waveform height
                                .cursor_pointer()
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(
                                        move |view, event: &MouseDownEvent, _window, cx| {
                                            if let Some(bounds) = *bounds_ref_clone.borrow() {
                                                // Calculate relative position
                                                let x = event.position.x - bounds.origin.x;
                                                let width = bounds.size.width;
                                                let ratio = (x / width).clamp(0.0, 1.0);

                                                view.state.update(cx, |state, _cx| {
                                                    let new_pos =
                                                        state.app.duration_secs * ratio as f64;
                                                    state.app.position_secs = new_pos;
                                                    if let Err(e) =
                                                        state.player.lock().seek(new_pos)
                                                    {
                                                        log::error!(
                                                            "Failed to seek from waveform: {}",
                                                            e
                                                        );
                                                    }
                                                });
                                                cx.notify();
                                            }
                                        },
                                    ),
                                )
                                .child(WaveformElement::new(
                                    waveform.clone(),
                                    progress,
                                    progress_bar_fill,
                                    progress_bar_bg,
                                    bounds_ref,
                                )),
                        )
                        // Total duration - vertically centered with waveform
                        .child(
                            div()
                                .text_xs()
                                .text_color(text_muted)
                                .min_w(px(40.0))
                                .mb(px(24.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(duration_str.clone()),
                        ),
                )
            })
            // When waveform is hidden, show compact time display below transport
            .when(!show_waveform, |el| {
                el.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap_1()
                        .mt(px(8.0))
                        .text_xs()
                        .text_color(text_muted)
                        .child(position_str)
                        .child("/")
                        .child(duration_str),
                )
            })
    }

    /// Right section: Device selection + Volume
    fn render_footer_right(
        &self,
        translations: &crate::i18n::Translations,
        show_studio_device: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let default_device_label = translations.playback_default_device;
        let (
            volume,
            muted,
            _show_device_popup, // Not used here anymore
            current_device,
            current_screen,
            text_secondary,
            surface_hover,
            theme_clone,
        ) = {
            let state = self.state.read(cx);
            let theme = &state.app.theme;
            // Get device name and truncate to 7 characters
            let device_name = state
                .app
                .current_output_device_name
                .clone()
                .unwrap_or_else(|| default_device_label.to_string());
            let truncated_device = if device_name.len() > 7 {
                device_name.chars().take(7).collect::<String>()
            } else {
                device_name
            };
            (
                state.app.volume,
                state.app.muted,
                state.app.show_device_popup,
                truncated_device,
                state.app.current_screen,
                theme.text_secondary,
                theme.surface_hover,
                theme.clone(),
            )
        };

        // Determine studio button label based on current screen
        let studio_label = match current_screen {
            crate::app::Screen::Studio => translations.screen_studio_rack,
            crate::app::Screen::PluginGraph => translations.screen_studio_full,
            crate::app::Screen::Recording => translations.screen_recording,
            crate::app::Screen::RoomEq => translations.screen_room_eq,
            crate::app::Screen::HeadphoneEq => translations.screen_headphone_eq,
            crate::app::Screen::Spinorama => translations.screen_spinorama,
            _ => translations.screen_studio_rack,
        };

        div()
            .flex()
            .items_center()
            .gap_3()
            .when(show_studio_device, |el| el.min_w(px(180.0)))
            .justify_end()
            .relative()
            // Studio button (Plugin Rack) - hidden on narrow screens
            .when(show_studio_device, |el| {
                el.child(
                    div()
                        .id("studio-button")
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .cursor_pointer()
                        .hover(|style| style.bg(surface_hover))
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                view.state.update(cx, |state, _cx| {
                                    state.app.show_studio_menu = !state.app.show_studio_menu;
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
                                .child(
                                    Icon::new(IconName::SlidersHorizontal)
                                        .size(IconSize::Xxl)
                                        .color(theme_clone.text_secondary),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(text_secondary)
                                        .text_center()
                                        .child(studio_label),
                                ),
                        ),
                )
            })
            // Device selection button - hidden on narrow screens
            .when(show_studio_device, |el| {
                el.child(
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
                                .child(
                                    Icon::new(IconName::Speaker)
                                        .size(IconSize::Xxl)
                                        .color(theme_clone.text_secondary),
                                )
                                // Device name below (truncated to 7 chars)
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(text_secondary)
                                        .text_center()
                                        .child(current_device),
                                ),
                        ),
                )
            })
            // Round volume button - always visible
            .child(self.render_volume_button(volume, muted, theme_clone, cx))
            // Studio menu dropdown - shown when show_studio_menu is true
            .when(
                self.state.read(cx).app.show_studio_menu && show_studio_device,
                |el| el.child(self.render_studio_menu(translations, cx)),
            )
    }

    /// Render the device selection popup
    pub(crate) fn render_device_popup(
        &self,
        header_label: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
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
            .bottom(px(100.0)) // Positioned above the footer
            .right(px(10.0))
            .w(px(250.0))
            .max_h(px(300.0))
            .bg(theme.surface)
            .border_1()
            .border_color(theme.border)
            .rounded(px(4.0))
            .shadow_lg()
            .py_1()
            .overflow_y_scroll()
            // Stop click propagation so overlay doesn't close popup
            .on_mouse_down(MouseButton::Left, |_, _, _| {})
            .on_mouse_up(MouseButton::Left, |_, _, _| {})
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
                    .child(header_label),
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
                        d.text_color(theme.text_secondary).hover(|style| {
                            style.bg(theme.surface_hover).text_color(theme.text_primary)
                        })
                    })
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                            view.state.update(cx, |state, _cx| {
                                state.app.selected_output_device_index = idx;
                                state.app.current_output_device_name = Some(device_name.clone());
                                state.app.show_device_popup = false;

                                // Apply the device selection to the player
                                let mut player = state.player.lock();
                                if let Err(e) = player.set_output_device(device_name.clone()) {
                                    log::error!("Failed to set output device: {}", e);
                                }
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
            // Use mouse_up so popup items can handle mouse_down first
            el.on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, _cx| {
                        state.app.show_device_popup = false;
                    });
                    cx.notify();
                }),
            )
        })
    }

    /// Render the studio menu overlay (click outside to close)
    pub(crate) fn render_studio_menu_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let show_menu = self.state.read(cx).app.show_studio_menu;

        div().absolute().inset_0().when(show_menu, |el| {
            el.on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, _cx| {
                        state.app.show_studio_menu = false;
                    });
                    cx.notify();
                }),
            )
        })
    }

    /// Render the studio menu dropdown
    fn render_studio_menu(
        &self,
        translations: &crate::i18n::Translations,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = self.state.read(cx).app.theme.clone();
        let state = self.state.clone();

        Menu::new(vec![
            MenuItem::new("library", translations.screen_library).with_shortcut("⌘0"),
            MenuItem::new("studio", translations.screen_studio_rack).with_shortcut("⌘1"),
            MenuItem::new("plugingraph", translations.screen_studio_full).with_shortcut("⌘2"),
            MenuItem::new("recording", translations.screen_recording).with_shortcut("⌘3"),
            MenuItem::new("roomeq", translations.screen_room_eq).with_shortcut("⌘4"),
            MenuItem::new("headphoneeq", translations.screen_headphone_eq).with_shortcut("⌘5"),
            MenuItem::new("spinorama", translations.screen_spinorama).with_shortcut("⌘6"),
        ])
        .theme(theme.to_menu_theme())
        .on_select(move |id, _window, cx| {
            state.update(cx, |state, _cx| {
                state.app.show_studio_menu = false;
                match id.as_ref() {
                    "library" => state.app.current_screen = crate::app::Screen::Library,
                    "studio" => state.app.current_screen = crate::app::Screen::Studio,
                    "plugingraph" => state.app.current_screen = crate::app::Screen::PluginGraph,
                    "recording" => state.app.current_screen = crate::app::Screen::Recording,
                    "roomeq" => state.app.current_screen = crate::app::Screen::RoomEq,
                    "headphoneeq" => state.app.current_screen = crate::app::Screen::HeadphoneEq,
                    "spinorama" => state.app.current_screen = crate::app::Screen::Spinorama,
                    _ => {}
                }
            });
        })
        .build_with_theme(&theme.to_menu_theme())
        .absolute()
        .bottom(px(100.0))
        .right(px(180.0))
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
                VolumeKnob::new()
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
