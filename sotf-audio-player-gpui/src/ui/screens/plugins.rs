//! Plugin screen rendering functions

use crate::app::AppState;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;

impl PlayerView {
    pub(crate) fn render_plugins_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .size_full()
            .child(
                // Left panel: Plugin List
                div()
                    .w_1_3()
                    .border_r_1()
                    .border_color(rgb(0x3e3e3e))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .p_4()
                            .border_b_1()
                            .border_color(rgb(0x3e3e3e))
                            .font_weight(FontWeight::BOLD)
                            .child("Plugin Chain"),
                    )
                    .child(self.render_plugin_list(cx))
                    .child(self.render_plugin_actions(cx)),
            )
            .child(
                // Right panel: Plugin Settings
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .p_4()
                            .border_b_1()
                            .border_color(rgb(0x3e3e3e))
                            .font_weight(FontWeight::BOLD)
                            .child("Settings"),
                    )
                    .child(self.render_plugin_settings(cx)),
            )
    }

    pub(crate) fn render_plugin_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let plugins = state.app.plugin_chain.plugins();

        div()
            .flex()
            .flex_col()
            .flex_1()
            .children(plugins.iter().enumerate().map(|(idx, plugin)| {
                let is_selected = state.app.selected_plugin_index == idx;
                let name = plugin.plugin_type().name().to_string();
                let enabled = plugin.enabled;

                div()
                    .p_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .when(is_selected, |div| div.bg(rgb(0x007acc)))
                    .when(!is_selected, |div| div.bg(rgb(0x2d2d2d)))
                    .hover(|style| style.bg(rgb(0x3e3e3e)))
                    .cursor_pointer()
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                            view.state
                                .update(cx, |state, _cx| state.app.selected_plugin_index = idx);
                            cx.notify();
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Right,
                        cx.listener(move |view, event: &MouseUpEvent, _window, cx| {
                            view.state.update(cx, |state, _cx| {
                                state.app.selected_plugin_index = idx;
                                state.app.context_menu = Some(crate::app::ContextMenuState {
                                    menu_type: crate::app::ContextMenuType::Plugin,
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
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .w_4()
                                    .h_4()
                                    .rounded_sm()
                                    .border_1()
                                    .border_color(rgb(0x999999))
                                    .bg(if enabled {
                                        rgb(0x00ff00)
                                    } else {
                                        rgb(0x000000)
                                    })
                                    .cursor_pointer()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                            view.state.update(cx, |state, cx| {
                                                state.app.plugin_chain.toggle_plugin(idx);
                                                state.app.needs_plugin_update = true;
                                            });
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .child(name),
                    )
            }))
    }

    pub(crate) fn render_plugin_actions(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .p_2()
            .border_t_1()
            .border_color(rgb(0x3e3e3e))
            .flex()
            .flex_wrap()
            .gap_2()
            .child(
                div()
                    .px_2()
                    .py_1()
                    .bg(rgb(0x4e4e4e))
                    .rounded_md()
                    .cursor_pointer()
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                            view.state.update(cx, |state, cx| {
                                // Add Upmixer by default for now, or show menu
                                // For simplicity, let's cycle or add a specific one
                                state
                                    .app
                                    .plugin_chain
                                    .add_plugin(&sotf_audio_player::PluginType::Upmixer);
                                state.app.needs_plugin_update = true;
                            });
                            cx.notify();
                        }),
                    )
                    .child("+ Upmixer"),
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .bg(rgb(0x4e4e4e))
                    .rounded_md()
                    .cursor_pointer()
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                            view.state.update(cx, |state, cx| {
                                state
                                    .app
                                    .plugin_chain
                                    .add_plugin(&sotf_audio_player::PluginType::EQ);
                                state.app.needs_plugin_update = true;
                            });
                            cx.notify();
                        }),
                    )
                    .child("+ EQ"),
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .bg(rgb(0x8e2e2e))
                    .rounded_md()
                    .cursor_pointer()
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                            view.state.update(cx, |state, cx| {
                                let idx = state.app.selected_plugin_index;
                                state.app.plugin_chain.remove_plugin(idx);
                                if state.app.selected_plugin_index >= state.app.plugin_chain.len()
                                    && state.app.plugin_chain.len() > 0
                                {
                                    state.app.selected_plugin_index =
                                        state.app.plugin_chain.len() - 1;
                                }
                                state.app.needs_plugin_update = true;
                            });
                            cx.notify();
                        }),
                    )
                    .child("Remove"),
            )
    }

    pub(crate) fn render_plugin_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        if let Some(plugin) = state
            .app
            .plugin_chain
            .get_plugin(state.app.selected_plugin_index)
        {
            let plugin_name = plugin.plugin_type().name().to_string();

            div()
                .p_4()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .mb_2()
                        .child(plugin_name),
                )
                .child(match &plugin.settings {
                    sotf_audio_player::PluginSettings::Upmixer {
                        speaker_config,
                        lfe_gain,
                        gain_front_direct,
                        gain_front_ambient,
                        gain_rear_ambient,
                        lfe_cutoff_hz,
                        stereo_width,
                        bandpass_hz: _,
                        height_gain,
                        enable_subharmonic_synth,
                        subharmonic_gain,
                        enable_hr_direct: _,
                        hr_sharpen: _,
                        safety_cap_db,
                        decorrelation_mode: _,
                    } => div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(self.render_param_group(
                            "Speaker Configuration",
                            vec![("Layout", speaker_config.to_string())],
                        ))
                        .child(self.render_param_group(
                            "Gain Settings",
                            vec![
                                ("LFE Gain", format!("{:.1} dB", lfe_gain)),
                                ("Front Direct", format!("{:.1} dB", gain_front_direct)),
                                ("Front Ambient", format!("{:.1} dB", gain_front_ambient)),
                                ("Rear Ambient", format!("{:.1} dB", gain_rear_ambient)),
                                ("Height Gain", format!("{:.1} dB", height_gain)),
                                ("Safety Cap", format!("{:.1} dB", safety_cap_db)),
                            ],
                        ))
                        .child(self.render_param_group(
                            "Processing",
                            vec![
                                    ("Stereo Width", format!("{:.2}", stereo_width)),
                                    ("LFE Cutoff", format!("{:.0} Hz", lfe_cutoff_hz)),
                                    (
                                        "Subharmonic Synth",
                                        if *enable_subharmonic_synth {
                                            "Enabled"
                                        } else {
                                            "Disabled"
                                        }
                                        .to_string(),
                                    ),
                                    ("Subharmonic Gain", format!("{:.1} dB", subharmonic_gain)),
                                ],
                        ))
                        .child(
                            div()
                                .mt_2()
                                .p_2()
                                .rounded_md()
                                .bg(rgb(0x2d2d2d))
                                .text_xs()
                                .text_color(rgb(0x999999))
                                .child("Press 'e' to edit parameters"),
                        ),
                    sotf_audio_player::PluginSettings::EQ { filters } => div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            div()
                                .text_sm()
                                .text_color(rgb(0x999999))
                                .child(format!("{} bands", filters.len())),
                        )
                        .children(filters.iter().enumerate().map(|(i, f)| {
                            div()
                                .p_3()
                                .mb_2()
                                .rounded_md()
                                .bg(rgb(0x2d2d2d))
                                .border_l_2()
                                .border_color(if f.gain_db > 0.0 {
                                    rgb(0x4ec9b0)
                                } else if f.gain_db < 0.0 {
                                    rgb(0xf48771)
                                } else {
                                    rgb(0x666666)
                                })
                                .child(
                                    div()
                                        .flex()
                                        .justify_between()
                                        .items_center()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(format!("Band {}", i + 1)),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .gap_4()
                                                .text_xs()
                                                .text_color(rgb(0x999999))
                                                .child(format!("{:.0} Hz", f.frequency))
                                                .child(format!("Q: {:.2}", f.q))
                                                .child(
                                                    div()
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_color(if f.gain_db > 0.0 {
                                                            rgb(0x4ec9b0)
                                                        } else if f.gain_db < 0.0 {
                                                            rgb(0xf48771)
                                                        } else {
                                                            rgb(0xcccccc)
                                                        })
                                                        .child(format!("{:+.1} dB", f.gain_db)),
                                                ),
                                        ),
                                )
                        }))
                        .child(
                            div()
                                .mt_2()
                                .p_2()
                                .rounded_md()
                                .bg(rgb(0x2d2d2d))
                                .text_xs()
                                .text_color(rgb(0x999999))
                                .child("Press 'e' to edit EQ bands"),
                        ),
                    sotf_audio_player::PluginSettings::Compressor {
                        threshold_db,
                        ratio,
                        attack_ms,
                        release_ms,
                        makeup_gain_db,
                        ..
                    } => div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(self.render_param_group(
                            "Compressor",
                            vec![
                                ("Threshold", format!("{:.1} dB", threshold_db)),
                                ("Ratio", format!("1:{:.1}", ratio)),
                                ("Attack", format!("{:.1} ms", attack_ms)),
                                ("Release", format!("{:.1} ms", release_ms)),
                                ("Makeup Gain", format!("{:+.1} dB", makeup_gain_db)),
                            ],
                        )),
                    sotf_audio_player::PluginSettings::Limiter {
                        threshold_db,
                        release_ms,
                        ..
                    } => div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(self.render_param_group(
                            "Limiter",
                            vec![
                                ("Threshold", format!("{:.1} dB", threshold_db)),
                                ("Release", format!("{:.1} ms", release_ms)),
                            ],
                        )),
                    _ => div()
                        .text_sm()
                        .text_color(rgb(0x999999))
                        .child("No configurable parameters for this plugin"),
                })
        } else {
            div()
                .p_4()
                .text_color(rgb(0x666666))
                .child("No plugin selected")
        }
    }

    pub(crate) fn render_param_group(&self, title: &str, params: Vec<(&str, String)>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0x569cd6))
                    .mb_1()
                    .child(title.to_string()),
            )
            .children(params.iter().map(|(name, value)| {
                div()
                    .flex()
                    .justify_between()
                    .p_2()
                    .rounded_md()
                    .bg(rgb(0x2d2d2d))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x999999))
                            .child(name.to_string()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0xcccccc))
                            .child(value.clone()),
                    )
            }))
    }
}
