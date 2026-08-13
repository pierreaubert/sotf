use rustfft::num_complex::Complex;

/// Pre-computed frequency-domain HRTF filter for a single reflection.
#[derive(Debug, Clone)]
pub struct ReflectionHrtf {
    /// Broadband left-ear gain derived from HRTF energy.
    pub left_gain_broadband: f32,
    /// Broadband right-ear gain derived from HRTF energy.
    pub right_gain_broadband: f32,
}

impl ReflectionHrtf {
    /// Create from frequency-domain HRTF data, computing broadband gains automatically.
    pub fn from_freq_domain(left: Vec<Complex<f32>>, right: Vec<Complex<f32>>) -> Self {
        // Compute broadband gain as RMS of magnitude spectrum
        let left_energy: f32 = left.iter().map(|c| c.norm_sqr()).sum();
        let right_energy: f32 = right.iter().map(|c| c.norm_sqr()).sum();

        let n = left.len().max(1) as f32;
        let left_rms = (left_energy / n).sqrt();
        let right_rms = (right_energy / n).sqrt();

        // Normalize so that the louder ear gets gain 1.0
        let max_rms = left_rms.max(right_rms).max(1e-12);
        let left_gain_broadband = left_rms / max_rms;
        let right_gain_broadband = right_rms / max_rms;

        Self {
            left_gain_broadband,
            right_gain_broadband,
        }
    }
}
