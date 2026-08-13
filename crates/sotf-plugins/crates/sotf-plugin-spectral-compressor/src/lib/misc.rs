#![allow(dead_code)]
pub(super) const FFT_SIZE_OPTIONS: [usize; 3] = [1024, 2048, 4096];

pub(super) fn fft_size_from_index(index: usize) -> usize {
    FFT_SIZE_OPTIONS.get(index).copied().unwrap_or(2048)
}

/// Compressor gain reduction: standard soft-knee formula.
///
/// Returns gain reduction in dB (positive value) for signals above threshold.
#[inline]
pub(super) fn compress_gr(input_db: f32, threshold: f32, ratio: f32, knee: f32) -> f32 {
    let slope = 1.0 - 1.0 / ratio.max(1.0);
    if knee < 0.1 {
        if input_db <= threshold {
            0.0
        } else {
            (input_db - threshold) * slope
        }
    } else if input_db < threshold - knee / 2.0 {
        0.0
    } else if input_db > threshold + knee / 2.0 {
        (input_db - threshold) * slope
    } else {
        let overshoot = input_db - threshold + knee / 2.0;
        let kf = overshoot / knee;
        kf * kf * (knee / 2.0) * slope
    }
}

/// Adaptive-estimator coefficient for a time constant expressed in seconds.
#[inline]
pub(super) fn adaptive_alpha(hop_size: usize, sample_rate: u32, tau_seconds: f32) -> f32 {
    (-(hop_size as f32) / (tau_seconds * sample_rate as f32)).exp()
}

/// Apply an edge-normalized, reversal-invariant box smoother.
///
/// `amount` maps linearly to a radius of 0..=12 FFT bins. The prefix-sum
/// scratch must contain at least `envelope.len() + 1` elements. Normalizing by
/// the number of available bins preserves a flat field at DC and Nyquist.
pub(super) fn smooth_spectral_envelope(envelope: &mut [f32], amount: f32, prefix: &mut [f32]) {
    if envelope.len() < 2 || amount <= 0.0 {
        return;
    }
    debug_assert!(prefix.len() > envelope.len());
    let radius = (amount.clamp(0.0, 1.0) * 12.0).ceil() as usize;
    prefix[0] = 0.0;
    for (index, value) in envelope.iter().enumerate() {
        prefix[index + 1] = prefix[index] + *value;
    }
    for (index, value) in envelope.iter_mut().enumerate() {
        let start = index.saturating_sub(radius);
        let end = (index + radius + 1).min(prefix.len() - 1);
        *value = (prefix[end] - prefix[start]) / (end - start) as f32;
    }
}
