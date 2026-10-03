//! A/B Compare plugin custom view.
//!
//! Renders two side-by-side sub-rack columns (Path A and Path B), each with
//! a scrollable plugin strip, an "add plugin" picker, and remove/move controls.

#[cfg(feature = "dev-api")]
use crate::app::dev_api::{DevElementState, DevTrackExt};
use crate::app::i18n::{ABCompareTranslations, PluginCommonTranslations};
use crate::app::state::plugin::ABPathTarget;
use crate::components::design::Ds;
use crate::components::plugins::actions::{
    ABPathAddPlugin, ABPathMovePlugin, ABPathRemovePlugin, ABPathToggleAddMenu, UpdatePluginParam,
};
use crate::components::plugins::custom_view_registry::CustomViewRenderContext;
use crate::components::plugins::ui_layout_renderer;
use crate::theme::Theme;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant, Divider, Text, TextSize, TextWeight};
use sotf_audio_player::controllers::ab_compare_path::{
    PluginInRack, allowed_plugin_types, parse_path_config,
};

/// Derive the custom A/B view from persisted settings. This keeps save/reload
/// and graph-modal discard consistent without transient-state synchronization.
#[doc(hidden)]
pub fn ab_compare_view_state(
    settings: &sotf_audio_player::PluginSettings,
) -> (
    Vec<PluginInRack>,
    Option<String>,
    Vec<PluginInRack>,
    Option<String>,
) {
    let sotf_audio_player::PluginSettings::ABCompare {
        path_a_config,
        path_b_config,
        path_a_file,
        path_b_file,
        ..
    } = settings
    else {
        return (Vec::new(), None, Vec::new(), None);
    };

    let non_empty = |path: &str| (!path.is_empty()).then(|| path.to_string());
    (
        parse_path_config(path_a_config),
        non_empty(path_a_file),
        parse_path_config(path_b_config),
        non_empty(path_b_file),
    )
}

fn path_selection_update(settings: &sotf_audio_player::PluginSettings, path: u8) -> (usize, f64) {
    let binary = matches!(
        settings,
        sotf_audio_player::PluginSettings::ABCompare { mix_mode, .. } if *mix_mode == 1
    );
    let key = if binary { "selected_path" } else { "mix" };
    let param_idx =
        sotf_plugins::param_specs::index_of(sotf_plugins::param_specs::ab_compare::PARAMS, key);
    let value = if binary {
        path.min(1) as f64
    } else if path == 0 {
        -sotf_plugins::param_specs::ab_compare::PARAMS[param_idx].display_scale
    } else {
        sotf_plugins::param_specs::ab_compare::PARAMS[param_idx].display_scale
    };
    (param_idx, value)
}

/// Render the A/B Compare custom plugin view.
pub fn render_ab_compare(
    ctx: &CustomViewRenderContext,
    cx: &mut Context<PlayerView>,
) -> AnyElement {
    let d = Ds::from_cx(cx);
    let state = ctx.entity.read(cx);
    let plugin_idx = ctx.plugin_idx;
    let (path_a, path_a_file, path_b, path_b_file) = ab_compare_view_state(ctx.settings);
    let add_menu_target = state.app.plugin_state.ab_compare_state.ab_add_menu_target;
    let (active_path, is_bypassed) = match ctx.settings {
        sotf_audio_player::PluginSettings::ABCompare {
            mix,
            mix_mode,
            selected_path,
            bypass,
            ..
        } => {
            let active_path = if *mix_mode == 1 {
                Some((*selected_path).clamp(0, 1) as u8)
            } else if *mix <= -0.999 {
                Some(0)
            } else if *mix >= 0.999 {
                Some(1)
            } else {
                None
            };
            (active_path, *bypass)
        }
        _ => (None, false),
    };
    let workflow_text =
        crate::app::i18n::WorkflowTranslations::for_language(state.app.ui_state.language);
    let rack_text =
        crate::app::i18n::PluginRackTranslations::for_language(state.app.ui_state.language);
    let text = ABCompareTranslations::for_language(state.app.ui_state.language);
    let common_text = PluginCommonTranslations::for_language(state.app.ui_state.language);
    let stack_paths = ctx.available_width / ctx.layout_scale.max(0.01) < 600.0;
    let (path_a_param_idx, path_a_value) = path_selection_update(ctx.settings, 0);
    let (path_b_param_idx, path_b_value) = path_selection_update(ctx.settings, 1);
    let bypass_param_idx = sotf_plugins::param_specs::index_of(
        sotf_plugins::param_specs::ab_compare::PARAMS,
        "bypass",
    );

    let bypass_button = Button::new(
        "ab-bypass-toggle",
        if is_bypassed {
            text.resume
        } else {
            text.bypass
        },
    )
    .aria_label(if is_bypassed {
        text.resume
    } else {
        text.bypass
    })
    .variant(if is_bypassed {
        ButtonVariant::Primary
    } else {
        ButtonVariant::Secondary
    })
    .size(ButtonSize::Xs)
    .theme(ctx.theme.to_button_theme())
    .on_click_event(cx.listener(move |_view, _: &ClickEvent, window, cx| {
        window.dispatch_action(
            Box::new(UpdatePluginParam {
                plugin_idx,
                param_idx: bypass_param_idx,
                value: if is_bypassed { 0.0 } else { 1.0 },
            }),
            cx,
        );
    }));
    #[cfg(feature = "dev-api")]
    let bypass_button = bypass_button.dev_track("ab.bypass.toggle");

    let warning = div()
        .flex()
        .flex_wrap()
        .min_w_0()
        .items_center()
        .justify_between()
        .gap(d.gap)
        .px(d.pad_y)
        .py(d.pad_y_half)
        .rounded(d.r_sm)
        .bg(ctx.theme.feedback.warning_background)
        .border_1()
        .border_color(ctx.theme.warning)
        .child(
            div()
                .flex_1()
                .min_w(rems(12.0))
                .child(Text::caption(text.comparison_warning).color(ctx.theme.warning)),
        )
        .child(div().flex_shrink_0().child(bypass_button));
    #[cfg(feature = "dev-api")]
    let warning = warning.dev_track("ab.warning.level-latency");

    let bypass_notice = div()
        .flex()
        .items_center()
        .px(d.pad_y)
        .py(d.pad_y_half)
        .rounded(d.r_sm)
        .bg(ctx.theme.accent_muted)
        .child(Text::caption(text.bypassed).color(ctx.theme.accent));
    #[cfg(feature = "dev-api")]
    let bypass_notice = bypass_notice.dev_track("ab.bypass.active");

    div()
        .key_context("ABCompare")
        .flex()
        .flex_col()
        .gap(d.section)
        .w_full()
        .child(warning)
        .when(is_bypassed, |root| root.child(bypass_notice))
        .child(
            div()
                .flex()
                .when(stack_paths, |paths| paths.flex_col())
                .gap(d.section)
                .w_full()
                .child(render_path_section(
                    text.path_a,
                    0,
                    plugin_idx,
                    &path_a,
                    path_a_file.as_deref(),
                    add_menu_target == Some(ABPathTarget::A),
                    active_path == Some(0),
                    path_a_param_idx,
                    path_a_value,
                    workflow_text,
                    rack_text,
                    text,
                    ctx.theme,
                    cx,
                ))
                .child(render_path_section(
                    text.path_b,
                    1,
                    plugin_idx,
                    &path_b,
                    path_b_file.as_deref(),
                    add_menu_target == Some(ABPathTarget::B),
                    active_path == Some(1),
                    path_b_param_idx,
                    path_b_value,
                    workflow_text,
                    rack_text,
                    text,
                    ctx.theme,
                    cx,
                )),
        )
        .child(ui_layout_renderer::render_main_controls_from_layout(
            &d,
            ctx.entity.clone(),
            plugin_idx,
            ctx.settings,
            ctx.is_editing,
            ctx.selected_param,
            ctx.plugin_data.as_ref(),
            ctx.available_width,
            ctx.layout_scale,
            common_text,
            ctx.theme,
        ))
        .child(ui_layout_renderer::render_tabs_from_layout(
            &d,
            ctx.entity.clone(),
            plugin_idx,
            ctx.settings,
            ctx.is_editing,
            ctx.selected_param,
            ctx.plugin_data.as_ref(),
            ctx.available_width,
            ctx.layout_scale,
            common_text,
            ctx.theme,
        ))
        .into_any_element()
}

fn render_path_section(
    label: &str,
    path: u8,
    plugin_idx: usize,
    plugins: &[PluginInRack],
    loaded_config_file: Option<&str>,
    add_menu_open: bool,
    is_active: bool,
    select_param_idx: usize,
    select_value: f64,
    workflow_text: crate::app::i18n::WorkflowTranslations,
    rack_text: crate::app::i18n::PluginRackTranslations,
    text: ABCompareTranslations,
    theme: &Theme,
    cx: &mut Context<PlayerView>,
) -> Div {
    let d = Ds::from_cx(cx);
    let mut section = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(d.gap)
        .p(d.pad_x)
        .bg(theme.background_secondary)
        .rounded(d.r_md)
        .border_1()
        .border_color(if is_active {
            theme.accent
        } else {
            theme.border
        });

    let select_label = if path == 0 {
        text.use_path_a
    } else {
        text.use_path_b
    };
    let select_aria = if is_active {
        if path == 0 {
            text.path_a_active
        } else {
            text.path_b_active
        }
    } else {
        select_label
    };
    let select_button = Button::new(
        SharedString::from(format!("ab-select-{path}")),
        if is_active { text.active } else { select_label },
    )
    .aria_label(select_aria)
    .variant(if is_active {
        ButtonVariant::Primary
    } else {
        ButtonVariant::Secondary
    })
    .size(ButtonSize::Xs)
    .theme(theme.to_button_theme())
    .on_click_event(cx.listener(move |_view, _: &ClickEvent, window, cx| {
        window.dispatch_action(
            Box::new(UpdatePluginParam {
                plugin_idx,
                param_idx: select_param_idx,
                value: select_value,
            }),
            cx,
        );
    }));
    #[cfg(feature = "dev-api")]
    let select_button = select_button.dev_track(format!("ab.path.{path}.select"));

    let add_button = Button::new(SharedString::from(format!("ab-add-{path}")), "+")
        .aria_label(rack_text.add_plugin_for_comparison)
        .variant(if add_menu_open {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Secondary
        })
        .size(ButtonSize::Xs)
        .theme(theme.to_button_theme())
        .on_click_event(cx.listener(move |_view, _: &ClickEvent, window, cx| {
            window.dispatch_action(Box::new(ABPathToggleAddMenu { plugin_idx, path }), cx);
        }));
    #[cfg(feature = "dev-api")]
    let add_button = add_button.dev_track(format!("ab.path.{path}.add"));

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
                    .child(Text::eyebrow(label.to_string()).color(if is_active {
                        theme.accent
                    } else {
                        theme.text_primary
                    }))
                    .child(Text::caption(format!(
                        "{} {}",
                        plugins.len(),
                        if plugins.len() == 1 {
                            text.plugin
                        } else {
                            text.plugins
                        }
                    ))),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(d.grid)
                    .child(select_button)
                    .child(add_button),
            ),
    );

    if let Some(file_path) = loaded_config_file.filter(|path| !path.is_empty()) {
        let file_name = display_file_name(file_path);
        let file_label = div()
            .text_size(d.text_xs)
            .text_color(theme.text_muted)
            .overflow_hidden()
            .text_ellipsis()
            .child(format!("{}: {file_name}", text.loaded));
        #[cfg(feature = "dev-api")]
        let file_label = file_label.dev_track_with_state(
            format!("ab.path.{path}.file"),
            DevElementState::default().text(file_name.to_string()),
        );
        section = section.child(file_label);
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
                .child(Text::caption(workflow_text.empty_pass_through)),
        );
    } else {
        let len = plugins.len();
        for (sub_idx, plugin) in plugins.iter().enumerate() {
            section = section.child(render_sub_plugin_card(
                path, plugin_idx, sub_idx, plugin, len, rack_text, text, theme, cx,
            ));
        }
    }

    section
}

fn display_file_name(path: &str) -> &str {
    path.rsplit(['/', '\\'])
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

    for (type_key, display_name) in allowed_plugin_types() {
        let plugin_type = type_key.to_string();
        let button = Button::new(
            SharedString::from(format!("ab-add-{path}-{type_key}")),
            display_name,
        )
        .aria_label(display_name)
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
        }));
        #[cfg(feature = "dev-api")]
        let button = button.dev_track(format!(
            "ab.path.{path}.add.{}",
            type_key.to_ascii_lowercase().replace([' ', '/'], "-")
        ));
        menu = menu.child(button);
    }

    menu
}

fn render_sub_plugin_card(
    path: u8,
    plugin_idx: usize,
    sub_idx: usize,
    plugin: &PluginInRack,
    total: usize,
    rack_text: crate::app::i18n::PluginRackTranslations,
    text: ABCompareTranslations,
    theme: &Theme,
    cx: &mut Context<PlayerView>,
) -> AnyElement {
    let display_name = plugin_display_name(&plugin.plugin_type).unwrap_or(text.unknown);

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
            div().flex_1().min_w_0().overflow_hidden().child(
                Text::new(display_name)
                    .size(TextSize::Xs)
                    .weight(TextWeight::Medium)
                    .color(theme.text_primary)
                    .truncate(true),
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
            .aria_label(rack_text.move_plugin_up)
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
            .aria_label(rack_text.move_plugin_down)
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
        .aria_label(rack_text.remove_plugin)
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

    #[cfg(feature = "dev-api")]
    let card = card.dev_track(format!(
        "ab.path.{path}.plugin.{sub_idx}.{}",
        plugin
            .plugin_type
            .to_ascii_lowercase()
            .replace([' ', '/'], "-")
    ));
    card.into_any_element()
}

fn plugin_display_name(plugin_type: &str) -> Option<&'static str> {
    for (key, name) in allowed_plugin_types() {
        if key == plugin_type {
            return Some(name);
        }
    }
    None
}
