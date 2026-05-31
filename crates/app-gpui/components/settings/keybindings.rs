//! Keybindings settings content

use crate::app::keybindings::{KeybindingCategory, KeymapPreset, get_documented_keybindings};
use crate::components::design::Ds;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{ButtonSet, ButtonSetOption};
use std::collections::HashMap;

impl PlayerView {
    #[allow(clippy::type_complexity)]
    pub(crate) fn render_keybindings_settings_content(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let d = Ds::from_cx(cx);
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
            .gap(d.section_lg)
            .size_full()
            .min_h(px(0.0))
            // Preset selection
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(d.gap_md)
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .child("Keymap Preset"),
                    )
                    .child({
                        let state_entity = self.state.clone();
                        ButtonSet::new("keymap-preset")
                            .options(
                                KeymapPreset::all()
                                    .iter()
                                    .map(|p| ButtonSetOption::new(p.name(), p.name()))
                                    .collect(),
                            )
                            .selected(current_preset.name())
                            .theme(theme.to_button_set_theme())
                            .on_change(move |value, _window, cx| {
                                let preset = KeymapPreset::all()
                                    .iter()
                                    .find(|p| p.name() == value.as_ref())
                                    .copied();
                                if let Some(preset) = preset {
                                    state_entity.update(cx, |state, _cx| {
                                        state.app.set_keymap_preset(preset);
                                    });
                                }
                            })
                    })
                    .child(
                        div()
                            .text_size(d.text_xs)
                            .text_color(theme.text_muted)
                            .child(current_preset.description()),
                    ),
            )
            // Comparison table
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.0))
                    .gap(d.gap_md)
                    .child(
                        div()
                            .text_size(d.text_sm)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_primary)
                            .child("Keybinding Comparison"),
                    )
                    .child(
                        div()
                            .id("keybindings-table")
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_h(px(0.0))
                            .overflow_y_scroll()
                            .border_1()
                            .border_color(theme.border)
                            .rounded(d.r_md)
                            .bg(theme.surface)
                            // Table header
                            .child(render_table_header(&d, &theme))
                            // Table body by category
                            .children(KeybindingCategory::all().iter().filter_map(|category| {
                                by_category.get(category).map(|rows| {
                                    render_category_section(
                                        &d,
                                        *category,
                                        rows,
                                        &theme,
                                        current_preset,
                                    )
                                })
                            })),
                    ),
            )
    }
}

fn render_table_header(d: &Ds, theme: &crate::app::Theme) -> impl IntoElement {
    div()
        .flex()
        .w_full()
        .bg(theme.surface_hover)
        .border_b_1()
        .border_color(theme.border)
        .px(d.pad_x)
        .py(d.pad_y)
        .child(
            div()
                .flex_1()
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_muted)
                .child("Action"),
        )
        .child(
            div()
                .w(rems(6.25))
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_muted)
                .text_align(gpui::TextAlign::Center)
                .child("Default"),
        )
        .child(
            div()
                .w(rems(6.25))
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_muted)
                .text_align(gpui::TextAlign::Center)
                .child("Vim"),
        )
        .child(
            div()
                .w(rems(6.25))
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_muted)
                .text_align(gpui::TextAlign::Center)
                .child("Emacs"),
        )
        .child(
            div()
                .w(rems(6.25))
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_muted)
                .text_align(gpui::TextAlign::Center)
                .child("VSCode"),
        )
}

fn render_category_section(
    d: &Ds,
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
                d,
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
                .px(d.pad_x)
                .py(d.pad_y_half)
                .child(
                    div()
                        .text_size(d.text_xs)
                        .font_weight(FontWeight::BOLD)
                        .text_color(accent)
                        .child(category.name()),
                ),
        )
        // Rows
        .children(row_elements)
}

fn render_comparison_row(
    d: &Ds,
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
                .w(rems(6.25))
                .text_size(d.text_xs)
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
        .px(d.pad_x)
        .py(d.pad_y_half)
        .hover(move |s| s.bg(surface_hover))
        .child(
            div()
                .flex_1()
                .text_size(d.text_xs)
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
