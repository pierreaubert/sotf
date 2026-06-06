//! Footer component rendering with transport controls, track info, and volume

#[cfg(all(target_os = "macos", feature = "hal"))]
use crate::app::types::PlaybackSource;
use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName, IconSize};
use crate::ui::{FOOTER_HEIGHT_REMS, PlayerView};
use gpui::prelude::*;
use gpui::*;
use gpui_audio_kit::VolumeKnob;
use gpui_ui_kit::{
    HStack, IconButton, IconButtonSize, IconButtonVariant, StackAlign, StackJustify, StackSpacing,
    VStack,
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

impl PlayerView {
    pub(crate) fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = &state.app.ui_state.theme;
        let translations = state.app.ui_state.translations.clone();
        let window_width = state.app.ui_state.window_width;
        let window_height = state.app.ui_state.window_height;
        let footer_collapsed = state.app.ui_state.footer_collapsed;

        let bg_surface = theme.surface;
        let border_color = theme.border;

        if footer_collapsed {
            return self
                .render_footer_collapsed(&translations, cx)
                .into_any_element();
        }

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
                    // Right section: footer collapse + volume
                    .child(self.render_footer_right(cx))
                    .build()
                    .flex_1()
                    .h_full()
                    .px(d.card),
            )
            .into_any_element()
    }

    fn render_footer_collapsed(
        &self,
        translations: &crate::i18n::Translations,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let d = Ds::from_cx(cx);
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let volume = state.app.playback.volume;
        let muted = state.app.playback.muted;
        let is_playing = state.app.playback.is_playing;
        let title = self.current_footer_title(translations, cx);
        let state_for_expand = self.state.clone();

        div()
            .id("footer-collapsed")
            .flex()
            .items_center()
            .h(rems(2.75))
            .bg(theme.surface)
            .border_t_1()
            .border_color(theme.border)
            .px(d.pad_x)
            .gap(d.gap_md)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(d.text_sm)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text_primary)
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(title),
            )
            .child(self.render_compact_transport(is_playing, theme.clone(), cx))
            .child(self.render_compact_volume(volume, muted, theme.clone(), cx))
            .child(
                div()
                    .id("footer-expand")
                    .flex_none()
                    .w(rems(1.75))
                    .h(rems(1.75))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(d.r_md)
                    .cursor_pointer()
                    .hover({
                        let theme = theme.clone();
                        move |style| style.bg(theme.surface_hover)
                    })
                    .child(
                        Icon::new(IconName::ChevronUp)
                            .size(IconSize::Sm)
                            .color(theme.text_muted),
                    )
                    .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                        state_for_expand.update(cx, |state, _cx| {
                            state.app.ui_state.footer_collapsed = false;
                        });
                    }),
            )
            .into_any_element()
    }

    fn current_footer_title(
        &self,
        translations: &crate::i18n::Translations,
        cx: &mut Context<Self>,
    ) -> String {
        let state = self.state.read(cx);

        #[cfg(all(target_os = "macos", feature = "hal"))]
        if matches!(
            state.app.audio_device_state.playback_source,
            PlaybackSource::HalDevice
        ) {
            return "HAL Input Active".to_string();
        }

        if let Some(queue_idx) = state.app.playback.current_queue_index
            && let Some(item) = state.app.queue_state.get(queue_idx)
        {
            return item
                .current_track()
                .and_then(|track| track.title.clone())
                .unwrap_or_else(|| item.album.title.clone());
        }

        translations.playback_no_track.to_string()
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
            .gap(d.gap)
            .pt(d.gap_md)
            .pb(d.gap_md)
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

    fn render_compact_transport(
        &self,
        is_playing: bool,
        theme: crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let d = Ds::from_cx(cx);
        let play_icon = if is_playing {
            IconName::Pause
        } else {
            IconName::Play
        };
        let play_label = if is_playing { "Pause" } else { "Play" };

        div()
            .id("footer-compact-transport")
            .flex()
            .items_center()
            .gap(d.grid)
            .flex_none()
            .child({
                let theme_clone = theme.clone();
                let tt = theme.clone();
                div()
                    .id("compact-transport-prev-wrapper")
                    .on_click(cx.listener(|view, _event: &ClickEvent, window, cx| {
                        view.prev_track(&crate::app::actions::PrevTrack, window, cx);
                    }))
                    .tooltip(move |_window, cx| footer_tooltip("Previous Track", &tt, cx))
                    .child(
                        IconButton::with_child(
                            "compact-transport-prev",
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
            .child({
                let theme_clone = theme.clone();
                let tt = theme.clone();
                div()
                    .id("compact-transport-play-wrapper")
                    .on_click(cx.listener(|view, _event: &ClickEvent, window, cx| {
                        view.toggle_playback(&crate::app::actions::PlayPause, window, cx);
                    }))
                    .tooltip(move |_window, cx| footer_tooltip(play_label, &tt, cx))
                    .child(
                        IconButton::with_child(
                            "compact-transport-play",
                            Icon::new(play_icon)
                                .size(IconSize::Sm)
                                .color(theme_clone.text_on_accent),
                        )
                        .variant(IconButtonVariant::Filled)
                        .size(IconButtonSize::Sm)
                        .rounded_full()
                        .selected(true)
                        .theme(theme_clone.to_icon_button_theme()),
                    )
            })
            .child({
                let theme_clone = theme.clone();
                let tt = theme.clone();
                div()
                    .id("compact-transport-next-wrapper")
                    .on_click(cx.listener(|view, _event: &ClickEvent, window, cx| {
                        view.next_track(&crate::app::actions::NextTrack, window, cx);
                    }))
                    .tooltip(move |_window, cx| footer_tooltip("Next Track", &tt, cx))
                    .child(
                        IconButton::with_child(
                            "compact-transport-next",
                            Icon::new(IconName::SkipForward)
                                .size(IconSize::Sm)
                                .color(theme_clone.text_primary),
                        )
                        .variant(IconButtonVariant::Ghost)
                        .size(IconButtonSize::Sm)
                        .rounded_full()
                        .theme(theme_clone.to_icon_button_theme()),
                    )
            })
            .into_any_element()
    }

    fn render_compact_volume(
        &self,
        volume: f32,
        muted: bool,
        theme: crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let d = Ds::from_cx(cx);
        let volume_percent = (volume * 100.0) as u32;
        let focus_handle = self.volume_focus_handle.clone();
        let icon = if volume <= 0.0 {
            IconName::VolumeX
        } else {
            IconName::Volume2
        };
        let text_color = if muted || volume <= 0.0 {
            theme.text_muted
        } else {
            theme.text_primary
        };
        let tt = theme.clone();

        div()
            .id("compact-volume")
            .flex()
            .items_center()
            .gap(d.grid)
            .h(rems(1.75))
            .px(d.pad_y)
            .rounded(d.r_md)
            .cursor_pointer()
            .track_focus(&focus_handle)
            .hover({
                let theme = theme.clone();
                move |style| style.bg(theme.surface_hover)
            })
            .tooltip(move |_window, cx| footer_tooltip("Volume (scroll to adjust)", &tt, cx))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, event: &MouseDownEvent, window, cx| {
                    window.focus(&focus_handle, cx);

                    if event.click_count == 2 {
                        view.state.update(cx, |state, _cx| {
                            state.app.playback.volume = 0.1;
                            let _ = state.player.lock().set_volume(0.1);
                        });
                        cx.notify();
                        return;
                    }

                    view.state.update(cx, |state, _cx| {
                        state.app.volume_drag = Some(crate::app::state::app::VolumeDragState {
                            start_y: event.position.y.into(),
                            start_value: state.app.playback.volume,
                        });
                    });
                }),
            )
            .on_scroll_wheel(cx.listener(|view, event: &ScrollWheelEvent, _window, cx| {
                let delta: f32 = match event.delta {
                    gpui::ScrollDelta::Lines(lines) => lines.y * 0.05,
                    gpui::ScrollDelta::Pixels(pixels) => {
                        let y_px: f32 = pixels.y.into();
                        y_px / 200.0
                    }
                };
                view.state.update(cx, |state, _cx| {
                    let new_volume = (state.app.playback.volume + delta).clamp(0.0, 1.0);
                    state.app.playback.volume = new_volume;
                    let _ = state.player.lock().set_volume(new_volume);
                });
                cx.notify();
            }))
            .child(Icon::new(icon).size(IconSize::Sm).color(text_color))
            .child(
                div()
                    .min_w(rems(2.0))
                    .text_size(d.text_xs)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(text_color)
                    .child(format!("{volume_percent}")),
            )
            .into_any_element()
    }

    /// Right section: footer collapse + volume
    fn render_footer_right(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let d = Ds::from_cx(cx);
        let (volume, muted, theme_clone) = {
            let state = self.state.read(cx);
            (
                state.app.playback.volume,
                state.app.playback.muted,
                state.app.ui_state.theme.clone(),
            )
        };
        let state_for_collapse = self.state.clone();

        div()
            .flex()
            .items_center()
            .gap(d.gap_md)
            .justify_end()
            .child(self.render_volume_button(volume, muted, theme_clone.clone(), cx))
            .child(
                div()
                    .id("footer-collapse")
                    .w(rems(1.75))
                    .h(rems(1.75))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(d.r_md)
                    .cursor_pointer()
                    .hover({
                        let theme = theme_clone.clone();
                        move |style| style.bg(theme.surface_hover)
                    })
                    .child(
                        Icon::new(IconName::ChevronDown)
                            .size(IconSize::Sm)
                            .color(theme_clone.text_muted),
                    )
                    .on_mouse_up(MouseButton::Left, move |_event, _window, cx| {
                        state_for_collapse.update(cx, |state, _cx| {
                            state.app.ui_state.footer_collapsed = true;
                        });
                    }),
            )
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
