//! Footer right section — device selection, volume control, and device popup menus.
//!
//! Extracted from footer.rs for maintainability.

#[cfg(all(target_os = "macos", feature = "hal"))]
use crate::app::types::PlaybackSource;
use crate::components::design::Ds;
use crate::components::icons::{Icon, IconName, IconSize};
use crate::components::themed_tooltip as footer_tooltip;
use crate::ui::{FOOTER_HEIGHT_REMS, PlayerView};
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Menu, MenuItem, VolumeKnob};

impl PlayerView {
    pub(crate) fn render_footer_right(
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
                                            .text_size(d.text_lg)
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
                                let current_path = state.app.queue.current_track_source();
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
                            .when(is_selected, |el| {
                                el.child(Icon::new(IconName::Check).size(IconSize::Xs).color(theme.accent))
                            })
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

                let mut section = div()
                    .flex()
                    .flex_col()
                    .border_t_1()
                    .border_color(theme_cast.border)
                    .mt(d.grid)
                    .pt(d.pad_y_half)
                    .child(
                        div()
                            .px(d.pad_x)
                            .py(d.pad_y_half)
                            .text_size(d.text_xs)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme_cast.text_muted)
                            .child(if cast_running {
                                "Cast Devices (scanning...)"
                            } else {
                                "Cast Devices"
                            }),
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
                                    .when(is_selected, |el| {
                                        el.child(Icon::new(IconName::Check).size(IconSize::Xs).color(theme.accent))
                                    })
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
                MenuItem::new("settings", translations.screen_settings),
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
                        state.app.is_dragging_volume = true;
                        state.app.volume_drag_start_y = Some(event.position.y.into());
                        state.app.volume_drag_start_value = state.app.playback.volume;
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
