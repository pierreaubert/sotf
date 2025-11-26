//! Devices screen rendering functions

use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;

impl PlayerView {
    pub(crate) fn render_devices_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .mb_4()
                    .child("Audio Output Devices"),
            )
            .child(
                // Grid layout with 2 columns
                div().grid().grid_cols(2).gap_3().flex_1().children(
                    state
                        .app
                        .output_devices
                        .iter()
                        .enumerate()
                        .map(|(idx, device)| {
                            let is_selected = state.app.selected_output_device_index == idx;
                            let sample_rate = device
                                .default_config
                                .as_ref()
                                .map(|c| c.sample_rate)
                                .unwrap_or(0);
                            let channels = device
                                .default_config
                                .as_ref()
                                .map(|c| c.channels)
                                .unwrap_or(0);

                            div()
                                .p_2()
                                .rounded_md()
                                .when(is_selected, |div| div.bg(rgb(0x007acc)))
                                .when(!is_selected, |div| div.bg(rgb(0x2d2d2d)))
                                .hover(|style| style.bg(rgb(0x3e3e3e)))
                                .cursor_pointer()
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                        view.state.update(cx, |state, _cx| {
                                            state.app.selected_output_device_index = idx;
                                            if let Some(device) = state.app.output_devices.get(idx)
                                            {
                                                state.app.current_output_device_name =
                                                    Some(device.name.clone());

                                                // If playing, restart track with new device
                                                if state.app.is_playing {
                                                    if let Some(queue_idx) =
                                                        state.app.current_queue_index
                                                    {
                                                        if let Some(item) =
                                                            state.app.queue.get(queue_idx)
                                                        {
                                                            if let Some(track) =
                                                                item.current_track()
                                                            {
                                                                let path = track.path.clone();
                                                                Self::play_track(state, path);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        });
                                        cx.notify();
                                    }),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(device.name.clone()),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .gap_3()
                                                .text_xs()
                                                .text_color(rgb(0x999999))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .gap_1()
                                                        .child("📊")
                                                        .child(format!("{} ch", channels)),
                                                )
                                                .child(div().flex().gap_1().child("🎵").child(
                                                    if sample_rate >= 1000 {
                                                        format!("{} kHz", sample_rate / 1000)
                                                    } else {
                                                        format!("{} Hz", sample_rate)
                                                    },
                                                )),
                                        )
                                        .when(device.is_default, |this| {
                                            this.child(
                                                div()
                                                    .text_xs()
                                                    .text_color(rgb(0x4ec9b0))
                                                    .child("✓ Default"),
                                            )
                                        }),
                                )
                        }),
                ),
            )
    }
}
