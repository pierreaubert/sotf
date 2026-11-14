//! Shared compact multiband header and band-range labels.

// Rust guideline compliant 2026-02-21
#[cfg(feature = "dev-api")]
use crate::app::dev_api::DevTrackExt;
use crate::app::{AppState, i18n::PluginCommonTranslations};
use crate::components::design::Ds;
use crate::components::plugins::editing::PluginEditingManager;
use crate::components::plugins::ui_layout_renderer::render_compact_choice_param;
use crate::multiband_presets::{matching_crossover_preset, preset_frequencies};
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::{AnyElement, App, Entity, div, rems};
use gpui_ui_kit::{NumberInput, NumberInputSize, Select, SelectOption, SelectSize, Text};
use sotf_audio_player::PluginSettings;

#[expect(
    clippy::too_many_lines,
    reason = "Keep the declarative layout and its interaction handlers together"
)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Element builders accept owned snapshots and clone handles into deferred callbacks"
)]
#[expect(
    clippy::large_types_passed_by_value,
    reason = "Copy translation snapshots remain owned by returned elements and deferred callbacks"
)]
#[expect(
    clippy::cast_sign_loss,
    reason = "Bounded UI controls deliberately round parameter values and navigation indices"
)]
pub(crate) fn render_dense_multiband_header(
    d: &Ds,
    entity: Entity<AppState>,
    plugin_idx: usize,
    settings: &PluginSettings,
    text: PluginCommonTranslations,
    theme: &Theme,
    cx: &App,
    choice_indices: &[usize],
) -> AnyElement {
    let params = settings.param_specs();
    let change_entity = entity.clone();
    let count = NumberInput::new(("multiband-count", plugin_idx))
        .value(settings.param_value(0).unwrap_or(params[0].default_f64()))
        .min(params[0].min_f64())
        .max(params[0].max_f64())
        .step(1.0)
        .decimals(0)
        .size(NumberInputSize::Xs)
        .aria_label(text.bands)
        .theme(theme.to_number_input_theme())
        .on_change(move |value, _, cx| {
            change_entity.update(cx, |state, cx| {
                state.app.set_plugin_param(plugin_idx, 0, value);
                cx.notify();
            });
        });
    #[cfg(feature = "dev-api")]
    let count = count.dev_track(format!("plugin.param.{plugin_idx}.0.exact"));
    let frequencies = [2, 3, 4, 5].map(|index| {
        settings
            .param_value(index)
            .unwrap_or(params[index].default_f64())
    });
    let inferred = matching_crossover_preset(frequencies);
    let labels = text.multiband_presets.labels();
    let app = &entity.read(cx).app;
    let instance = app
        .plugin_state
        .graph_state
        .editing_graph_node_uuid
        .and_then(|id| app.plugin_state.routing_controller().graph.nodes.get(&id))
        .map(|node| node.plugin.id)
        .or_else(|| {
            app.plugin_state
                .graph
                .get_plugin(plugin_idx)
                .map(|plugin| plugin.id)
        })
        .unwrap_or(plugin_idx);
    let menu_key = (instance, 1);
    let open = app.plugin_ui.layout_choice_menu == Some(menu_key);
    let highlight = app.plugin_ui.layout_choice_highlight;
    let toggle_entity = entity.clone();
    let highlight_entity = entity.clone();
    let change_entity = entity.clone();
    let preset = Select::new(("multiband-preset", plugin_idx))
        .aria_label(text.label("Preset"))
        .options(
            labels
                .iter()
                .enumerate()
                .map(|(i, label)| SelectOption::new(i.to_string(), *label).disabled(i == 0))
                .collect(),
        )
        .selected(inferred.to_string())
        .is_open(open)
        .highlighted_index(open.then_some(highlight).flatten())
        .on_toggle(move |open, _, cx| {
            toggle_entity.update(cx, |state, cx| {
                state.app.plugin_ui.layout_choice_menu = open.then_some(menu_key);
                state.app.plugin_ui.layout_choice_highlight = open.then_some(inferred as usize);
                cx.notify();
            });
        })
        .on_highlight(move |index, _, cx| {
            highlight_entity.update(cx, |state, cx| {
                state.app.plugin_ui.layout_choice_highlight = index;
                cx.notify();
            });
        })
        .size(SelectSize::Xs)
        .theme(theme.to_select_theme())
        .on_change(move |value, _, cx| {
            let Some(preset) = value.parse::<i32>().ok() else {
                return;
            };
            // Custom is derived from actual crossovers, not a frequency profile.
            let Some(frequencies) = preset_frequencies(preset) else {
                return;
            };
            change_entity.update(cx, |state, cx| {
                state.app.set_plugin_param(plugin_idx, 1, f64::from(preset));
                for (offset, frequency) in frequencies.into_iter().enumerate() {
                    state
                        .app
                        .set_plugin_param(plugin_idx, offset + 2, frequency);
                }
                state.app.plugin_ui.layout_choice_menu = None;
                state.app.plugin_ui.layout_choice_highlight = None;
                cx.notify();
            });
        });
    #[cfg(feature = "dev-api")]
    let preset = preset.dev_track(format!("plugin.select.{plugin_idx}.1"));
    let mut row = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(d.grid)
        .child(Text::label(text.bands))
        .child(count)
        .child(Text::label(text.label("Preset")))
        .child(
            div()
                .key_context("PluginLayoutChoiceSelect")
                .w(rems(10.0))
                .child(preset),
        );
    for &index in choice_indices {
        let mut spec = params[index];
        spec.name = text.label(spec.name);
        row = row.child(render_compact_choice_param(
            d,
            entity.clone(),
            plugin_idx,
            index,
            &spec,
            settings.param_value(index).unwrap_or(spec.default_f64()),
            theme,
        ));
    }
    row.into_any_element()
}

/// Keep primary dynamics side by side when vertical space is constrained.
pub(crate) fn primary_uses_columns(width: f32, height: f32, scale: f32) -> bool {
    let scale = scale.max(0.01);
    let width = width / scale;
    let height = height / scale;
    // Below 480 logical pixels, six stacked controls plus band/header choices
    // exceed the rack. Two 240-pixel lanes let each label wrap above its
    // slider and exact value while retaining all primary controls;
    // narrower panes keep the single-column layout to avoid horizontal loss.
    width >= 800.0 || (width >= 480.0 && height < 480.0)
}

pub(crate) fn band_tab_label(band: usize, num_bands: usize, crossovers: [f64; 4]) -> String {
    if band == 0 {
        return "Global".to_string();
    }

    let lower = band.checked_sub(2).and_then(|index| crossovers.get(index));
    let upper = (band < num_bands)
        .then_some(band - 1)
        .and_then(|index| crossovers.get(index));

    match (lower, upper) {
        (None, Some(upper)) => format!("{band} · ≤{}", format_frequency(*upper)),
        (Some(lower), Some(upper)) => format!(
            "{band} · {}–{}",
            format_frequency(*lower),
            format_frequency(*upper)
        ),
        (Some(lower), None) => format!("{band} · ≥{}", format_frequency(*lower)),
        (None, None) => band.to_string(),
    }
}

fn format_frequency(frequency_hz: f64) -> String {
    if frequency_hz >= 1_000.0 {
        let khz = frequency_hz / 1_000.0;
        if (khz - khz.round()).abs() < 0.05 {
            format!("{khz:.0}k")
        } else {
            format!("{khz:.1}k")
        }
    } else {
        format!("{frequency_hz:.0}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_multiband_ranges() {
        let crossovers = [250.0, 2_000.0, 8_000.0, 16_000.0];
        assert_eq!(band_tab_label(0, 4, crossovers), "Global");
        assert_eq!(band_tab_label(1, 4, crossovers), "1 · ≤250");
        assert_eq!(band_tab_label(2, 4, crossovers), "2 · 250–2k");
        assert_eq!(band_tab_label(4, 4, crossovers), "4 · ≥8k");
    }
}
