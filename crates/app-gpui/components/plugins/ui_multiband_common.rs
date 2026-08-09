use crate::app::AppState;
use crate::app::types::PluginUpdateType;
use crate::components::design::Ds;
use crate::components::plugins::editing::PluginEditingManager;
use crate::multiband_presets::{
    crossover_preset_from_key, crossover_preset_key, matching_crossover_preset, preset_frequencies,
};
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{ButtonSet, ButtonSetOption, ButtonSetSize, NumberInput, NumberInputSize};

pub(crate) fn render_crossover_preset_editor(
    d: &Ds,
    id: &'static str,
    compact_detail_id: &'static str,
    preset_label: &'static str,
    preset_option_labels: [&'static str; 4],
    compact: bool,
    entity: Entity<AppState>,
    plugin_idx: usize,
    crossovers: [f64; 4],
    theme: &Theme,
) -> impl IntoElement {
    // Custom is derived from the crossover values rather than an action: a
    // named button applies a profile, while any manual edit updates this status.
    let inferred = matching_crossover_preset(crossovers);
    let selected = crossover_preset_key(inferred).unwrap_or("");
    let status = usize::try_from(inferred)
        .ok()
        .and_then(|index| preset_option_labels.get(index))
        .copied()
        .unwrap_or(preset_option_labels[0]);
    let make_button_set = |set_id: &'static str, presets: &[i32]| {
        let options = presets
            .iter()
            .filter_map(|&preset| {
                let key = crossover_preset_key(preset)?;
                let label = preset_option_labels.get(usize::try_from(preset).ok()?)?;
                Some(ButtonSetOption::new(key, *label))
            })
            .collect();
        let entity = entity.clone();

        ButtonSet::new(set_id)
            .options(options)
            .selected(selected)
            .size(ButtonSetSize::Sm)
            .theme(theme.to_button_set_theme())
            .on_change(move |value, _window, cx| {
                let Some(preset) = crossover_preset_from_key(value.as_ref()) else {
                    return;
                };
                entity.update(cx, |state, cx| {
                    state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                    state.app.set_plugin_param(plugin_idx, 1, preset as f64);
                    if let Some(frequencies) = preset_frequencies(preset) {
                        for (offset, frequency) in frequencies.into_iter().enumerate() {
                            state
                                .app
                                .set_plugin_param(plugin_idx, 2 + offset, frequency);
                        }
                    }
                    cx.notify();
                });
            })
    };

    div()
        .flex()
        .flex_col()
        .gap(d.grid)
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(d.gap)
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_muted)
                .child(preset_label)
                .child(status),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(d.grid)
                .when(compact, |this| {
                    this.child(make_button_set(id, &[1, 2]))
                        .child(make_button_set(compact_detail_id, &[3]))
                })
                .when(!compact, |this| this.child(make_button_set(id, &[1, 2, 3]))),
        )
}

pub(crate) fn render_band_count_editor(
    d: &Ds,
    id: &'static str,
    entity: Entity<AppState>,
    plugin_idx: usize,
    current: usize,
    min: f64,
    max: f64,
    label: &'static str,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(d.grid)
        .child(
            div()
                .text_size(d.text_xs)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_secondary)
                .child(label),
        )
        .child(
            div().w(rems(5.0)).child(
                NumberInput::new(id)
                    .value(current as f64)
                    .min(min)
                    .max(max)
                    .step(1.0)
                    .decimals(0)
                    .size(NumberInputSize::Xs)
                    .aria_label(label)
                    .on_change(move |value, _window, cx| {
                        entity.update(cx, |state, _| {
                            state.app.set_plugin_param(plugin_idx, 0, value);
                            state.app.plugin_state.update_state.pending_plugin_update =
                                Some(PluginUpdateType::Structural);
                        });
                    }),
            ),
        )
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
