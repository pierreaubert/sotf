//! A/B Compare plugin custom view.
//!
//! Renders two side-by-side sub-rack columns (Path A and Path B), each with
//! a scrollable plugin strip, an "add plugin" picker, and remove/move controls.

use crate::app::state::plugin::ABPathTarget;
use crate::components::design::Ds;
use crate::components::plugins::actions::{
    ABPathAddPlugin, ABPathMovePlugin, ABPathRemovePlugin, ABPathToggleAddMenu, OpenAbConfigFile,
};
use crate::components::plugins::custom_view_registry::CustomViewRenderContext;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant, Divider, Text, TextSize, TextWeight};
use sotf_audio_player::controllers::ab_compare_path::{ALLOWED_PLUGIN_TYPES, PluginInRack};

/// Render the A/B Compare custom plugin view.
pub fn render_ab_compare(
    ctx: &CustomViewRenderContext,
    cx: &mut Context<PlayerView>,
) -> AnyElement {
    let d = Ds::from_cx(cx);
    let state = ctx.entity.read(cx);
    let plugin_idx = ctx.plugin_idx;
    let path_a = state.app.plugin_state.ab_path_a.clone();
    let path_b = state.app.plugin_state.ab_path_b.clone();
    let path_a_file = state.app.plugin_state.ab_compare_file_a.clone();
    let path_b_file = state.app.plugin_state.ab_compare_file_b.clone();
    let add_menu_target = state.app.plugin_state.ab_add_menu_target;

    div()
        .flex()
        .flex_col()
        .gap(d.section)
        .w_full()
        .child(
            div()
                .flex()
                .gap(d.section)
                .w_full()
                .child(render_path_section(
                    "PATH A",
                    0,
                    plugin_idx,
                    &path_a,
                    path_a_file.as_deref(),
                    add_menu_target == Some(ABPathTarget::A),
                    ctx.theme,
                    cx,
                ))
                .child(render_path_section(
                    "PATH B",
                    1,
                    plugin_idx,
                    &path_b,
                    path_b_file.as_deref(),
                    add_menu_target == Some(ABPathTarget::B),
                    ctx.theme,
                    cx,
                )),
        )
        .into_any_element()
}

fn render_path_section(
    label: &str,
    path: u8,
    plugin_idx: usize,
    plugins: &[PluginInRack],
    loaded_config_file: Option<&str>,
    add_menu_open: bool,
    theme: &Theme,
    cx: &mut Context<PlayerView>,
) -> Div {
    let d = Ds::from_cx(cx);
    let mut section = div()
        .flex_1()
        .flex()
        .flex_col()
        .gap(d.gap)
        .p(d.pad_x)
        .bg(theme.background_secondary)
        .rounded(d.r_md)
        .border_1()
        .border_color(theme.border);

    // Header
    section = section.child(
        div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(Text::eyebrow(label.to_string()).color(theme.text_primary))
                    .child(Text::caption(format!(
                        "{} plugin{}",
                        plugins.len(),
                        if plugins.len() == 1 { "" } else { "s" }
                    ))),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(d.grid)
                    .child(
                        Button::new(SharedString::from(format!("ab-load-{path}")), "Load")
                            .aria_label(format!("{label} config file"))
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Xs)
                            .theme(theme.to_button_theme())
                            .on_click_event(cx.listener(
                                move |_view, _: &ClickEvent, window, cx| {
                                    window.dispatch_action(
                                        Box::new(OpenAbConfigFile {
                                            plugin_idx,
                                            path_id: if path == 0 {
                                                "a".to_string()
                                            } else {
                                                "b".to_string()
                                            },
                                        }),
                                        cx,
                                    );
                                },
                            )),
                    )
                    .child(
                        Button::new(SharedString::from(format!("ab-add-{path}")), "+")
                            .aria_label("Add plugin for comparison")
                            .variant(if add_menu_open {
                                ButtonVariant::Primary
                            } else {
                                ButtonVariant::Secondary
                            })
                            .size(ButtonSize::Xs)
                            .theme(theme.to_button_theme())
                            .on_click_event(cx.listener(
                                move |_view, _: &ClickEvent, window, cx| {
                                    window.dispatch_action(
                                        Box::new(ABPathToggleAddMenu { plugin_idx, path }),
                                        cx,
                                    );
                                },
                            )),
                    ),
            ),
    );

    if let Some(file_path) = loaded_config_file.filter(|path| !path.is_empty()) {
        section = section.child(
            div()
                .text_size(d.text_xs)
                .text_color(theme.text_muted)
                .overflow_hidden()
                .text_ellipsis()
                .child(format!("Loaded: {}", display_file_name(file_path))),
        );
    }

    // Add menu dropdown
    if add_menu_open {
        section = section.child(render_add_menu(path, plugin_idx, theme, cx));
    }

    section = section.child(Divider::new().color(theme.border));

    // Plugin list
    if plugins.is_empty() {
        section = section.child(
            div()
                .py(d.pad_y)
                .child(Text::caption("Empty (pass-through)")),
        );
    } else {
        let len = plugins.len();
        for (sub_idx, plugin) in plugins.iter().enumerate() {
            section = section.child(render_sub_plugin_card(
                path, plugin_idx, sub_idx, plugin, len, theme, cx,
            ));
        }
    }

    section
}

fn display_file_name(path: &str) -> &str {
    path.rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(path)
}

fn render_add_menu(
    path: u8,
    plugin_idx: usize,
    theme: &Theme,
    cx: &mut Context<PlayerView>,
) -> Div {
    let d = Ds::from_cx(cx);
    let mut menu = div()
        .flex()
        .flex_wrap()
        .gap(d.grid)
        .p(d.pad_y)
        .bg(theme.background)
        .rounded(d.r_sm)
        .border_1()
        .border_color(theme.accent);

    for &(type_key, display_name) in ALLOWED_PLUGIN_TYPES {
        let plugin_type = type_key.to_string();
        menu = menu.child(
            Button::new(
                SharedString::from(format!("ab-add-{path}-{type_key}")),
                display_name,
            )
            .variant(ButtonVariant::Secondary)
            .size(ButtonSize::Xs)
            .theme(theme.to_button_theme())
            .on_click_event(cx.listener(move |_view, _: &ClickEvent, window, cx| {
                window.dispatch_action(
                    Box::new(ABPathAddPlugin {
                        plugin_idx,
                        path,
                        plugin_type: plugin_type.clone(),
                    }),
                    cx,
                );
            })),
        );
    }

    menu
}

fn render_sub_plugin_card(
    path: u8,
    plugin_idx: usize,
    sub_idx: usize,
    plugin: &PluginInRack,
    total: usize,
    theme: &Theme,
    cx: &mut Context<PlayerView>,
) -> Div {
    let display_name = plugin_display_name(&plugin.plugin_type);

    let d = Ds::from_cx(cx);
    let mut card = div()
        .flex()
        .items_center()
        .gap(d.gap)
        .px(d.pad_y)
        .py(d.pad_y_half)
        .rounded(d.r_sm)
        .bg(theme.surface)
        .border_1()
        .border_color(theme.border)
        // Plugin type label
        .child(
            div().flex_1().child(
                Text::new(display_name)
                    .size(TextSize::Xs)
                    .weight(TextWeight::Medium)
                    .color(theme.text_primary),
            ),
        );

    // Move up button
    if sub_idx > 0 {
        let from = sub_idx;
        let to = sub_idx - 1;
        card = card.child(
            Button::new(
                SharedString::from(format!("ab-up-{path}-{sub_idx}")),
                "\u{25B2}",
            )
            .aria_label("Move plugin up")
            .variant(ButtonVariant::Ghost)
            .size(ButtonSize::Xs)
            .theme(theme.to_button_theme())
            .on_click_event(cx.listener(move |_view, _: &ClickEvent, window, cx| {
                window.dispatch_action(
                    Box::new(ABPathMovePlugin {
                        plugin_idx,
                        path,
                        from,
                        to,
                    }),
                    cx,
                );
            })),
        );
    }

    // Move down button
    if sub_idx < total - 1 {
        let from = sub_idx;
        let to = sub_idx + 1;
        card = card.child(
            Button::new(
                SharedString::from(format!("ab-down-{path}-{sub_idx}")),
                "\u{25BC}",
            )
            .aria_label("Move plugin down")
            .variant(ButtonVariant::Ghost)
            .size(ButtonSize::Xs)
            .theme(theme.to_button_theme())
            .on_click_event(cx.listener(move |_view, _: &ClickEvent, window, cx| {
                window.dispatch_action(
                    Box::new(ABPathMovePlugin {
                        plugin_idx,
                        path,
                        from,
                        to,
                    }),
                    cx,
                );
            })),
        );
    }

    // Remove button
    card = card.child(
        Button::new(
            SharedString::from(format!("ab-rm-{path}-{sub_idx}")),
            "\u{2715}",
        )
        .aria_label("Remove plugin")
        .variant(ButtonVariant::Ghost)
        .size(ButtonSize::Xs)
        .theme(theme.to_button_theme())
        .on_click_event(cx.listener(move |_view, _: &ClickEvent, window, cx| {
            window.dispatch_action(
                Box::new(ABPathRemovePlugin {
                    plugin_idx,
                    path,
                    sub_idx,
                }),
                cx,
            );
        })),
    );

    card
}

fn plugin_display_name(plugin_type: &str) -> &'static str {
    for &(key, name) in ALLOWED_PLUGIN_TYPES {
        if key == plugin_type {
            return name;
        }
    }
    "Unknown"
}
