use crate::app::AppState;
use crate::components::plugins::editing::PluginEditingManager;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{ButtonSet, ButtonSetOption, ButtonSetSize};

pub(crate) fn render_crossover_preset(
    id: &'static str,
    entity: Entity<AppState>,
    plugin_idx: usize,
    current: i32,
    theme: &Theme,
) -> impl IntoElement {
    ButtonSet::new(id)
        .options(
            (0..=3)
                .map(|preset| ButtonSetOption::new(preset.to_string(), format!("P{}", preset + 1)))
                .collect(),
        )
        .selected(current.clamp(0, 3).to_string())
        .size(ButtonSetSize::Sm)
        .theme(theme.to_button_set_theme())
        .on_change(move |value, _window, cx| {
            let Ok(preset) = value.parse::<i32>() else {
                return;
            };
            entity.update(cx, |state, _| {
                state.app.set_plugin_param(plugin_idx, 1, preset as f64);
            });
        })
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
