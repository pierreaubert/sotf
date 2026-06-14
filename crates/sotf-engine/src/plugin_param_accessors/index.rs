use super::aae::aae_room_presets;
use super::aae::aae_speaker_configs;
use super::ambisonics::ambisonics_layouts;
use super::crossover::crossover_types;
use super::de::de_esser_modes;
use super::detection::detection_modes;
use super::hpf::hpf_orders;
use super::speaker::speaker_configs;
use sotf_plugins::{CrossfeedMode, CrossfeedPreset, SpectralTiltCorrection, TiltReferenceFreq};

pub(super) fn index_to_speaker_config(index: f64) -> String {
    let idx = index as usize;
    speaker_configs().get(idx).unwrap_or(&"5.1").to_string()
}

pub(super) fn index_to_de_esser_mode(index: f64) -> String {
    let idx = index as usize;
    de_esser_modes()
        .get(idx)
        .unwrap_or(&"Split-Band")
        .to_string()
}

pub(super) fn index_to_aae_speaker_config(index: f64) -> String {
    let idx = index as usize;
    aae_speaker_configs().get(idx).unwrap_or(&"5.1").to_string()
}

pub(super) fn index_to_aae_room_preset(index: f64) -> String {
    let idx = index as usize;
    aae_room_presets().get(idx).unwrap_or(&"medium").to_string()
}

pub(super) fn index_to_crossfeed_mode(index: f64) -> CrossfeedMode {
    match index as usize {
        0 => CrossfeedMode::Off,
        1 => CrossfeedMode::Bauer,
        2 => CrossfeedMode::Meier,
        3 => CrossfeedMode::Mb,
        _ => CrossfeedMode::Off,
    }
}

pub(super) fn index_to_crossfeed_preset(index: f64) -> CrossfeedPreset {
    match index as usize {
        0 => CrossfeedPreset::Default,
        1 => CrossfeedPreset::Cmoy,
        2 => CrossfeedPreset::Meier,
        3 => CrossfeedPreset::Mb,
        4 => CrossfeedPreset::Off,
        _ => CrossfeedPreset::Default,
    }
}

pub(super) fn index_to_detection_mode(index: f64) -> String {
    let idx = index as usize;
    detection_modes().get(idx).unwrap_or(&"Peak").to_string()
}

pub(super) fn index_to_hpf_order(index: f64) -> String {
    let idx = index as usize;
    hpf_orders().get(idx).unwrap_or(&"2nd").to_string()
}

pub(super) fn index_to_ambisonics_layout(index: f64) -> String {
    let idx = index as usize;
    ambisonics_layouts().get(idx).unwrap_or(&"5.1").to_string()
}

pub(super) fn index_to_crossover_type(index: f64) -> String {
    let idx = index as usize;
    crossover_types().get(idx).unwrap_or(&"LR24").to_string()
}

pub(super) fn index_to_spectral_tilt(index: f64) -> SpectralTiltCorrection {
    match index as usize {
        0 => SpectralTiltCorrection::None,
        1 => SpectralTiltCorrection::ThreeDbPerOctave,
        2 => SpectralTiltCorrection::SixDbPerOctave,
        _ => SpectralTiltCorrection::Pink,
    }
}

pub(super) fn index_to_tilt_reference(index: f64) -> TiltReferenceFreq {
    match index as usize {
        0 => TiltReferenceFreq::Standard,
        1 => TiltReferenceFreq::OneKilohertz,
        2 => TiltReferenceFreq::TwoKilohertz,
        _ => TiltReferenceFreq::MinFreq,
    }
}
