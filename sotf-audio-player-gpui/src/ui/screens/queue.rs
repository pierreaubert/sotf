//! Queue screen rendering functions

use crate::ui::components::plugins::{MeterTheme, TickConfig, render_tick_row};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;

/// dB scale positions: maps dB value to visual position (0.0 = bottom, 1.0 = top)
/// Using non-linear scale for better visual representation
fn db_to_position(db: f64) -> f32 {
    // -60dB = 0%, -30dB = 33%, -10dB = 66%, 0dB = 100%
    let normalized = if db <= -60.0 {
        0.0
    } else if db <= -30.0 {
        // -60 to -30: linear from 0 to 0.33
        ((db + 60.0) / 30.0) * 0.33
    } else if db <= -10.0 {
        // -30 to -10: linear from 0.33 to 0.66
        0.33 + ((db + 30.0) / 20.0) * 0.33
    } else {
        // -10 to 0: linear from 0.66 to 1.0
        0.66 + ((db + 10.0) / 10.0) * 0.34
    };
    normalized.clamp(0.0, 1.0) as f32
}


impl PlayerView {
    /// Render a single meter group with M/S/D buttons below the channels
    fn render_meter_group(
        &self,
        group: &crate::app::ChannelGroup,
        group_idx: usize,
        is_selected: bool,
        loudness: Option<&sotf_audio_player::LoudnessData>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let muted = group.muted;
        let soloed = group.soloed;
        let dimmed = group.dimmed;

        // Pre-compute channel data to avoid closure issues with cx
        let channel_data: Vec<_> = group.channels.iter().map(|channel| {
            let peak = loudness
                .and_then(|l| l.channel_peaks.get(channel.index))
                .copied()
                .unwrap_or(0.0);

            let peak_db = if peak > 0.0001 {
                20.0 * peak.log10()
            } else {
                -60.0
            };

            let fill_ratio = db_to_position(peak_db);
            let yellow_threshold = db_to_position(-6.0);
            let red_threshold = db_to_position(-1.0);

            (fill_ratio, yellow_threshold, red_threshold, channel.name.clone())
        }).collect();

        div()
            .flex()
            .flex_col()
            .flex_1()
            .p_2()
            .rounded_md()
            .when(is_selected, |d| d.bg(rgb(0x2d3748)))
            .when(!is_selected, |d| d.bg(rgb(0x252525)))
            // Group header (just the name)
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(0xcccccc))
                    .mb_1()
                    .child(group.name.clone()),
            )
            // Channel meters
            .child(
                div()
                    .flex()
                    .gap_1()
                    .flex_1()
                    .min_h(px(80.0))
                    .children(channel_data.into_iter().map(|(fill_ratio, yellow_threshold, red_threshold, name)| {
                        render_gradient_meter(fill_ratio, yellow_threshold, red_threshold, name)
                    })),
            )
            // M/S/D buttons below channels (spans all channels in group)
            .child(
                div()
                    .flex()
                    .gap(px(2.0))
                    .mt_1()
                    .justify_center()
                    .child(self.render_msd_button("M", muted, rgb(0xdc2626), group_idx, "mute", cx))
                    .child(self.render_msd_button("S", soloed, rgb(0xf59e0b), group_idx, "solo", cx))
                    .child(self.render_msd_button("D", dimmed, rgb(0x6366f1), group_idx, "dim", cx)),
            )
    }

    /// Render M/S/D button (interactive)
    fn render_msd_button(
        &self,
        label: &'static str,
        active: bool,
        active_color: gpui::Rgba,
        group_idx: usize,
        button_type: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(SharedString::from(format!("msd-{}-{}", button_type, group_idx)))
            .px_2()
            .py(px(2.0))
            .rounded(px(2.0))
            .text_xs()
            .cursor_pointer()
            .when(active, |d| d.bg(active_color).text_color(rgb(0xffffff)))
            .when(!active, |d| {
                d.bg(rgb(0x3e3e3e))
                    .text_color(rgb(0x999999))
                    .hover(|style| style.bg(rgb(0x4e4e4e)))
            })
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                    view.state.update(cx, |state, _cx| {
                        if group_idx < state.app.level_meter_groups.len() {
                            match button_type {
                                "mute" => state.app.level_meter_groups[group_idx].muted = !state.app.level_meter_groups[group_idx].muted,
                                "solo" => state.app.level_meter_groups[group_idx].soloed = !state.app.level_meter_groups[group_idx].soloed,
                                "dim" => state.app.level_meter_groups[group_idx].dimmed = !state.app.level_meter_groups[group_idx].dimmed,
                                _ => {}
                            }
                        }
                    });
                    cx.notify();
                }),
            )
            .child(label)
    }
}

/// Render a meter with gradient coloring (green, yellow at top, red at clip)
fn render_gradient_meter(
    fill_ratio: f32,
    yellow_threshold: f32,
    red_threshold: f32,
    channel_name: String,
) -> impl IntoElement {
    // Calculate segment heights
    let green_height = fill_ratio.min(yellow_threshold);
    let yellow_height = if fill_ratio > yellow_threshold {
        (fill_ratio - yellow_threshold).min(red_threshold - yellow_threshold)
    } else {
        0.0
    };
    let red_height = if fill_ratio > red_threshold {
        fill_ratio - red_threshold
    } else {
        0.0
    };

    div()
        .flex()
        .flex_col()
        .items_center()
        .flex_1()
        // Meter bar container
        .child(
            div()
                .w(px(16.0))
                .flex_1()
                .bg(rgb(0x1e1e1e))
                .rounded(px(2.0))
                .overflow_hidden()
                .relative()
                // Green segment (base)
                .child(
                    div()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .h(gpui::Length::Definite(gpui::DefiniteLength::Fraction(green_height)))
                        .bg(rgb(0x22c55e)),
                )
                // Yellow segment (above green)
                .when(yellow_height > 0.001, |el| {
                    el.child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .bottom(gpui::Length::Definite(gpui::DefiniteLength::Fraction(yellow_threshold)))
                            .h(gpui::Length::Definite(gpui::DefiniteLength::Fraction(yellow_height)))
                            .bg(rgb(0xf59e0b)),
                    )
                })
                // Red segment (above yellow)
                .when(red_height > 0.001, |el| {
                    el.child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .bottom(gpui::Length::Definite(gpui::DefiniteLength::Fraction(red_threshold)))
                            .h(gpui::Length::Definite(gpui::DefiniteLength::Fraction(red_height)))
                            .bg(rgb(0xdc2626)),
                    )
                }),
        )
        // Channel name
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x999999))
                .mt_1()
                .child(channel_name),
        )
}

impl PlayerView {
    pub(crate) fn render_queue_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();

        div()
            .flex()
            .size_full()
            .child(
                // Left panel: Queue list (narrower)
                div()
                    .flex()
                    .flex_col()
                    .w(px(300.0))
                    .p_4()
                    .border_r_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .mb_4()
                            .flex()
                            .justify_between()
                            .items_center()
                            .child(format!("Queue ({} albums)", state.app.queue.len()))
                            .child(
                                div()
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .bg(theme.surface)
                                    .hover(|style| style.bg(rgb(0x8e2e2e)))
                                    .cursor_pointer()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                            view.state.update(cx, |state, _cx| {
                                                state.app.clear_queue();
                                            });
                                            cx.notify();
                                        }),
                                    )
                                    .child("Clear"),
                            ),
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
                                div()
                                    .p_3()
                                    .rounded_md()
                                    .when(is_current, |d| d.bg(theme.accent))
                                    .when(!is_current, |d| d.bg(theme.surface))
                                    .hover(|style| style.bg(theme.surface_hover))
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
                                        cx.listener(move |view, event: &MouseUpEvent, _window, cx| {
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
                                        }),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_sm()
                                                    .child(item.album.title.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(theme.text_muted)
                                                    .child(item.album.artist()),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(theme.text_secondary)
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
            .child(
                // Center panel: Now playing info
                self.render_now_playing_info(cx),
            )
            .child(
                // Right panel: Level meters with LUFS (no volume - it's in footer now)
                self.render_level_meters(cx),
            )
    }

    /// Render the now playing information panel (center)
    pub(crate) fn render_now_playing_info(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();

        // Get current queue item with all album info
        let queue_item = state
            .app
            .current_queue_index
            .and_then(|idx| state.app.queue.get(idx));

        let has_content = queue_item.is_some();

        div()
            .flex()
            .flex_col()
            .flex_1()
            .p_4()
            .border_r_1()
            .border_color(theme.border)
            .bg(theme.background_secondary)
            .when(has_content, |d| {
                let item = queue_item.unwrap();
                let album = &item.album;
                let current_track_idx = item.current_track_index;
                let current_track = item.current_track();

                // Get replay gain from current track (or first track with it)
                let replay_gain = current_track
                    .and_then(|t| t.replay_gain)
                    .or_else(|| album.tracks.iter().find_map(|t| t.replay_gain));

                // Get channel count from current track
                let channels = current_track
                    .and_then(|t| t.channels)
                    .unwrap_or(2);

                let album_title = album.title.clone();
                let artist = album.artist();
                let art_path = album.album_art_path.clone();
                let tracks: Vec<_> = album.tracks.iter().enumerate().map(|(idx, track)| {
                    let title = track.title.clone().unwrap_or_else(|| "Unknown".to_string());
                    let duration = track.duration_secs.unwrap_or(0);
                    let is_current = idx == current_track_idx;
                    (idx, title, duration, is_current)
                }).collect();

                d.child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .mb_3()
                        .child("Now Playing"),
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
                                        .object_fit(gpui::ObjectFit::Cover)
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
                                        .child(album_title),
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
                                                .text_color(if replay_gain.is_some() { theme.text_secondary } else { theme.text_muted })
                                                .child(
                                                    replay_gain
                                                        .map(|g| format!("{:+.1} dB", g))
                                                        .unwrap_or_else(|| "N/A".to_string())
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
                // Track list (scrollable)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .overflow_hidden()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text_secondary)
                                .mb_2()
                                .child(format!("Tracks ({})", tracks.len())),
                        )
                        .child(
                            div()
                                .id("track-list")
                                .flex()
                                .flex_col()
                                .gap(px(2.0))
                                .flex_1()
                                .overflow_y_scroll()
                                .children(tracks.into_iter().map(|(idx, title, duration, is_current)| {
                                    let duration_str = format!("{}:{:02}", duration / 60, duration % 60);
                                    let theme_c = theme.clone();

                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .px_2()
                                        .py(px(4.0))
                                        .rounded(px(4.0))
                                        .when(is_current, |d| {
                                            d.bg(theme_c.accent)
                                                .text_color(rgb(0xffffff))
                                        })
                                        .when(!is_current, |d| {
                                            d.hover(|s| s.bg(theme_c.surface_hover))
                                        })
                                        // Track number
                                        .child(
                                            div()
                                                .w(px(24.0))
                                                .text_xs()
                                                .text_color(if is_current { rgb(0xffffff) } else { theme_c.text_muted })
                                                .child(format!("{}", idx + 1)),
                                        )
                                        // Track title
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
                                        // Duration
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(if is_current { rgb(0xffffff) } else { theme_c.text_muted })
                                                .child(duration_str),
                                        )
                                })),
                        ),
                )
            })
            .when(!has_content, |d| {
                d.child(
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .flex_col()
                        .gap_2()
                        .text_color(theme.text_muted)
                        .child("No track playing")
                        .child(
                            div()
                                .text_sm()
                                .child("Select an album from the queue"),
                        ),
                )
            })
    }

    /// Render the graphical level meters panel with LUFS display
    pub(crate) fn render_level_meters(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (theme, loudness, groups, selected_group) = {
            let state = self.state.read(cx);
            (
                state.app.theme.clone(),
                state.app.loudness_info.clone(),
                state.app.level_meter_groups.clone(),
                state.app.selected_level_meter_group,
            )
        };

        let has_groups = !groups.is_empty();

        div()
            .w(px(320.0))
            .flex()
            .flex_col()
            .p_4()
            .bg(rgb(0x1e1e1e))
            // LUFS section on TOP (40% height) with True Peak
            .child(
                div()
                    .flex()
                    .flex_col()
                    .h(gpui::Length::Definite(gpui::DefiniteLength::Fraction(0.4)))
                    .pb_3()
                    .border_b_1()
                    .border_color(rgb(0x3e3e3e))
                    .child(self.render_lufs_with_true_peak(loudness.as_ref(), &theme)),
            )
            // Level meters section (60% height)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .pt_3()
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .mb_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Level Meters"),
                            )
                            .child(
                                div()
                                    .px_2()
                                    .py(px(2.0))
                                    .rounded(px(3.0))
                                    .text_xs()
                                    .bg(rgb(0x3a3a3a))
                                    .text_color(rgb(0xcccccc))
                                    .cursor_pointer()
                                    .hover(|style| style.bg(rgb(0x4a4a4a)))
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                            view.state.update(cx, |state, _cx| {
                                                for group in &mut state.app.level_meter_groups {
                                                    group.muted = false;
                                                    group.soloed = false;
                                                    group.dimmed = false;
                                                }
                                            });
                                            cx.notify();
                                        }),
                                    )
                                    .child("Clear All"),
                            ),
                    )
                    .when(!has_groups, |d| {
                        d.child(
                            gpui::div()
                                .flex_1()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(rgb(0x666666))
                                .text_sm()
                                .child("No audio playing"),
                        )
                    })
                    .when(has_groups, |d| {
                        d.child(self.render_meters_with_legend(loudness.as_ref(), &groups, selected_group, cx))
                    }),
            )
            // Keyboard hints at bottom
            .child(
                div()
                    .mt_auto()
                    .pt_2()
                    .text_xs()
                    .text_color(rgb(0x666666))
                    .child("Tab: Select group | M: Mute | S: Solo"),
            )
    }

    /// Render meters with vertical legend on sides with dotted grid lines
    fn render_meters_with_legend(
        &self,
        loudness: Option<&sotf_audio_player::LoudnessData>,
        groups: &[crate::app::ChannelGroup],
        selected_group: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let groups_len = groups.len();

        // Build meter group children
        let mut meter_children = Vec::new();
        for (group_idx, group) in groups.iter().enumerate() {
            let is_selected = group_idx == selected_group;
            meter_children.push(self.render_meter_group(group, group_idx, is_selected, loudness, cx).into_any_element());
        }

        div()
            .flex()
            .flex_1()
            .gap_1()
            // Left scale legend - aligned with meter group box edges
            .child(
                div()
                    .flex()
                    .flex_col()
                    .justify_between()
                    .w(px(28.0))
                    .text_xs()
                    .text_color(rgb(0x666666))
                    .child(div().child("  0"))
                    .child(div().child("-60")),
            )
            // Meters area
            .child(
                div()
                    .flex()
                    .flex_1()
                    .gap_2()
                    .children(meter_children),
            )
            // Right scale legend (if more than 2 groups)
            .when(groups_len > 2, |d| {
                d.child(
                    div()
                        .flex()
                        .flex_col()
                        .justify_between()
                        .w(px(28.0))
                        .text_xs()
                        .text_color(rgb(0x666666))
                        .child(div().child("0"))
                        .child(div().child("-60")),
                )
            })
    }

    /// Render unified meter bar with consistent styling
    /// Uses the TickConfig's scale for bar fill to match tick mark positions
    fn render_meter_bar(
        label: &str,
        value: f64,
        tick_config: &TickConfig,
        meter_theme: &MeterTheme,
    ) -> impl IntoElement {
        // Use the same scale as the ticks for bar fill
        let ratio = tick_config.value_to_position(value);
        let bar_color = meter_theme.color_for_ratio(ratio);

        div()
            .flex()
            .items_center()
            .gap(px(4.0))  // Tighter gap for more bar space
            // Label
            .child(
                div()
                    .w(px(meter_theme.label_width))
                    .text_xs()
                    .text_color(meter_theme.color_text)
                    .child(label.to_string()),
            )
            // Bar with border
            .child(
                div()
                    .flex_1()
                    .h(px(meter_theme.bar_height))
                    .rounded(px(meter_theme.border_radius))
                    .border(px(meter_theme.border_width))
                    .border_color(meter_theme.color_border)
                    .bg(meter_theme.color_background)
                    .overflow_hidden()
                    .child(
                        div()
                            .h_full()
                            .w(gpui::relative(ratio))
                            .bg(bar_color),
                    ),
            )
            // Value display
            .child(
                div()
                    .w(px(meter_theme.value_width))
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(meter_theme.color_text)
                    .text_align(TextAlign::Right)
                    .child(format!("{:.1}", value)),
            )
    }

    /// Render LUFS display with True Peak bars at top
    fn render_lufs_with_true_peak(
        &self,
        loudness: Option<&sotf_audio_player::LoudnessData>,
        theme: &crate::theme::Theme,
    ) -> impl IntoElement {
        let (integrated_lufs, shortterm_lufs, momentary_lufs, true_peak_left, true_peak_right, stereo_width) =
            if let Some(l) = loudness {
                let tp_left = l.true_peaks_dbtp.first().copied().unwrap_or(-60.0);
                let tp_right = l.true_peaks_dbtp.get(1).copied().unwrap_or(tp_left);
                // Stereo width derived from correlation: +1 = mono (0), 0 = uncorrelated (0.5), -1 = out of phase (1)
                let width = l.correlation_lr
                    .map(|c| ((1.0 - c) / 2.0).clamp(0.0, 1.0))
                    .unwrap_or(0.5);
                (
                    l.integrated_lufs,
                    l.shortterm_lufs,
                    l.momentary_lufs,
                    tp_left,
                    tp_right,
                    width,
                )
            } else {
                (-60.0, -60.0, -60.0, -60.0, -60.0, 0.5)
            };

        let meter_theme = MeterTheme::default();

        div()
            .flex()
            .flex_col()
            .gap_4()
            .p_3()
            // True Peak section (on top)
            .child({
                // Use TickConfig preset for True Peak (quadratic scale from -60 to +6)
                let tick_config = TickConfig::true_peak();

                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .mb_1()
                            .child("True Peak"),
                    )
                    // Left channel bar (uses same scale as ticks)
                    .child(Self::render_meter_bar(
                        "L",
                        true_peak_left,
                        &tick_config,
                        &meter_theme,
                    ))
                    // Right channel bar (uses same scale as ticks)
                    .child(Self::render_meter_bar(
                        "R",
                        true_peak_right,
                        &tick_config,
                        &meter_theme,
                    ))
                    // Tick marks (aligned with bar using same flex layout)
                    .child(render_tick_row(
                        &tick_config,
                        meter_theme.label_width,
                        meter_theme.value_width,
                    ))
                    // True Peak legend (same flex layout as bar and ticks)
                    .child(
                        div()
                            .flex()
                            .gap(px(4.0))
                            // Label spacer
                            .child(div().w(px(meter_theme.label_width)))
                            // Legend area (flex-1, justify_between for labels)
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .justify_between()
                                    .text_xs()
                                    .text_color(meter_theme.color_text_muted)
                                    .children(tick_config.major_values.iter().map(|db| {
                                        let label = if *db > 0.0 {
                                            format!("+{}", *db as i32)
                                        } else {
                                            format!("{}", *db as i32)
                                        };
                                        div().child(label)
                                    })),
                            )
                            // Value spacer
                            .child(div().w(px(meter_theme.value_width))),
                    )
            })
            // LUFS section (below)
            .child({
                // Use TickConfig preset for LUFS (quadratic scale from -60 to 0)
                let tick_config = TickConfig::lufs();

                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .mb_1()
                            .child("LUFS"),
                    )
                    // Integrated LUFS (uses same scale as ticks)
                    .child(Self::render_meter_bar(
                        "I",
                        integrated_lufs,
                        &tick_config,
                        &meter_theme,
                    ))
                    // Short-term LUFS (uses same scale as ticks)
                    .child(Self::render_meter_bar(
                        "S",
                        shortterm_lufs,
                        &tick_config,
                        &meter_theme,
                    ))
                    // Momentary LUFS (uses same scale as ticks)
                    .child(Self::render_meter_bar(
                        "M",
                        momentary_lufs,
                        &tick_config,
                        &meter_theme,
                    ))
                    // Tick marks (aligned with bar using same flex layout)
                    .child(render_tick_row(
                        &tick_config,
                        meter_theme.label_width,
                        meter_theme.value_width,
                    ))
                    // LUFS legend (same flex layout as bar and ticks)
                    .child(
                        div()
                            .flex()
                            .gap(px(4.0))
                            // Label spacer
                            .child(div().w(px(meter_theme.label_width)))
                            // Legend area (flex-1, justify_between for labels)
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .justify_between()
                                    .text_xs()
                                    .text_color(meter_theme.color_text_muted)
                                    .child(div().child("-60"))
                                    .child(div().child("-30"))
                                    .child(div().child("-10"))
                                    .child(div().child("0")),
                            )
                            // Value spacer
                            .child(div().w(px(meter_theme.value_width))),
                    )
            })
            // Stereo Width section
            .child({
                // Use TickConfig preset for stereo width (linear scale 0 to 1)
                let tick_config = TickConfig::stereo_width();

                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .mb_1()
                            .child("Stereo Width"),
                    )
                    // Width bar (uses same scale as ticks)
                    .child(Self::render_width_bar(stereo_width, &tick_config, &meter_theme))
                    // Tick marks (aligned with bar using same flex layout)
                    .child(render_tick_row(
                        &tick_config,
                        meter_theme.label_width,
                        meter_theme.value_width,
                    ))
                    // Width legend (same flex layout as bar and ticks)
                    .child(
                        div()
                            .flex()
                            .gap(px(4.0))
                            // Label spacer
                            .child(div().w(px(meter_theme.label_width)))
                            // Legend area (flex-1, justify_between for labels)
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .justify_between()
                                    .text_xs()
                                    .text_color(meter_theme.color_text_muted)
                                    .child(div().child("Mono"))
                                    .child(div().child("50%"))
                                    .child(div().child("Wide")),
                            )
                            // Value spacer
                            .child(div().w(px(meter_theme.value_width))),
                    )
            })
    }

    /// Render stereo width bar (0 = mono, 1 = wide)
    /// Uses the TickConfig's scale for bar fill to match tick mark positions
    fn render_width_bar(width: f64, tick_config: &TickConfig, meter_theme: &MeterTheme) -> impl IntoElement {
        // Use the same scale as the ticks for bar fill
        let ratio = tick_config.value_to_position(width);
        // Color: cyan/teal for width
        let bar_color = rgb(0x06b6d4);

        div()
            .flex()
            .items_center()
            .gap(px(4.0))
            // Label
            .child(
                div()
                    .w(px(meter_theme.label_width))
                    .text_xs()
                    .text_color(meter_theme.color_text)
                    .child("W"),
            )
            // Bar with border
            .child(
                div()
                    .flex_1()
                    .h(px(meter_theme.bar_height))
                    .rounded(px(meter_theme.border_radius))
                    .border(px(meter_theme.border_width))
                    .border_color(meter_theme.color_border)
                    .bg(meter_theme.color_background)
                    .overflow_hidden()
                    .child(
                        div()
                            .h_full()
                            .w(gpui::relative(ratio))
                            .bg(bar_color),
                    ),
            )
            // Value display
            .child(
                div()
                    .w(px(meter_theme.value_width))
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(meter_theme.color_text)
                    .text_align(TextAlign::Right)
                    .child(format!("{:.0}%", width * 100.0)),
            )
    }

    /// Render LUFS display (like TUI player) - DEPRECATED, use render_lufs_with_true_peak instead
    fn render_lufs_display(
        &self,
        loudness: Option<&sotf_audio_player::LoudnessData>,
        theme: &crate::theme::Theme,
    ) -> impl IntoElement {
        let (integrated_lufs, shortterm_lufs, momentary_lufs, true_peak_dbtp, stereo_width) =
            if let Some(l) = loudness {
                (
                    l.integrated_lufs,
                    l.shortterm_lufs,
                    l.momentary_lufs,
                    l.true_peaks_dbtp.first().copied().unwrap_or(-60.0),
                    // Calculate stereo width from channel correlation if available
                    0.5, // Placeholder - would need actual calculation
                )
            } else {
                (-60.0, -60.0, -60.0, -60.0, 0.5)
            };

        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text_primary)
                    .child("LUFS"),
            )
            // Integrated LUFS (main loudness)
            .child(
                self.render_lufs_row("Integrated", integrated_lufs, theme.accent.into(), -24.0, true),
            )
            // Short-term LUFS
            .child(
                self.render_lufs_row("Short-term", shortterm_lufs, theme.success.into(), -24.0, false),
            )
            // Momentary LUFS
            .child(
                self.render_lufs_row("Momentary", momentary_lufs, theme.progress_bar_fill.into(), -24.0, false),
            )
            // True Peak
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .text_xs()
                    .child(
                        div()
                            .text_color(rgb(0x999999))
                            .child("True Peak"),
                    )
                    .child(
                        div()
                            .when(true_peak_dbtp > -1.0, |d| d.text_color(rgb(0xf59e0b)))
                            .when(true_peak_dbtp > 0.0, |d| d.text_color(rgb(0xdc2626)))
                            .when(true_peak_dbtp <= -1.0, |d| d.text_color(theme.text_secondary))
                            .child(format!("{:.1} dBTP", true_peak_dbtp)),
                    ),
            )
            // Stereo Width indicator
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .mt_2()
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .text_xs()
                            .child(div().text_color(rgb(0x999999)).child("Width"))
                            .child(div().text_color(theme.text_secondary).child(format!("{:.0}%", stereo_width * 100.0))),
                    )
                    .child(
                        div()
                            .h(px(4.0))
                            .bg(rgb(0x333333))
                            .rounded_full()
                            .overflow_hidden()
                            .child(
                                div()
                                    .h_full()
                                    .w(gpui::Length::Definite(gpui::DefiniteLength::Fraction(
                                        stereo_width as f32,
                                    )))
                                    .bg(theme.accent)
                                    .rounded_full(),
                            ),
                    ),
            )
    }

    /// Render a single LUFS row with bar
    fn render_lufs_row(
        &self,
        label: &'static str,
        value: f64,
        color: gpui::Hsla,
        target: f64,
        is_main: bool,
    ) -> impl IntoElement {
        // Map -60 to 0 LUFS as 0% to 100%
        let ratio = if value.is_finite() {
            ((value + 60.0) / 60.0).clamp(0.0, 1.0)
        } else {
            0.0
        } as f32;

        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .text_xs()
                    .child(
                        div()
                            .text_color(rgb(0x999999))
                            .child(label),
                    )
                    .child(
                        div()
                            .when(is_main, |d| d.font_weight(FontWeight::BOLD))
                            .text_color(if is_main {
                                color
                            } else {
                                gpui::Hsla::from(rgb(0xcccccc))
                            })
                            .child(format!("{:.1} LUFS", value)),
                    ),
            )
            .child(
                div()
                    .h(px(if is_main { 6.0 } else { 4.0 }))
                    .bg(rgb(0x333333))
                    .rounded_full()
                    .overflow_hidden()
                    .child(
                        div()
                            .h_full()
                            .w(gpui::Length::Definite(gpui::DefiniteLength::Fraction(ratio)))
                            .bg(color)
                            .rounded_full(),
                    ),
            )
    }
}
