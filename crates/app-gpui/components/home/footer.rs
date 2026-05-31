//! Footer component rendering with transport controls, track info, and volume

#[cfg(all(target_os = "macos", feature = "hal"))]
use crate::app::types::PlaybackSource;
use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName, IconSize};
use crate::ui::{FOOTER_HEIGHT_REMS, PlayerView};
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    HStack, IconButton, IconButtonSize, IconButtonVariant, Menu, MenuItem, StackAlign,
    StackJustify, StackSpacing, VStack, VolumeKnob,
};

use std::cell::RefCell;
use std::rc::Rc;

use crate::components::themed_tooltip as footer_tooltip;

const WAVEFORM_NUM_BARS: usize = 128;
const DEFAULT_WAVEFORM: [u8; WAVEFORM_NUM_BARS] = [64; WAVEFORM_NUM_BARS];
const WAVEFORM_MAX_HEIGHT_PX: f32 = 12.0;
const WAVEFORM_MIN_HEIGHT_PX: f32 = 0.0;
const WAVEFORM_BAR_GAP_PX: f32 = 1.0;

/// Custom element to capture waveform bounds and render bars
struct WaveformElement {
    waveform: Option<sotf_audio_player::TrackWaveform>,
    progress: f32,
    played_color: gpui::Rgba,
    unplayed_color: gpui::Rgba,
    bounds_ref: Rc<RefCell<Option<Bounds<Pixels>>>>,
}

impl WaveformElement {
    fn new(
        waveform: Option<sotf_audio_player::TrackWaveform>,
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
        let layout_id = window.request_layout(
            Style {
                size: Size {
                    width: relative(1.0).into(),
                    height: relative(1.0).into(),
                },
                ..Default::default()
            },
            [],
            cx,
        );
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
        let samples = self
            .waveform
            .as_deref()
            .map(|samples| &samples[..])
            .unwrap_or(&DEFAULT_WAVEFORM);

        let progress_bar_idx = (self.progress * WAVEFORM_NUM_BARS as f32) as usize;

        // intentional: paint-space baseline nudge aligning bar midpoint with
        // label baseline in the waveform row (matches the integer pixel grid
        // used by WAVEFORM_MAX_HEIGHT_PX above).
        let center_y = bounds.origin.y + bounds.size.height / 2.0 + px(6.0);

        for (idx, amplitude) in samples.iter().enumerate() {
            let height_ratio = *amplitude as f32 / 255.0;
            let bar_height = WAVEFORM_MIN_HEIGHT_PX
                + (WAVEFORM_MAX_HEIGHT_PX - WAVEFORM_MIN_HEIGHT_PX) * height_ratio;
            let bar_color = if idx < progress_bar_idx {
                self.played_color
            } else {
                self.unplayed_color
            };

            let (x, width) = waveform_bar_x_and_width(bounds.size.width, idx, WAVEFORM_NUM_BARS);

            // Draw top half
            window.paint_quad(PaintQuad {
                bounds: Bounds {
                    origin: point(bounds.origin.x + x, center_y - px(bar_height)),
                    size: size(width, px(bar_height * 2.0)),
                },
                corner_radii: Corners::all(px(1.0)), // intentional: 1px paint-math rounding inside Element::paint
                background: bar_color.into(),
                border_widths: Edges::default(),
                border_color: Hsla::transparent_black(),
                border_style: Default::default(),
            });
        }
    }
}

fn waveform_bar_x_and_width(
    bounds_width: Pixels,
    idx: usize,
    bar_count: usize,
) -> (Pixels, Pixels) {
    if bar_count == 0 {
        return (px(0.0), px(0.0));
    }

    let slot_width = bounds_width / bar_count as f32;
    let x = slot_width * idx as f32;
    let right = if idx + 1 == bar_count {
        bounds_width
    } else {
        slot_width * (idx + 1) as f32
    };
    let available_width = (right - x).max(px(0.0));
    let gap = px(WAVEFORM_BAR_GAP_PX).min(available_width * 0.25);
    let width = if idx + 1 == bar_count {
        available_width
    } else {
        (available_width - gap).max(px(1.0).min(available_width))
    };

    (x, width)
}

#[cfg(test)]
mod tests {
    use super::{WAVEFORM_NUM_BARS, waveform_bar_x_and_width};
    use gpui::{Pixels, px};

    fn px_f32(value: Pixels) -> f32 {
        value.to_f64() as f32
    }

    #[test]
    fn waveform_bars_span_measured_bounds() {
        let bounds_width = px(600.0);
        let (first_x, _) = waveform_bar_x_and_width(bounds_width, 0, WAVEFORM_NUM_BARS);
        let (last_x, last_width) =
            waveform_bar_x_and_width(bounds_width, WAVEFORM_NUM_BARS - 1, WAVEFORM_NUM_BARS);

        assert_eq!(px_f32(first_x), 0.0);
        assert!((px_f32(last_x + last_width) - 600.0).abs() < 0.001);
    }

    #[test]
    fn waveform_bars_do_not_overflow_when_narrow() {
        let bounds_width = px(64.0);

        for idx in 0..WAVEFORM_NUM_BARS {
            let (x, width) = waveform_bar_x_and_width(bounds_width, idx, WAVEFORM_NUM_BARS);
            assert!(x >= px(0.0));
            assert!(width >= px(0.0));
            assert!(x + width <= bounds_width);
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
        let responsive_scale = crate::ui::compute_responsive_scale(window_width, window_height);
        let effective_rem = 16.0
            * (state.app.ui_state.font_scale * responsive_scale).clamp(
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
            if let Some(item) = state.app.queue_state.get(queue_idx) {
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
                if let Some(item) = state.app.queue_state.get(queue_idx) {
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
            if let Some(item) = state.app.queue_state.get(queue_idx) {
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
                                #[cfg(feature = "dev-api")]
                                use crate::app::dev_api::DevTrackExt;
                                let play_icon = if is_playing {
                                    IconName::Pause
                                } else {
                                    IconName::Play
                                };
                                let tt = theme_clone.clone();
                                let play_label = if is_playing { "Pause" } else { "Play" };
                                let wrapper = div()
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
                                    );
                                #[cfg(feature = "dev-api")]
                                let wrapper = wrapper.dev_track("transport.play");
                                wrapper
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

    /// Right section: Device selection + Volume
    fn render_footer_right(
        &self,
        translations: &crate::i18n::Translations,
        show_studio_device: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
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
            let theme = &state.app.ui_state.theme;
            let device_name = state
                .app
                .audio_device_state
                .current_output_device_name
                .clone()
                .unwrap_or_else(|| default_device_label.to_string());
            (
                state.app.playback.volume,
                state.app.playback.muted,
                state.app.ui_state.show_device_popup,
                device_name,
                state.app.ui_state.current_screen,
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
            _ => translations.screen_tools,
        };

        div()
            .flex()
            .items_center()
            .gap(d.gap_md)
            .when(show_studio_device, |el| el.min_w(rems(11.25)))
            .justify_end()
            .relative()
            // Studio button (Plugin Rack) - hidden on narrow screens
            .when(show_studio_device, |el| {
                el.child(
                    div()
                        .id("studio-button")
                        .px(d.pad_y)
                        .py(d.pad_y_half)
                        .rounded(d.r_md)
                        .cursor_pointer()
                        .hover(|style| style.bg(surface_hover))
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                view.state.update(cx, |state, _cx| {
                                    state.app.ui_state.show_studio_menu =
                                        !state.app.ui_state.show_studio_menu;
                                });
                                cx.notify();
                            }),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap(d.grid)
                                .child(
                                    Icon::new(IconName::SlidersHorizontal)
                                        .size(IconSize::Xxl)
                                        .color(theme_clone.text_secondary),
                                )
                                .child(
                                    div()
                                        .text_size(d.text_xs)
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
                        .px(d.pad_y)
                        .py(d.pad_y_half)
                        .rounded(d.r_md)
                        .cursor_pointer()
                        .hover(|style| style.bg(surface_hover))
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                view.state.update(cx, |state, _cx| {
                                    state.app.ui_state.show_device_popup =
                                        !state.app.ui_state.show_device_popup;
                                });
                                cx.notify();
                            }),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap(d.grid)
                                // Speaker icon
                                .child(
                                    Icon::new(IconName::Speaker)
                                        .size(IconSize::Xxl)
                                        .color(theme_clone.text_secondary),
                                )
                                // Device name below
                                .child(
                                    div()
                                        .text_size(d.text_xs)
                                        .text_color(text_secondary)
                                        .text_center()
                                        .max_w(rems(5.0))
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .whitespace_nowrap()
                                        .child(current_device),
                                ),
                        ),
                )
            })
            // Round volume button - always visible
            .child(self.render_volume_button(volume, muted, theme_clone, cx))
            // Studio menu dropdown - shown when show_studio_menu is true
            .when(
                self.state.read(cx).app.ui_state.show_studio_menu && show_studio_device,
                |el| el.child(self.render_studio_menu(translations, cx)),
            )
    }

    /// Render the device selection popup
    pub(crate) fn render_device_popup(
        &self,
        header_label: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let (devices, selected_index, theme) = {
            let state = self.state.read(cx);
            (
                state.app.audio_device_state.output_devices.clone(),
                state.app.audio_device_state.selected_output_device_index,
                state.app.ui_state.theme.clone(),
            )
        };

        div()
            .id("device-popup")
            .absolute()
            .bottom(rems(FOOTER_HEIGHT_REMS)) // Positioned above the footer
            .right(rems(0.625))
            .w(rems(15.625))
            .h(rems(30.0)) // Fixed height for up to ~20 devices
            .min_h(rems(5.0))
            .bg(theme.surface)
            .border_1()
            .border_color(theme.border)
            .rounded(d.r_md)
            .shadow_lg()
            .py(d.pad_y_half)
            .overflow_y_scroll()
            // Stop click propagation so overlay doesn't close popup
            .on_mouse_down(MouseButton::Left, |_, _, _| {})
            .on_mouse_up(MouseButton::Left, |_, _, _| {})
            // Header with refresh button
            .child({
                let theme_header = theme.clone();
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(d.pad_x)
                    .py(d.pad_y)
                    .border_b_1()
                    .border_color(theme_header.border)
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme_header.text_muted)
                            .child(header_label),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(d.grid)
                            .child(
                                div()
                                    .id("scan-cast")
                                    .cursor_pointer()
                                    .px(d.pad_y)
                                    .py(d.pad_y_half)
                                    .rounded(d.r_sm)
                                    .text_size(d.text_xs)
                                    .text_color(theme_header.text_muted)
                                    .hover(|s| s.bg(theme_header.surface_hover))
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                            view.state.update(cx, |state, _cx| {
                                                state.app.start_cast_discovery();
                                            });
                                            cx.notify();
                                        }),
                                    )
                                    .child("Cast"),
                            )
                            .child(
                                div()
                                    .id("refresh-devices")
                                    .cursor_pointer()
                                    .p(d.grid)
                                    .rounded(d.r_sm)
                                    .hover(|s| s.bg(theme_header.surface_hover))
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                            view.state.update(cx, |state, _cx| {
                                                state.app.load_audio_devices();
                                            });
                                            cx.notify();
                                        }),
                                    )
                                    .child(
                                        div()
                                            // intentional: 2× d.text_lg (1.125 rem) per UX request
                                            .text_size(rems(2.25))
                                            .text_color(theme_header.text_muted)
                                            .child("⟳"),
                                    ),
                            ),
                    )
            })
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
                    .px(d.pad_x)
                    .py(d.pad_y_half)
                    .mx(d.grid)
                    .my(px(1.0)) // intentional: hairline gap between list rows, no matching token
                    .rounded(d.r_sm)
                    .cursor_pointer()
                    .text_size(d.text_sm)
                    .when(is_selected, |el| {
                        el.bg(theme.surface_hover)
                            .text_color(theme.text_primary)
                            .font_weight(FontWeight::MEDIUM)
                    })
                    .when(!is_selected, |el| {
                        el.text_color(theme.text_secondary).hover(|style| {
                            style.bg(theme.surface_hover).text_color(theme.text_primary)
                        })
                    })
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                            view.state.update(cx, |state, _cx| {
                                let was_playing = state.app.playback.is_playing;
                                let current_path = state.app.queue_state.current_track_source();
                                let current_pos = state.app.playback.position_secs;

                                state.app.audio_device_state.selected_output_device_index = idx;
                                state.app.audio_device_state.current_output_device_name =
                                    Some(device_name.clone());
                                state.app.ui_state.show_device_popup = false;

                                // Apply the device selection to the player
                                let mut player = state.player.lock();
                                if let Err(e) = player.set_output_device(device_name.clone()) {
                                    log::error!("Failed to set output device: {}", e);
                                } else if was_playing && let Some(path) = current_path {
                                    // Drop the player lock before calling play_track which also locks it
                                    drop(player);
                                    Self::play_track_at(state, path, Some(current_pos));
                                }
                            });
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(d.gap)
                            .when(is_selected, |el| el.child("✓"))
                            .child(display_name),
                    )
            }))
            // Cast Devices section
            .child({
                let state = self.state.read(cx);
                let cast_devices = state.app.audio_device_state.cast_devices.clone();
                let cast_running = state.app.audio_device_state.cast_discovery_running;
                let selected_cast = state.app.audio_device_state.selected_cast_device;
                let theme_cast = theme.clone();

                let theme_cast_header = theme_cast.clone();
                let mut section = div()
                    .flex()
                    .flex_col()
                    .border_t_1()
                    .border_color(theme_cast.border)
                    .mt(d.grid)
                    .pt(d.pad_y_half)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(d.pad_x)
                            .py(d.pad_y_half)
                            .child(
                                div()
                                    .text_size(d.text_xs)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme_cast_header.text_muted)
                                    .child(if cast_running {
                                        "Cast Devices (scanning...)"
                                    } else {
                                        "Cast Devices"
                                    }),
                            )
                            .child(
                                div()
                                    .id("refresh-cast-devices")
                                    .cursor_pointer()
                                    .p(d.grid)
                                    .rounded(d.r_sm)
                                    .hover(|s| s.bg(theme_cast_header.surface_hover))
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                            view.state.update(cx, |state, _cx| {
                                                state.app.start_cast_discovery();
                                            });
                                            cx.notify();
                                        }),
                                    )
                                    .child(
                                        div()
                                            // intentional: 2× d.text_lg (1.125 rem), matches top refresh icon
                                            .text_size(rems(2.25))
                                            .text_color(theme_cast_header.text_muted)
                                            .child("⟳"),
                                    ),
                            ),
                    );

                if cast_devices.is_empty() && !cast_running {
                    section = section.child(
                        div()
                            .px(d.pad_x)
                            .py(d.pad_y_half)
                            .text_size(d.text_xs)
                            .text_color(theme_cast.text_muted)
                            .child("No Cast devices found"),
                    );
                }

                for (idx, device) in cast_devices.iter().enumerate() {
                    let is_selected = selected_cast == Some(idx);
                    let name = device.name.clone();
                    let dtype = device.device_type.clone();
                    let theme_item = theme_cast.clone();

                    section = section.child(
                        div()
                            .id(SharedString::from(format!("cast-device-{}", idx)))
                            .px(d.pad_x)
                            .py(d.pad_y_half)
                            .mx(d.grid)
                            .my(px(1.0)) // intentional: hairline gap between list rows, no matching token
                            .rounded(d.r_sm)
                            .cursor_pointer()
                            .text_size(d.text_sm)
                            .when(is_selected, |el| {
                                el.bg(theme_item.surface_hover)
                                    .text_color(theme_item.text_primary)
                                    .font_weight(FontWeight::MEDIUM)
                            })
                            .when(!is_selected, |el| {
                                el.text_color(theme_item.text_secondary).hover(|style| {
                                    style
                                        .bg(theme_item.surface_hover)
                                        .text_color(theme_item.text_primary)
                                })
                            })
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                    view.state.update(cx, |state, _cx| {
                                        if state.app.audio_device_state.selected_cast_device
                                            == Some(idx)
                                        {
                                            state.app.deselect_cast_device();
                                        } else {
                                            state.app.select_cast_device(idx);
                                        }
                                        state.app.ui_state.show_device_popup = false;
                                    });
                                    cx.notify();
                                }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(d.gap)
                                    .when(is_selected, |el| el.child("✓"))
                                    .child(name)
                                    .child(
                                        div()
                                            .text_size(d.text_xs)
                                            .text_color(theme_cast.text_muted)
                                            .child(format!("({})", dtype)),
                                    ),
                            ),
                    );
                }

                section
            })
    }

    /// Render the device popup overlay (click outside to close)
    pub(crate) fn render_device_popup_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let show_popup = self.state.read(cx).app.ui_state.show_device_popup;

        div().absolute().inset_0().when(show_popup, |el| {
            // Use mouse_up so popup items can handle mouse_down first
            el.on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, _cx| {
                        state.app.ui_state.show_device_popup = false;
                    });
                    cx.notify();
                }),
            )
        })
    }

    /// Render the studio menu overlay (click outside to close)
    pub(crate) fn render_studio_menu_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let show_menu = self.state.read(cx).app.ui_state.show_studio_menu;

        div().absolute().inset_0().when(show_menu, |el| {
            el.on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, _cx| {
                        state.app.ui_state.show_studio_menu = false;
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
        let app = &self.state.read(cx).app;
        let theme = app.ui_state.theme.clone();
        let channel = app.ui_state.release_channel;
        let state = self.state.clone();

        Menu::new(
            "studio-menu",
            vec![
                MenuItem::new("library", translations.screen_library).with_shortcut("⌘0"),
                MenuItem::new("studio", translations.screen_studio_rack)
                    .with_shortcut("⌘1")
                    .disabled(!channel.allows(crate::app::Screen::Studio.maturity())),
                MenuItem::new("plugingraph", translations.screen_studio_full)
                    .with_shortcut("⌘2")
                    .disabled(!channel.allows(crate::app::Screen::PluginGraph.maturity())),
                MenuItem::new("recording", translations.screen_recording).with_shortcut("⌘3"),
                MenuItem::new("roomeq", translations.screen_room_eq)
                    .with_shortcut("⌘4")
                    .disabled(!channel.allows(crate::app::Screen::RoomEq.maturity())),
                MenuItem::new("headphoneeq", translations.screen_headphone_eq).with_shortcut("⌘5"),
                MenuItem::new("spinorama", translations.screen_spinorama).with_shortcut("⌘6"),
                MenuItem::separator(),
                MenuItem::new("tutorial", "Show Tutorial"),
                MenuItem::separator(),
                MenuItem::new("settings", translations.screen_settings).with_shortcut("⌘,"),
            ],
        )
        .theme(theme.to_menu_theme())
        .on_select(move |id, _window, cx| {
            state.update(cx, |state, _cx| {
                state.app.ui_state.show_studio_menu = false;
                match id.as_ref() {
                    "library" => state.app.ui_state.current_screen = crate::app::Screen::Library,
                    "studio" => state.app.ui_state.current_screen = crate::app::Screen::Studio,
                    "plugingraph" => {
                        state.app.ui_state.current_screen = crate::app::Screen::PluginGraph
                    }
                    "recording" => {
                        state.app.ui_state.current_screen = crate::app::Screen::Recording
                    }
                    "roomeq" => state.app.ui_state.current_screen = crate::app::Screen::RoomEq,
                    "headphoneeq" => {
                        state.app.ui_state.current_screen = crate::app::Screen::HeadphoneEq
                    }
                    "spinorama" => {
                        state.app.ui_state.current_screen = crate::app::Screen::Spinorama
                    }
                    "tutorial" => {
                        state.app.ui_state.input_mode = crate::app::InputMode::Tutorial;
                        state.app.ui_state.tutorial_screen = 0;
                    }
                    "settings" => state.app.ui_state.current_screen = crate::app::Screen::Settings,
                    _ => {}
                }
            });
        })
        .build_with_theme(&theme.to_menu_theme())
        .absolute()
        .bottom(rems(FOOTER_HEIGHT_REMS))
        .right(rems(11.25))
    }

    /// Render a round volume button with circular progress indicator
    /// Supports mouse scroll and keyboard input to change volume
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
        let focus_ring_color: gpui::Hsla = theme.accent.into();

        let focus_handle = self.volume_focus_handle.clone();

        let tt = theme.clone();
        div()
            .id("volume-button")
            .cursor_pointer()
            .tooltip(move |_window, cx| footer_tooltip("Volume (scroll to adjust)", &tt, cx))
            .track_focus(&focus_handle)
            .focus(|style| {
                style
                    .border_2()
                    .border_color(focus_ring_color)
                    .rounded_full()
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, event: &MouseDownEvent, window, cx| {
                    window.focus(&focus_handle, cx);

                    if event.click_count == 2 {
                        // Double click resets volume to 10%
                        view.state.update(cx, |state, _cx| {
                            state.app.playback.volume = 0.1;
                            let _ = state.player.lock().set_volume(0.1);
                        });
                        cx.notify();
                        return;
                    }
                    // Start volume drag
                    view.state.update(cx, |state, _cx| {
                        state.app.volume_drag = Some(crate::app::state::app::VolumeDragState {
                            start_y: event.position.y.into(),
                            start_value: state.app.playback.volume,
                        });
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
                    let new_volume = (state.app.playback.volume + delta).clamp(0.0, 1.0);
                    state.app.playback.volume = new_volume;
                    // Apply volume change to player
                    let _ = state.player.lock().set_volume(new_volume);
                });
                cx.notify();
            }))
            .key_context("volume-control")
            .child(
                VolumeKnob::new()
                    .value(volume)
                    .label(format!("{}", volume_percent))
                    .size(rems(4.5))
                    .muted(muted)
                    .accent_color(accent_color)
                    .muted_color(muted_color)
                    .bg_color(bg_color)
                    .text_color(text_color),
            )
    }
}
