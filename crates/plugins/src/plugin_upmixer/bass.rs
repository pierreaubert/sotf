// ============================================================================
// Bass Processing and Crossover Management
// ============================================================================

use super::UpmixerPlugin;
use math_audio_iir_fir::{Biquad, BiquadFilterType};

impl UpmixerPlugin {
    /// Update Linkwitz-Riley crossover gains for mains and LFE separation
    ///
    /// This creates frequency-domain magnitude tables for a 4th-order (LR4)
    /// Linkwitz-Riley crossover between main speakers and LFE channel.
    pub(super) fn update_crossover_gains(&mut self) {
        let num_bins = self.fft_size / 2 + 1;

        if self.lfe_low_gains.len() != num_bins {
            self.lfe_low_gains = vec![0.0; num_bins];
            self.mains_high_gains = vec![1.0; num_bins];
        }

        // Fallback: if we don't have a valid sample rate yet, keep all bass in mains
        if self.sample_rate == 0 || self.lfe_cutoff_hz <= 0.0 {
            for i in 0..num_bins {
                self.lfe_low_gains[i] = 0.0;
                self.mains_high_gains[i] = 1.0;
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

            let mut low_mag = 1.0_f64;
            let mut high_mag = 1.0_f64;

            for sec in &low_sections {
                low_mag *= sec.result(f);
            }
            for sec in &high_sections {
                high_mag *= sec.result(f);
            }

            // Normalize so that low^2 + high^2 ≈ 1.0 to avoid level shifts
            let power = low_mag * low_mag + high_mag * high_mag;
            if power > 0.0 {
                let norm = power.sqrt();
                low_mag /= norm;
                high_mag /= norm;
            }

            self.lfe_low_gains[i] = low_mag as f32;
            self.mains_high_gains[i] = high_mag as f32;
        }
    }

    /// Apply sub-harmonic synthesis to LFE channel
    ///
    /// Generates a configurable rumble tone modulated by the LFE envelope
    /// with smooth attack/release to prevent clicks and pops.
    pub(super) fn apply_subharmonic_synthesis(&mut self) {
        if !self.enable_subharmonic_synth {
            return;
        }

        if let Some(lfe_idx) = self.speaker_config.speakers.iter().position(|s| s.is_lfe) {
            // Generate subharmonics based on LFE amplitude
            // Use configurable frequency (default 40Hz rumble) modulated by the LFE envelope
            let phase_inc =
                2.0 * std::f32::consts::PI * self.subharmonic_freq_hz / self.sample_rate as f32;

            // Envelope smoothing parameters (time constants in samples)
            // Convert attack/release times from ms to seconds for coefficient calculation
            let attack_time_sec = self.subharmonic_attack_ms / 1000.0;
            let release_time_sec = self.subharmonic_release_ms / 1000.0;
            let attack_coeff = 1.0 - (-1.0 / (attack_time_sec * self.sample_rate as f32)).exp();
            let release_coeff = 1.0 - (-1.0 / (release_time_sec * self.sample_rate as f32)).exp();

            for i in 0..self.fft_size {
                // Use the time-domain LFE signal as the envelope
                let lfe_amp = self.time_out_channels[lfe_idx][i].abs();

                // Smooth envelope: gradually ramp up/down instead of hard switching
                // This prevents clicks and pops when sub-harmonic synthesis turns on/off
                if lfe_amp > 0.001 {
                    // Attack: envelope moves toward 1.0
                    self.subharmonic_envelope += (1.0 - self.subharmonic_envelope) * attack_coeff;
                } else {
                    // Release: envelope moves toward 0.0
                    self.subharmonic_envelope += (0.0 - self.subharmonic_envelope) * release_coeff;
                }

                // Only generate sub-harmonic if envelope is above threshold
                if self.subharmonic_envelope > 0.0001 {
                    self.subharmonic_phase += phase_inc;
                    if self.subharmonic_phase > 2.0 * std::f32::consts::PI {
                        self.subharmonic_phase -= 2.0 * std::f32::consts::PI;
                    }

                    // Apply envelope to sub-harmonic for smooth transitions
                    let sub = self.subharmonic_phase.sin()
                        * lfe_amp
                        * self.subharmonic_gain.current()
                        * self.subharmonic_envelope;
                    self.time_out_channels[lfe_idx][i] += sub;
                }
            }
        }
    }
}
