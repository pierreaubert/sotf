use math_audio_iir_fir::{Biquad, BiquadFilterType};
use sotf_audio::LoudnessCompensation;
use sotf_plugins::{CrossfeedMode, CrossfeedPreset};

pub(super) fn parse_loudness_compensation(
    vals: &[f64],
) -> Result<Option<LoudnessCompensation>, String> {
    let (ref_level, low, high) = match vals {
        [r, l] => (*r, *l, *l),
        [r, l, h] => (*r, *l, *h),
        _ => return Err("Expected 2 or 3 values: REF,LOW[,HIGH]".to_string()),
    };
    LoudnessCompensation::new(ref_level, low, high)
        .map(Some)
        .map_err(|e| e.to_string())
}

pub(super) fn parse_crossfeed_mode(mode: &str) -> Result<CrossfeedMode, String> {
    match mode.to_lowercase().as_str() {
        "bauer" => Ok(CrossfeedMode::Bauer),
        "meier" => Ok(CrossfeedMode::Meier),
        "multiband" | "mb" => Ok(CrossfeedMode::Mb),
        "hrtf" => Ok(CrossfeedMode::Hrtf),
        "off" => Ok(CrossfeedMode::Off),
        _ => Err(format!(
            "Invalid crossfeed mode '{}'. Valid: bauer, meier, multiband/mb, hrtf, off",
            mode
        )),
    }
}

pub(super) fn parse_crossfeed_preset(preset: &str) -> Result<CrossfeedPreset, String> {
    match preset.to_lowercase().as_str() {
        "default" => Ok(CrossfeedPreset::Default),
        "cmoy" => Ok(CrossfeedPreset::Cmoy),
        "meier" => Ok(CrossfeedPreset::Meier),
        "multiband" | "mb" => Ok(CrossfeedPreset::Mb),
        "off" => Ok(CrossfeedPreset::Off),
        "hrtf" => Ok(CrossfeedPreset::Hrtf),
        _ => Err(format!(
            "Invalid crossfeed preset '{}'. Valid: default, cmoy, meier, multiband/mb, hrtf, off",
            preset
        )),
    }
}

pub(super) fn parse_filter_type(type_str: &str) -> Result<BiquadFilterType, String> {
    match type_str.to_uppercase().as_str() {
        "PK" | "PEAK" => Ok(BiquadFilterType::Peak),
        "LS" | "LOWSHELF" => Ok(BiquadFilterType::Lowshelf),
        "HS" | "HIGHSHELF" => Ok(BiquadFilterType::Highshelf),
        "LP" | "LOWPASS" => Ok(BiquadFilterType::Lowpass),
        "HP" | "HIGHPASS" => Ok(BiquadFilterType::Highpass),
        "NO" | "NOTCH" => Ok(BiquadFilterType::Notch),
        "BP" | "BANDPASS" => Ok(BiquadFilterType::Bandpass),
        _ => Err(format!(
            "Unknown filter type '{}'. Valid types: PK/PEAK, LS/LOWSHELF, HS/HIGHSHELF, LP/LOWPASS, HP/HIGHPASS, NO/NOTCH, BP/BANDPASS",
            type_str
        )),
    }
}

pub(super) fn parse_filters(
    filter_strings: &[String],
    sample_rate: f64,
) -> Result<Vec<Biquad>, String> {
    if !sample_rate.is_finite() || sample_rate <= 0.0 {
        return Err(format!(
            "Invalid sample rate for filter construction: {}",
            sample_rate
        ));
    }
    filter_strings
        .iter()
        .map(|filter_str| {
            let parts: Vec<&str> = filter_str.split(':').collect();

            let (filter_type, frequency, q, gain) = match parts.len() {
                3 => {
                    let frequency = parts[0]
                        .parse::<f64>()
                        .map_err(|_| format!("Invalid frequency: {}", parts[0]))?;
                    let q = parts[1]
                        .parse::<f64>()
                        .map_err(|_| format!("Invalid Q: {}", parts[1]))?;
                    let gain = parts[2]
                        .parse::<f64>()
                        .map_err(|_| format!("Invalid gain: {}", parts[2]))?;
                    (BiquadFilterType::Peak, frequency, q, gain)
                }
                4 => {
                    let filter_type = parse_filter_type(parts[0])?;
                    let frequency = parts[1]
                        .parse::<f64>()
                        .map_err(|_| format!("Invalid frequency: {}", parts[1]))?;
                    let q = parts[2]
                        .parse::<f64>()
                        .map_err(|_| format!("Invalid Q: {}", parts[2]))?;
                    let gain = parts[3]
                        .parse::<f64>()
                        .map_err(|_| format!("Invalid gain: {}", parts[3]))?;
                    (filter_type, frequency, q, gain)
                }
                _ => {
                    return Err(format!(
                        "Invalid filter format '{}'. Expected 'freq:q:gain' or 'type:freq:q:gain'",
                        filter_str
                    ));
                }
            };

            if !(20.0..=20000.0).contains(&frequency) {
                return Err(format!(
                    "Frequency must be between 20 and 20000 Hz, got {}",
                    frequency
                ));
            }
            if q <= 0.0 || q > 100.0 {
                return Err(format!("Q must be between 0 and 100, got {}", q));
            }
            if gain.abs() > 30.0 {
                return Err(format!("Gain must be between -30 and +30 dB, got {}", gain));
            }
            // Frequencies are in Hz; the design rate must satisfy Nyquist.
            let nyquist_hz = sample_rate / 2.0;
            if frequency >= nyquist_hz {
                return Err(format!(
                    "Frequency {} Hz must be below the Nyquist rate ({:.1} Hz at {:.0} Hz sample rate)",
                    frequency, nyquist_hz, sample_rate
                ));
            }

            Ok(Biquad::new(filter_type, frequency, sample_rate, q, gain))
        })
        .collect()
}

/// Parse channel mapping specification and create matrix plugin config.
///
/// `"_"` output positions are gaps: they consume no input and stay silent.
/// Gaps are encoded sparsely — the returned `output_channel_map` lists only
/// the physical (0-indexed) hardware channels that are driven, so a skipped
/// physical index IS the gap.
///
/// `physical_output_channels` (max hardware index + 1, gaps included) sizes
/// the device buffer; it must be used instead of `output_channel_map.len()`
/// whenever a physical width is needed.
#[allow(clippy::type_complexity)]
pub(super) fn parse_channel_mapping(
    mapping_str: &str,
) -> Result<(Vec<usize>, Vec<usize>, Vec<f32>, usize), String> {
    let parts: Vec<&str> = mapping_str.split("->").collect();
    if parts.len() != 2 {
        return Err(format!(
            "Invalid mapping format '{}'. Expected 'in1,in2,...->out1,out2,...'",
            mapping_str
        ));
    }

    let input_channels: Result<Vec<usize>, _> = parts[0]
        .split(',')
        .map(|s| {
            s.trim()
                .parse::<usize>()
                .map_err(|_| format!("Invalid input channel: '{}'", s))
        })
        .collect();
    let input_channels = input_channels?;

    if input_channels.is_empty() {
        return Err("No input channels specified".to_string());
    }

    // Channels are 1-indexed in the user-facing syntax; reject 0 explicitly
    // to avoid `ch - 1` underflowing to `usize::MAX` below.
    if let Some(&bad) = input_channels.iter().find(|&&ch| ch == 0) {
        return Err(format!(
            "Input channel index must be >= 1 (1-indexed), got {}",
            bad
        ));
    }

    let output_spec: Vec<&str> = parts[1].split(',').map(|s| s.trim()).collect();
    if output_spec.is_empty() {
        return Err("No output channels specified".to_string());
    }

    let mut channel_map: Vec<Option<usize>> = Vec::new();
    let mut physical_output_channels = 0;

    for spec in output_spec.iter() {
        if *spec == "_" {
            channel_map.push(None);
        } else {
            let hw_ch = spec
                .parse::<usize>()
                .map_err(|_| format!("Invalid output channel: '{}'", spec))?;
            if hw_ch == 0 {
                return Err("Channel indices must be >= 1 (1-indexed)".to_string());
            }
            channel_map.push(Some(hw_ch - 1));
            physical_output_channels = physical_output_channels.max(hw_ch);
        }
    }

    let non_gap_outputs: Vec<_> = channel_map.iter().filter_map(|&x| x).collect();
    if non_gap_outputs.len() != input_channels.len() {
        return Err(format!(
            "Mismatch: {} input channels but {} non-gap output positions",
            input_channels.len(),
            non_gap_outputs.len()
        ));
    }

    let input_channel_map: Vec<usize> = input_channels.iter().map(|&ch| ch - 1).collect();
    let output_channel_map: Vec<usize> = channel_map.iter().filter_map(|&x| x).collect();

    let input_count = input_channel_map.len();
    let output_count = output_channel_map.len();

    let mut matrix = vec![0.0f32; output_count * input_count];
    for i in 0..output_count.min(input_count) {
        matrix[i * input_count + i] = 1.0;
    }

    Ok((
        input_channel_map,
        output_channel_map,
        matrix,
        physical_output_channels,
    ))
}
