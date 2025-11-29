//! Devices screen rendering functions

use crate::ui::PlayerView;
use gpui_ui_kit::{Badge, BadgeVariant, Text, TextSize, TextWeight, HStack, VStack, StackSpacing, Heading};
use gpui::prelude::*;
use gpui::*;

impl PlayerView {
    pub(crate) fn render_devices_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let theme = state.app.theme.clone();

        VStack::new()
            .spacing(StackSpacing::Lg)
            .child(
                Heading::h2("Audio Output Devices")
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
                            let is_default = device.is_default;
                            let device_name = device.name.clone();
                            let theme = theme.clone();

                            div()
                                .id(ElementId::Name(format!("device-{}", idx).into()))
                                .p_3()
                                .rounded_lg()
                                .border_1()
                                .when(is_selected, |d| {
                                    d.bg(theme.accent_muted)
                                     .border_color(theme.accent)
                                })
                                .when(!is_selected, |d| {
                                    d.bg(theme.surface)
                                     .border_color(theme.border)
                                     .hover(|s| s.bg(theme.surface_hover))
                                })
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
                                    VStack::new()
                                        .spacing(StackSpacing::Sm)
                                        .child(
                                            Text::new(device_name)
                                                .size(TextSize::Sm)
                                                .weight(TextWeight::Semibold)
                                                .color(theme.text_primary)
                                        )
                                        .child(
                                            HStack::new()
                                                .spacing(StackSpacing::Md)
                                                .child(
                                                    Badge::new(format!("{} ch", channels))
                                                        .variant(BadgeVariant::Default)
                                                )
                                                .child(
                                                    Badge::new(
                                                        if sample_rate >= 1000 {
                                                            format!("{} kHz", sample_rate / 1000)
                                                        } else {
                                                            format!("{} Hz", sample_rate)
                                                        }
                                                    )
                                                    .variant(BadgeVariant::Default)
                                                )
                                        )
                                        .when(is_default, |v| {
                                            v.child(
                                                Badge::new("Default")
                                                    .variant(BadgeVariant::Success)
                                            )
                                        })
                                )
                        }),
                ),
            )
    }
}
