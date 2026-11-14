//! Stable desktop addresses for independent crossover channels.

use crate::PluginSettings;
use sotf_plugins::param_specs::{
    ParamSpec,
    crossover::{CHANNEL_PARAM_START, CHANNEL_TEMPLATE},
};

pub(super) fn channel_count(settings: &PluginSettings) -> usize {
    match settings {
        PluginSettings::Crossover {
            channel_frequencies_hz,
            ..
        } if settings.param_value(4) == Some(1.0) => {
            channel_frequencies_hz.as_ref().map_or(0, Vec::len)
        }
        _ => 0,
    }
}

pub(super) fn spec(settings: &PluginSettings, index: usize) -> Option<&'static ParamSpec> {
    let offset = index.checked_sub(CHANNEL_PARAM_START)?;
    (offset / 2 < channel_count(settings)).then(|| &CHANNEL_TEMPLATE[offset % 2])
}

#[expect(
    clippy::cast_precision_loss,
    reason = "Crossover parameter specs bound channel counts and integer selections"
)]
pub(super) fn value(settings: &PluginSettings, index: usize) -> Option<f64> {
    spec(settings, index)?;
    let channel = (index - CHANNEL_PARAM_START) / 2;
    let PluginSettings::Crossover {
        channel_frequencies_hz,
        channel_modes,
        output,
        ..
    } = settings
    else {
        return None;
    };
    if (index - CHANNEL_PARAM_START).is_multiple_of(2) {
        channel_frequencies_hz.as_ref()?.get(channel).copied()
    } else {
        let mode = channel_modes
            .as_ref()
            .and_then(|m| m.get(channel))
            .unwrap_or(output);
        ["lowpass", "highpass", "mute", "passthrough"]
            .iter()
            .position(|candidate| mode.eq_ignore_ascii_case(candidate))
            .map(|i| i as f64)
    }
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Crossover parameter specs bound channel counts and integer selections"
)]
pub(super) fn set(settings: &mut PluginSettings, index: usize, value: f64) -> bool {
    let Some(spec) = spec(settings, index) else {
        return false;
    };
    if !value.is_finite() {
        return false;
    }
    let value = spec.clamp_f64(value / spec.display_scale);
    let channel = (index - CHANNEL_PARAM_START) / 2;
    let PluginSettings::Crossover {
        channel_frequencies_hz,
        channel_modes,
        ..
    } = settings
    else {
        return false;
    };
    if (index - CHANNEL_PARAM_START).is_multiple_of(2) {
        let Some(frequency) = channel_frequencies_hz
            .as_mut()
            .and_then(|f| f.get_mut(channel))
        else {
            return false;
        };
        *frequency = value;
    } else {
        let Some(mode) = channel_modes.as_mut().and_then(|m| m.get_mut(channel)) else {
            return false;
        };
        *mode = ["lowpass", "highpass", "mute", "passthrough"][value.round() as usize].into();
    }
    true
}

pub(super) fn reset(settings: &mut PluginSettings, index: usize) -> Option<bool> {
    if !matches!(settings, PluginSettings::Crossover { .. }) || index < CHANNEL_PARAM_START {
        return None;
    }
    Some(
        spec(settings, index)
            .is_some_and(|spec| set(settings, index, spec.default_f64() * spec.display_scale)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_plugins::plugin_crossover::CrossoverTopology;

    #[test]
    fn channel_addresses_edit_reset_and_preserve_neighbors() {
        let mut settings = PluginSettings::Crossover {
            crossover_type: "LR24".into(),
            frequency: 700.0,
            output: "both".into(),
            fir_taps: 1025,
            topology: Some(CrossoverTopology::PerChannel),
            band_count: None,
            extra_frequencies: vec![],
            channel_frequencies_hz: Some(vec![400.0, 900.0]),
            channel_modes: Some(vec!["lowpass".into(), "highpass".into()]),
        };
        assert_eq!(
            crate::controllers::plugin::desktop_param_indices(&settings),
            [0, 1, 2, 3, 4, 5, 6, 7, 100, 101, 102, 103]
        );
        assert!(super::super::set::set_plugin_param_value(
            &mut settings,
            102,
            1250.0,
            &mut false
        ));
        assert!(super::super::set::set_plugin_param_value(
            &mut settings,
            103,
            2.0,
            &mut false
        ));
        assert_eq!(value(&settings, 100), Some(400.0));
        assert_eq!(value(&settings, 101), Some(0.0));
        assert_eq!(value(&settings, 102), Some(1250.0));
        assert_eq!(value(&settings, 103), Some(2.0));
        assert!(crate::param_index_to_engine_param(&settings, 102).is_none());
        assert_eq!(reset(&mut settings, 102), Some(true));
        assert_eq!(reset(&mut settings, 103), Some(true));
        assert_eq!(value(&settings, 102), Some(1000.0));
        assert_eq!(value(&settings, 103), Some(0.0));
        let before = serde_json::to_value(&settings).unwrap();
        assert!(!set(&mut settings, 104, 600.0));
        assert!(!set(&mut settings, 102, f64::NAN));
        assert_eq!(serde_json::to_value(&settings).unwrap(), before);
        if let PluginSettings::Crossover { topology, .. } = &mut settings {
            *topology = Some(CrossoverTopology::Bands);
        }
        assert_eq!(channel_count(&settings), 0);
        assert!(!set(&mut settings, 100, 600.0));
    }
}
