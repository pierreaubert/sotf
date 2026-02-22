// ============================================================================
// Bass Processing and Crossover Management
// ============================================================================

use super::UpmixerPlugin;
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use rustfft::num_complex::Complex;

type Complex64 = Complex<f64>;

impl UpmixerPlugin {
    /// Update Linkwitz-Riley crossover gains for mains and LFE separation
    ///
    /// This creates frequency-domain magnitude tables for a 4th-order (LR4)
    /// Linkwitz-Riley crossover between main speakers and LFE channel.
    pub(super) fn update_crossover_gains(&mut self) {
        let num_bins = self.fft_size / 2 + 1;

        if self.lfe_low_gains.len() != num_bins {
            self.lfe_low_gains = vec![Complex::new(0.0, 0.0); num_bins];
            self.mains_high_gains = vec![Complex::new(1.0, 0.0); num_bins];
        }

        // Fallback: if we don't have a valid sample rate yet, keep all bass in mains
        if self.sample_rate == 0 || self.lfe_cutoff_hz <= 0.0 {
            for i in 0..num_bins {
                self.lfe_low_gains[i] = Complex::new(0.0, 0.0);
                self.mains_high_gains[i] = Complex::new(1.0, 0.0);
            }
            return;
        }

        let cutoff = self.lfe_cutoff_hz as f64;
        let srate = self.sample_rate as f64;

        // LR4: cascade two 2nd-order Butterworth sections for low-pass and high-pass
        let q = 1.0 / std::f64::consts::SQRT_2;

        let mut low_sections = Vec::new();
        let mut high_sections = Vec::new();
        for _ in 0..2 {
            low_sections.push(Biquad::new(
                BiquadFilterType::Lowpass,
                cutoff,
                srate,
                q,
                0.0,
            ));
            high_sections.push(Biquad::new(
                BiquadFilterType::Highpass,
                cutoff,
                srate,
                q,
                0.0,
            ));
        }

        let freq_per_bin = srate / self.fft_size as f64;

        for i in 0..num_bins {
            let f = i as f64 * freq_per_bin;

            // Use complex response to preserve phase information
            let mut low_h = Complex64::new(1.0, 0.0);
            let mut high_h = Complex64::new(1.0, 0.0);

            for sec in &low_sections {
                low_h *= sec.complex_response(f);
            }
            for sec in &high_sections {
                high_h *= sec.complex_response(f);
            }

            // Normalize so that |low|^2 + |high|^2 ≈ 1.0 to avoid level shifts
            let power = low_h.norm_sqr() + high_h.norm_sqr();
            if power > 0.0 {
                let norm = power.sqrt();
                low_h /= norm;
                high_h /= norm;
            }

            self.lfe_low_gains[i] = Complex::new(low_h.re as f32, low_h.im as f32);
            self.mains_high_gains[i] = Complex::new(high_h.re as f32, high_h.im as f32);
        }
    }

    /// Apply sub-harmonic synthesis to LFE channel
    ///
    /// Generates a configurable rumble tone modulated by the LFE envelope
    /// with smooth attack/release to prevent clicks and pops.
    pub(super) fn apply_subharmonic_synthesis(&mut self) {
        // When disabled, let the envelope release smoothly instead of hard-cutting
        if !self.enable_subharmonic_synth && self.subharmonic_envelope < 1e-6 {
            return;
        }

        if let Some(lfe_idx) = self.speaker_config.speakers.iter().position(|s| s.is_lfe) {
            // Generate subharmonics based on LFE amplitude.
            // Phase increment and envelope coefficients are pre-computed in
            // initialize() / set_parameter() to avoid per-block transcendental calls.
            let phase_inc = self.cached_subharmonic_phase_inc;
            let attack_coeff = self.cached_subharmonic_attack_coeff;
            let release_coeff = self.cached_subharmonic_release_coeff;

            // Soft threshold for envelope detection - prevents clicks at threshold crossing
            // Using a sigmoid-like transition instead of hard threshold
            let threshold = 0.001_f32;
            let soft_knee = 0.0005_f32; // Transition zone width

            for i in 0..self.fft_size {
                // Use the time-domain LFE signal as the envelope
                let lfe_amp = self.time_out_channels[lfe_idx][i].abs();

                // Smooth the amplitude envelope to prevent raw AM distortion
                let amp_coeff = if lfe_amp > self.subharmonic_amp_envelope {
                    attack_coeff
                } else {
                    release_coeff
                };
                self.subharmonic_amp_envelope +=
                    (lfe_amp - self.subharmonic_amp_envelope) * amp_coeff;

                // Smooth envelope using soft threshold for click-free transitions
                // Instead of hard threshold, use continuous envelope tracking
                // that responds proportionally to input amplitude
                let target_envelope = if !self.enable_subharmonic_synth {
                    0.0 // Force release when disabled
                } else if lfe_amp < threshold - soft_knee {
                    0.0
                } else if lfe_amp > threshold + soft_knee {
                    1.0
                } else {
                    // Soft knee region: smooth transition
                    0.5 + (lfe_amp - threshold) / (2.0 * soft_knee)
                };

                // Apply attack or release based on whether we're going up or down
                let envelope_coeff = if target_envelope > self.subharmonic_envelope {
                    attack_coeff
                } else {
                    release_coeff
                };
                self.subharmonic_envelope +=
                    (target_envelope - self.subharmonic_envelope) * envelope_coeff;

                // Always generate sub-harmonic with envelope applied
                // Very small envelopes will be inaudible but still smooth
                self.subharmonic_phase += phase_inc;
                if self.subharmonic_phase > 2.0 * std::f32::consts::PI {
                    self.subharmonic_phase -= 2.0 * std::f32::consts::PI;
                }

                // Use smoothed amplitude envelope instead of raw lfe_amp to prevent
                // sub-harmonic distortion from instantaneous amplitude modulation
                let sub = self.subharmonic_phase.sin()
                    * self.subharmonic_amp_envelope
                    * self.subharmonic_gain.current()
                    * self.subharmonic_envelope;

                // Only add if envelope is significant (prevents denormal issues)
                if self.subharmonic_envelope > 1e-6 {
                    self.time_out_channels[lfe_idx][i] += sub;
                }
            }
        }
    }
}
