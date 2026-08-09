const CROSSOVER_PRESET_KEYS: [&str; 3] = ["balanced", "wide-mids", "high-detail"];

pub(crate) fn crossover_preset_key(preset: i32) -> Option<&'static str> {
    let index = usize::try_from(preset.checked_sub(1)?).ok()?;
    CROSSOVER_PRESET_KEYS.get(index).copied()
}

pub(crate) fn crossover_preset_from_key(key: &str) -> Option<i32> {
    CROSSOVER_PRESET_KEYS
        .iter()
        .position(|candidate| *candidate == key)
        .map(|index| index as i32 + 1)
}

pub(crate) fn preset_frequencies(preset: i32) -> Option<[f64; 4]> {
    let index = usize::try_from(preset.checked_sub(1)?).ok()?;
    let &(a, b, c, d) = sotf_plugins::plugin_multiband_compressor::CROSSOVER_PRESETS.get(index)?;
    Some([a as f64, b as f64, c as f64, d as f64])
}

pub(crate) fn matching_crossover_preset(crossovers: [f64; 4]) -> i32 {
    (1..=3)
        .find(|&preset| {
            preset_frequencies(preset).is_some_and(|expected| {
                expected
                    .iter()
                    .zip(crossovers)
                    .all(|(expected, actual)| (expected - actual).abs() <= 0.5)
            })
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_presets_map_to_documented_frequency_profiles() {
        assert_eq!(
            preset_frequencies(1),
            Some([200.0, 2_000.0, 8_000.0, 12_000.0])
        );
        assert_eq!(
            preset_frequencies(2),
            Some([100.0, 3_000.0, 8_000.0, 12_000.0])
        );
        assert_eq!(
            preset_frequencies(3),
            Some([250.0, 4_000.0, 10_000.0, 14_000.0])
        );
        assert_eq!(preset_frequencies(0), None);
    }

    #[test]
    fn edited_crossovers_are_reported_as_custom() {
        assert_eq!(
            matching_crossover_preset([200.0, 2_000.0, 8_000.0, 12_000.0]),
            1
        );
        assert_eq!(
            matching_crossover_preset([210.0, 2_000.0, 8_000.0, 12_000.0]),
            0
        );
        assert_eq!(crossover_preset_key(0), None);
        assert_eq!(crossover_preset_from_key("custom"), None);
    }
}
