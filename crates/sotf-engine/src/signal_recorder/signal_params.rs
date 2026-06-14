use super::signal_type::SignalType;
use super::types::SignalParams;

/// Build `SignalParams` for the sweep path used by `record_and_analyze`.
///
/// When `RecordingConfiguration` carries the GD-Opt v2 fields, returns an
/// [`SignalParams::OctaveSweep`]; otherwise falls back to legacy
/// [`SignalParams::Sweep`] so existing callers are not affected.
pub fn sweep_params_from_config(
    start_freq: f32,
    end_freq: f32,
    amp: f32,
    bass_octave_duration_s: Option<f32>,
    pre_silence_s: Option<f32>,
    post_silence_s: Option<f32>,
) -> SignalParams {
    match bass_octave_duration_s {
        Some(bass_dur) => SignalParams::OctaveSweep {
            start_freq,
            end_freq,
            amp,
            bass_octave_duration_s: bass_dur.clamp(1.0, 10.0),
            pre_silence_s: pre_silence_s.unwrap_or(2.0).max(0.0),
            post_silence_s: post_silence_s.unwrap_or(2.0).max(0.0),
        },
        None => SignalParams::Sweep {
            start_freq,
            end_freq,
            amp,
        },
    }
}

/// Validate signal parameters
pub fn validate_signal_params(
    signal_type: SignalType,
    params: &SignalParams,
    duration: f32,
    sample_rate: u32,
) -> Result<(), String> {
    if signal_type != SignalType::Mls && duration <= 0.0 {
        return Err("Duration must be positive".to_string());
    }

    let nyquist = sample_rate as f32 / 2.0;

    match (signal_type, params) {
        (SignalType::Tone, SignalParams::Tone { freq, amp }) => {
            if *freq <= 0.0 || *freq >= nyquist {
                return Err(format!(
                    "Tone frequency {} Hz must be in range (0, {} Hz)",
                    freq, nyquist
                ));
            }
            if *amp <= 0.0 || *amp > 1.0 {
                return Err(format!("Amplitude {} must be in range (0, 1]", amp));
            }
        }
        (
            SignalType::TwoTone,
            SignalParams::TwoTone {
                freq1,
                amp1,
                freq2,
                amp2,
            },
        ) => {
            if *freq1 <= 0.0 || *freq1 >= nyquist {
                return Err(format!(
                    "First frequency {} Hz must be in range (0, {} Hz)",
                    freq1, nyquist
                ));
            }
            if *freq2 <= 0.0 || *freq2 >= nyquist {
                return Err(format!(
                    "Second frequency {} Hz must be in range (0, {} Hz)",
                    freq2, nyquist
                ));
            }
            if *amp1 <= 0.0 || *amp1 > 1.0 {
                return Err(format!("First amplitude {} must be in range (0, 1]", amp1));
            }
            if *amp2 <= 0.0 || *amp2 > 1.0 {
                return Err(format!("Second amplitude {} must be in range (0, 1]", amp2));
            }
        }
        (
            SignalType::Sweep,
            SignalParams::Sweep {
                start_freq,
                end_freq,
                amp,
            },
        ) => {
            if *start_freq <= 0.0 || *start_freq >= nyquist {
                return Err(format!(
                    "Start frequency {} Hz must be in range (0, {} Hz)",
                    start_freq, nyquist
                ));
            }
            if *end_freq <= 0.0 || *end_freq >= nyquist {
                return Err(format!(
                    "End frequency {} Hz must be in range (0, {} Hz)",
                    end_freq, nyquist
                ));
            }
            if *start_freq >= *end_freq {
                return Err(format!(
                    "Start frequency {} Hz must be less than end frequency {} Hz",
                    start_freq, end_freq
                ));
            }
            if *amp <= 0.0 || *amp > 1.0 {
                return Err(format!("Amplitude {} must be in range (0, 1]", amp));
            }
        }
        (_, SignalParams::Noise { amp }) if (*amp <= 0.0 || *amp > 1.0) => {
            return Err(format!("Amplitude {} must be in range (0, 1]", amp));
        }
        (SignalType::Mls, SignalParams::Mls { order, amp }) => {
            if !(2..=24).contains(order) {
                return Err(format!("MLS order {} must be in range [2, 24]", order));
            }
            if *amp <= 0.0 || *amp > 1.0 {
                return Err(format!("Amplitude {} must be in range (0, 1]", amp));
            }
        }
        (SignalType::Dirac, SignalParams::Dirac { amp }) if (*amp <= 0.0 || *amp > 1.0) => {
            return Err(format!("Amplitude {} must be in range (0, 1]", amp));
        }
        _ => {}
    }

    Ok(())
}
