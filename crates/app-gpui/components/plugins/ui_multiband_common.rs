use crate::app::AppState;
use crate::app::types::PluginUpdateType;
use crate::components::design::Ds;
use crate::components::plugins::editing::PluginEditingManager;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{NumberInput, NumberInputSize};

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
