//! Room EQ wizard — Step 2: Delay.
//!
//! Shows a table of per-channel alignment delays with editable delay
//! values. Data comes from the Recording wizard's Probe step (persisted
//! in the session file as `probe_results`) or from a previous live
//! measurement stored in `DelayDetectionState`. The user can override
//! any delay value before advancing to Configure.

use crate::app::types::room_eq::DelayDetectionStatus;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{
    Card, HStack, Input, StackAlign, StackSpacing, Text, TextSize, TextWeight, VStack,
};

impl PlayerView {
    pub(crate) fn render_room_eq_delay_detection(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.ui_state.theme.clone();
        let dd = &state.app.measurement_state.room_eq_state.delay_detection;
        let has_results =
            dd.results.is_some() && matches!(dd.status, DelayDetectionStatus::Complete);

        let mut content = VStack::new()
            .spacing(StackSpacing::Md)
            .child(
                Text::new("Per-Channel Alignment Delays")
                    .weight(TextWeight::Bold)
                    .size(TextSize::Md),
            )
            .child(
                Text::new(
                    "Delays are auto-fed into the optimizer. Edit any delay value \
                     to override, or leave as-is.",
                )
                .size(TextSize::Xs)
                .color(theme.text_secondary),
            );

        if has_results {
            let results = dd.results.as_ref().unwrap();
            let live_align = dd.edited_alignment_delays_ms();
            let mut has_low_delay = false;

            // Header row.
            // intentional: fixed pixel column widths below (80/90/120) form a
            // tabular layout for the per-channel delay table and should not
            // scale with font zoom.
            let header = HStack::new()
                .spacing(StackSpacing::Md)
                .align(StackAlign::Center)
                .child(
                    div().w(px(80.0)).child(
                        Text::new("Channel")
                            .weight(TextWeight::Bold)
                            .size(TextSize::Xs)
                            .color(theme.text_primary),
                    ),
                )
                .child(
                    div().w(px(90.0)).child(
                        Text::new("Arrival (ms)")
                            .weight(TextWeight::Bold)
                            .size(TextSize::Xs)
                            .color(theme.text_primary),
                    ),
                )
                .child(
                    div().w(px(80.0)).child(
                        Text::new("Gain (dB)")
                            .weight(TextWeight::Bold)
                            .size(TextSize::Xs)
                            .color(theme.text_primary),
                    ),
                )
                .child(
                    div().w(px(80.0)).child(
                        Text::new("SNR (dB)")
                            .weight(TextWeight::Bold)
                            .size(TextSize::Xs)
                            .color(theme.text_primary),
                    ),
                )
                .child(
                    div().w(px(120.0)).child(
                        Text::new("Delay (ms)")
                            .weight(TextWeight::Bold)
                            .size(TextSize::Xs)
                            .color(theme.text_primary),
                    ),
                );

            let mut rows = VStack::new().spacing(StackSpacing::Xs).child(header);

            for (i, ch) in results.channels.iter().enumerate() {
                let snr_color = if ch.snr_db > 10.0 {
                    theme.success
                } else if ch.snr_db > 0.0 {
                    theme.accent
                } else {
                    theme.error
                };
                let arrival = dd
                    .edited_arrival_ms
                    .get(i)
                    .copied()
                    .unwrap_or(ch.arrival_ms);
                let align = live_align.get(i).copied().unwrap_or(0.0);
                let low = align > 0.0 && align < 0.3;
                if low {
                    has_low_delay = true;
                }

                let view = cx.entity().clone();
                let row =
                    HStack::new()
                        .spacing(StackSpacing::Md)
                        .align(StackAlign::Center)
                        .child(
                            div().w(px(80.0)).child(
                                Text::new(ch.channel_name.clone())
                                    .size(TextSize::Xs)
                                    .weight(TextWeight::Semibold)
                                    .color(theme.text_primary),
                            ),
                        )
                        .child(
                            div().w(px(90.0)).child(
                                Text::new(format!("{:.2}", arrival))
                                    .size(TextSize::Xs)
                                    .color(theme.text_secondary),
                            ),
                        )
                        .child(
                            div().w(px(80.0)).child(
                                Text::new(format!("{:+.1}", ch.gain_db))
                                    .size(TextSize::Xs)
                                    .color(theme.text_secondary),
                            ),
                        )
                        .child(
                            div().w(px(80.0)).child(
                                Text::new(format!("{:+.1}", ch.snr_db))
                                    .size(TextSize::Xs)
                                    .weight(TextWeight::Semibold)
                                    .color(snr_color),
                            ),
                        )
                        .child(
                            HStack::new()
                                .spacing(StackSpacing::Xs)
                                .align(StackAlign::Center)
                                .child(
                                    div()
                                        .w(px(80.0))
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation();
                                        })
                                        .child(
                                            Input::new(SharedString::from(format!(
                                                "delay_input_{}",
                                                i
                                            )))
                                            .value(format!("{:.2}", align))
                                            .placeholder("0.00")
                                            .on_text_change({
                                                let view = view.clone();
                                                move |value, _, cx| {
                                                    let parsed = value
                                                        .trim()
                                                        .parse::<f64>()
                                                        .unwrap_or(0.0)
                                                        .max(0.0);
                                                    // Convert alignment delay back to
                                                    // arrival time: arrival = max_arrival - delay.
                                                    // But we don't know max_arrival here easily.
                                                    // Instead, store the raw arrival and let
                                                    // `edited_alignment_delays_ms()` recompute.
                                                    // For now: directly set arrival_ms[i] by
                                                    // working backward from the desired delay.
                                                    view.update(cx, |this, cx| {
                                                        this.state.update(cx, |state, _| {
                                                            let dd = &mut state
                                                                .app
                                                                .measurement_state
                                                                .room_eq_state
                                                                .delay_detection;
                                                            if let Some(results) =
                                                                dd.results.as_ref()
                                                            {
                                                                // max_arrival across all channels
                                                                let max_arrival = results
                                                                    .channels
                                                                    .iter()
                                                                    .enumerate()
                                                                    .map(|(j, c)| {
                                                                        dd.edited_arrival_ms
                                                                            .get(j)
                                                                            .copied()
                                                                            .unwrap_or(c.arrival_ms)
                                                                    })
                                                                    .fold(
                                                                        f64::NEG_INFINITY,
                                                                        f64::max,
                                                                    );
                                                                // arrival = max_arrival - delay
                                                                let new_arrival =
                                                                    max_arrival - parsed;
                                                                if i < dd.edited_arrival_ms.len() {
                                                                    dd.edited_arrival_ms[i] =
                                                                        new_arrival.max(0.0);
                                                                }
                                                            }
                                                        });
                                                        cx.notify();
                                                    });
                                                }
                                            }),
                                        ),
                                )
                                .when(low, |el| {
                                    el.child(Text::new("⚠").size(TextSize::Xs).color(theme.warning))
                                }),
                        );

                rows = rows.child(row);
            }

            content = content.child(
                Card::new()
                    .background(theme.surface)
                    .border(theme.border)
                    .content(rows),
            );

            if has_low_delay {
                content = content.child(
                    Text::new(
                        "⚠ Delays < 0.3 ms have negligible audible impact — \
                         consider setting to 0.",
                    )
                    .size(TextSize::Xs)
                    .color(theme.warning),
                );
            }
        } else {
            content = content.child(
                Card::new()
                    .background(theme.surface)
                    .border(theme.border)
                    .content(
                        VStack::new()
                            .spacing(StackSpacing::Sm)
                            .child(
                                Text::new("No delay data available")
                                    .weight(TextWeight::Semibold)
                                    .size(TextSize::Sm)
                                    .color(theme.text_secondary),
                            )
                            .child(
                                Text::new(
                                    "Run the Probe step in the Recording wizard to \
                                     capture per-channel delays, or enter values manually \
                                     after loading a file that contains probe results.",
                                )
                                .size(TextSize::Xs)
                                .color(theme.text_muted),
                            ),
                    ),
            );
        }

        content
    }
}
