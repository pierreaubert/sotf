use super::build::build_octave_sweep_with_silence;
use super::misc::prepare_signal;
use super::signal_params::validate_signal_params;
use super::signal_type::SignalType;
use super::types::SignalParams;
use crate::signals::*;
use std::path::PathBuf;

/// One-stop measurement stimulus preparation: validate the parameters,
/// generate the raw signal, and apply the standard 20 ms fades + 250 ms
/// pre/post silence padding ([`prepare_signal`]).
///
/// CLI/TUI/GPUI should all route through this single entry point so every
/// frontend gets identical validation (Nyquist, start < end, amplitude
/// range) and a click-free, padded stimulus instead of re-implementing
/// the steps piecemeal.
///
/// `SignalType::Sweep` with [`SignalParams::OctaveSweep`] is self-timed:
/// `duration` is ignored by generation (as in [`generate_signal`]) but
/// must still be positive for validation. The octave sweep already
/// carries its own silence windows; the extra padding simply extends
/// them and the fades land on silence.
pub fn prepare_measurement_signal(
    signal_type: SignalType,
    params: &SignalParams,
    duration: f32,
    sample_rate: u32,
) -> Result<Vec<f32>, String> {
    validate_signal_params(signal_type, params, duration, sample_rate)?;
    let signal = generate_signal(signal_type, params, duration, sample_rate)?;
    Ok(prepare_signal(signal, sample_rate))
}

/// Generate a signal based on parameters
pub fn generate_signal(
    signal_type: SignalType,
    params: &SignalParams,
    duration: f32,
    sample_rate: u32,
) -> Result<Vec<f32>, String> {
    let signal = match (signal_type, params) {
        (SignalType::Tone, SignalParams::Tone { freq, amp }) => {
            gen_tone(*freq, *amp, sample_rate, duration)
        }
        (
            SignalType::TwoTone,
            SignalParams::TwoTone {
                freq1,
                amp1,
                freq2,
                amp2,
            },
        ) => gen_two_tone(*freq1, *amp1, *freq2, *amp2, sample_rate, duration),
        (
            SignalType::Sweep,
            SignalParams::Sweep {
                start_freq,
                end_freq,
                amp,
            },
        ) => gen_log_sweep(*start_freq, *end_freq, *amp, sample_rate, duration),
        // OctaveSweep is self-timed: the `duration` argument is ignored because
        // the total length is determined by the octave budget and silence windows.
        (
            SignalType::Sweep,
            SignalParams::OctaveSweep {
                start_freq,
                end_freq,
                amp,
                bass_octave_duration_s,
                pre_silence_s,
                post_silence_s,
            },
        ) => build_octave_sweep_with_silence(
            *start_freq,
            *end_freq,
            *amp,
            *bass_octave_duration_s,
            *pre_silence_s,
            *post_silence_s,
            sample_rate,
        ),
        (SignalType::WhiteNoise, SignalParams::Noise { amp }) => {
            gen_white_noise(*amp, sample_rate, duration)
        }
        (SignalType::PinkNoise, SignalParams::Noise { amp }) => {
            gen_pink_noise(*amp, sample_rate, duration)
        }
        (SignalType::MNoise, SignalParams::Noise { amp }) => {
            gen_m_noise(*amp, sample_rate, duration)
        }
        (SignalType::Mls, SignalParams::Mls { order, amp }) => gen_mls(*order, *amp),
        (SignalType::Dirac, SignalParams::Dirac { amp }) => gen_dirac(*amp, sample_rate, duration),
        _ => {
            return Err(format!(
                "Signal type {:?} does not match parameters {:?}",
                signal_type, params
            ));
        }
    };

    Ok(signal)
}

/// Generate output filenames for a recording with both send and record channels
pub fn generate_output_filenames_stereo(
    name_prefix: Option<&str>,
    signal_type: SignalType,
    send_channel: u16,
    record_channel: u16,
    sample_rate: u32,
) -> (PathBuf, PathBuf) {
    let base_name = if let Some(prefix) = name_prefix {
        format!(
            "{}_{}_send{}_rec{}_{}",
            prefix,
            signal_type.as_str(),
            send_channel,
            record_channel,
            sample_rate
        )
    } else {
        format!(
            "{}_send{}_rec{}_{}",
            signal_type.as_str(),
            send_channel,
            record_channel,
            sample_rate
        )
    };

    let wav_path = PathBuf::from(format!("{}.wav", base_name));
    let csv_path = PathBuf::from(format!("{}.csv", base_name));

    (wav_path, csv_path)
}

/// Generate output filenames for a recording
pub fn generate_output_filenames(
    name_prefix: Option<&str>,
    signal_type: SignalType,
    channel: u16,
    sample_rate: u32,
) -> (PathBuf, PathBuf) {
    let base_name = if let Some(prefix) = name_prefix {
        format!(
            "{}_{}_ch{}_{}",
            prefix,
            signal_type.as_str(),
            channel,
            sample_rate
        )
    } else {
        format!("{}_ch{}_{}", signal_type.as_str(), channel, sample_rate)
    };

    let wav_path = PathBuf::from(format!("{}.wav", base_name));
    let csv_path = PathBuf::from(format!("{}.csv", base_name));

    (wav_path, csv_path)
}
