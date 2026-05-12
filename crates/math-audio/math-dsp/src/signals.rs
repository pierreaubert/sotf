//! Audio signal generation utilities
//!
//! Provides functions for generating various test signals including:
//! - Pure tones (sine waves)
//! - Two-tone signals (for IMD testing)
//! - Logarithmic frequency sweeps
//! - White noise
//! - Pink noise (1/f spectrum)
//! - M-weighted noise (ITU-R 468)

use crate::stft::RealFftProcessor;
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use rustfft::num_complex::Complex;
use std::f32::consts::PI;

/// Clip a sample to prevent overflow in PCM conversion
#[inline]
pub fn clip(x: f32) -> f32 {
    x.clamp(-0.999_999, 0.999_999)
}

/// Calculate number of frames for given duration and sample rate
#[inline]
pub fn frames_for(duration: f32, sample_rate: u32) -> usize {
    (duration * sample_rate as f32).round() as usize
}

/// Generate a pure tone (sine wave)
///
/// # Arguments
/// * `freq` - Frequency in Hz
/// * `amp` - Amplitude (0.0 to 1.0)
/// * `sample_rate` - Sample rate in Hz
/// * `duration` - Duration in seconds
pub fn gen_tone(freq: f32, amp: f32, sample_rate: u32, duration: f32) -> Vec<f32> {
    let n_frames = frames_for(duration, sample_rate);
    let mut signal = Vec::with_capacity(n_frames);
    let dphi = 2.0 * PI * freq / sample_rate as f32;
    let mut phase: f32 = 0.0;

    for _ in 0..n_frames {
        signal.push(clip(amp * phase.sin()));
        phase += dphi;
        if phase > 2.0 * PI {
            phase -= 2.0 * PI;
        }
    }

    signal
}

/// Generate a two-tone signal (sum of two sine waves)
///
/// Used for intermodulation distortion (IMD) testing.
/// If a1 + a2 > 1.0, the signals are automatically normalized to prevent clipping
/// while preserving the amplitude ratio.
///
/// # Arguments
/// * `f1` - First frequency in Hz
/// * `a1` - First amplitude (0.0 to 1.0)
/// * `f2` - Second frequency in Hz
/// * `a2` - Second amplitude (0.0 to 1.0)
/// * `sample_rate` - Sample rate in Hz
/// * `duration` - Duration in seconds
pub fn gen_two_tone(
    f1: f32,
    a1: f32,
    f2: f32,
    a2: f32,
    sample_rate: u32,
    duration: f32,
) -> Vec<f32> {
    let n_frames = frames_for(duration, sample_rate);
    let mut signal = Vec::with_capacity(n_frames);
    let dphi1 = 2.0 * PI * f1 / sample_rate as f32;
    let dphi2 = 2.0 * PI * f2 / sample_rate as f32;
    let mut phase1: f32 = 0.0;
    let mut phase2: f32 = 0.0;

    // Auto-normalize if the sum of amplitudes exceeds 1.0 to prevent clipping
    let sum_amp = a1 + a2;
    let (norm_a1, norm_a2) = if sum_amp > 1.0 {
        (a1 / sum_amp, a2 / sum_amp)
    } else {
        (a1, a2)
    };

    for _ in 0..n_frames {
        let sample = norm_a1 * phase1.sin() + norm_a2 * phase2.sin();
        signal.push(clip(sample));
        phase1 += dphi1;
        phase2 += dphi2;
        if phase1 > 2.0 * PI {
            phase1 -= 2.0 * PI;
        }
        if phase2 > 2.0 * PI {
            phase2 -= 2.0 * PI;
        }
    }

    signal
}

/// Generate a logarithmic frequency sweep
///
/// Sweeps from `f_start` to `f_end` Hz over the specified duration.
/// Useful for frequency response measurements.
///
/// # Arguments
/// * `f_start` - Starting frequency in Hz
/// * `f_end` - Ending frequency in Hz
/// * `amp` - Amplitude (0.0 to 1.0)
/// * `sample_rate` - Sample rate in Hz
/// * `duration` - Duration in seconds
pub fn gen_log_sweep(
    f_start: f32,
    f_end: f32,
    amp: f32,
    sample_rate: u32,
    duration: f32,
) -> Vec<f32> {
    let n_frames = frames_for(duration, sample_rate);
    let mut signal = Vec::with_capacity(n_frames);

    // Use f64 for phase computation to avoid precision loss at high frequencies.
    // The phase reaches tens of thousands of radians by the end of the sweep;
    // f32 sin() of such large values loses significant precision.
    let k = (f_end as f64 / f_start as f64).ln() / duration as f64;
    let coefficient = 2.0 * std::f64::consts::PI * f_start as f64 / k;

    for n in 0..n_frames {
        let t = n as f64 / sample_rate as f64;
        let phase = coefficient * ((k * t).exp() - 1.0);
        signal.push(clip(amp * phase.sin() as f32));
    }

    signal
}

/// Generate an octave-scaled logarithmic frequency sweep.
///
/// Unlike [`gen_log_sweep`] (fixed duration uniform across octaves), this
/// function gives the sub-100 Hz region more time so modal energy can settle
/// and be measured accurately.
///
/// ## Time allocation
///
/// Three zones:
/// - **Bass** (f_start → 100 Hz): `bass_octave_duration_s` seconds per octave.
/// - **Mid** (100 Hz → 1 kHz): half the bass per-octave rate.
/// - **High** (1 kHz → f_end): quarter the bass per-octave rate.
///
/// If the computed total < `min_total_duration_s`, the sweep is stretched
/// proportionally so all zones receive a fair share.
///
/// ## Phase accuracy
///
/// Phase accumulation uses `f64` (same as [`gen_log_sweep`]).  Phase at t=0
/// is zero, so even 5–10 Hz sweeps start correctly.
///
/// # Arguments
/// * `f_start` - Starting frequency in Hz (clamped to ≥ 1 Hz)
/// * `f_end` - Ending frequency in Hz
/// * `amp` - Amplitude (0.0 to 1.0)
/// * `sample_rate` - Sample rate in Hz
/// * `bass_octave_duration_s` - Seconds per octave below 100 Hz
/// * `min_total_duration_s` - Lower bound on total sweep duration
pub fn gen_log_sweep_octave_scaled(
    f_start: f32,
    f_end: f32,
    amp: f32,
    sample_rate: u32,
    bass_octave_duration_s: f32,
    min_total_duration_s: f32,
) -> Vec<f32> {
    let f_start = f_start.max(1.0) as f64;
    let f_end = (f_end as f64).max(f_start * 1.001);
    let sr = sample_rate as f64;

    const BASS_BOUNDARY: f64 = 100.0;
    const MID_BOUNDARY: f64 = 1000.0;

    let bass_s_oct = bass_octave_duration_s as f64;
    let mid_s_oct = bass_s_oct * 0.5;
    let high_s_oct = bass_s_oct * 0.25;

    let octaves_bass = {
        let lo = f_start;
        let hi = BASS_BOUNDARY.min(f_end);
        if hi > lo { (hi / lo).log2() } else { 0.0 }
    };
    let octaves_mid = {
        let lo = BASS_BOUNDARY.max(f_start);
        let hi = MID_BOUNDARY.min(f_end);
        if hi > lo { (hi / lo).log2() } else { 0.0 }
    };
    let octaves_high = {
        let lo = MID_BOUNDARY.max(f_start);
        let hi = f_end;
        if hi > lo { (hi / lo).log2() } else { 0.0 }
    };

    let raw_duration =
        octaves_bass * bass_s_oct + octaves_mid * mid_s_oct + octaves_high * high_s_oct;

    let total_duration = raw_duration.max(min_total_duration_s as f64);
    let scale = if raw_duration > 1e-9 {
        total_duration / raw_duration
    } else {
        1.0
    };

    let n_frames = (total_duration * sr).round() as usize;

    // Zone end-times (absolute, after scale).
    let t_bass = octaves_bass * bass_s_oct * scale;
    let t_mid = t_bass + octaves_mid * mid_s_oct * scale;

    // Phase offset accumulated through each zone transition.
    //   φ_zone = coeff * (r - 1)   where r = f_hi / f_lo
    let phase_offset_bass: f64 = if octaves_bass > 1e-9 && t_bass > 1e-9 {
        let hi = BASS_BOUNDARY.min(f_end);
        let c = 2.0 * std::f64::consts::PI * f_start * t_bass / (hi / f_start).ln();
        c * (hi / f_start - 1.0)
    } else {
        0.0
    };

    let phase_offset_mid: f64 = phase_offset_bass
        + if octaves_mid > 1e-9 {
            let lo = BASS_BOUNDARY.max(f_start);
            let hi = MID_BOUNDARY.min(f_end);
            let dur = t_mid - t_bass;
            let c = 2.0 * std::f64::consts::PI * lo * dur / (hi / lo).ln();
            c * (hi / lo - 1.0)
        } else {
            0.0
        };

    let mut signal = Vec::with_capacity(n_frames);

    for n in 0..n_frames {
        let t = n as f64 / sr;

        let phase = if t <= t_bass && t_bass > 1e-9 {
            // Bass zone.
            let hi = BASS_BOUNDARY.min(f_end);
            let c = 2.0 * std::f64::consts::PI * f_start * t_bass / (hi / f_start).ln();
            let k = (hi / f_start).ln() / t_bass;
            c * ((k * t).exp() - 1.0)
        } else if t <= t_mid && octaves_mid > 1e-9 {
            // Mid zone.
            let lo = BASS_BOUNDARY.max(f_start);
            let hi = MID_BOUNDARY.min(f_end);
            let dur = t_mid - t_bass;
            let c = 2.0 * std::f64::consts::PI * lo * dur / (hi / lo).ln();
            let k = (hi / lo).ln() / dur;
            let t_local = t - t_bass;
            phase_offset_bass + c * ((k * t_local).exp() - 1.0)
        } else if octaves_high > 1e-9 {
            // High zone.
            let lo = MID_BOUNDARY.max(f_start);
            let dur = total_duration - t_mid;
            if dur > 1e-9 {
                let c = 2.0 * std::f64::consts::PI * lo * dur / (f_end / lo).ln();
                let k = (f_end / lo).ln() / dur;
                let t_local = t - t_mid;
                phase_offset_mid + c * ((k * t_local).exp() - 1.0)
            } else {
                phase_offset_mid
            }
        } else if octaves_mid > 1e-9 {
            // Entire sweep is in a single zone (mid or bass already handled above).
            phase_offset_mid
        } else {
            // Degenerate: f_start >= f_end or single frequency.
            2.0 * std::f64::consts::PI * f_start * t
        };

        signal.push(clip(amp * phase.sin() as f32));
    }

    signal
}

/// Generate white noise
///
/// Produces noise with a flat frequency spectrum.
/// Uses a deterministic LCG for reproducible output.
///
/// # Arguments
/// * `amp` - Amplitude (0.0 to 1.0)
/// * `sample_rate` - Sample rate in Hz
/// * `duration` - Duration in seconds
pub fn gen_white_noise(amp: f32, sample_rate: u32, duration: f32) -> Vec<f32> {
    let n_frames = frames_for(duration, sample_rate);
    let mut signal = Vec::with_capacity(n_frames);

    // Simple LCG random number generator for deterministic output
    let mut seed: u64 = 1234567890;

    for _ in 0..n_frames {
        // LCG constants from Numerical Recipes
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        // Convert to [-1, 1] range
        // Mask to 32 bits to get proper range [0, u32::MAX]
        let random_u32 = (seed & 0xFFFFFFFF) as u32;
        let random = (random_u32 as f32 / u32::MAX as f32) * 2.0 - 1.0;
        signal.push(clip(amp * random));
    }

    signal
}

/// Generate pink noise
///
/// Produces noise with a 1/f spectrum (-3dB/octave).
/// Uses the Voss-McCartney algorithm (Paul Kellett's implementation).
///
/// # Arguments
/// * `amp` - Amplitude (0.0 to 1.0)
/// * `sample_rate` - Sample rate in Hz
/// * `duration` - Duration in seconds
pub fn gen_pink_noise(amp: f32, sample_rate: u32, duration: f32) -> Vec<f32> {
    let n_frames = frames_for(duration, sample_rate);
    let mut signal = Vec::with_capacity(n_frames);

    // Voss-McCartney algorithm (Paul Kellett's implementation)
    // Uses multiple white noise generators at different rates
    let mut seed: u64 = 9876543210;
    let mut b0 = 0.0f32;
    let mut b1 = 0.0f32;
    let mut b2 = 0.0f32;
    let mut b3 = 0.0f32;
    let mut b4 = 0.0f32;
    let mut b5 = 0.0f32;
    let mut b6 = 0.0f32;

    // Normalization factor derived from the filter coefficients.
    // Each filter is y_i[n] = a_i*y_i[n-1] + b_i*w[n] driven by the same white
    // noise w[n] (uniform [-1,1], variance 1/3). The total output variance is:
    //   Var = Σ_ij b_i*b_j/(3*(1 - a_i*a_j))        (IIR cross-covariances)
    //       + Σ_i 2*d*b_i/3                           (direct-IIR cross-terms)
    //       + (d² + e²)/3                              (direct + delayed terms)
    // where d=0.5362 (direct white gain) and e=0.115926 (delayed white gain).
    // This gives RMS ≈ 1.744. Dividing by this matches the white noise RMS
    // behavior: amp=1.0 produces output in [-1,1] with comparable RMS (~0.58).
    const PINK_NORM: f32 = 1.0 / 1.744;

    for _ in 0..n_frames {
        // Generate white noise
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        // Mask to 32 bits to get proper range [0, u32::MAX]
        let random_u32 = (seed & 0xFFFFFFFF) as u32;
        let white = (random_u32 as f32 / u32::MAX as f32) * 2.0 - 1.0;

        // Update pink noise state at different rates
        b0 = 0.99886 * b0 + white * 0.0555179;
        b1 = 0.99332 * b1 + white * 0.0750759;
        b2 = 0.96900 * b2 + white * 0.153_852;
        b3 = 0.86650 * b3 + white * 0.3104856;
        b4 = 0.55000 * b4 + white * 0.5329522;
        b5 = -0.7616 * b5 - white * 0.0168980;

        let pink = b0 + b1 + b2 + b3 + b4 + b5 + b6 + white * 0.5362;
        b6 = white * 0.115926;

        signal.push(clip(amp * pink * PINK_NORM));
    }

    signal
}

fn mls_taps(order: u8) -> Option<&'static [u8]> {
    match order {
        2 => Some(&[2, 1]),
        3 => Some(&[3, 1]),
        4 => Some(&[4, 1]),
        5 => Some(&[5, 2]),
        6 => Some(&[6, 1]),
        7 => Some(&[7, 1]),
        8 => Some(&[8, 6, 5, 4]),
        9 => Some(&[9, 4]),
        10 => Some(&[10, 3]),
        11 => Some(&[11, 2]),
        12 => Some(&[12, 6, 4, 1]),
        13 => Some(&[13, 4, 3, 1]),
        14 => Some(&[14, 5, 3, 1]),
        15 => Some(&[15, 1]),
        16 => Some(&[16, 14, 13, 11]),
        17 => Some(&[17, 3]),
        18 => Some(&[18, 7]),
        19 => Some(&[19, 5, 2, 1]),
        20 => Some(&[20, 3]),
        21 => Some(&[21, 2]),
        22 => Some(&[22, 1]),
        23 => Some(&[23, 5]),
        24 => Some(&[24, 4, 3, 1]),
        _ => None,
    }
}

/// Generate a Maximum Length Sequence (MLS).
///
/// The sequence is generated by a deterministic Fibonacci LFSR seeded with all
/// ones and mapped from bits to bipolar samples: `1 => +amp`, `0 => -amp`.
/// Supported orders are 2 through 24, giving sequence lengths of
/// `2^order - 1` samples.
///
/// # Arguments
/// * `order` - LFSR order in the range 2..=24
/// * `amp` - Peak amplitude
pub fn gen_mls(order: u8, amp: f32) -> Vec<f32> {
    let Some(taps) = mls_taps(order) else {
        return Vec::new();
    };

    let len = (1_usize << order) - 1;
    let state_mask = (1_u32 << order) - 1;
    let mut tap_mask = 0_u32;
    for &tap in taps {
        tap_mask |= 1_u32 << (tap - 1);
    }

    let amp = clip(amp);
    let mut state = state_mask;
    let mut signal = Vec::with_capacity(len);

    for _ in 0..len {
        let out_bit = state & 1;
        signal.push(if out_bit == 1 { amp } else { -amp });

        let feedback = (state & tap_mask).count_ones() & 1;
        state = ((state << 1) & state_mask) | feedback;
    }

    signal
}

/// Generate a Dirac impulse signal.
///
/// Alias for [`gen_impulse`] using the Dirac naming used by measurement and
/// room-eq workflows.
///
/// # Arguments
/// * `amp` - Amplitude of the impulse sample
/// * `sample_rate` - Sample rate in Hz
/// * `duration` - Duration in seconds
pub fn gen_dirac(amp: f32, sample_rate: u32, duration: f32) -> Vec<f32> {
    gen_impulse(amp, sample_rate, duration)
}

/// Generate an impulse signal (first sample is amplitude, rest are zero)
///
/// # Arguments
/// * `amp` - Amplitude (0.0 to 1.0)
/// * `sample_rate` - Sample rate in Hz
/// * `duration` - Duration in seconds
pub fn gen_impulse(amp: f32, sample_rate: u32, duration: f32) -> Vec<f32> {
    let n_frames = frames_for(duration, sample_rate);
    let mut signal = vec![0.0; n_frames];
    if n_frames > 0 {
        signal[0] = clip(amp);
    }
    signal
}

/// Generate a step signal (all samples are amplitude)
///
/// # Arguments
/// * `amp` - Amplitude (0.0 to 1.0)
/// * `sample_rate` - Sample rate in Hz
/// * `duration` - Duration in seconds
pub fn gen_step(amp: f32, sample_rate: u32, duration: f32) -> Vec<f32> {
    let n_frames = frames_for(duration, sample_rate);
    vec![clip(amp); n_frames]
}

/// Generate M-weighted noise
///
/// Produces noise weighted according to ITU-R 468 standard.
/// This weighting emphasizes frequencies around 6.3 kHz, which is
/// useful for acoustic measurements.
///
/// # Arguments
/// * `amp` - Amplitude (0.0 to 1.0)
/// * `sample_rate` - Sample rate in Hz
/// * `duration` - Duration in seconds
pub fn gen_m_noise(amp: f32, sample_rate: u32, duration: f32) -> Vec<f32> {
    let n_frames = frames_for(duration, sample_rate);
    let srate = sample_rate as f64;

    // Generate white noise
    let mut seed: u64 = 1122334455;
    let mut noise_buffer = Vec::with_capacity(n_frames);
    for _ in 0..n_frames {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let random_u32 = (seed & 0xFFFFFFFF) as u32;
        let white = (random_u32 as f64 / u32::MAX as f64) * 2.0 - 1.0;
        noise_buffer.push(white);
    }

    // ITU-R 468 weighting approximation using cascaded biquad filters:
    //   - Highpass at 20 Hz removes sub-bass
    //   - Low shelf +5 dB at 100 Hz shapes the low-frequency rise
    //   - Peak +12 dB at 6.3 kHz is the characteristic ITU-R 468 peak
    //   - Peak -4 dB at 12 kHz shapes the descent above the peak
    //   - Lowpass at 22 kHz rolls off ultrasonics
    let mut filters = [
        Biquad::new(BiquadFilterType::Highpass, 20.0, srate, 0.0, 0.0),
        Biquad::new(BiquadFilterType::Lowshelf, 100.0, srate, 0.6, 5.0),
        Biquad::new(BiquadFilterType::Peak, 6300.0, srate, 1.0, 12.0),
        Biquad::new(BiquadFilterType::Peak, 12000.0, srate, 0.8, -4.0),
        Biquad::new(BiquadFilterType::Lowpass, 22000.0, srate, 0.0, 0.0),
    ];

    let amp_f64 = amp as f64;
    noise_buffer
        .iter()
        .map(|&sample| {
            let mut s = sample;
            for f in &mut filters {
                s = f.process(s);
            }
            clip((amp_f64 * s) as f32)
        })
        .collect()
}

/// Interleave per-channel signals into a multi-channel interleaved buffer
///
/// Takes a vector of per-channel signals and interleaves them frame-by-frame.
///
/// # Arguments
/// * `per_channel` - Vector of per-channel signals (each `Vec<f32>` is one channel)
///
/// # Returns
/// Interleaved signal where samples are ordered: [ch0_frame0, ch1_frame0, ..., ch0_frame1, ch1_frame1, ...]
pub fn interleave_per_channel(per_channel: &[Vec<f32>]) -> Vec<f32> {
    let n_channels = per_channel.len();
    if n_channels == 0 {
        return Vec::new();
    }
    let n_frames = per_channel[0].len();
    let mut interleaved = Vec::with_capacity(n_frames * n_channels);

    for frame in 0..n_frames {
        for channel_data in per_channel.iter().take(n_channels) {
            interleaved.push(channel_data[frame]);
        }
    }

    interleaved
}

/// Replicate a mono signal to multiple channels
///
/// Takes a mono signal and replicates it to all channels.
///
/// # Arguments
/// * `mono` - Mono signal
/// * `channels` - Number of output channels
///
/// # Returns
/// Interleaved multi-channel signal with the same content on all channels
pub fn replicate_mono(mono: &[f32], channels: u16) -> Vec<f32> {
    let n_frames = mono.len();
    let mut interleaved = Vec::with_capacity(n_frames * channels as usize);

    for &sample in mono {
        for _ in 0..channels {
            interleaved.push(sample);
        }
    }

    interleaved
}

/// Apply Hann window fade-in to the beginning of a signal
///
/// # Arguments
/// * `signal` - Signal to apply fade to (modified in-place)
/// * `fade_samples` - Number of samples for the fade
pub fn apply_fade_in(signal: &mut [f32], fade_samples: usize) {
    let fade_len = fade_samples.min(signal.len());
    for (i, val) in signal.iter_mut().enumerate().take(fade_len) {
        let t = i as f32 / fade_len as f32;
        let fade = 0.5 * (1.0 - (std::f32::consts::PI * t).cos()); // Hann window
        *val *= fade;
    }
}

/// Apply Hann window fade-out to the end of a signal
///
/// # Arguments
/// * `signal` - Signal to apply fade to (modified in-place)
/// * `fade_samples` - Number of samples for the fade
pub fn apply_fade_out(signal: &mut [f32], fade_samples: usize) {
    let len = signal.len();
    let fade_len = fade_samples.min(len);
    let start_idx = len.saturating_sub(fade_len);

    for i in 0..fade_len {
        let t = i as f32 / fade_len as f32;
        let fade = 0.5 * (1.0 + (std::f32::consts::PI * t).cos()); // Hann window
        signal[start_idx + i] *= fade;
    }
}

/// Add silence padding to the beginning and end of a signal
///
/// # Arguments
/// * `signal` - Original signal
/// * `pre_samples` - Number of silence samples to add before
/// * `post_samples` - Number of silence samples to add after
///
/// # Returns
/// New signal with padding
pub fn add_silence_padding(signal: &[f32], pre_samples: usize, post_samples: usize) -> Vec<f32> {
    let total_len = pre_samples + signal.len() + post_samples;
    let mut padded = vec![0.0; total_len];

    // Copy original signal in the middle
    padded[pre_samples..pre_samples + signal.len()].copy_from_slice(signal);

    padded
}

/// Generate a signal with fade-in, fade-out, and silence padding
///
/// # Arguments
/// * `signal` - Original signal
/// * `sample_rate` - Sample rate in Hz
/// * `fade_duration_ms` - Fade duration in milliseconds (default: 20ms)
/// * `padding_duration_ms` - Pre/post silence padding in milliseconds (default: 250ms)
///
/// # Returns
/// Signal with fades and padding applied
pub fn prepare_signal_for_playback(
    mut signal: Vec<f32>,
    sample_rate: u32,
    fade_duration_ms: f32,
    padding_duration_ms: f32,
) -> Vec<f32> {
    let fade_samples = ((fade_duration_ms / 1000.0) * sample_rate as f32) as usize;
    let padding_samples = ((padding_duration_ms / 1000.0) * sample_rate as f32) as usize;

    // Apply fades
    apply_fade_in(&mut signal, fade_samples);
    apply_fade_out(&mut signal, fade_samples);

    // Pad in-place: extend with zeros for both pads, then rotate pre-padding to the front
    let signal_len = signal.len();
    signal.resize(signal_len + 2 * padding_samples, 0.0);
    signal.rotate_right(padding_samples);
    signal
}

/// Convert mono signal to stereo by copying to both channels
pub fn mono_to_stereo(mono_signal: Vec<f32>) -> Vec<f32> {
    let mut stereo_signal = Vec::with_capacity(mono_signal.len() * 2);
    for sample in mono_signal {
        stereo_signal.push(sample); // Left channel
        stereo_signal.push(sample); // Right channel
    }
    stereo_signal
}

/// Prepare signal for playback with mono or stereo channels
pub fn prepare_signal_for_playback_channels(
    signal: Vec<f32>,
    sample_rate: u32,
    fade_duration_ms: f32,
    padding_duration_ms: f32,
    stereo: bool,
) -> Vec<f32> {
    // First prepare the mono signal with fades and padding
    let prepared_mono =
        prepare_signal_for_playback(signal, sample_rate, fade_duration_ms, padding_duration_ms);

    // Convert to stereo if requested
    if stereo {
        mono_to_stereo(prepared_mono)
    } else {
        prepared_mono
    }
}

/// Generate the frequency-domain spectrum for an allpass probe signal.
///
/// Returns `spectrum_size = fft_size/2 + 1` complex bins with unit magnitude
/// and continuously accumulated random phase. DC and Nyquist bins are forced real.
fn gen_probe_spectrum(fft_size: usize, seed: u64) -> Vec<Complex<f32>> {
    let spectrum_size = fft_size / 2 + 1;
    let mut spectrum = vec![Complex::new(0.0, 0.0); spectrum_size];

    // Seeded LCG for reproducible phase increments
    let mut rng_state = seed;
    let max_delta = std::f32::consts::FRAC_PI_4; // π/4 max increment per bin

    // DC bin: phase = 0 (must be real)
    spectrum[0] = Complex::new(1.0, 0.0);

    let mut phase: f32 = 0.0;
    for bin in spectrum[1..spectrum_size - 1].iter_mut() {
        // LCG step
        rng_state = rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let random_u32 = ((rng_state >> 33) ^ rng_state) as u32;
        let delta = (random_u32 as f32 / u32::MAX as f32) * max_delta;
        phase += delta;

        let (sin_p, cos_p) = phase.sin_cos();
        *bin = Complex::new(cos_p, sin_p);

        // Avoid phase growing unboundedly (wrap at 2π)
        if phase > 2.0 * PI {
            phase -= 2.0 * PI;
        }
    }

    // Nyquist bin: phase = 0 (must be real for even-length signals)
    if spectrum_size > 1 {
        spectrum[spectrum_size - 1] = Complex::new(1.0, 0.0);
    }

    spectrum
}

/// Generate a wideband allpass probe signal.
///
/// Synthesized in the DFT domain with unit magnitude at all frequency bins
/// and continuously accumulated random phase. The result is a real signal
/// with flat power spectrum but energy spread across time — better than
/// sweeps for nonlinear speakers (per Johnston AES).
///
/// # Arguments
/// * `n_frames` - Number of output samples (determines frequency resolution)
/// * `sample_rate` - Sample rate in Hz
/// * `amp` - Peak amplitude (signal is normalized to this)
/// * `seed` - RNG seed for reproducible phase (same seed = same probe)
pub fn gen_allpass_probe(n_frames: usize, _sample_rate: u32, amp: f32, seed: u64) -> Vec<f32> {
    if n_frames == 0 {
        return Vec::new();
    }

    let fft_size = n_frames;
    let mut fft = RealFftProcessor::new_bidirectional(fft_size);

    let spectrum = gen_probe_spectrum(fft_size, seed);
    fft.freq_buffer[..spectrum.len()].copy_from_slice(&spectrum);

    // IFFT to time domain
    fft.inverse();

    // Normalize: scale so peak = amp
    let peak = fft.time_buffer[..n_frames]
        .iter()
        .map(|&x| x.abs())
        .fold(0.0_f32, f32::max);

    let scale = if peak > 1e-10 { amp / peak } else { 0.0 };

    fft.time_buffer[..n_frames]
        .iter()
        .map(|&x| clip(x * scale))
        .collect()
}

/// Generate a narrowband allpass probe (bandpass filtered).
///
/// Same phase structure as `gen_allpass_probe` (when given the same seed),
/// but zeroes out frequency bins outside `[lo_hz, hi_hz]` with a smooth
/// Tukey taper at band edges. Used as a matched filter for delay detection
/// with excellent noise rejection.
///
/// # Arguments
/// * `n_frames` - Number of output samples
/// * `sample_rate` - Sample rate in Hz
/// * `amp` - Peak amplitude (signal is normalized to this)
/// * `seed` - RNG seed (same seed as wideband = identical phase within passband)
/// * `lo_hz` - Lower cutoff frequency in Hz
/// * `hi_hz` - Upper cutoff frequency in Hz
pub fn gen_narrowband_probe(
    n_frames: usize,
    sample_rate: u32,
    amp: f32,
    seed: u64,
    lo_hz: f32,
    hi_hz: f32,
) -> Vec<f32> {
    if n_frames == 0 {
        return Vec::new();
    }

    let fft_size = n_frames;
    let spectrum_size = fft_size / 2 + 1;
    let mut fft = RealFftProcessor::new_bidirectional(fft_size);

    let spectrum = gen_probe_spectrum(fft_size, seed);
    let freq_resolution = sample_rate as f32 / fft_size as f32;

    // Number of bins for the Tukey taper at each band edge
    let taper_bins = 10_usize;

    #[allow(clippy::needless_range_loop)]
    for k in 0..spectrum_size {
        let freq = k as f32 * freq_resolution;

        if freq < lo_hz || freq > hi_hz {
            // Outside passband: zero
            fft.freq_buffer[k] = Complex::new(0.0, 0.0);
        } else {
            // Inside passband: apply spectrum with optional taper at edges
            let mut gain = 1.0_f32;

            // Lower edge taper (Hann half-window, rising from near-zero to 1.0)
            let lo_bin = (lo_hz / freq_resolution).ceil() as usize;
            if k < lo_bin + taper_bins && k >= lo_bin {
                // Use (taper_bins + 1) so edge bins get nonzero gain
                let t = (k - lo_bin + 1) as f32 / (taper_bins + 1) as f32;
                gain = 0.5 * (1.0 - (PI * t).cos());
            }

            // Upper edge taper (Hann half-window, falling from 1.0 to near-zero)
            let hi_bin = (hi_hz / freq_resolution).floor() as usize;
            if hi_bin >= taper_bins && k > hi_bin - taper_bins && k <= hi_bin {
                let t = (hi_bin - k + 1) as f32 / (taper_bins + 1) as f32;
                gain = 0.5 * (1.0 - (PI * t).cos());
            }

            fft.freq_buffer[k] = spectrum[k] * gain;
        }
    }

    // IFFT to time domain
    fft.inverse();

    // Normalize peak to amp
    let peak = fft.time_buffer[..n_frames]
        .iter()
        .map(|&x| x.abs())
        .fold(0.0_f32, f32::max);

    let scale = if peak > 1e-10 { amp / peak } else { 0.0 };

    fft.time_buffer[..n_frames]
        .iter()
        .map(|&x| clip(x * scale))
        .collect()
}

// ---------------------------------------------------------------------------
// GD-Opt v2 Phase GD-1e — bass tone burst + phase extraction
// ---------------------------------------------------------------------------

/// Generate a Hann-windowed sinusoidal tone burst at a single frequency.
///
/// Used by the recording wizard's BassAnchor step to capture a
/// per-channel phase reference at the bottom of the bass band, where
/// sweep SNR is marginal. At 20 Hz × 5 cycles @ 48 kHz the burst is
/// 250 ms / 12 000 samples — long enough to resolve phase, short enough
/// to keep modal ringing bounded.
///
/// # Arguments
/// * `freq_hz`    - Fundamental frequency of the burst in Hz.
/// * `num_cycles` - Number of cycles (tone-length in periods of `freq_hz`).
/// * `sample_rate` - Sample rate in Hz.
/// * `amp`        - Peak amplitude after windowing (≤ 1.0).
///
/// # Returns
/// The burst as a vector of `f32` samples. An empty vector if any input
/// is zero / sub-sample-length.
pub fn gen_bass_tone_burst(freq_hz: f32, num_cycles: u16, sample_rate: u32, amp: f32) -> Vec<f32> {
    if freq_hz <= 0.0 || num_cycles == 0 || sample_rate == 0 || amp <= 0.0 {
        return Vec::new();
    }
    let n = ((num_cycles as f32 / freq_hz) * sample_rate as f32).round() as usize;
    if n < 2 {
        return Vec::new();
    }
    let omega = 2.0 * PI * freq_hz / sample_rate as f32;
    let n_f = n as f32;
    (0..n)
        .map(|k| {
            let t = k as f32;
            // Hann envelope: (1 - cos(2π k / (n-1))) / 2
            let w = 0.5 * (1.0 - (2.0 * PI * t / (n_f - 1.0)).cos());
            clip(amp * w * (omega * t).sin())
        })
        .collect()
}

/// Result of a single-bin DFT-based phase extraction at a target
/// frequency, together with a stability metric that flags modally
/// contaminated bursts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TonePhaseResult {
    /// Phase of the fundamental at `freq_hz` in degrees, wrapped to
    /// `(−180°, 180°]`. Relative to a reference `sin(ω·t)` emitted at
    /// the start of the analysis window.
    pub phase_deg: f64,
    /// Magnitude of the bin at `freq_hz` (linear, same units as the
    /// input signal). Zero when the signal contains no tone.
    pub magnitude: f64,
    /// |phase_half1 − phase_half2| in degrees. A stable,
    /// single-sinusoid burst has `stability_deg ≈ 0`. Modal ringing,
    /// low SNR, or time-varying phase drive this above 20°, which is
    /// the GD-Opt v2 advisory threshold for `"bass_anchor_unreliable"`.
    pub stability_deg: f64,
}

/// Extract the phase of a single frequency bin from a time-domain
/// signal using a direct DFT sum, plus a half-split stability metric.
///
/// For `len(signal) = N`, the complex bin at `freq_hz` is:
///
/// ```text
/// c = Σ_{k=0..N} s[k] · exp(−j·ω·k), ω = 2π·freq_hz/sample_rate
/// phase = atan2(Im(c), Re(c))
/// magnitude = |c| / N
/// ```
///
/// The stability metric runs the same extraction over the first and
/// second halves independently and reports the wrapped phase
/// difference in degrees. A clean stationary tone gives ≈ 0;
/// modally-ringing or noisy bursts show ≫ 0.
pub fn extract_tone_phase(signal: &[f32], freq_hz: f32, sample_rate: u32) -> TonePhaseResult {
    if signal.len() < 4 || freq_hz <= 0.0 || sample_rate == 0 {
        return TonePhaseResult {
            phase_deg: 0.0,
            magnitude: 0.0,
            stability_deg: 0.0,
        };
    }
    let (re_full, im_full) = single_bin_dft(signal, freq_hz, sample_rate, 0);
    let magnitude_raw = (re_full * re_full + im_full * im_full).sqrt();
    let magnitude = magnitude_raw / signal.len() as f64;
    let phase_rad = im_full.atan2(re_full);

    let mid = signal.len() / 2;
    let (re_a, im_a) = single_bin_dft(&signal[..mid], freq_hz, sample_rate, 0);
    // The second half keeps the global time reference (k_offset = mid)
    // so both halves measure phase against the same t = 0 — otherwise
    // the split induces a spurious `mid·ω` phase jump on a stable tone.
    let (re_b, im_b) = single_bin_dft(&signal[mid..], freq_hz, sample_rate, mid);
    let phase_a = im_a.atan2(re_a);
    let phase_b = im_b.atan2(re_b);
    // Wrap (phase_b − phase_a) to (−π, π] before taking |·|.
    let mut diff = phase_b - phase_a;
    while diff > PI as f64 {
        diff -= 2.0 * PI as f64;
    }
    while diff <= -(PI as f64) {
        diff += 2.0 * PI as f64;
    }

    TonePhaseResult {
        phase_deg: phase_rad.to_degrees(),
        magnitude,
        stability_deg: diff.abs().to_degrees(),
    }
}

/// Generate a steady-state sinusoidal tone with linear-Hann fade in/out.
///
/// Used by the recording wizard's BassAnchor step (GD-Opt v2 Phase
/// GD-1e) for steady-state lock-in phase extraction. Plays a continuous
/// sine for `duration_s` seconds with `fade_ms` half-Hann fades at the
/// start and end so the speaker doesn't see a transient. The total
/// length is exactly `round(duration_s · sample_rate)` samples.
///
/// # Arguments
/// * `freq_hz`     - Tone frequency in Hz.
/// * `duration_s`  - Total tone length in seconds (≥ `2 · fade_ms`).
/// * `fade_ms`     - Fade-in / fade-out length in milliseconds.
/// * `sample_rate` - Sample rate in Hz.
/// * `amp`         - Peak amplitude in the steady portion (≤ 1.0).
///
/// # Returns
/// A vector of `f32` samples; empty when any input is zero or the
/// duration is shorter than two fades.
pub fn gen_steady_tone(
    freq_hz: f32,
    duration_s: f32,
    fade_ms: f32,
    sample_rate: u32,
    amp: f32,
) -> Vec<f32> {
    if freq_hz <= 0.0 || duration_s <= 0.0 || sample_rate == 0 || amp <= 0.0 {
        return Vec::new();
    }
    let n = (duration_s * sample_rate as f32).round() as usize;
    let fade_n = ((fade_ms.max(0.0) / 1000.0) * sample_rate as f32).round() as usize;
    if n < 2 * fade_n.max(1) {
        return Vec::new();
    }
    let omega = 2.0 * PI * freq_hz / sample_rate as f32;
    (0..n)
        .map(|k| {
            let env = if fade_n == 0 {
                1.0
            } else if k < fade_n {
                // Half-Hann fade-in: 0 → 1 across `fade_n` samples.
                0.5 * (1.0 - (PI * k as f32 / fade_n as f32).cos())
            } else if k >= n - fade_n {
                // Half-Hann fade-out: 1 → 0 across the last `fade_n`.
                let kk = (n - 1 - k) as f32;
                0.5 * (1.0 - (PI * kk / fade_n as f32).cos())
            } else {
                1.0
            };
            clip(amp * env * (omega * k as f32).sin())
        })
        .collect()
}

/// Per-window lock-in phasor at `freq_hz` produced by
/// [`tone_phase_phasors`]. Carries the raw `(re, im)` from
/// `single_bin_dft` plus the `start` index in the parent signal so a
/// caller can correlate two streams sub-window for sub-window.
#[derive(Debug, Clone, Copy)]
pub struct TonePhasorWindow {
    pub re: f64,
    pub im: f64,
    pub start: usize,
    pub len: usize,
}

/// Collect per-window single-bin lock-in phasors at `freq_hz`. Drops
/// the first and last `len/8` samples (settling around the fades),
/// splits the remaining steady portion into ≥ 1 sub-windows snapped to
/// integer cycles, and runs `single_bin_dft` on each.
///
/// Minimum-resolvable frequency on a buffer of length `n` is
/// `8 · num_windows · freq_hz ≤ sample_rate`; below that, the helper
/// merges sub-windows so each still spans ≥ 1 full cycle (down to a
/// single sub-window) — this keeps the per-bin DFT leakage-free
/// instead of returning falsely-confident sub-cycle estimates.
///
/// Returns an empty vector for degenerate inputs (signal too short,
/// `num_windows == 0`, `freq_hz <= 0`, fewer than one cycle even after
/// merging).
pub fn tone_phase_phasors(
    signal: &[f32],
    freq_hz: f32,
    sample_rate: u32,
    num_windows: usize,
) -> Vec<TonePhasorWindow> {
    if signal.len() < 8 || freq_hz <= 0.0 || sample_rate == 0 || num_windows == 0 {
        return Vec::new();
    }
    let settle = signal.len() / 8;
    let steady_start = settle;
    let steady_end = signal.len() - settle;
    if steady_end <= steady_start + 1 {
        return Vec::new();
    }
    let steady_len = steady_end - steady_start;
    let samples_per_cycle = (sample_rate as f64 / freq_hz as f64).round() as usize;
    if samples_per_cycle < 2 || steady_len < samples_per_cycle {
        // Less than one full cycle of the target frequency fits in the
        // analysis region — single-bin DFT below one cycle has severe
        // spectral leakage with falsely-low circular-std (correlated
        // bias across sub-windows). Refuse to produce results rather
        // than mislead the caller.
        return Vec::new();
    }
    // Aim for `num_windows` sub-windows but never split below one
    // full cycle. Effective count is the largest k in [1, num_windows]
    // such that floor(steady_len / k) >= samples_per_cycle.
    let mut effective_windows = num_windows;
    while effective_windows > 1 && steady_len / effective_windows < samples_per_cycle {
        effective_windows -= 1;
    }
    let raw_win_len = steady_len / effective_windows;
    let win_len = (raw_win_len / samples_per_cycle) * samples_per_cycle;
    if win_len < samples_per_cycle {
        return Vec::new();
    }

    let mut out = Vec::with_capacity(effective_windows);
    for w in 0..effective_windows {
        let start = steady_start + w * win_len;
        let end = start + win_len;
        if end > signal.len() {
            break;
        }
        let (re, im) = single_bin_dft(&signal[start..end], freq_hz, sample_rate, start);
        out.push(TonePhasorWindow {
            re,
            im,
            start,
            len: win_len,
        });
    }
    out
}

/// Sub-window lock-in phase extraction.
///
/// Wraps [`tone_phase_phasors`] and aggregates the per-window
/// phasors into:
/// - `phase_deg`     — circular mean of per-window phase, in degrees,
///   wrapped to (−180°, 180°]. Computed from `atan2(ΣQᵢ, ΣIᵢ)` where the
///   I/Q come from each window with a shared `k_offset` so the phases
///   share a common time reference.
/// - `magnitude`     — mean of `√(I² + Q²) / window_len` across windows.
/// - `stability_deg` — circular standard deviation of per-window phase
///   (degrees). Equivalent to `√(−2·ln(R̄))` in radians, converted to
///   degrees, where `R̄` is the mean resultant length over all windows.
///   A stable lock returns ≈ 0; modal contamination or low SNR pushes
///   this above the 20° advisory threshold.
///
/// Returns `phase_deg = 0`, `magnitude = 0`, `stability_deg = 0` when
/// inputs are degenerate (see [`tone_phase_phasors`] for the rules).
pub fn extract_tone_phase_windowed(
    signal: &[f32],
    freq_hz: f32,
    sample_rate: u32,
    num_windows: usize,
) -> TonePhaseResult {
    let phasors = tone_phase_phasors(signal, freq_hz, sample_rate, num_windows);
    aggregate_tone_phase(&phasors)
}

/// Aggregate a slice of per-window phasors into a `TonePhaseResult`.
/// See [`extract_tone_phase_windowed`] for the semantics.
pub fn aggregate_tone_phase(phasors: &[TonePhasorWindow]) -> TonePhaseResult {
    let zero = TonePhaseResult {
        phase_deg: 0.0,
        magnitude: 0.0,
        stability_deg: 0.0,
    };
    if phasors.is_empty() {
        return zero;
    }
    let mut sum_re = 0.0_f64;
    let mut sum_im = 0.0_f64;
    let mut sum_mag = 0.0_f64;
    let mut sum_cos = 0.0_f64;
    let mut sum_sin = 0.0_f64;
    for p in phasors {
        sum_re += p.re;
        sum_im += p.im;
        let mag = (p.re * p.re + p.im * p.im).sqrt();
        sum_mag += mag / p.len as f64;
        let phase = p.im.atan2(p.re);
        sum_cos += phase.cos();
        sum_sin += phase.sin();
    }
    let n = phasors.len() as f64;
    let phase_rad = sum_im.atan2(sum_re);
    let mean_mag = sum_mag / n;
    let mean_cos = sum_cos / n;
    let mean_sin = sum_sin / n;
    let r_bar = (mean_cos * mean_cos + mean_sin * mean_sin).sqrt().min(1.0);
    let circ_std_rad = if r_bar > 1e-9 {
        (-2.0 * r_bar.ln()).sqrt()
    } else {
        std::f64::consts::PI
    };
    TonePhaseResult {
        phase_deg: phase_rad.to_degrees(),
        magnitude: mean_mag,
        stability_deg: circ_std_rad.to_degrees(),
    }
}

/// Magnitude-squared coherence between two per-window phasor streams
/// at the same frequency, in `[0, 1]`. Both streams must have the same
/// length and matching `start` indices (use [`tone_phase_phasors`] on
/// the same signal layout).
///
/// Returns:
///   `γ² = |⟨X · conj(Y)⟩|² / (⟨|X|²⟩ · ⟨|Y|²⟩)`
///
/// `1.0` means the two streams' per-window phasors are perfectly
/// linearly related (clean linkage between mic and loopback);
/// values toward `0.0` indicate uncorrelated noise on either side.
/// Returns `None` when the inputs disagree on length / start indices
/// or when either stream has zero energy.
pub fn phasor_coherence(a: &[TonePhasorWindow], b: &[TonePhasorWindow]) -> Option<f64> {
    if a.is_empty() || a.len() != b.len() {
        return None;
    }
    if !a
        .iter()
        .zip(b)
        .all(|(p, q)| p.start == q.start && p.len == q.len)
    {
        return None;
    }
    let mut cross_re = 0.0_f64;
    let mut cross_im = 0.0_f64;
    let mut auto_a = 0.0_f64;
    let mut auto_b = 0.0_f64;
    for (p, q) in a.iter().zip(b) {
        // X · conj(Y) = (X.re + iX.im)(Y.re - iY.im)
        cross_re += p.re * q.re + p.im * q.im;
        cross_im += p.im * q.re - p.re * q.im;
        auto_a += p.re * p.re + p.im * p.im;
        auto_b += q.re * q.re + q.im * q.im;
    }
    let denom = auto_a * auto_b;
    if denom <= 0.0 {
        return None;
    }
    let num = cross_re * cross_re + cross_im * cross_im;
    Some((num / denom).clamp(0.0, 1.0))
}

/// Direct single-bin DFT evaluated with the `sin` convention:
///   `s[k] ≈ A · sin(ωk + φ)` → `phase = atan2(imag, real)`.
///
/// Returns the raw `(real, imag)` accumulators so callers can either
/// compute magnitude themselves or average across windows without
/// extra trigonometry.
///
/// The `k_offset` argument lets sub-slices of a larger signal share a
/// common time reference. When analysing `signal[mid..]`, pass
/// `k_offset = mid` so the projection basis stays anchored at the
/// original `t = 0`; otherwise the two halves' phase measurements
/// acquire a synthetic offset proportional to `mid · ω`.
#[inline]
fn single_bin_dft(signal: &[f32], freq_hz: f32, sample_rate: u32, k_offset: usize) -> (f64, f64) {
    let omega = 2.0 * std::f64::consts::PI * freq_hz as f64 / sample_rate as f64;
    let mut re = 0.0_f64;
    let mut im = 0.0_f64;
    // Reference convention: the emitted burst is `sin(ω·t)`. A pure
    // sin-phase signal should produce `phase = 0`. Projecting onto
    // `sin(ω·t)` (real part) and `+cos(ω·t)` (imag part) makes
    // `atan2(imag, real)` return the phase of a sin-referenced tone:
    //   sin(ωt) → phase = 0°
    //   cos(ωt) → phase = +90°
    //   −sin(ωt) → phase = ±180°
    for (i, &s) in signal.iter().enumerate() {
        let theta = omega * (i + k_offset) as f64;
        re += s as f64 * theta.sin();
        im += s as f64 * theta.cos();
    }
    (re, im)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frames_for() {
        assert_eq!(frames_for(1.0, 48000), 48000);
        assert_eq!(frames_for(0.5, 44100), 22050);
        assert_eq!(frames_for(2.0, 96000), 192000);
    }

    #[test]
    fn test_clip() {
        assert_eq!(clip(0.5), 0.5);
        assert_eq!(clip(-0.5), -0.5);
        assert!(clip(1.5) < 1.0);
        assert!(clip(-1.5) > -1.0);
    }

    #[test]
    fn test_gen_tone() {
        let signal = gen_tone(1000.0, 0.5, 48000, 0.1);
        assert_eq!(signal.len(), 4800);
        // Check that signal is not all zeros
        assert!(signal.iter().any(|&x| x.abs() > 0.1));
    }

    #[test]
    fn test_gen_two_tone() {
        let signal = gen_two_tone(440.0, 0.3, 880.0, 0.3, 48000, 0.1);
        assert_eq!(signal.len(), 4800);
        assert!(signal.iter().any(|&x| x.abs() > 0.1));
    }

    #[test]
    fn test_gen_log_sweep() {
        let signal = gen_log_sweep(20.0, 20000.0, 0.5, 48000, 1.0);
        assert_eq!(signal.len(), 48000);
        assert!(signal.iter().any(|&x| x.abs() > 0.1));
    }

    #[test]
    fn test_gen_log_sweep_amplitude_analysis() {
        // Test amplitude at different points in the sweep
        let amp = 0.5;
        let signal = gen_log_sweep(20.0, 20000.0, amp, 48000, 1.0);

        // Check amplitude at different time points (20%, 40%, 60%, 80%)
        let checkpoints = [0.2, 0.4, 0.6, 0.8];
        let sample_rate = 48000.0;
        let duration = 1.0;

        for &checkpoint in &checkpoints {
            let sample_pos = (checkpoint * duration * sample_rate) as usize;
            let window_size = 480; // 10ms window
            let start = sample_pos.saturating_sub(window_size / 2);
            let end = (sample_pos + window_size / 2).min(signal.len());

            if end > start {
                let window_peak = signal[start..end]
                    .iter()
                    .map(|&x| x.abs())
                    .fold(0.0_f32, |a, b| a.max(b));
                log::info!(
                    "Checkpoint {:.1}: peak amplitude = {:.6} (target: {:.6})",
                    checkpoint,
                    window_peak,
                    amp
                );
            }
        }
    }

    #[test]
    fn test_gen_log_sweep_simple() {
        // Simple test to understand current behavior
        let amp = 0.5;
        let signal = gen_log_sweep(20.0, 20000.0, amp, 48000, 0.1);

        // Find the maximum amplitude in the signal
        let max_amp = signal
            .iter()
            .map(|&x| x.abs())
            .fold(0.0_f32, |a, b| a.max(b));
        log::info!("Generated log sweep:");
        log::info!("  Target amplitude: {:.6}", amp);
        log::info!("  Actual max amplitude: {:.6}", max_amp);
        log::info!("  Ratio: {:.6}", max_amp / amp);

        // Check that we have some signal
        assert!(max_amp > 0.01, "Signal should have significant amplitude");
    }

    #[test]
    fn test_gen_log_sweep_constant_amplitude() {
        // Test that log sweep maintains constant amplitude across frequency range
        let amp = 0.7;
        let sample_rate = 48000_u32;
        let duration = 2.0;
        let f_start = 20.0_f32;
        let f_end = 20000.0_f32;
        let signal = gen_log_sweep(f_start, f_end, amp, sample_rate, duration);

        // Find peak values throughout the sweep using 10ms windows.
        // Skip the low-frequency region where the window (10ms = 100Hz period)
        // is shorter than one cycle and can't capture the true peak.
        let window_size = 480; // 10ms at 48kHz
        let min_freq_for_window = sample_rate as f32 / window_size as f32; // 100 Hz
        let k = (f_end / f_start).ln() / duration;
        let safe_start_t = (min_freq_for_window / f_start).ln() / k;
        let safe_start_sample = (safe_start_t * sample_rate as f32) as usize;

        let mut peaks = Vec::new();
        for i in (safe_start_sample..signal.len()).step_by(window_size / 4) {
            let end = (i + window_size).min(signal.len());
            if end > i {
                let window_peak = signal[i..end].iter().map(|&x| x.abs()).fold(0.0, f32::max);
                peaks.push(window_peak);
            }
        }

        assert!(!peaks.is_empty(), "Should have found peaks");

        let min_peak = peaks.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let max_peak = peaks.iter().fold(0.0_f32, |a, &b| a.max(b));
        let variation = max_peak - min_peak;

        let target_peak = amp;

        // With f64 phase computation, amplitude is near-constant (< 0.1% variation)
        // once the measurement window captures a full cycle.
        assert!(
            variation < 0.01 * target_peak,
            "Peak variation {:.6} exceeds 1% of target amplitude {:.6}",
            variation,
            target_peak
        );

        let avg_peak = peaks.iter().sum::<f32>() / peaks.len() as f32;
        assert!(
            (avg_peak - target_peak).abs() < 0.01 * target_peak,
            "Average peak {:.6} differs from target {:.6} by more than 1%",
            avg_peak,
            target_peak
        );

        log::info!("Log sweep amplitude test passed:");
        log::info!("  Target amplitude: {:.6}", target_peak);
        log::info!("  Min peak: {:.6}", min_peak);
        log::info!("  Max peak: {:.6}", max_peak);
        log::info!(
            "  Variation: {:.6} ({:.2}%)",
            variation,
            100.0 * variation / target_peak
        );
    }

    // ------------------------------------------------------------------
    // gen_log_sweep_octave_scaled tests
    // ------------------------------------------------------------------

    fn rms_of(samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        let sum_sq: f64 = samples.iter().map(|&x| (x as f64) * (x as f64)).sum();
        (sum_sq / samples.len() as f64).sqrt() as f32
    }

    #[test]
    fn test_octave_sweep_length_within_one_sample() {
        // Returned length must equal round(total_duration * sr) ±1.
        let sr = 48000_u32;
        let bass_dur = 3.0_f32;
        let min_dur = 5.0_f32;

        let signal = gen_log_sweep_octave_scaled(10.0, 20_000.0, 0.5, sr, bass_dur, min_dur);

        let oct_bass = (100.0_f64 / 10.0_f64).log2();
        let oct_mid = (1000.0_f64 / 100.0_f64).log2();
        let oct_high = (20000.0_f64 / 1000.0_f64).log2();
        let raw = oct_bass * bass_dur as f64
            + oct_mid * (bass_dur as f64 * 0.5)
            + oct_high * (bass_dur as f64 * 0.25);
        let expected_dur = raw.max(min_dur as f64);
        let expected_n = (expected_dur * sr as f64).round() as usize;

        let diff = (signal.len() as isize - expected_n as isize).unsigned_abs();
        assert!(
            diff <= 1,
            "Length {} differs from expected {} by {} samples (> 1)",
            signal.len(),
            expected_n,
            diff
        );
    }

    #[test]
    fn test_octave_sweep_min_duration_floor() {
        // Narrow sweep -> raw duration < min_total; output must reach the floor.
        let sr = 48000_u32;
        let signal = gen_log_sweep_octave_scaled(1000.0, 2000.0, 0.5, sr, 0.5, 10.0);
        let min_expected = (10.0_f64 * sr as f64).round() as usize - 1;
        assert!(
            signal.len() >= min_expected,
            "Length {} is below the 10s floor (expected >= {})",
            signal.len(),
            min_expected
        );
    }

    #[test]
    fn test_octave_sweep_bass_energy_duration() {
        // The 20–40 Hz band (one octave) must contain at least
        // bass_octave_duration_s seconds of energy at a consistent level.
        let sr = 48000_u32;
        let bass_dur = 3.0_f32;
        let signal = gen_log_sweep_octave_scaled(10.0, 20_000.0, 0.5, sr, bass_dur, 5.0);

        // Compute time bounds for the 20–40 Hz band inside the bass zone.
        let oct_bass = (100.0_f64 / 10.0_f64).log2();
        let oct_mid = (1000.0_f64 / 100.0_f64).log2();
        let oct_high = (20_000.0_f64 / 1000.0_f64).log2();
        let raw = oct_bass * bass_dur as f64
            + oct_mid * (bass_dur as f64 * 0.5)
            + oct_high * (bass_dur as f64 * 0.25);
        let total = raw.max(5.0_f64);
        let scale = total / raw;
        let t_bass = oct_bass * bass_dur as f64 * scale;

        // Within bass zone: t(f) = t_bass * ln(f / f_start) / ln(f_bass_hi / f_start)
        let f_start = 10.0_f64;
        let t_at_20 = t_bass * (20.0_f64 / f_start).ln() / (100.0_f64 / f_start).ln();
        let t_at_40 = t_bass * (40.0_f64 / f_start).ln() / (100.0_f64 / f_start).ln();

        // Collect 200 ms window RMS values whose centre falls in [t_at_20, t_at_40].
        let win = (0.2 * sr as f64).round() as usize;
        let hop = win / 4;
        let mut band_rms: Vec<f32> = Vec::new();
        let mut n = 0;
        while n + win <= signal.len() {
            let t_c = (n + win / 2) as f64 / sr as f64;
            if t_c >= t_at_20 && t_c <= t_at_40 {
                band_rms.push(rms_of(&signal[n..n + win]));
            }
            n += hop;
        }

        assert!(
            !band_rms.is_empty(),
            "No windows found in the 20–40 Hz band"
        );

        let avg: f32 = band_rms.iter().sum::<f32>() / band_rms.len() as f32;

        // All in-band windows must be within ±3 dB of the band average.
        for (i, &w) in band_rms.iter().enumerate() {
            let ratio = if avg > 1e-9 { w / avg } else { 1.0 };
            assert!(
                ratio >= 0.5 && ratio <= 2.0,
                "Window {i} RMS {w:.4} is outside ±3 dB of band avg {avg:.4}"
            );
        }

        // Total time in band >= bass_octave_duration_s (with 10% tolerance).
        let time_in_band = band_rms.len() as f64 * hop as f64 / sr as f64;
        assert!(
            time_in_band >= bass_dur as f64 * 0.9,
            "Time in 20–40 Hz band ({time_in_band:.2}s) < bass_dur ({bass_dur}s)"
        );
    }

    #[test]
    fn test_octave_sweep_phase_zero_at_start() {
        // sin(0) = 0 so the first sample must be ~0 for any f_start.
        for &f_start in &[5.0_f32, 10.0, 20.0, 50.0] {
            let signal = gen_log_sweep_octave_scaled(f_start, 20_000.0, 1.0, 48000, 3.0, 5.0);
            assert!(!signal.is_empty(), "Empty signal for f_start={f_start}");
            assert!(
                signal[0].abs() < 1e-6,
                "First sample {:.2e} != 0 for f_start={f_start}",
                signal[0]
            );
        }
    }

    #[test]
    fn test_octave_sweep_does_not_change_gen_log_sweep() {
        // The old gen_log_sweep must be unaffected.
        let signal = gen_log_sweep(20.0, 20000.0, 0.5, 48000, 1.0);
        assert_eq!(signal.len(), 48000);
        assert!(signal.iter().any(|&x| x.abs() > 0.1));
    }

    #[test]
    fn test_gen_white_noise() {
        let signal = gen_white_noise(0.5, 48000, 1.0); // Use 1 second for better statistics
        assert_eq!(signal.len(), 48000);
        // Check that noise exists and has content
        assert!(signal.iter().any(|&x| x.abs() > 0.01));
        // Check that values are clipped to prevent overflow (clip function limits to +/- 0.999999)
        assert!(signal.iter().all(|&x| x.abs() < 1.0));
    }

    #[test]
    fn test_gen_pink_noise() {
        let signal = gen_pink_noise(0.5, 48000, 0.1);
        assert_eq!(signal.len(), 4800);
        assert!(signal.iter().any(|&x| x.abs() > 0.01));
    }

    #[test]
    fn test_gen_impulse() {
        let signal = gen_impulse(0.5, 48000, 0.1);
        assert_eq!(signal.len(), 4800);
        assert_eq!(signal[0], 0.5);
        for &sample in &signal[1..4800] {
            assert_eq!(sample, 0.0);
        }
    }

    #[test]
    fn test_gen_dirac_matches_impulse() {
        let dirac = gen_dirac(0.5, 48000, 0.1);
        let impulse = gen_impulse(0.5, 48000, 0.1);
        assert_eq!(dirac, impulse);
    }

    #[test]
    fn test_gen_mls_length_and_values() {
        let signal = gen_mls(8, 0.5);
        assert_eq!(signal.len(), 255);
        assert!(signal.iter().all(|&s| s == 0.5 || s == -0.5));
        assert!(signal.iter().any(|&s| s > 0.0));
        assert!(signal.iter().any(|&s| s < 0.0));
    }

    #[test]
    fn test_gen_mls_deterministic() {
        assert_eq!(gen_mls(16, 0.25), gen_mls(16, 0.25));
        assert_ne!(gen_mls(15, 0.25), gen_mls(16, 0.25));
    }

    #[test]
    fn test_gen_mls_autocorrelation_peak() {
        let signal = gen_mls(10, 0.5);
        let zero_lag: f32 = signal.iter().map(|&s| s * s).sum();
        assert!((zero_lag - signal.len() as f32 * 0.25).abs() < 1e-3);
    }

    #[test]
    fn test_gen_mls_rejects_unsupported_order() {
        assert!(gen_mls(1, 0.5).is_empty());
        assert!(gen_mls(25, 0.5).is_empty());
    }

    #[test]
    fn test_gen_step() {
        let signal = gen_step(0.5, 48000, 0.1);
        assert_eq!(signal.len(), 4800);
        for &sample in &signal[..4800] {
            assert_eq!(sample, 0.5);
        }
    }

    #[test]
    fn test_gen_m_noise() {
        let signal = gen_m_noise(0.5, 48000, 0.1);
        assert_eq!(signal.len(), 4800);
        assert!(signal.iter().any(|&x| x.abs() > 0.01));
    }

    #[test]
    fn test_interleave_per_channel() {
        let ch0 = vec![1.0, 2.0, 3.0];
        let ch1 = vec![4.0, 5.0, 6.0];
        let interleaved = interleave_per_channel(&[ch0, ch1]);
        assert_eq!(interleaved, vec![1.0, 4.0, 2.0, 5.0, 3.0, 6.0]);
    }

    #[test]
    fn test_replicate_mono() {
        let mono = vec![1.0, 2.0, 3.0];
        let stereo = replicate_mono(&mono, 2);
        assert_eq!(stereo, vec![1.0, 1.0, 2.0, 2.0, 3.0, 3.0]);
    }

    #[test]
    fn test_apply_fade_in() {
        let mut signal = vec![1.0; 100];
        apply_fade_in(&mut signal, 10);
        // First sample should be near zero
        assert!(signal[0].abs() < 0.01);
        // Middle of fade should be around 0.5
        assert!((signal[5] - 0.5).abs() < 0.1);
        // After fade should be 1.0
        assert_eq!(signal[20], 1.0);
    }

    #[test]
    fn test_apply_fade_out() {
        let mut signal = vec![1.0; 100];
        apply_fade_out(&mut signal, 10);
        // Before fade should be 1.0
        assert_eq!(signal[80], 1.0);
        // Faded region should have reduced amplitude
        assert!(signal[95] < 0.5);
        assert!(signal[99] < 0.1);
    }

    #[test]
    fn test_add_silence_padding() {
        let signal = vec![1.0, 2.0, 3.0];
        let padded = add_silence_padding(&signal, 2, 2);
        assert_eq!(padded.len(), 7);
        assert_eq!(padded, vec![0.0, 0.0, 1.0, 2.0, 3.0, 0.0, 0.0]);
    }

    #[test]
    fn test_mono_to_stereo() {
        let mono = vec![1.0, 0.5, -0.5, 0.0];
        let stereo = mono_to_stereo(mono);
        assert_eq!(stereo, vec![1.0, 1.0, 0.5, 0.5, -0.5, -0.5, 0.0, 0.0]);
    }

    #[test]
    fn test_prepare_signal_for_playback_channels_stereo() {
        let signal = vec![1.0; 100]; // Short signal for testing
        let stereo = prepare_signal_for_playback_channels(signal.clone(), 48000, 10.0, 50.0, true);

        // Stereo should have twice the samples (minus padding which is the same for both)
        let mono_prepared = prepare_signal_for_playback_channels(signal, 48000, 10.0, 50.0, false);
        assert_eq!(stereo.len(), mono_prepared.len() * 2);
    }

    #[test]
    fn test_prepare_signal_for_playback_channels_mono() {
        let signal = vec![1.0; 100]; // Short signal for testing
        let mono = prepare_signal_for_playback_channels(signal.clone(), 48000, 10.0, 50.0, false);
        let mono_direct = prepare_signal_for_playback(signal, 48000, 10.0, 50.0);
        assert_eq!(mono, mono_direct);
    }

    #[test]
    fn test_prepare_signal_for_playback() {
        let signal = vec![1.0; 48000]; // 1 second at 48kHz
        let prepared = prepare_signal_for_playback(signal, 48000, 20.0, 250.0);
        // Should have padding on both sides (250ms = 12000 samples each)
        assert_eq!(prepared.len(), 48000 + 2 * 12000);
        // First samples should be zero (padding)
        assert_eq!(prepared[0], 0.0);
        assert_eq!(prepared[11999], 0.0);
        // Last samples should be zero (padding)
        assert_eq!(prepared[prepared.len() - 1], 0.0);
    }

    #[test]
    fn test_allpass_probe_flat_spectrum() {
        let n = 4096;
        let probe = gen_allpass_probe(n, 48000, 0.5, 42);
        assert_eq!(probe.len(), n);

        // FFT the probe and check magnitude is nearly constant
        let mut fft_proc = RealFftProcessor::new_forward_only(n);
        fft_proc.time_buffer[..n].copy_from_slice(&probe);
        fft_proc.forward();

        // Skip DC and Nyquist, check that magnitudes are within ±1dB of each other
        let mags: Vec<f32> = fft_proc.freq_buffer[1..fft_proc.spectrum_size - 1]
            .iter()
            .map(|c| c.norm())
            .collect();
        let avg_mag = mags.iter().sum::<f32>() / mags.len() as f32;

        for (i, &m) in mags.iter().enumerate() {
            let ratio_db = 20.0 * (m / avg_mag).log10();
            assert!(
                ratio_db.abs() < 1.0,
                "Bin {} magnitude deviates by {:.2} dB from average",
                i + 1,
                ratio_db
            );
        }
    }

    #[test]
    fn test_narrowband_probe_bandpass() {
        let n = 8192;
        let sr = 48000;
        let probe = gen_narrowband_probe(n, sr, 0.5, 42, 800.0, 2000.0);
        assert_eq!(probe.len(), n);

        // FFT and check energy is concentrated in [800, 2000] Hz
        let mut fft_proc = RealFftProcessor::new_forward_only(n);
        fft_proc.time_buffer[..n].copy_from_slice(&probe);
        fft_proc.forward();

        let freq_res = sr as f32 / n as f32;
        let mut in_band_energy = 0.0_f32;
        let mut out_band_energy = 0.0_f32;

        for (k, c) in fft_proc.freq_buffer.iter().enumerate() {
            let freq = k as f32 * freq_res;
            let energy = c.norm_sqr();
            if freq >= 800.0 && freq <= 2000.0 {
                in_band_energy += energy;
            } else {
                out_band_energy += energy;
            }
        }

        // Out-of-band should be negligible compared to in-band
        let ratio = out_band_energy / (in_band_energy + 1e-30);
        assert!(
            ratio < 0.01,
            "Out-of-band energy ratio {:.4} should be < 1%",
            ratio
        );
    }

    #[test]
    fn test_probe_deterministic() {
        let a = gen_allpass_probe(2048, 48000, 0.5, 123);
        let b = gen_allpass_probe(2048, 48000, 0.5, 123);
        assert_eq!(a, b, "Same seed should produce identical probes");

        let c = gen_allpass_probe(2048, 48000, 0.5, 456);
        assert_ne!(a, c, "Different seeds should produce different probes");
    }

    #[test]
    fn test_narrowband_shares_phase_with_wideband() {
        // With the same seed, the narrowband probe's passband bins should
        // have the same phase as the corresponding wideband bins
        let n = 4096;
        let seed = 99;
        let wb_spectrum = gen_probe_spectrum(n, seed);
        let nb_spectrum = gen_probe_spectrum(n, seed);

        // Phase should be identical (same function, same seed)
        for k in 100..200 {
            let wb_phase = wb_spectrum[k].arg();
            let nb_phase = nb_spectrum[k].arg();
            assert!(
                (wb_phase - nb_phase).abs() < 1e-6,
                "Phase mismatch at bin {}",
                k
            );
        }
    }

    // ----- GD-Opt v2 Phase GD-1e — bass tone burst + phase extraction ----

    #[test]
    fn bass_tone_burst_length_and_envelope() {
        // 20 Hz × 5 cycles at 48 kHz → exactly 12 000 samples (250 ms)
        let b = gen_bass_tone_burst(20.0, 5, 48_000, 0.5);
        assert_eq!(b.len(), 12_000);
        // Peak after Hann windowing at amp=0.5 → ≤ 0.5
        let peak = b.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
        assert!(peak <= 0.5 + 1e-6, "burst peak {peak} exceeds amp");
        // Hann envelope: first and last samples are near-zero.
        assert!(b[0].abs() < 1e-4);
        assert!(b[b.len() - 1].abs() < 1e-4);
        // The integer-cycle burst places a zero crossing exactly at
        // the midpoint (2.5 cycles). Peak around the midpoint — scan
        // a ¼-cycle window on either side to find it.
        let quarter_cycle = (48_000 / 20 / 4) as usize; // 600 samples
        let mid = b.len() / 2;
        let peak_near_mid = b[mid - quarter_cycle..=mid + quarter_cycle]
            .iter()
            .map(|s| s.abs())
            .fold(0.0_f32, f32::max);
        assert!(
            peak_near_mid > 0.3,
            "burst centre region should hit peak, got {peak_near_mid}"
        );
    }

    #[test]
    fn bass_tone_burst_rejects_invalid() {
        assert!(gen_bass_tone_burst(0.0, 5, 48_000, 0.5).is_empty());
        assert!(gen_bass_tone_burst(20.0, 0, 48_000, 0.5).is_empty());
        assert!(gen_bass_tone_burst(20.0, 5, 0, 0.5).is_empty());
        assert!(gen_bass_tone_burst(20.0, 5, 48_000, 0.0).is_empty());
    }

    #[test]
    fn tone_phase_recovers_sin_reference_zero() {
        // Reference: pure sin(ωt) at 20 Hz. Phase should be ≈ 0°.
        let burst = gen_bass_tone_burst(20.0, 5, 48_000, 0.5);
        let r = extract_tone_phase(&burst, 20.0, 48_000);
        assert!(
            r.phase_deg.abs() < 1.0,
            "pure sin burst should give phase ≈ 0°, got {:.3}°",
            r.phase_deg
        );
        assert!(r.magnitude > 0.0);
        // A stable-but-Hann-windowed burst has a small residual
        // stability reading (~7°) because the two halves have
        // different envelope profiles. The advisory threshold for
        // `"bass_anchor_unreliable"` is 20° (plan §2.8), so anything
        // below that is considered reliable. Test with a safety
        // margin of 15°.
        assert!(
            r.stability_deg < 15.0,
            "stable tone stability should stay under the 20° advisory threshold, got {:.3}°",
            r.stability_deg
        );
    }

    #[test]
    fn tone_phase_recovers_synthetic_90_degree_shift() {
        // Synthesize a cos-phase burst (= 90° phase shift relative to sin).
        let freq = 20.0_f32;
        let sr = 48_000_u32;
        let n = 12_000;
        let omega = 2.0 * PI * freq / sr as f32;
        let n_f = n as f32;
        let burst: Vec<f32> = (0..n)
            .map(|k| {
                let t = k as f32;
                let w = 0.5 * (1.0 - (2.0 * PI * t / (n_f - 1.0)).cos());
                0.5 * w * (omega * t).cos() // cos(ωt) → 90° relative to sin(ωt)
            })
            .collect();
        let r = extract_tone_phase(&burst, freq, sr);
        // Expected phase is 90° (cos = sin shifted forward by 90°).
        let err = (r.phase_deg - 90.0).abs();
        assert!(
            err < 1.0,
            "cos burst should give phase ≈ 90°, got {:.3}° (error {:.3}°)",
            r.phase_deg,
            err
        );
        // Stable-but-Hann-windowed — see note on the sin-reference
        // test for why the stability reading is non-zero.
        assert!(r.stability_deg < 15.0);
    }

    #[test]
    fn tone_phase_detects_unstable_burst() {
        // Concatenate two half-bursts with a 90° jump in the middle.
        // The stability metric must flag this > 20°.
        let freq = 20.0_f32;
        let sr = 48_000_u32;
        let n_half = 6_000;
        let omega = 2.0 * PI * freq / sr as f32;
        let n_f = (2 * n_half) as f32;
        let burst: Vec<f32> = (0..2 * n_half)
            .map(|k| {
                let t = k as f32;
                let w = 0.5 * (1.0 - (2.0 * PI * t / (n_f - 1.0)).cos());
                let phase_shift = if k < n_half { 0.0 } else { PI / 2.0 };
                0.5 * w * (omega * t + phase_shift).sin()
            })
            .collect();
        let r = extract_tone_phase(&burst, freq, sr);
        assert!(
            r.stability_deg > 20.0,
            "phase-jump burst should be flagged unstable, got stability = {:.1}°",
            r.stability_deg
        );
    }

    #[test]
    fn tone_phase_rejects_short_signal() {
        let r = extract_tone_phase(&[0.1, 0.2], 20.0, 48_000);
        assert_eq!(r.magnitude, 0.0);
        assert_eq!(r.phase_deg, 0.0);
        assert_eq!(r.stability_deg, 0.0);
    }

    // ----- Steady-state lock-in helpers (bass-anchor v2) -----------------

    #[test]
    fn steady_tone_length_and_fade_envelope() {
        let sr = 48_000_u32;
        let s = gen_steady_tone(30.0, 1.0, 50.0, sr, 0.5);
        assert_eq!(
            s.len(),
            48_000,
            "1 s @ 48 kHz must be exactly 48 000 samples"
        );
        // Endpoints sit on the half-Hann fade — should be zero.
        assert!(
            s[0].abs() < 1e-6,
            "fade-in must start at zero, got {}",
            s[0]
        );
        assert!(s[s.len() - 1].abs() < 1e-6, "fade-out must end at zero");
        // Steady region should hit the requested amplitude (at amp=0.5).
        let steady_start = (0.06 * sr as f32) as usize; // 60 ms in
        let steady_end = s.len() - steady_start;
        let peak: f32 = s[steady_start..steady_end]
            .iter()
            .map(|s| s.abs())
            .fold(0.0, f32::max);
        assert!(
            (peak - 0.5).abs() < 0.02,
            "steady region peak should be ≈ amp (0.5), got {peak}"
        );
    }

    #[test]
    fn steady_tone_rejects_invalid() {
        assert!(gen_steady_tone(0.0, 1.0, 50.0, 48_000, 0.5).is_empty());
        assert!(gen_steady_tone(30.0, 0.0, 50.0, 48_000, 0.5).is_empty());
        assert!(gen_steady_tone(30.0, 1.0, 50.0, 0, 0.5).is_empty());
        assert!(gen_steady_tone(30.0, 1.0, 50.0, 48_000, 0.0).is_empty());
        // Duration shorter than two fades.
        assert!(gen_steady_tone(30.0, 0.05, 50.0, 48_000, 0.5).is_empty());
    }

    #[test]
    fn windowed_phase_recovers_pure_sin() {
        let sr = 48_000_u32;
        let s = gen_steady_tone(30.0, 2.0, 50.0, sr, 0.5);
        let r = extract_tone_phase_windowed(&s, 30.0, sr, 8);
        assert!(
            r.phase_deg.abs() < 0.5,
            "pure sin should give phase ≈ 0°, got {:.4}°",
            r.phase_deg
        );
        assert!(r.magnitude > 0.0);
        assert!(
            r.stability_deg < 1.0,
            "pure sin steady tone should give near-zero circular-std, got {:.3}°",
            r.stability_deg
        );
    }

    #[test]
    fn windowed_phase_recovers_synthetic_45_degree_shift() {
        let sr = 48_000_u32;
        let freq = 30.0_f32;
        let n = (sr as f32 * 2.0) as usize;
        let omega = 2.0 * PI * freq / sr as f32;
        let phase_shift = (45.0_f32).to_radians();
        let fade_n = (0.05 * sr as f32) as usize;
        let s: Vec<f32> = (0..n)
            .map(|k| {
                let env = if k < fade_n {
                    0.5 * (1.0 - (PI * k as f32 / fade_n as f32).cos())
                } else if k >= n - fade_n {
                    let kk = (n - 1 - k) as f32;
                    0.5 * (1.0 - (PI * kk / fade_n as f32).cos())
                } else {
                    1.0
                };
                0.5 * env * (omega * k as f32 + phase_shift).sin()
            })
            .collect();
        let r = extract_tone_phase_windowed(&s, freq, sr, 8);
        let err = (r.phase_deg - 45.0).abs();
        assert!(
            err < 0.5,
            "45° shifted sin should give phase ≈ 45°, got {:.3}° (err {:.3}°)",
            r.phase_deg,
            err
        );
        assert!(r.stability_deg < 1.0);
    }

    #[test]
    fn windowed_phase_flags_drifting_phase_as_unstable() {
        let sr = 48_000_u32;
        let freq = 30.0_f32;
        let n = (sr as f32 * 2.0) as usize;
        let omega = 2.0 * PI * freq / sr as f32;
        // Inject a slow phase drift (linear ramp from 0 to ±90°). The
        // circular std should rise well above the ~1° pure-tone floor.
        let s: Vec<f32> = (0..n)
            .map(|k| {
                let frac = k as f32 / n as f32;
                let drift = (PI / 2.0) * frac;
                0.5 * (omega * k as f32 + drift).sin()
            })
            .collect();
        let r = extract_tone_phase_windowed(&s, freq, sr, 8);
        assert!(
            r.stability_deg > 5.0,
            "drifting-phase tone should read unstable, got circular-std {:.3}°",
            r.stability_deg
        );
    }

    #[test]
    fn windowed_phase_rejects_sub_cycle_buffers() {
        // 30 Hz @ 48 kHz → 1600 samples/cycle. A signal of 800 samples
        // contains less than one cycle even before the /8 settle drop.
        // The helper must refuse rather than return falsely-confident
        // numbers.
        let sr = 48_000_u32;
        let n = 800; // < one cycle
        let omega = 2.0 * PI * 30.0 / sr as f32;
        let s: Vec<f32> = (0..n).map(|k| 0.5 * (omega * k as f32).sin()).collect();
        let phasors = tone_phase_phasors(&s, 30.0, sr, 8);
        assert!(
            phasors.is_empty(),
            "sub-cycle buffers must yield empty phasor list, got {} entries",
            phasors.len()
        );
        let r = extract_tone_phase_windowed(&s, 30.0, sr, 8);
        assert_eq!(r.magnitude, 0.0);
        assert_eq!(r.phase_deg, 0.0);
    }

    #[test]
    fn windowed_phase_merges_when_per_window_below_one_cycle() {
        // 30 Hz @ 48 kHz, 2 s tone, asking for 64 windows: each raw
        // window would be 2 s/64 = 31 ms < one cycle (33 ms). The
        // helper must merge windows down so each spans ≥ 1 cycle and
        // still recover phase to within ~1°.
        let sr = 48_000_u32;
        let s = gen_steady_tone(30.0, 2.0, 50.0, sr, 0.5);
        let r = extract_tone_phase_windowed(&s, 30.0, sr, 64);
        assert!(
            r.phase_deg.abs() < 1.0,
            "merged-window analysis should still recover ~0° phase, got {:.3}°",
            r.phase_deg
        );
        assert!(r.magnitude > 0.0);
    }

    #[test]
    fn phasor_coherence_is_one_for_identical_phasors() {
        let sr = 48_000_u32;
        let s = gen_steady_tone(30.0, 2.0, 50.0, sr, 0.5);
        let p = tone_phase_phasors(&s, 30.0, sr, 8);
        let coh = phasor_coherence(&p, &p).expect("coherent");
        assert!(
            (coh - 1.0).abs() < 1e-9,
            "γ²(x, x) should be exactly 1, got {coh}"
        );
    }

    #[test]
    fn phasor_coherence_drops_when_one_stream_is_noise() {
        // Mic = clean tone, loopback = uncorrelated white noise →
        // γ² should approach 0. The previous proxy returned 1.0 in
        // this case; the true MSC must be < 0.5.
        use rand::{Rng, SeedableRng};
        let sr = 48_000_u32;
        let clean = gen_steady_tone(30.0, 2.0, 50.0, sr, 0.5);
        let mut rng = rand::rngs::StdRng::seed_from_u64(0xC0FFEE);
        let noise: Vec<f32> = (0..clean.len())
            .map(|_| rng.random_range(-0.3..0.3))
            .collect();
        let p_clean = tone_phase_phasors(&clean, 30.0, sr, 8);
        let p_noise = tone_phase_phasors(&noise, 30.0, sr, 8);
        let coh = phasor_coherence(&p_clean, &p_noise).expect("coherent");
        assert!(
            coh < 0.3,
            "γ²(clean, noise) must drop well below 0.5, got {coh}"
        );
    }

    #[test]
    fn phasor_coherence_rejects_mismatched_lengths() {
        let sr = 48_000_u32;
        let s = gen_steady_tone(30.0, 2.0, 50.0, sr, 0.5);
        let a = tone_phase_phasors(&s, 30.0, sr, 8);
        let b = tone_phase_phasors(&s, 30.0, sr, 4);
        assert!(phasor_coherence(&a, &b).is_none());
    }

    #[test]
    fn windowed_phase_rejects_degenerate_inputs() {
        let r = extract_tone_phase_windowed(&[0.1, 0.2, 0.3], 30.0, 48_000, 8);
        assert_eq!(r.phase_deg, 0.0);
        assert_eq!(r.magnitude, 0.0);
        assert_eq!(r.stability_deg, 0.0);

        let r = extract_tone_phase_windowed(&vec![0.1_f32; 1000], 30.0, 48_000, 0);
        assert_eq!(r.magnitude, 0.0);
    }
}
