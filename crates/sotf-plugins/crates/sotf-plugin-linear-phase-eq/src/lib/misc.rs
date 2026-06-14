#![allow(dead_code)]
use math_audio_iir_fir::BiquadFilterType;

/// Number of frequency points to sample for magnitude response.
/// Must cover 0 Hz to Nyquist with sufficient density.
pub(super) const MAG_RESPONSE_POINTS: usize = 4096;

pub(super) fn fir_length_from_index(index: usize) -> usize {
    match index {
        0 => 1024,
        1 => 2048,
        2 => 4096,
        3 => 8192,
        _ => 2048,
    }
}

pub(super) fn parse_filter_type(s: &str) -> Result<BiquadFilterType, String> {
    match s {
        "Peak" | "peak" => Ok(BiquadFilterType::Peak),
        "Lowshelf" | "lowshelf" => Ok(BiquadFilterType::Lowshelf),
        "Highshelf" | "highshelf" => Ok(BiquadFilterType::Highshelf),
        "Lowpass" | "lowpass" => Ok(BiquadFilterType::Lowpass),
        "Highpass" | "highpass" => Ok(BiquadFilterType::Highpass),
        other => Err(format!("Unknown filter type: {other}")),
    }
}

pub(super) fn filter_type_to_index(ft: BiquadFilterType) -> usize {
    match ft {
        BiquadFilterType::Peak => 0,
        BiquadFilterType::Lowshelf => 1,
        BiquadFilterType::Highshelf => 2,
        BiquadFilterType::Lowpass => 3,
        BiquadFilterType::Highpass => 4,
        // All other types map to Peak as default for this plugin
        _ => 0,
    }
}

pub(super) fn index_to_filter_type(index: usize) -> BiquadFilterType {
    match index {
        0 => BiquadFilterType::Peak,
        1 => BiquadFilterType::Lowshelf,
        2 => BiquadFilterType::Highshelf,
        3 => BiquadFilterType::Lowpass,
        4 => BiquadFilterType::Highpass,
        _ => BiquadFilterType::Peak,
    }
}
