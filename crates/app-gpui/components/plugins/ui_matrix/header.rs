//! Compact Matrix preset selection.

// Rust guideline compliant 2026-02-21

use crate::app::AppState;
use crate::app::i18n::PluginCommonTranslations;
use crate::components::design::Ds;
use crate::theme::Theme;
use gpui::{AnyElement, Entity, InteractiveElement, IntoElement, ParentElement, Styled, div};
use gpui_ui_kit::{Select, SelectOption, SelectSize, Text};
use sotf_audio_player::{
    PluginSettings, PluginUpdateEffect, apply_matrix_preset, available_matrix_presets,
    detect_matrix_preset,
};

/// Render the connected preset dropdown beside the Matrix title.
#[expect(
    clippy::too_many_lines,
    reason = "Keep the declarative layout and its interaction handlers together"
)]
pub(crate) fn render_header(
    d: &Ds,
    entity: Entity<AppState>,
    instance_id: usize,
    settings: &PluginSettings,
    theme: &Theme,
    cx: &gpui::App,
) -> AnyElement {
    let PluginSettings::Matrix {
        input_channels,
        output_channels,
        matrix,
        ..
    } = settings
    else {
        return div().into_any_element();
    };
    let text = PluginCommonTranslations::for_language(entity.read(cx).app.ui_state.language);
    let current = detect_matrix_preset(*input_channels, *output_channels, matrix);
    let mut options: Vec<_> = available_matrix_presets(*input_channels, *output_channels)
        .into_iter()
        .map(|name| {
            SelectOption::new(name, name)
                .disabled(*input_channels > 2 && matches!(name, "M/S Encode" | "M/S Decode"))
        })
        .collect();
    options.push(SelectOption::new("Custom", text.label("Custom")).disabled(true));
    let open = entity
        .read(cx)
        .app
        .plugin_state
        .plugin_ui_state
        .matrix_preset_menu
        == Some(instance_id);
    let highlight = entity
        .read(cx)
        .app
        .plugin_state
        .plugin_ui_state
        .matrix_choice_highlight;
    let selected_index = options
        .iter()
        .position(|option| option.value.as_ref() == current);
    let highlight_entity = entity.downgrade();
    let toggle = entity.downgrade();
    let select = Select::new(("matrix-preset", instance_id))
        .aria_label(text.label("Matrix preset"))
        .options(options)
        .selected(current)
        .size(SelectSize::Xs)
        .is_open(open)
        .highlighted_index(highlight)
        .theme(theme.to_select_theme())
        .on_toggle(move |open, _, cx| {
            let _ = toggle.update(cx, |state, cx| {
                let ui = &mut state.app.plugin_state.plugin_ui_state;
                ui.matrix_preset_menu = open.then_some(instance_id);
                ui.matrix_choice_highlight = open.then_some(selected_index).flatten();
                if open {
                    ui.matrix_route_menu = None;
                }
                cx.notify();
            });
        })
        .on_highlight(move |index, _, cx| {
            let _ = highlight_entity.update(cx, |state, cx| {
                state
                    .app
                    .plugin_state
                    .plugin_ui_state
                    .matrix_choice_highlight = index;
                cx.notify();
            });
        })
        .on_change(move |name, _, cx| {
            entity.update(cx, |state, cx| {
                if let Some(PluginSettings::Matrix {
                    input_channels,
                    output_channels,
                    matrix,
                    ..
                }) = state
                    .app
                    .plugin_state
                    .editor_settings_mut_by_instance_id(instance_id)
                {
                    if !available_matrix_presets(*input_channels, *output_channels)
                        .contains(&name.as_ref())
                        || (*input_channels > 2
                            && matches!(name.as_ref(), "M/S Encode" | "M/S Decode"))
                    {
                        return;
                    }
                    apply_matrix_preset(*input_channels, *output_channels, matrix, name.as_ref());
                    state
                        .app
                        .plugin_state
                        .record_editor_effect(PluginUpdateEffect::Structural);
                }
                state.app.plugin_state.plugin_ui_state.matrix_preset_menu = None;
                cx.notify();
            });
        });
    #[cfg(feature = "dev-api")]
    let select = {
        use crate::app::dev_api::DevTrackExt;
        select.dev_track("matrix.preset")
    };
    div()
        .key_context("PluginLayoutChoiceSelect")
        .flex_1()
        .min_w_0()
        .flex()
        .items_center()
        .gap(d.grid)
        .child(Text::label(text.label("Preset")))
        .child(select)
        .into_any_element()
}
