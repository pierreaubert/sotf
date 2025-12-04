//! Queue screen rendering functions
//!
//! Level meter UI components are now consolidated in `ui/components/plugins/level_meters.rs`

use std::collections::BTreeMap;

use gpui::prelude::*;
use gpui::*;
use gpui::{InteractiveElement, Styled};
use gpui_ui_kit::{
    Button, ButtonSize, ButtonVariant, HStack, StackSpacing, Text, TextSize, TextWeight, VStack,
};
use sotf_audio_player::Track;

use crate::ui::PlayerView;

impl PlayerView {
    pub(crate) fn render_queue_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();

        // Use ratios for panel widths (layout will compute actual sizes)
        let queue_list_ratio = state.app.queue_list_ratio;
        let meters_ratio = state.app.meters_panel_ratio;
        let lufs_ratio = state.app.lufs_panel_ratio;

        let queue_collapsed = queue_list_ratio < 0.05;
        let meters_collapsed = meters_ratio < 0.05;

        div()
            .flex()
            .size_full()
            // Left panel: Queue list
            .when(!queue_collapsed, |d| {
                let button_theme = theme.to_button_theme();
                d.child(
                    div()
                        .flex()
                        .flex_col()
                        .w(relative(queue_list_ratio))
                        .px_2()
                        .pt_2()
                        .border_r_1()
                        .border_color(theme.border)
                        .child(
                            HStack::new()
                                .spacing(StackSpacing::Md)
                                .child(
                                    Text::new(format!("Queue ({} albums)", state.app.queue.len()))
                                        .size(TextSize::Lg)
                                        .weight(TextWeight::Bold)
                                        .color(theme.text_primary),
                                )
                                .child(
                                    Button::new("clear-queue-btn", "Clear")
                                        .variant(ButtonVariant::Ghost)
                                        .size(ButtonSize::Xs)
                                        .theme(button_theme)
                                        .build()
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                                view.state.update(cx, |state, _cx| {
                                                    state.app.clear_queue();
                                                });
                                                cx.notify();
                                            }),
                                        ),
                                )
                                .build()
                                .mb_2()
                                .flex_1()
                                .justify_between(),
                        )
                        .child(
                            div()
                                .id("queue-list")
                                .flex()
                                .flex_col()
                                .gap_2()
                                .flex_1()
                                .overflow_y_scroll()
                                .children(state.app.queue.iter().enumerate().map(|(idx, item)| {
                                    let is_current = state.app.current_queue_index == Some(idx);
                                    let theme = theme.clone();
                                    let theme_hover = theme.clone();
                                    div()
                                        .p_2()
                                        .rounded_md()
                                        .when(is_current, |d| {
                                            // Current item: accent background, subtle hover
                                            let mut hover_bg = theme.accent;
                                            hover_bg.a = 0.8;
                                            d.bg(theme.accent)
                                             .hover(move |style| style.bg(hover_bg))
                                        })
                                        .when(!is_current, |d| {
                                            d.bg(theme_hover.surface)
                                             .hover(|style| style.bg(theme_hover.surface_hover))
                                        })
                                        .cursor_pointer()
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                                view.state.update(cx, |state, _cx| {
                                                    state.app.current_queue_index = Some(idx);
                                                    if let Some(path) = state.app.queue[idx]
                                                        .current_track()
                                                        .map(|t| t.path.clone())
                                                    {
                                                        Self::play_track(state, path);
                                                    }
                                                });
                                                cx.notify();
                                            }),
                                        )
                                        .on_mouse_up(
                                            MouseButton::Right,
                                            cx.listener(
                                                move |view, event: &MouseUpEvent, _window, cx| {
                                                    view.state.update(cx, |state, _cx| {
                                                        state.app.current_queue_index = Some(idx);
                                                        state.app.context_menu =
                                                        Some(crate::app::ContextMenuState {
                                                            menu_type:
                                                                crate::app::ContextMenuType::QueueItem,
                                                            position_x: event.position.x.into(),
                                                            position_y: event.position.y.into(),
                                                            item_index: idx,
                                                        });
                                                    });
                                                    cx.notify();
                                                },
                                            ),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .child(
                                                    div()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_sm()
                                                        .text_color(if is_current { theme.text_on_accent } else { theme.text_primary })
                                                        .child(item.album.title.clone()),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(if is_current { theme.text_on_accent_muted } else { theme.text_muted })
                                                        .child(item.album.artist()),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(if is_current { theme.text_on_accent_muted } else { theme.text_secondary })
                                                        .child(format!(
                                                            "Track {}/{}",
                                                            item.current_track_index + 1,
                                                            item.album.tracks.len()
                                                        )),
                                                ),
                                        )
                                })),
                        ),
                )
            })
            // Separator 1 (Queue <-> Center)
            .child(
                div()
                    .w(px(6.0))
                    .h_full()
                    .bg(theme.background)
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_col_resize()
                    .child(
                        div()
                            .w(px(1.0))
                            .h_full()
                            .bg(theme.border)
                    )
                    .on_mouse_down(MouseButton::Left, cx.listener(move |view, _, _, cx| {
                        view.state.update(cx, |state, _| {
                            state.app.is_dragging_queue_list_divider = true;
                            state.app.divider_click_start = Some(std::time::Instant::now());
                        });
                    }))
            )
            // Center panel: Now playing info
            .child(
                self.render_now_playing_info(cx)
            )
            // Separator 2 (Center <-> Right)
            .child(
                div()
                    .w(px(6.0))
                    .h_full()
                    .bg(theme.background)
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_col_resize()
                    .child(
                        div()
                            .w(px(1.0))
                            .h_full()
                            .bg(theme.border)
                    )
                    .on_mouse_down(MouseButton::Left, cx.listener(move |view, _, _, cx| {
                        view.state.update(cx, |state, _| {
                            state.app.is_dragging_meters_divider = true;
                            state.app.divider_click_start = Some(std::time::Instant::now());
                        });
                    }))
            )
            // Right panels: LUFS and Level meters as separate columns
            .when(!meters_collapsed, |d| {
                let theme_sep = theme.clone();
                d.child(
                    div()
                        .w(relative(lufs_ratio))
                        .flex()
                        .flex_col()
                        .h_full()
                        .child(self.render_lufs_panel(cx)),
                )
                .child(
                    div()
                        .w(px(6.0))
                        .h_full()
                        .bg(theme.background)
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_col_resize()
                        .child(div().w(px(1.0)).h_full().bg(theme_sep.border))
                        .on_mouse_down(MouseButton::Left, cx.listener(move |view, _, _, cx| {
                            view.state.update(cx, |state, _| {
                                state.app.is_dragging_lufs_divider = true;
                                state.app.divider_click_start = Some(std::time::Instant::now());
                            });
                        }))
                )
                .child(
                    div()
                        .w(relative(meters_ratio))
                        .flex()
                        .flex_col()
                        .h_full()
                        .child(self.render_meters_panel(cx)),
                )
            })
    }

    // Level meter methods (render_lufs_panel, render_meters_panel, render_meter_group, etc.)
    // are now in ui/components/plugins/level_meters.rs

    /// Render the now playing information panel (center)
    pub(crate) fn render_now_playing_info(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();
        // Clone theme for use in closures (moved into flat_map)
        let theme_for_closure = theme.clone();

        // Get current queue item with all album info
        let queue_item = state
            .app
            .current_queue_index
            .and_then(|idx| state.app.queue.get(idx));

        let content: AnyElement = if let Some(item) = queue_item {
            let album = &item.album;
            let current_track_idx = item.current_track_index;
            let current_track = item.current_track();

            // Get replay gain from current track (or first track with it)
            let replay_gain = current_track
                .and_then(|t| t.replay_gain)
                .or_else(|| album.tracks.iter().find_map(|t| t.replay_gain));

            // Get channel count from current track
            let channels = current_track.and_then(|t| t.channels).unwrap_or(2);

            let album_title = album.title.clone();
            let artist = album.artist();
            let art_path = album.album_art_path.clone();

            // Group tracks by disc number
            let mut disc_map: BTreeMap<u32, Vec<(usize, Track)>> = BTreeMap::new();
            for (idx, track) in album.tracks.iter().enumerate() {
                let disc = track.disc_number.unwrap_or(1);
                disc_map.entry(disc).or_default().push((idx, track.clone()));
            }

            div()
                .flex()
                .flex_col()
                .flex_1()
                .child(
                    div().mb_3().child(
                        Text::new("Now Playing")
                            .size(TextSize::Lg)
                            .weight(TextWeight::Bold)
                            .color(theme.text_primary),
                    ),
                )
                // Top row: Album art (left) + Album info (right)
                .child(
                    div()
                        .flex()
                        .gap_4()
                        .mb_4()
                        // Album art (smaller, top-left)
                        .child({
                            let art_div = div()
                                .w(px(120.0))
                                .h(px(120.0))
                                .bg(theme.surface)
                                .rounded_lg()
                                .overflow_hidden()
                                .flex_shrink_0();

                            if let Some(path) = art_path {
                                art_div.child(
                                    img(path)
                                        .w_full()
                                        .h_full()
                                        .object_fit(gpui::ObjectFit::Cover),
                                )
                            } else {
                                art_div
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(theme.text_muted)
                                    .text_2xl()
                                    .child("♪")
                            }
                        })
                        // Album info (right of art)
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .flex_1()
                                // Album title
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme.text_primary)
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .child(album_title.clone()),
                                )
                                // Artist
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(theme.text_secondary)
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .child(artist),
                                )
                                // Replay Gain
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .mt_2()
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.text_muted)
                                                .child("ReplayGain:"),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(if replay_gain.is_some() {
                                                    theme.text_secondary
                                                } else {
                                                    theme.text_muted
                                                })
                                                .child(
                                                    replay_gain
                                                        .map(|g| format!("{:+.1} dB", g))
                                                        .unwrap_or_else(|| "N/A".to_string()),
                                                ),
                                        ),
                                )
                                // Channels
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.text_muted)
                                                .child("Channels:"),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(theme.text_secondary)
                                                .child(format!("{}", channels)),
                                        ),
                                ),
                        ),
                )
                .child(self.render_track_list(&disc_map, current_track_idx, &theme_for_closure, cx))
                .into_any_element()
        } else {
            VStack::new()
                .spacing(StackSpacing::Sm)
                .child(
                    Text::new("No track playing")
                        .size(TextSize::Lg)
                        .color(theme.text_muted),
                )
                .child(
                    Text::new("Select an album from the queue")
                        .size(TextSize::Sm)
                        .color(theme.text_muted),
                )
                .build()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .flex_1()
            .p_2()
            .border_r_1()
            .border_color(theme.border)
            .bg(theme.background_secondary)
            .child(
                div()
                    .size_full()
                    .id("now-playing-scroll")
                    .overflow_y_scroll()
                    .child(content),
            )
    }

    /// Render the track list with clickable items
    fn render_track_list(
        &self,
        disc_map: &BTreeMap<u32, Vec<(usize, Track)>>,
        current_track_idx: usize,
        theme: &crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let disc_count = disc_map.len();
        let mut all_elements: Vec<AnyElement> = Vec::new();

        for (disc_num, tracks) in disc_map.iter() {
            let show_header = disc_count > 1;

            if show_header {
                all_elements.push(
                    div()
                        .py_1()
                        .mt_2()
                        .mb_1()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.text_secondary)
                        .child(format!("Disc {}", disc_num))
                        .into_any_element(),
                );
            }

            for (idx, track) in tracks.iter() {
                let idx = *idx;
                let title = track.title.clone().unwrap_or_else(|| "Unknown".to_string());
                let duration = track.duration_secs.unwrap_or(0);
                let is_current = idx == current_track_idx;
                let duration_str = format!("{}:{:02}", duration / 60, duration % 60);
                let theme_c = theme.clone();
                let track_path = track.path.clone();

                all_elements.push(
                    div()
                        .id(SharedString::from(format!("track-{}", idx)))
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .py(px(4.0))
                        .rounded(px(4.0))
                        .cursor_pointer()
                        .when(is_current, |d| {
                            d.bg(theme_c.accent).text_color(theme_c.text_on_accent)
                        })
                        .when(!is_current, |d| d.hover(|s| s.bg(theme_c.surface_hover)))
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                let path = track_path.clone();
                                view.state.update(cx, |state, _cx| {
                                    // Update the track index in the current queue item
                                    if let Some(queue_idx) = state.app.current_queue_index {
                                        if let Some(item) = state.app.queue.get_mut(queue_idx) {
                                            item.current_track_index = idx;
                                        }
                                    }
                                    Self::play_track(state, path);
                                });
                                cx.notify();
                            }),
                        )
                        .child(
                            div()
                                .w(px(24.0))
                                .text_xs()
                                .text_color(if is_current {
                                    theme_c.text_on_accent
                                } else {
                                    theme_c.text_muted
                                })
                                .child(format!(
                                    "{}",
                                    track.track_number.unwrap_or((idx + 1) as u32)
                                )),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_sm()
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .when(is_current, |d| d.font_weight(FontWeight::SEMIBOLD))
                                .child(title),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(if is_current {
                                    theme_c.text_on_accent
                                } else {
                                    theme_c.text_muted
                                })
                                .child(duration_str),
                        )
                        .into_any_element(),
                );
            }
        }

        div()
            .flex()
            .flex_col()
            .flex_1()
            .overflow_hidden()
            .child(
                Text::new(format!(
                    "Tracks ({})",
                    disc_map.values().map(|v| v.len()).sum::<usize>()
                ))
                .size(TextSize::Sm)
                .weight(TextWeight::Semibold)
                .color(theme.text_secondary)
                .build()
                .mb_2(),
            )
            .child(
                div()
                    .id("track-list")
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .flex_1()
                    .overflow_y_scroll()
                    .children(all_elements),
            )
    }
}
