//! Queue screen rendering functions

use crate::app::AppState;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;

impl PlayerView {
    pub(crate) fn render_queue_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        div()
            .flex()
            .size_full()
            .child(
                // Left panel: Queue list
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .p_4()
                    .border_r_1()
                    .border_color(rgb(0x3e3e3e))
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
                                    .bg(rgb(0x2d2d2d))
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
                                div()
                                    .p_3()
                                    .rounded_md()
                                    .when(is_current, |div| div.bg(rgb(0x007acc)))
                                    .when(!is_current, |div| div.bg(rgb(0x2d2d2d)))
                                    .hover(|style| style.bg(rgb(0x3e3e3e)))
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
                                                    .child(item.album.title.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(rgb(0x999999))
                                                    .child(item.album.artist.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(rgb(0x666666))
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
                // Right panel: Level meters
                self.render_level_meters(cx),
            )
    }

    /// Render the graphical level meters panel
    pub(crate) fn render_level_meters(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        // Get loudness data for peak levels
        let loudness = state.app.loudness_info.as_ref();
        let groups = &state.app.level_meter_groups;
        let selected_group = state.app.selected_level_meter_group;

        div()
            .w(px(280.0))
            .flex()
            .flex_col()
            .p_4()
            .bg(rgb(0x1e1e1e))
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .mb_4()
                    .child("Level Meters"),
            )
            .when(groups.is_empty(), |d| {
                d.child(
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(rgb(0x666666))
                        .child("No audio playing"),
                )
            })
            .when(!groups.is_empty(), |d| {
                d.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .flex_1()
                        .children(groups.iter().enumerate().map(|(group_idx, group)| {
                            let is_selected = group_idx == selected_group;

                            div()
                                .p_3()
                                .rounded_md()
                                .when(is_selected, |div| div.bg(rgb(0x2d3748)))
                                .when(!is_selected, |div| div.bg(rgb(0x252525)))
                                .flex()
                                .flex_col()
                                .gap_2()
                                // Group header with name and M/S/D controls
                                .child(
                                    div()
                                        .flex()
                                        .justify_between()
                                        .items_center()
                                        .child(
                                            div()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_sm()
                                                .child(group.name.clone()),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .gap_1()
                                                // Mute button
                                                .child(
                                                    div()
                                                        .px_2()
                                                        .py(px(2.0))
                                                        .rounded(px(4.0))
                                                        .text_xs()
                                                        .when(group.muted, |d| {
                                                            d.bg(rgb(0xdc2626))
                                                                .text_color(rgb(0xffffff))
                                                        })
                                                        .when(!group.muted, |d| {
                                                            d.bg(rgb(0x3e3e3e))
                                                                .text_color(rgb(0x999999))
                                                        })
                                                        .cursor_pointer()
                                                        .on_mouse_up(
                                                            MouseButton::Left,
                                                            cx.listener(
                                                                move |view,
                                                                      _: &MouseUpEvent,
                                                                      _window,
                                                                      cx| {
                                                                    view.state.update(
                                                                        cx,
                                                                        |state, _cx| {
                                                                            state
                                                                                .app
                                                                                .selected_level_meter_group =
                                                                                group_idx;
                                                                            state
                                                                                .app
                                                                                .toggle_level_meter_mute();
                                                                        },
                                                                    );
                                                                    cx.notify();
                                                                },
                                                            ),
                                                        )
                                                        .child("M"),
                                                )
                                                // Solo button
                                                .child(
                                                    div()
                                                        .px_2()
                                                        .py(px(2.0))
                                                        .rounded(px(4.0))
                                                        .text_xs()
                                                        .when(group.soloed, |d| {
                                                            d.bg(rgb(0xf59e0b))
                                                                .text_color(rgb(0x000000))
                                                        })
                                                        .when(!group.soloed, |d| {
                                                            d.bg(rgb(0x3e3e3e))
                                                                .text_color(rgb(0x999999))
                                                        })
                                                        .cursor_pointer()
                                                        .on_mouse_up(
                                                            MouseButton::Left,
                                                            cx.listener(
                                                                move |view,
                                                                      _: &MouseUpEvent,
                                                                      _window,
                                                                      cx| {
                                                                    view.state.update(
                                                                        cx,
                                                                        |state, _cx| {
                                                                            state
                                                                                .app
                                                                                .selected_level_meter_group =
                                                                                group_idx;
                                                                            state
                                                                                .app
                                                                                .toggle_level_meter_solo();
                                                                        },
                                                                    );
                                                                    cx.notify();
                                                                },
                                                            ),
                                                        )
                                                        .child("S"),
                                                )
                                                // Dim button
                                                .child(
                                                    div()
                                                        .px_2()
                                                        .py(px(2.0))
                                                        .rounded(px(4.0))
                                                        .text_xs()
                                                        .when(group.dimmed, |d| {
                                                            d.bg(rgb(0x6366f1))
                                                                .text_color(rgb(0xffffff))
                                                        })
                                                        .when(!group.dimmed, |d| {
                                                            d.bg(rgb(0x3e3e3e))
                                                                .text_color(rgb(0x999999))
                                                        })
                                                        .cursor_pointer()
                                                        .on_mouse_up(
                                                            MouseButton::Left,
                                                            cx.listener(
                                                                move |view,
                                                                      _: &MouseUpEvent,
                                                                      _window,
                                                                      cx| {
                                                                    view.state.update(
                                                                        cx,
                                                                        |state, _cx| {
                                                                            state
                                                                                .app
                                                                                .selected_level_meter_group =
                                                                                group_idx;
                                                                            state
                                                                                .app
                                                                                .toggle_level_meter_dim();
                                                                        },
                                                                    );
                                                                    cx.notify();
                                                                },
                                                            ),
                                                        )
                                                        .child("D"),
                                                ),
                                        ),
                                )
                                // Channel meters
                                .child(
                                    div()
                                        .flex()
                                        .gap_2()
                                        .h(px(100.0))
                                        .children(group.channels.iter().map(|channel| {
                                            // Get peak level for this channel
                                            let peak = loudness
                                                .and_then(|l| l.channel_peaks.get(channel.index))
                                                .copied()
                                                .unwrap_or(0.0);

                                            // Convert to dB: 20 * log10(peak)
                                            let peak_db = if peak > 0.0001 {
                                                20.0 * peak.log10()
                                            } else {
                                                -60.0
                                            };

                                            // Fill ratio: -60dB = 0%, 0dB = 100%
                                            let fill_ratio =
                                                ((peak_db + 60.0) / 60.0).clamp(0.0, 1.0) as f32;

                                            // Color based on level
                                            let color = if fill_ratio > 0.95 {
                                                rgb(0xdc2626) // Red - clipping
                                            } else if fill_ratio > 0.85 {
                                                rgb(0xf59e0b) // Yellow - warning
                                            } else {
                                                rgb(0x22c55e) // Green - normal
                                            };

                                            div()
                                                .flex()
                                                .flex_col()
                                                .items_center()
                                                .gap_1()
                                                .flex_1()
                                                // Meter bar container
                                                .child(
                                                    div()
                                                        .w(px(24.0))
                                                        .flex_1()
                                                        .bg(rgb(0x1e1e1e))
                                                        .rounded(px(2.0))
                                                        .overflow_hidden()
                                                        .flex()
                                                        .flex_col()
                                                        .justify_end()
                                                        .child(
                                                            div()
                                                                .w_full()
                                                                .h(gpui::Length::Definite(
                                                                    gpui::DefiniteLength::Fraction(
                                                                        fill_ratio,
                                                                    ),
                                                                ))
                                                                .bg(color),
                                                        ),
                                                )
                                                // Channel name
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(rgb(0x999999))
                                                        .child(channel.name.clone()),
                                                )
                                                // dB value
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(rgb(0x666666))
                                                        .child(format!("{:.0}", peak_db)),
                                                )
                                        })),
                                )
                        })),
                )
            })
            // Keyboard hints at bottom
            .child(
                div()
                    .mt_auto()
                    .pt_4()
                    .border_t_1()
                    .border_color(rgb(0x3e3e3e))
                    .text_xs()
                    .text_color(rgb(0x666666))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child("Tab/Shift-Tab: Select group")
                    .child("M: Mute | Shift-M: Solo | Ctrl-M: Dim")
                    .child("X: Clear all"),
            )
    }
}
