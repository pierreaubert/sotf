//! Footer component rendering with transport controls, track info, and volume

#[cfg(all(target_os = "macos", feature = "hal"))]
use crate::app::types::PlaybackSource;
use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName, IconSize};
use crate::ui::{FOOTER_HEIGHT_REMS, PlayerView};
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    HStack, IconButton, IconButtonSize, IconButtonVariant, StackAlign,
    StackJustify, StackSpacing, VStack,
};

use std::cell::RefCell;
use std::rc::Rc;

use crate::components::themed_tooltip as footer_tooltip;

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
        let center_y = bounds.origin.y + bounds.size.height / 2.0 + px(6.0);

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

/// Responsive breakpoints for footer layout (in rems).
/// Compared against window width in rem units so they scale with font size.
const BREAKPOINT_HIDE_WAVEFORM_REMS: f32 = 43.75; // ~700px at 16px rem
const BREAKPOINT_HIDE_TRACK_INFO_REMS: f32 = 34.375; // ~550px at 16px rem
const BREAKPOINT_HIDE_STUDIO_DEVICE_REMS: f32 = 25.0; // ~400px at 16px rem

impl PlayerView {
    pub(crate) fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = &state.app.ui_state.theme;
        let translations = state.app.ui_state.translations.clone();
        let window_width = state.app.ui_state.window_width;
        let window_height = state.app.ui_state.window_height;

        let bg_surface = theme.surface;
        let border_color = theme.border;

        // Compute window width in rems for responsive breakpoints
        let responsive_scale =
            crate::ui::compute_responsive_scale(window_width, window_height);
        let effective_rem = 16.0 * (state.app.ui_state.font_scale * responsive_scale)
            .clamp(
                crate::ui::DEFAULT_MIN_FONT_SIZE_PX / 16.0,
                crate::ui::DEFAULT_MAX_FONT_SIZE_PX / 16.0,
            );
        let window_width_rems = window_width / effective_rem;

        // Determine what to show based on width in rems
        let show_waveform = window_width_rems >= BREAKPOINT_HIDE_WAVEFORM_REMS;
        let show_track_info = window_width_rems >= BREAKPOINT_HIDE_TRACK_INFO_REMS;
        let show_studio_device = window_width_rems >= BREAKPOINT_HIDE_STUDIO_DEVICE_REMS;

        let footer_height_rems = FOOTER_HEIGHT_REMS;

        div()
            .flex()
            .flex_row()
            .h(rems(footer_height_rems))
            .bg(bg_surface)
            .border_t_1()
            .border_color(border_color)
            // Album art aligned to left corner with window-matching rounded corners
            .when(show_track_info, |el| {
                el.child(self.render_footer_album_art(footer_height_rems, cx))
            })
            // Main content area with padding
            .child(
                HStack::new()
                    .spacing(StackSpacing::None)
                    .justify(if show_track_info {
                        StackJustify::SpaceBetween
                    } else {
                        StackJustify::Center
                    })
                    .align(StackAlign::Center)
                    // Left section: Track info text (hidden on narrow screens)
                    .when(show_track_info, |el| {
                        el.child(self.render_footer_track_info(&translations, cx))
                    })
                    // Center section: Transport + waveform
                    .child(self.render_footer_center(show_waveform, cx))
                    // Right section: Device + Volume (partially hidden on narrow screens)
                    .child(self.render_footer_right(&translations, show_studio_device, cx))
                    .build()
                    .flex_1()
                    .h_full()
                    .px(d.card),
            )
    }

    /// Album artwork aligned to left corner with window-matching rounded corners.
    ///
    /// `footer_height_rems`: footer height in rem units (e.g. 6.25 ≈ 100px at 16px rem).
    /// The art is rendered as a square of this size.
    fn render_footer_album_art(
        &self,
        footer_height_rems: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = &state.app.ui_state.theme;

        // Get album art path from current queue item
        let album_art_path = if let Some(queue_idx) = state.app.playback.current_queue_index {
            if let Some(item) = state.app.queue.get(queue_idx) {
                item.album.album_art_path.clone()
            } else {
                None
            }
        } else {
            None
        };

        let surface_hover = theme.surface_hover;
        let text_muted = theme.text_muted;

        // Album art is square, matching footer height (rem-based)
        let art_div = div()
            .w(rems(footer_height_rems))
            .h(rems(footer_height_rems))
            // Only round bottom-left corner to match window (0.625rem ≈ 10px at base)
            .rounded_bl(rems(0.625))
            .bg(surface_hover)
            .overflow_hidden()
            .flex_shrink_0();

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
                .text_3xl()
                .child("♪")
        }
    }

    /// Track info text (title, album, artist) - displayed next to album art
    fn render_footer_track_info(
        &self,
        translations: &crate::i18n::Translations,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = &state.app.ui_state.theme;
        let no_track_label = translations.playback_no_track;

        // Check if we're in HAL input mode (macOS only)
        #[cfg(all(target_os = "macos", feature = "hal"))]
        if matches!(
            state.app.audio_device_state.playback_source,
            PlaybackSource::HalDevice
        ) {
            let text_primary = theme.text_primary;
            let text_secondary = theme.text_secondary;
            let accent = theme.accent;

            return VStack::new()
                .spacing(StackSpacing::Xs)
                .align(StackAlign::Start)
                .child(
                    div()
                        .text_size(d.text_sm)
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(accent)
                        .child("HAL Input Active"),
                )
                .child(
                    div()
                        .text_size(d.text_xs)
                        .text_color(text_secondary)
                        .child("Processing system audio"),
                )
                .child(
                    div()
                        .text_size(d.text_xs)
                        .text_color(text_primary)
                        .child(format!(
                            "{} plugins active",
                            state.app.plugin_state.graph.len()
                        )),
                )
                .build()
                .min_w(rems(9.375))
                .max_w(rems(15.625));
        }

        // Get current track info from queue
        let (title, album_name, artist) =
            if let Some(queue_idx) = state.app.playback.current_queue_index {
                if let Some(item) = state.app.queue.get(queue_idx) {
                    let track_title = item
                        .current_track()
                        .and_then(|t| t.title.clone())
                        .unwrap_or_else(|| "Unknown Track".to_string());

                    (track_title, item.album.title.clone(), item.album.artist())
                } else {
                    (String::new(), String::new(), String::new())
                }
            } else {
                (String::new(), String::new(), String::new())
            };

        let text_primary = theme.text_primary;
        let text_secondary = theme.text_secondary;
        let text_muted = theme.text_muted;

        let title_text = if title.is_empty() {
            no_track_label.to_string()
        } else {
            title.clone()
        };

        let album_text = album_name.clone();
        let artist_text = artist.clone();

        VStack::new()
            .spacing(StackSpacing::Xs)
            .align(StackAlign::Start)
            // Title
            .child(
                div()
                    .text_size(d.text_sm)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(text_primary)
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(title_text),
            )
            // Album
            .child(
                div()
                    .text_size(d.text_xs)
                    .text_color(text_secondary)
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(album_text),
            )
            // Artist
            .child(
                div()
                    .text_size(d.text_xs)
                    .text_color(text_muted)
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(artist_text),
            )
            .build()
            .min_w(rems(9.375))
            .max_w(rems(15.625))
    }

    /// Center section: Transport controls + waveform + time
    fn render_footer_center(
        &self,
        show_waveform: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = &state.app.ui_state.theme;

        // Check if we're in HAL mode - hide waveform/time display
        #[cfg(all(target_os = "macos", feature = "hal"))]
        let is_hal_mode = matches!(
            state.app.audio_device_state.playback_source,
            PlaybackSource::HalDevice
        );
        #[cfg(not(all(target_os = "macos", feature = "hal")))]
        let is_hal_mode = false;

        let position_secs = state.app.playback.position_secs;
        let duration_secs = state.app.playback.duration_secs;
        let is_playing = state.app.playback.is_playing;

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
        let waveform = if let Some(queue_idx) = state.app.playback.current_queue_index {
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
            state.app.ui_state.theme.clone()
        };

        let bounds_ref = Rc::new(RefCell::new(None::<Bounds<Pixels>>));
        let bounds_ref_clone = bounds_ref.clone();

        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(d.section)
            .pt(d.pad_y)
            .pb(d.pad_y)
            .justify_between()
            .flex_1()
            .max_w(rems(37.5))
            // Row 1: [time] [<< < ▶ > >>] [time] — timestamps at far edges
            .child(
                div()
                    .flex()
                    .items_center()
                    .w_full()
                    .justify_between()
                    .when(!is_hal_mode, |el| {
                        el.child(
                            div()
                                .text_size(d.text_xs)
                                .text_color(text_muted)
                                .min_w(rems(2.5))
                                .child(position_str.clone()),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(d.grid)
                            // Previous track
                            .child({
                                let tt = theme_clone.clone();
                                div()
                                    .id("transport-prev-wrapper")
                                    .on_click(cx.listener(
                                        |view, _event: &ClickEvent, window, cx| {
                                            view.prev_track(
                                                &crate::app::actions::PrevTrack,
                                                window,
                                                cx,
                                            );
                                        },
                                    ))
                                    .tooltip(move |_window, cx| {
                                        footer_tooltip("Previous Track", &tt, cx)
                                    })
                                    .child(
                                        IconButton::with_child(
                                            "transport-prev",
                                            Icon::new(IconName::SkipBack)
                                                .size(IconSize::Sm)
                                                .color(theme_clone.text_primary),
                                        )
                                        .variant(IconButtonVariant::Ghost)
                                        .size(IconButtonSize::Sm)
                                        .rounded_full()
                                        .theme(theme_clone.to_icon_button_theme()),
                                    )
                            })
                            // Seek backward
                            .child({
                                let tt = theme_clone.clone();
                                div()
                                    .id("transport-seek-back-wrapper")
                                    .tooltip(move |_window, cx| {
                                        footer_tooltip("Seek Back 30s", &tt, cx)
                                    })
                                    .on_click(cx.listener(
                                        |view, _event: &ClickEvent, _window, cx| {
                                            view.state.update(cx, |state, _cx| {
                                                let new_position =
                                                    (state.app.playback.position_secs - 30.0)
                                                        .max(0.0);
                                                state.app.playback.position_secs = new_position;
                                                if let Err(e) =
                                                    state.player.lock().seek(new_position)
                                                {
                                                    log::error!("Failed to seek backward: {}", e);
                                                }
                                            });
                                            cx.notify();
                                        },
                                    ))
                                    .child(
                                        IconButton::with_child(
                                            "transport-seek-back",
                                            Icon::new(IconName::Rewind)
                                                .size(IconSize::Sm)
                                                .color(theme_clone.text_primary),
                                        )
                                        .variant(IconButtonVariant::Ghost)
                                        .size(IconButtonSize::Sm)
                                        .rounded_full()
                                        .theme(theme_clone.to_icon_button_theme()),
                                    )
                            })
                            // Play/Pause
                            .child({
                                let play_icon = if is_playing {
                                    IconName::Pause
                                } else {
                                    IconName::Play
                                };
                                let tt = theme_clone.clone();
                                let play_label = if is_playing { "Pause" } else { "Play" };
                                div()
                                    .id("transport-play-wrapper")
                                    .on_click(cx.listener(
                                        |view, _event: &ClickEvent, window, cx| {
                                            view.toggle_playback(
                                                &crate::app::actions::PlayPause,
                                                window,
                                                cx,
                                            );
                                        },
                                    ))
                                    .tooltip(move |_window, cx| footer_tooltip(play_label, &tt, cx))
                                    .child(
                                        IconButton::with_child(
                                            "transport-play",
                                            Icon::new(play_icon)
                                                .size(IconSize::Sm)
                                                .color(theme_clone.text_on_accent),
                                        )
                                        .variant(IconButtonVariant::Filled)
                                        .size(IconButtonSize::Md)
                                        .rounded_full()
                                        .selected(true)
                                        .theme(theme_clone.to_icon_button_theme()),
                                    )
                            })
                            // Seek forward
                            .child({
                                let tt = theme_clone.clone();
                                div()
                                    .id("transport-seek-fwd-wrapper")
                                    .tooltip(move |_window, cx| {
                                        footer_tooltip("Seek Forward 30s", &tt, cx)
                                    })
                                    .on_click(cx.listener(
                                        |view, _event: &ClickEvent, _window, cx| {
                                            view.state.update(cx, |state, _cx| {
                                                let max = state.app.playback.duration_secs;
                                                let new_position =
                                                    (state.app.playback.position_secs + 30.0)
                                                        .min(max);
                                                state.app.playback.position_secs = new_position;
                                                if let Err(e) =
                                                    state.player.lock().seek(new_position)
                                                {
                                                    log::error!("Failed to seek forward: {}", e);
                                                }
                                            });
                                            cx.notify();
                                        },
                                    ))
                                    .child(
                                        IconButton::with_child(
                                            "transport-seek-fwd",
                                            Icon::new(IconName::FastForward)
                                                .size(IconSize::Sm)
                                                .color(theme_clone.text_primary),
                                        )
                                        .variant(IconButtonVariant::Ghost)
                                        .size(IconButtonSize::Sm)
                                        .rounded_full()
                                        .theme(theme_clone.to_icon_button_theme()),
                                    )
                            })
                            // Next track
                            .child({
                                let tt = theme_clone.clone();
                                div()
                                    .id("transport-next-wrapper")
                                    .tooltip(move |_window, cx| {
                                        footer_tooltip("Next Track", &tt, cx)
                                    })
                                    .on_click(cx.listener(
                                        |view, _event: &ClickEvent, window, cx| {
                                            view.next_track(
                                                &crate::app::actions::NextTrack,
                                                window,
                                                cx,
                                            );
                                        },
                                    ))
                                    .child(
                                        IconButton::with_child(
                                            "transport-next",
                                            Icon::new(IconName::SkipForward)
                                                .size(IconSize::Sm)
                                                .color(theme_clone.text_primary),
                                        )
                                        .variant(IconButtonVariant::Ghost)
                                        .size(IconButtonSize::Sm)
                                        .rounded_full()
                                        .theme(theme_clone.to_icon_button_theme()),
                                    )
                            }),
                    ) // close inner transport div
                    .when(!is_hal_mode, |el| {
                        el.child(
                            div()
                                .text_size(d.text_xs)
                                .text_color(text_muted)
                                .min_w(rems(2.5))
                                .flex()
                                .justify_end()
                                .child(duration_str.clone()),
                        )
                    }),
            )
            // Row 2: Waveform spanning full width
            .when(show_waveform && !is_hal_mode, |el| {
                el.child(
                    div()
                        .id("waveform-bar")
                        .w_full()
                        .h(rems(2.0))
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |view, event: &MouseDownEvent, _window, cx| {
                                if let Some(bounds) = *bounds_ref_clone.borrow() {
                                    let x = event.position.x - bounds.origin.x;
                                    let width = bounds.size.width;
                                    let ratio = (x / width).clamp(0.0, 1.0);

                                    view.state.update(cx, |state, _cx| {
                                        let new_pos =
                                            state.app.playback.duration_secs * ratio as f64;
                                        state.app.playback.position_secs = new_pos;
                                        if let Err(e) = state.player.lock().seek(new_pos) {
                                            log::error!("Failed to seek from waveform: {}", e);
                                        }
                                    });
                                    cx.notify();
                                }
                            }),
                        )
                        .child(WaveformElement::new(
                            waveform.clone(),
                            progress,
                            progress_bar_fill,
                            progress_bar_bg,
                            bounds_ref,
                        )),
                )
            })
            // When waveform is hidden, show compact time display below transport (not in HAL mode)
            .when(!show_waveform && !is_hal_mode, |el| {
                el.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap(d.grid)
                        .mt(d.gap)
                        .text_size(d.text_xs)
                        .text_color(text_muted)
                        .child(position_str)
                        .child("/")
                        .child(duration_str),
                )
            })
    }

}
