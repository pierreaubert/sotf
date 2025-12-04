//! Plugin screen rendering functions - Professional DAW-style interface

use crate::ui::PlayerView;
use super::render_plugin_content;
use gpui::prelude::*;
use gpui::*;
use sotf_audio_player::PluginType;

/// Drag information for plugin reordering
#[derive(Clone)]
pub struct PluginDragInfo {
    pub source_index: usize,
    pub name: String,
    pub color: Rgba,
    pub icon: &'static str,
}

impl Render for PluginDragInfo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // Drag preview - a smaller version of the plugin module
        div()
            .w(px(90.0))
            .h(px(70.0))
            .flex()
            .flex_col()
            .rounded_lg()
            .border_2()
            .border_color(self.color)
            .bg(rgba(0x1e1e2edd)) // Semi-transparent dark background
            .shadow_lg()
            .opacity(0.9)
            // Top bar with color
            .child(div().h(px(3.0)).w_full().bg(self.color).rounded_t_md())
            // Icon
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_xl()
                    .text_color(self.color)
                    .child(self.icon),
            )
            // Name
            .child(
                div()
                    .px_2()
                    .pb_1()
                    .text_xs()
                    .text_color(rgb(0xffffff))
                    .font_weight(FontWeight::MEDIUM)
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(self.name.clone()),
            )
    }
}

// Plugin color scheme for different types
fn plugin_color(plugin_type: &PluginType) -> Rgba {
    match plugin_type {
        PluginType::EQ => rgb(0x2563eb),                   // Blue - EQ
        PluginType::Gain => rgb(0x059669),                 // Green - Gain
        PluginType::Upmixer => rgb(0x7c3aed),              // Purple - Upmixer
        PluginType::Compressor => rgb(0xdc2626),           // Red - Compressor
        PluginType::Limiter => rgb(0xea580c),              // Orange - Limiter
        PluginType::Gate => rgb(0xca8a04),                 // Yellow - Gate
        PluginType::LoudnessCompensation => rgb(0x0891b2), // Cyan - Loudness
        PluginType::BinauralDecoder => rgb(0xdb2777),      // Pink - Binaural
        PluginType::Convolution => rgb(0x4f46e5),          // Indigo - Convolution
        PluginType::LoudnessMonitor => rgb(0x14b8a6),      // Teal - Monitor
        PluginType::SpectrumAnalyzer => rgb(0x8b5cf6),     // Violet - Spectrum
        PluginType::ChannelMuteSolo => rgb(0x6366f1),      // Blue-violet - Mute/Solo
    }
}

fn plugin_icon(plugin_type: &PluginType) -> &'static str {
    match plugin_type {
        PluginType::EQ => "≈",
        PluginType::Gain => "▲",
        PluginType::Upmixer => "◈",
        PluginType::Compressor => "◉",
        PluginType::Limiter => "█",
        PluginType::Gate => "⊡",
        PluginType::LoudnessCompensation => "♫",
        PluginType::BinauralDecoder => "◎",
        PluginType::Convolution => "∿",
        PluginType::LoudnessMonitor => "◐",
        PluginType::SpectrumAnalyzer => "▓",
        PluginType::ChannelMuteSolo => "◧",
    }
}

fn short_name(plugin_type: &PluginType) -> &'static str {
    match plugin_type {
        PluginType::EQ => "EQ",
        PluginType::Gain => "Gain",
        PluginType::Upmixer => "Upmix",
        PluginType::Compressor => "Comp",
        PluginType::Limiter => "Limit",
        PluginType::Gate => "Gate",
        PluginType::LoudnessCompensation => "Loud",
        PluginType::BinauralDecoder => "Bin",
        PluginType::Convolution => "Conv",
        PluginType::LoudnessMonitor => "LUFS",
        PluginType::SpectrumAnalyzer => "Spec",
        PluginType::ChannelMuteSolo => "M/S",
    }
}

impl PlayerView {
    pub(crate) fn render_plugins_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.state.read(cx).app.theme.clone();

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            // Plugin Rack Strip (top)
            .child(self.render_plugin_rack(cx))
            // Parameter Panel (bottom, fills remaining space)
            .child(self.render_plugin_detail_panel(cx))
    }

    /// Render the horizontal plugin rack strip
    fn render_plugin_rack(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (plugins_data, selected_idx, theme) = {
            let state = self.state.read(cx);
            let plugins: Vec<_> = state
                .app
                .plugin_chain
                .plugins()
                .iter()
                .map(|p| {
                    (
                        p.plugin_type().clone(),
                        p.enabled,
                        p.plugin_type().name().to_string(),
                    )
                })
                .collect();
            (
                plugins,
                state.app.selected_plugin_index,
                state.app.theme.clone(),
            )
        };

        // Pre-compute static data for plugin modules
        let modules_info: Vec<_> = plugins_data
            .iter()
            .enumerate()
            .map(|(idx, (pt, enabled, name))| {
                (
                    idx,
                    plugin_color(pt),
                    plugin_icon(pt),
                    name.clone(),
                    *enabled,
                    selected_idx == idx,
                )
            })
            .collect();

        let is_empty = plugins_data.is_empty();
        let plugin_count = plugins_data.len();

        div()
            .flex()
            .flex_col()
            .bg(theme.background_secondary)
            .border_b_1()
            .border_color(theme.border)
            // Header
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .px_4()
                    .py_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.text_primary)
                                    .child("SIGNAL CHAIN"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.text_muted)
                                    .child(format!("{} plugins", plugin_count)),
                            ),
                    )
                    .child(self.render_add_plugin_buttons(cx)),
            )
            // Plugin modules strip
            .child(
                div()
                    .id("plugin-rack")
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_4()
                    .py_3()
                    .overflow_x_scroll()
                    .min_h(px(140.0))
                    // Signal flow indicator (input)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .child(div().text_xs().text_color(theme.text_muted).child("IN"))
                            .child(div().w(px(2.0)).h(px(30.0)).bg(theme.accent)),
                    )
                    // Plugin modules - inline creation with drag-and-drop
                    .children(modules_info.into_iter().map(
                        |(idx, color, icon, name, enabled, is_selected)| {
                            let theme_c = theme.clone();
                            let drag_info = PluginDragInfo {
                                source_index: idx,
                                name: name.clone(),
                                color,
                                icon,
                            };
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                // Connection line before
                                .child(div().w(px(20.0)).h(px(2.0)).bg(if enabled {
                                    theme_c.accent
                                } else {
                                    theme_c.text_muted
                                }))
                                // Plugin module box - draggable and droppable
                                .child(
                                    div()
                                        .id(SharedString::from(format!("plugin-module-{}", idx)))
                                        .w(px(100.0))
                                        .h(px(110.0))
                                        .flex()
                                        .flex_col()
                                        .rounded_lg()
                                        .border_2()
                                        .border_color(if is_selected {
                                            color
                                        } else {
                                            theme_c.border
                                        })
                                        .bg(theme_c.surface)
                                        .when(!enabled, |d| d.opacity(0.5))
                                        .shadow_md()
                                        .cursor_grab()
                                        .hover(|s| s.border_color(color))
                                        // Drag-over visual feedback - highlight when dragging over
                                        .drag_over::<PluginDragInfo>(|style, _, _, _| {
                                            style
                                                .bg(rgba(0x3b82f640)) // Blue tint when dragging over
                                                .border_color(rgb(0x3b82f6))
                                        })
                                        // Handle drop - reorder plugins
                                        .on_drop(cx.listener(
                                            move |view, info: &PluginDragInfo, _window, cx| {
                                                let source = info.source_index;
                                                let target = idx;
                                                if source != target {
                                                    view.state.update(cx, |state, _cx| {
                                                        state
                                                            .app
                                                            .plugin_chain
                                                            .move_plugin(source, target);
                                                        state.app.selected_plugin_index = target;
                                                        state.app.needs_plugin_update = true;
                                                    });
                                                    cx.notify();
                                                }
                                            },
                                        ))
                                        // Start drag
                                        .on_drag(drag_info, |info, _position, _window, cx| {
                                            cx.new(|_| info.clone())
                                        })
                                        // Click to select
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(
                                                move |view, _: &MouseUpEvent, _window, cx| {
                                                    view.state.update(cx, |state, _cx| {
                                                        state.app.selected_plugin_index = idx
                                                    });
                                                    cx.notify();
                                                },
                                            ),
                                        )
                                        // Top bar with color
                                        .child(div().h(px(4.0)).w_full().bg(color).rounded_t_md())
                                        // Icon
                                        .child(
                                            div()
                                                .flex_1()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .text_2xl()
                                                .text_color(color)
                                                .child(icon),
                                        )
                                        // Name
                                        .child(
                                            div()
                                                .px_2()
                                                .text_xs()
                                                .text_color(theme_c.text_primary)
                                                .font_weight(FontWeight::MEDIUM)
                                                .overflow_hidden()
                                                .text_ellipsis()
                                                .child(name),
                                        )
                                        // Bottom controls
                                        .child(
                                            div()
                                                .flex()
                                                .justify_between()
                                                .items_center()
                                                .px_2()
                                                .py_1()
                                                .border_t_1()
                                                .border_color(theme_c.border)
                                                // Power indicator
                                                .child(
                                                    div()
                                                        .w(px(16.0))
                                                        .h(px(16.0))
                                                        .rounded_full()
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .bg(if enabled {
                                                            theme_c.success
                                                        } else {
                                                            theme_c.error
                                                        })
                                                        .text_xs()
                                                        .text_color(rgb(0xffffff))
                                                        .child(if enabled { "●" } else { "○" }),
                                                )
                                                // Index indicator
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(theme_c.text_muted)
                                                        .child(format!("{}", idx + 1)),
                                                ),
                                        ),
                                )
                                // Connection line after
                                .child(div().w(px(20.0)).h(px(2.0)).bg(if enabled {
                                    theme_c.accent
                                } else {
                                    theme_c.text_muted
                                }))
                        },
                    ))
                    // Signal flow indicator (output)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .child(div().text_xs().text_color(theme.text_muted).child("OUT"))
                            .child(div().w(px(2.0)).h(px(30.0)).bg(theme.success)),
                    )
                    // Empty state
                    .when(is_empty, |d| {
                        d.child(
                            div()
                                .flex_1()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(theme.text_muted)
                                .child("Click + Add Plugin to insert effects"),
                        )
                    }),
            )
    }

    /// Render add plugin buttons grouped by category
    fn render_add_plugin_buttons(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.state.read(cx).app.theme.clone();

        // All plugin types with categories
        let all_plugins = [
            // Effects
            (PluginType::EQ, "Effects"),
            (PluginType::Gain, "Effects"),
            (PluginType::Compressor, "Effects"),
            (PluginType::Limiter, "Effects"),
            (PluginType::Gate, "Effects"),
            // Spatial
            (PluginType::Upmixer, "Spatial"),
            (PluginType::BinauralDecoder, "Spatial"),
            (PluginType::Convolution, "Spatial"),
            // Monitoring
            (PluginType::LoudnessCompensation, "Monitor"),
            (PluginType::LoudnessMonitor, "Monitor"),
            (PluginType::SpectrumAnalyzer, "Monitor"),
            (PluginType::ChannelMuteSolo, "Monitor"),
        ];

        div()
            .flex()
            .items_center()
            .gap_2()
            .children(all_plugins.into_iter().map(|(pt, _category)| {
                let color = plugin_color(&pt);
                let name = short_name(&pt);
                let theme_c = theme.clone();

                div()
                    .id(SharedString::from(format!("add-{}", name)))
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(theme_c.surface)
                    .border_1()
                    .border_color(color)
                    .text_xs()
                    .text_color(color)
                    .cursor_pointer()
                    .hover(|s| s.bg(color).text_color(rgb(0xffffff)))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                            view.state.update(cx, |state, _cx| {
                                state.app.plugin_chain.add_plugin(&pt);
                                state.app.needs_plugin_update = true;
                            });
                            cx.notify();
                        }),
                    )
                    .child(format!("+{}", name))
            }))
    }

    /// Render the plugin detail/settings panel
    fn render_plugin_detail_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (plugin_data, selected_idx, editing_idx, param_selection, theme) = {
            let state = self.state.read(cx);
            let plugin = state
                .app
                .plugin_chain
                .get_plugin(state.app.selected_plugin_index)
                .cloned();
            (
                plugin,
                state.app.selected_plugin_index,
                state.app.editing_plugin_index,
                state.app.plugin_param_selection,
                state.app.theme.clone(),
            )
        };

        let has_plugin = plugin_data.is_some();

        div()
            .flex_1()
            .flex()
            .flex_col()
            .overflow_hidden()
            .when(has_plugin, |d| {
                let plugin = plugin_data.clone().unwrap();
                let plugin_type = plugin.plugin_type().clone();
                let plugin_name = plugin_type.name().to_string();
                let color = plugin_color(&plugin_type);
                let is_editing = editing_idx.is_some();
                let plugin_enabled = plugin.enabled;

                d.child(
                    // Plugin header bar
                    div()
                        .flex()
                        .justify_between()
                        .items_center()
                        .px_4()
                        .py_3()
                        .bg(theme.background_secondary)
                        .border_b_1()
                        .border_color(theme.border)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_3()
                                // Color indicator
                                .child(div().w(px(4.0)).h(px(24.0)).rounded_full().bg(color))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .child(
                                            div()
                                                .text_lg()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(theme.text_primary)
                                                .child(plugin_name),
                                        )
                                        .child(div().text_xs().text_color(theme.text_muted).child(
                                            format!(
                                                "Slot {} • {}",
                                                selected_idx + 1,
                                                if plugin_enabled { "Active" } else { "Bypassed" }
                                            ),
                                        )),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                // Move buttons
                                .child(
                                    div()
                                        .id("move-up")
                                        .px_3()
                                        .py_1()
                                        .rounded_md()
                                        .bg(theme.surface)
                                        .text_sm()
                                        .cursor_pointer()
                                        .hover(|s| s.bg(theme.surface_hover))
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                                view.state.update(cx, |state, _cx| {
                                                    let idx = state.app.selected_plugin_index;
                                                    state.app.move_plugin_up(idx);
                                                });
                                                cx.notify();
                                            }),
                                        )
                                        .child("◀ Move"),
                                )
                                .child(
                                    div()
                                        .id("move-down")
                                        .px_3()
                                        .py_1()
                                        .rounded_md()
                                        .bg(theme.surface)
                                        .text_sm()
                                        .cursor_pointer()
                                        .hover(|s| s.bg(theme.surface_hover))
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                                view.state.update(cx, |state, _cx| {
                                                    let idx = state.app.selected_plugin_index;
                                                    state.app.move_plugin_down(idx);
                                                });
                                                cx.notify();
                                            }),
                                        )
                                        .child("Move ▶"),
                                )
                                // Toggle power
                                .child(
                                    div()
                                        .id("toggle-power")
                                        .px_3()
                                        .py_1()
                                        .rounded_md()
                                        .bg(if plugin_enabled {
                                            theme.success
                                        } else {
                                            theme.error
                                        })
                                        .text_sm()
                                        .text_color(rgb(0xffffff))
                                        .cursor_pointer()
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                                view.state.update(cx, |state, _cx| {
                                                    let idx = state.app.selected_plugin_index;
                                                    state.app.plugin_chain.toggle_plugin(idx);
                                                    state.app.needs_plugin_update = true;
                                                });
                                                cx.notify();
                                            }),
                                        )
                                        .child(if plugin_enabled { "ON" } else { "OFF" }),
                                )
                                // Edit button
                                .child(
                                    div()
                                        .id("edit-btn")
                                        .px_4()
                                        .py_1()
                                        .rounded_md()
                                        .bg(if is_editing {
                                            theme.accent
                                        } else {
                                            theme.surface
                                        })
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .cursor_pointer()
                                        .hover(|s| s.bg(theme.accent_hover))
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                                view.state.update(cx, |state, _cx| {
                                                    if state.app.editing_plugin_index.is_some() {
                                                        state.app.exit_plugin_edit_mode();
                                                    } else {
                                                        state.app.enter_plugin_edit_mode();
                                                    }
                                                });
                                                cx.notify();
                                            }),
                                        )
                                        .child(if is_editing { "Done" } else { "Edit" }),
                                )
                                // Remove button
                                .child(
                                    div()
                                        .id("remove-btn")
                                        .px_3()
                                        .py_1()
                                        .rounded_md()
                                        .bg(theme.error)
                                        .text_sm()
                                        .text_color(rgb(0xffffff))
                                        .cursor_pointer()
                                        .hover(|s| s.opacity(0.8))
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                                view.state.update(cx, |state, _cx| {
                                                    let idx = state.app.selected_plugin_index;
                                                    state.app.plugin_chain.remove_plugin(idx);
                                                    if state.app.selected_plugin_index
                                                        >= state.app.plugin_chain.len()
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
                                ),
                        ),
                )
                // Plugin-specific visualization + Parameters
                .child(
                    div()
                        .id("params-scroll")
                        .flex_1()
                        .overflow_y_scroll()
                        .p_4()
                        .child(render_plugin_content(
                            &plugin.settings,
                            is_editing,
                            param_selection,
                            &theme,
                        )),
                )
            })
            .when(!has_plugin, |d| {
                d.child(
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .flex_col()
                        .gap_2()
                        .text_color(theme.text_muted)
                        .child("No plugin selected")
                        .child(div().text_sm().child("Add a plugin to get started")),
                )
            })
    }
}
