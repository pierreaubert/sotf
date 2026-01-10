//! Keybindings settings content

use crate::app::keybindings::{KeybindingCategory, KeymapPreset, get_documented_keybindings};
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant};
use std::collections::HashMap;

impl PlayerView {
    pub(crate) fn render_keybindings_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let current_preset = state.app.ui_state.keymap_preset;
        let theme = state.app.ui_state.theme.clone();

        // Build comparison data: action -> preset -> key
        let comparison = build_keybinding_comparison();

        // Group by category
        let mut by_category: HashMap<
            KeybindingCategory,
            Vec<(String, HashMap<KeymapPreset, String>)>,
        > = HashMap::new();
        for (action, cat, keys) in comparison {
            by_category.entry(cat).or_default().push((action, keys));
        }

        div()
            .flex()
            .flex_col()
            .gap_6()
            .size_full()
            // Preset selection
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .child("Keymap Preset"),
                    )
                    .child({
                        let button_theme = theme.to_button_theme();
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .children(KeymapPreset::all().iter().map(|preset| {
                                let is_selected = current_preset == *preset;
                                let preset = *preset;
                                let btn_theme = button_theme.clone();
                                Button::new(
                                    SharedString::from(format!("preset-{}", preset.name())),
                                    preset.name(),
                                )
                                .variant(if is_selected {
                                    ButtonVariant::Primary
                                } else {
                                    ButtonVariant::Secondary
                                })
                                .size(ButtonSize::Sm)
                                .selected(is_selected)
                                .theme(btn_theme)
                                .build()
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                        view.state.update(cx, |state, _cx| {
                                            state.app.set_keymap_preset(preset);
                                        });
                                        cx.notify();
                                    }),
                                )
                            }))
                    })
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.text_muted)
                            .child(current_preset.description()),
                    ),
            )
            // Comparison table
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .child("Keybinding Comparison"),
                    )
                    .child(
                        div()
                            .id("keybindings-table")
                            .flex()
                            .flex_col()
                            .overflow_y_scroll()
                            .max_h(px(500.0))
                            .border_1()
                            .border_color(theme.border)
                            .rounded_md()
                            .bg(theme.surface)
                            // Table header
                            .child(render_table_header(&theme))
                            // Table body by category
                            .children(KeybindingCategory::all().iter().filter_map(|category| {
                                by_category.get(category).map(|rows| {
                                    render_category_section(*category, rows, &theme, current_preset)
                                })
                            })),
                    ),
            )
    }
}

fn render_table_header(theme: &crate::app::Theme) -> impl IntoElement {
    div()
        .flex()
        .w_full()
        .bg(theme.surface_hover)
        .border_b_1()
        .border_color(theme.border)
        .px_3()
        .py_2()
        .child(
            div()
                .flex_1()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_muted)
                .child("Action"),
        )
        .child(
            div()
                .w(px(100.0))
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_muted)
                .text_align(gpui::TextAlign::Center)
                .child("Default"),
        )
        .child(
            div()
                .w(px(100.0))
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_muted)
                .text_align(gpui::TextAlign::Center)
                .child("Vim"),
        )
        .child(
            div()
                .w(px(100.0))
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_muted)
                .text_align(gpui::TextAlign::Center)
                .child("Emacs"),
        )
        .child(
            div()
                .w(px(100.0))
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_muted)
                .text_align(gpui::TextAlign::Center)
                .child("VSCode"),
        )
}

fn render_category_section(
    category: KeybindingCategory,
    rows: &[(String, HashMap<KeymapPreset, String>)],
    theme: &crate::app::Theme,
    current_preset: KeymapPreset,
) -> impl IntoElement {
    let accent = theme.accent;
    let border = theme.border;
    let background = theme.background;
    let text_secondary = theme.text_secondary;
    let text_muted = theme.text_muted;
    let surface_hover = theme.surface_hover;

    let row_elements: Vec<_> = rows
        .iter()
        .map(|(action, keys)| {
            render_comparison_row(
                action.clone(),
                keys.clone(),
                current_preset,
                accent,
                text_secondary,
                text_muted,
                border,
                surface_hover,
            )
        })
        .collect();

    div()
        .flex()
        .flex_col()
        // Category header
        .child(
            div()
                .w_full()
                .bg(background)
                .border_b_1()
                .border_color(border)
                .px_3()
                .py_1()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(accent)
                        .child(category.name()),
                ),
        )
        // Rows
        .children(row_elements)
}

fn render_comparison_row(
    action: String,
    keys: HashMap<KeymapPreset, String>,
    current_preset: KeymapPreset,
    accent: gpui::Rgba,
    text_secondary: gpui::Rgba,
    text_muted: gpui::Rgba,
    border: gpui::Rgba,
    surface_hover: gpui::Rgba,
) -> impl IntoElement {
    let presets = [
        KeymapPreset::Default,
        KeymapPreset::Vim,
        KeymapPreset::Emacs,
        KeymapPreset::VSCode,
    ];

    let key_cells: Vec<_> = presets
        .iter()
        .map(|preset| {
            let key = keys.get(preset).map(|s| s.as_str()).unwrap_or("-");
            let is_current = *preset == current_preset;
            div()
                .w(px(100.0))
                .text_xs()
                .text_align(gpui::TextAlign::Center)
                .text_color(if is_current { accent } else { text_muted })
                .font_weight(if is_current {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::NORMAL
                })
                .child(key.to_string())
        })
        .collect();

    div()
        .flex()
        .w_full()
        .border_b_1()
        .border_color(border)
        .px_3()
        .py_1()
        .hover(move |s| s.bg(surface_hover))
        .child(
            div()
                .flex_1()
                .text_xs()
                .text_color(text_secondary)
                .child(action),
        )
        .children(key_cells)
}

/// Build a comparison of keybindings across all presets
/// Returns: Vec<(action_description, category, HashMap<preset, key>)>
fn build_keybinding_comparison() -> Vec<(String, KeybindingCategory, HashMap<KeymapPreset, String>)>
{
    // Collect all unique actions across all presets
    let mut action_map: HashMap<String, (KeybindingCategory, HashMap<KeymapPreset, String>)> =
        HashMap::new();

    for preset in KeymapPreset::all() {
        let bindings = get_documented_keybindings(*preset);
        for binding in bindings {
            let entry = action_map
                .entry(binding.description.to_string())
                .or_insert_with(|| (binding.category, HashMap::new()));
            entry.1.insert(*preset, binding.key.to_string());
        }
    }

    // Convert to vec and sort by category then action name
    let mut result: Vec<_> = action_map
        .into_iter()
        .map(|(action, (cat, keys))| (action, cat, keys))
        .collect();

    result.sort_by(|a, b| {
        let cat_order = |c: &KeybindingCategory| match c {
            KeybindingCategory::Playback => 0,
            KeybindingCategory::Navigation => 1,
            KeybindingCategory::ScreenSwitch => 2,
            KeybindingCategory::Library => 3,
            KeybindingCategory::Queue => 4,
            KeybindingCategory::Plugins => 5,
            KeybindingCategory::LevelMeters => 6,
            KeybindingCategory::System => 7,
        };
        cat_order(&a.1).cmp(&cat_order(&b.1)).then(a.0.cmp(&b.0))
    });

    result
}
