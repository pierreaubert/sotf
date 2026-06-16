// ============================================================================
// Bass Processing and Crossover Management
// ============================================================================

use super::UpmixerPlugin;
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use rustfft::num_complex::Complex;

#[inline]
fn lr4_crossover_response_pair(
    frequency: f64,
    low_section: &Biquad<f64>,
    high_section: &Biquad<f64>,
) -> (Complex<f64>, Complex<f64>) {
    let low_response = low_section.complex_response(frequency);
    let high_response = high_section.complex_response(frequency);
    (low_response * low_response, high_response * high_response)
}

#[inline(always)]
fn subharmonic_envelope_source_sample(lfe_sample_before_synthesis: f32) -> f32 {
    lfe_sample_before_synthesis.abs()
}

impl UpmixerPlugin {
    /// Update Linkwitz-Riley crossover gains for mains and LFE separation
    ///
    /// This creates frequency-domain magnitude tables for a 4th-order (LR4)
    /// Linkwitz-Riley crossover between main speakers and LFE channel.
    pub(super) fn update_crossover_gains(&mut self) {
        let num_bins = self.core.fft_size / 2 + 1;

        if self.spectral.lfe_low_gains.len() != num_bins {
            self.spectral.lfe_low_gains = vec![Complex::new(0.0, 0.0); num_bins];
            self.spectral.mains_high_gains = vec![Complex::new(1.0, 0.0); num_bins];
        }

        // Fallback: if we don't have a valid sample rate yet, keep all bass in mains
        if self.core.sample_rate == 0 || self.params.lfe_cutoff_hz <= 0.0 {
            for i in 0..num_bins {
                self.spectral.lfe_low_gains[i] = Complex::new(0.0, 0.0);
                self.spectral.mains_high_gains[i] = Complex::new(1.0, 0.0);
            }
            return;
        }

        let cutoff = self.params.lfe_cutoff_hz as f64;
        let srate = self.core.sample_rate as f64;
        let q = 1.0 / std::f64::consts::SQRT_2;
        let low_section = Biquad::new(BiquadFilterType::Lowpass, cutoff, srate, q, 0.0);
        let high_section = Biquad::new(BiquadFilterType::Highpass, cutoff, srate, q, 0.0);

        let freq_per_bin = srate / self.core.fft_size as f64;

        for i in 0..num_bins {
            let f = i as f64 * freq_per_bin;

            // LR4 is the cascade of two matched 2nd-order Butterworth sections.
            // Do not magnitude-normalize the pair; the complex sum carries the
            // crossover phase relationship.
            let (low_h, high_h) = lr4_crossover_response_pair(f, &low_section, &high_section);

            self.spectral.lfe_low_gains[i] = Complex::new(low_h.re as f32, low_h.im as f32);
            self.spectral.mains_high_gains[i] = Complex::new(high_h.re as f32, high_h.im as f32);
        }
    }

    /// Apply sub-harmonic synthesis to LFE channel
    ///
    /// Generates a configurable rumble tone modulated by the LFE envelope
    /// with smooth attack/release to prevent clicks and pops.
    pub(super) fn apply_subharmonic_synthesis(&mut self) {
        // When disabled, let the envelope release smoothly instead of hard-cutting
        if !self.subharmonic.enable_subharmonic_synth
            && self.subharmonic.subharmonic_envelope < 1e-6
        {
            return;
        }

        if let Some(lfe_idx) = self
            .core
            .speaker_config
            .speakers
            .iter()
            .position(|s| s.is_lfe)
        {
            // Generate subharmonics based on LFE amplitude.
            // Phase increment and envelope coefficients are pre-computed in
            // initialize() / set_parameter() to avoid per-block transcendental calls.
            let phase_inc = self.subharmonic.cached_subharmonic_phase_inc;
            let attack_coeff = self.subharmonic.cached_subharmonic_attack_coeff;
            let release_coeff = self.subharmonic.cached_subharmonic_release_coeff;

            // Soft threshold for envelope detection - prevents clicks at threshold crossing
            // Using a sigmoid-like transition instead of hard threshold
            let threshold = 0.001_f32;
            let soft_knee = 0.0005_f32; // Transition zone width

            for i in 0..self.core.fft_size {
                // Use the current time-domain LFE sample before adding the synthesized
                // subharmonic. This keeps the envelope independent of the generated tone.
                let lfe_amp = subharmonic_envelope_source_sample(
                    self.main_buffers.time_out_channels[lfe_idx][i],
                );

                // Smooth the amplitude envelope to prevent raw AM distortion
                let amp_coeff = if lfe_amp > self.subharmonic.subharmonic_amp_envelope {
                    attack_coeff
                } else {
                    release_coeff
                };
                self.subharmonic.subharmonic_amp_envelope +=
                    (lfe_amp - self.subharmonic.subharmonic_amp_envelope) * amp_coeff;

                // Smooth envelope using soft threshold for click-free transitions
                // Instead of hard threshold, use continuous envelope tracking
                // that responds proportionally to input amplitude
                let target_envelope = if !self.subharmonic.enable_subharmonic_synth
                    || lfe_amp < threshold - soft_knee
                {
                    0.0 // Force release when disabled or below threshold
                } else if lfe_amp > threshold + soft_knee {
                    1.0
                } else {
                    // Soft knee region: smooth transition
                    0.5 + (lfe_amp - threshold) / (2.0 * soft_knee)
                };

                // Apply attack or release based on whether we're going up or down
                let envelope_coeff = if target_envelope > self.subharmonic.subharmonic_envelope {
                    attack_coeff
                } else {
                    release_coeff
                };
                self.subharmonic.subharmonic_envelope +=
                    (target_envelope - self.subharmonic.subharmonic_envelope) * envelope_coeff;

                // Always generate sub-harmonic with envelope applied
                // Very small envelopes will be inaudible but still smooth
                self.subharmonic.subharmonic_phase += phase_inc;
                if self.subharmonic.subharmonic_phase > 2.0 * std::f32::consts::PI {
                    self.subharmonic.subharmonic_phase -= 2.0 * std::f32::consts::PI;
                }

                // Use smoothed amplitude envelope instead of raw lfe_amp to prevent
                // sub-harmonic distortion from instantaneous amplitude modulation
                let sub = self.subharmonic.subharmonic_phase.sin()
                    * self.subharmonic.subharmonic_amp_envelope
                    * self.subharmonic.subharmonic_gain.current()
                    * self.subharmonic.subharmonic_envelope;

                // Only add if envelope is significant (prevents denormal issues)
                if self.subharmonic.subharmonic_envelope > 1e-6 {
                    self.main_buffers.time_out_channels[lfe_idx][i] += sub;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lr4_crossover_pair_preserves_complex_unity_sum() {
        let cutoff = 120.0;
        let sample_rate = 48_000.0;
        let q = 1.0 / std::f64::consts::SQRT_2;
        let low_section = Biquad::new(BiquadFilterType::Lowpass, cutoff, sample_rate, q, 0.0);
        let high_section = Biquad::new(BiquadFilterType::Highpass, cutoff, sample_rate, q, 0.0);

        for frequency in [30.0, 90.0, 120.0, 180.0, 480.0] {
            let (low, high) = lr4_crossover_response_pair(frequency, &low_section, &high_section);
            let summed = low + high;

            assert!(
                (summed.norm() - 1.0).abs() < 1e-3,
                "LR4 complex sum should stay near unity at {frequency} Hz, got {summed:?}"
            );
        }
    }

    #[test]
    fn subharmonic_envelope_source_uses_pre_synthesis_sample() {
        let before = -0.25;
        let synthesized = 0.75;

        assert_eq!(subharmonic_envelope_source_sample(before), 0.25);
        assert_ne!(
            subharmonic_envelope_source_sample(before),
            (before + synthesized).abs()
        );
    }
}
