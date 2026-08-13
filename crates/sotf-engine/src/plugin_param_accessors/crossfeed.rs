use sotf_plugins::{CrossfeedMode, CrossfeedPreset};

pub(super) fn crossfeed_mode_to_index(mode: &CrossfeedMode) -> f64 {
    match mode {
        CrossfeedMode::Off => 0.0,
        CrossfeedMode::Bauer => 1.0,
        CrossfeedMode::Meier => 2.0,
        CrossfeedMode::Mb => 3.0,
        CrossfeedMode::Hrtf => 4.0,
    }
}

pub(super) fn crossfeed_preset_to_index(preset: &CrossfeedPreset) -> f64 {
    match preset {
        CrossfeedPreset::Default => 0.0,
        CrossfeedPreset::Cmoy => 1.0,
        CrossfeedPreset::Meier => 2.0,
        CrossfeedPreset::Mb => 3.0,
        CrossfeedPreset::Off => 4.0,
        CrossfeedPreset::Hrtf => 5.0,
    }
}
