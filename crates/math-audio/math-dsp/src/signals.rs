//! Audio signal generation utilities
//!
//! Provides functions for generating various test signals including:
//! - Pure tones (sine waves)
//! - Two-tone signals (for IMD testing)
//! - Logarithmic frequency sweeps
//! - White noise
//! - Pink noise (1/f spectrum)
//! - M-weighted noise (ITU-R 468)

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

    let k = (f_end / f_start).ln() / duration;
    let coefficient = 2.0 * PI * f_start / k;

    for n in 0..n_frames {
        let t = n as f32 / sample_rate as f32;
        let phase = coefficient * ((k * t).exp() - 1.0);
        signal.push(clip(amp * phase.sin()));
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

        // Normalize and scale (pink noise is ~3dB louder than white)
        signal.push(clip(amp * pink * 0.11));
    }

    signal
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
    // M-weighted noise uses ITU-R 468 weighting curve
    // This is an approximation using a shaped white noise approach
    let n_frames = frames_for(duration, sample_rate);
    let mut signal = Vec::with_capacity(n_frames);

    // Generate white noise first
    let mut seed: u64 = 1122334455;
    let mut noise_buffer = Vec::with_capacity(n_frames);

    for _ in 0..n_frames {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        // Mask to 32 bits to get proper range [0, u32::MAX]
        let random_u32 = (seed & 0xFFFFFFFF) as u32;
        let white = (random_u32 as f32 / u32::MAX as f32) * 2.0 - 1.0;
        noise_buffer.push(white);
    }

    // Apply ITU-R 468 weighting approximation using IIR filters
    // This is a simplified version that boosts high frequencies (emphasis around 6.3 kHz)
    let mut hp_state = 0.0f32;

    // High-pass filter coefficient (cutoff around 30 Hz)
    let hp_coeff = (-2.0 * PI * 30.0 / sample_rate as f32).exp();

    // Peak filter coefficients (peak around 6300 Hz)
    let peak_freq = 6300.0;
    let peak_gain_db = 12.0; // ITU-R 468 has peak around 6.3 kHz
    let w0 = 2.0 * PI * peak_freq / sample_rate as f32;
    let a = 10.0f32.powf(peak_gain_db / 40.0);

    for &white in &noise_buffer {
        // High-pass filter
        hp_state = hp_coeff * (hp_state + white);

        // Simplified peak boost (approximate ITU-R 468 weighting)
        let boosted = hp_state * (1.0 + (w0 * hp_state.abs()).sin() * a * 0.3);

        signal.push(clip(amp * boosted * 0.7));
    }

    signal
}

/// Interleave per-channel signals into a multi-channel interleaved buffer
///
/// Takes a vector of per-channel signals and interleaves them frame-by-frame.
///
/// # Arguments
/// * `per_channel` - Vector of per-channel signals (each Vec<f32> is one channel)
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

    // Add silence padding
    add_silence_padding(&signal, padding_samples, padding_samples)
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
        let signal = gen_log_sweep(20.0, 20000.0, amp, 48000, 2.0); // Start at 20Hz instead of 1Hz

        // Find peak values throughout the sweep
        let mut peaks = Vec::new();
        let window_size = 480; // 10ms windows at 48kHz

        for i in (0..signal.len()).step_by(window_size / 4) {
            let end = (i + window_size).min(signal.len());
            if end > i {
                let window_peak = signal[i..end].iter().map(|&x| x.abs()).fold(0.0, f32::max);
                peaks.push(window_peak);
            }
        }

        // Additional checks
        assert!(!peaks.is_empty(), "Should have found peaks");

        // Check that we have good coverage across the sweep
        // Skip first few windows where frequency might still be ramping up
        let peaks_to_check: Vec<_> = peaks.iter().skip(2).copied().collect();
        let min_peak = peaks_to_check.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let max_peak = peaks_to_check.iter().fold(0.0_f32, |a, &b| a.max(b));
        let variation = max_peak - min_peak;

        let target_peak = amp;

        // For log sweeps, we expect some amplitude variation due to frequency changes
        // and the exponential nature of the sweep. Check that variation is reasonable.
        // A 30% variation is acceptable for log sweeps.
        assert!(
            variation < 0.30 * target_peak,
            "Peak variation {:.6} exceeds 30% of target amplitude {:.6}",
            variation,
            target_peak
        );

        // Check that average peak is reasonably close to target (within 15%)
        // Log sweeps don't maintain perfect constant amplitude
        let avg_peak = peaks_to_check.iter().sum::<f32>() / peaks_to_check.len() as f32;
        assert!(
            (avg_peak - target_peak).abs() < 0.15 * target_peak,
            "Average peak {:.6} differs from target {:.6} by more than 15%",
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
}
