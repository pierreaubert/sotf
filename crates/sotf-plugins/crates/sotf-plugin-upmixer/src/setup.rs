// ============================================================================
// Setup and Configuration Management
// ============================================================================

use super::UpmixerPlugin;
use rustfft::num_complex::Complex;
use sotf_host::plugin::{Plugin, PluginResult};
use sotf_host::speaker_config::{
    calculate_panning_gain, calculate_panning_gain_with_wraparound, get_speaker_config,
};

#[inline]
fn subharmonic_envelope_coeff(time_ms: f32, sample_rate: u32) -> f32 {
    let time_sec = (time_ms / 1000.0).max(1e-6);
    let sample_rate = (sample_rate as f32).max(1.0);
    1.0 - (-1.0_f32 / (time_sec * sample_rate)).exp()
}

#[inline]
fn inclusive_voice_bin_range(
    voice_min_hz: f32,
    voice_max_hz: f32,
    freq_per_bin: f32,
    spectrum_size: usize,
) -> (usize, usize) {
    if spectrum_size <= 1 || freq_per_bin <= 0.0 {
        return (0, 0);
    }

    // The real FFT Nyquist bin is valid memory, but it has no neighboring
    // positive-frequency partner and is not useful for voice-band centroiding.
    let last_voice_bin = spectrum_size.saturating_sub(2);
    let start = (voice_min_hz / freq_per_bin).max(0.0) as usize;
    let end = (voice_max_hz / freq_per_bin).max(0.0) as usize;
    let start = start.min(last_voice_bin);
    let end = end.min(last_voice_bin).max(start);
    (start, end)
}

impl UpmixerPlugin {
    /// Change speaker configuration at runtime
    pub(super) fn change_speaker_config(&mut self, config_id: &str) -> PluginResult<()> {
        let new_config = get_speaker_config(config_id)
            .ok_or_else(|| format!("Invalid speaker config: {}", config_id))?;

        // Layout identity affects routing and decorrelation even when width is unchanged.
        // Rebuild every channel-dependent buffer and clear queued audio atomically.
        self.core.speaker_config = new_config;
        self.core.num_output_channels = new_config.total_channels;

        // Reallocate output buffers
        // Note: time_out_channels are now real (f32) for RealFFT inverse output
        self.main_buffers.time_out_channels =
            vec![vec![0.0; self.core.fft_size]; self.core.num_output_channels];
        // Also reallocate HR output buffers which depend on channel count
        self.hr_buffers.hr_time_out_channels =
            vec![vec![0.0; self.fft.hr_fft_size]; self.core.num_output_channels];
        let accumulator_frames = self.core.fft_size * 4;
        debug_assert!(accumulator_frames.is_power_of_two());
        self.output.output_accumulator =
            vec![0.0; accumulator_frames * self.core.num_output_channels];
        self.output.output_accumulator_mask = accumulator_frames - 1;
        self.output.output_block = vec![0.0; self.core.fft_size * self.core.num_output_channels];
        self.hr_buffers.hr_output_accumulator =
            vec![0.0; accumulator_frames * self.core.num_output_channels];
        self.hr_buffers.hr_output_accumulator_mask = accumulator_frames - 1;
        // Re-allocate blended decorrelation filters for new channel count
        let spectrum_size = self.core.fft_size / 2 + 1;
        self.decorrelation.blended_decorrelation_filters =
            vec![vec![Complex::new(1.0, 0.0); spectrum_size]; self.core.num_output_channels];
        self.decorrelation.prev_decorrelation_strength = -1.0; // Force recompute

        self.recalculate_panning_gains();
        self.generate_per_channel_decorrelation_filters();
        self.reset();
        self.rebuild_cached_parameters();

        Ok(())
    }

    /// Recalculate panning gains for current speaker configuration
    pub(super) fn recalculate_panning_gains(&mut self) {
        const LEFT_AZIMUTH: f32 = 30.0;
        const RIGHT_AZIMUTH: f32 = -30.0;
        // Attenuation for rear speakers receiving wrapped-around sources
        // This maintains front-back separation while ensuring rear speakers get audio
        const WRAP_ATTENUATION: f32 = 0.7;

        self.panning.panning_gains_left.clear();
        self.panning.panning_gains_right.clear();

        for speaker in self.core.speaker_config.speakers.iter() {
            if speaker.is_lfe {
                self.panning.panning_gains_left.push(0.5);
                self.panning.panning_gains_right.push(0.5);
            } else if speaker.label == "C" {
                // Zero out center speaker in standard panning gains.
                // The upmixer handles the center speaker explicitly using the extracted
                // 'direct' signal and the center_spread parameter.
                self.panning.panning_gains_left.push(0.0);
                self.panning.panning_gains_right.push(0.0);
            } else {
                // Rear speakers (|azimuth| > 90°) need wrap-around panning
                // because they're more than 90° from both L/R source positions
                let is_rear = speaker.azimuth.abs() > 90.0;

                let (left_gain, right_gain) = if is_rear {
                    (
                        calculate_panning_gain_with_wraparound(
                            LEFT_AZIMUTH,
                            0.0,
                            speaker.azimuth,
                            speaker.elevation,
                            WRAP_ATTENUATION,
                        ),
                        calculate_panning_gain_with_wraparound(
                            RIGHT_AZIMUTH,
                            0.0,
                            speaker.azimuth,
                            speaker.elevation,
                            WRAP_ATTENUATION,
                        ),
                    )
                } else {
                    (
                        calculate_panning_gain(
                            LEFT_AZIMUTH,
                            0.0,
                            speaker.azimuth,
                            speaker.elevation,
                        ),
                        calculate_panning_gain(
                            RIGHT_AZIMUTH,
                            0.0,
                            speaker.azimuth,
                            speaker.elevation,
                        ),
                    )
                };

                self.panning.panning_gains_left.push(left_gain);
                self.panning.panning_gains_right.push(right_gain);
            }
        }

        // Cache per-speaker flags to avoid string/float comparisons in hot path
        self.panning.cached_is_front.clear();
        self.panning.cached_is_height.clear();
        self.panning.cached_is_center.clear();
        self.panning.cached_hr_active_channels.clear();
        for (i, speaker) in self.core.speaker_config.speakers.iter().enumerate() {
            let is_front = speaker.azimuth.abs() < 80.0;
            let is_height = speaker.elevation > 10.0;
            let is_center = speaker.label == "C";
            self.panning.cached_is_front.push(is_front);
            self.panning.cached_is_height.push(is_height);
            self.panning.cached_is_center.push(is_center);
            // HR path processes front, non-LFE, non-height channels
            if !speaker.is_lfe && !is_height && speaker.azimuth.abs() < 80.0 {
                self.panning.cached_hr_active_channels.push(i);
            }
        }

        // Normalize gains using energy-preserving normalization
        // For each source (left and right), normalize so sum of squared gains = 1
        let left_energy: f32 = self.panning.panning_gains_left.iter().map(|g| g * g).sum();
        let right_energy: f32 = self.panning.panning_gains_right.iter().map(|g| g * g).sum();

        if left_energy > 0.0 {
            let left_scale = 1.0 / left_energy.sqrt();
            for i in 0..self.core.num_output_channels {
                self.panning.panning_gains_left[i] *= left_scale;
            }
        }

        if right_energy > 0.0 {
            let right_scale = 1.0 / right_energy.sqrt();
            for i in 0..self.core.num_output_channels {
                self.panning.panning_gains_right[i] *= right_scale;
            }
        }
    }

    /// Precompute per-bin frequency weights for height mask (hf_ratio^0.7).
    ///
    /// These depend only on sample_rate, bandpass_hz, height_hf_cap_hz and fft_size,
    /// all of which are constant between initialize() calls (or parameter changes).
    pub(super) fn precompute_height_freq_weights(&mut self) {
        let spectrum_size = self.core.fft_size / 2 + 1;
        self.height.height_freq_weights.resize(spectrum_size, 0.0);

        let nyquist = self.core.sample_rate as f32 / 2.0;
        let hf_start = self.params.bandpass_hz.max(self.params.lfe_cutoff_hz);
        let hf_end = self.height.height_hf_cap_hz.min(nyquist);
        let freq_per_bin = self.core.sample_rate as f32 / self.core.fft_size as f32;

        for i in 0..spectrum_size {
            let freq = i as f32 * freq_per_bin;
            let hf_ratio = if freq <= hf_start {
                0.0
            } else if freq >= hf_end {
                1.0
            } else {
                (freq - hf_start) / (hf_end - hf_start)
            };
            self.height.height_freq_weights[i] = hf_ratio.powf(0.7);
        }
    }

    /// Update cached safety_cap linear values from safety_cap_db
    pub(super) fn update_safety_cap_cache(&mut self) {
        if self.safety.safety_cap_db >= 0.0 {
            self.safety.safety_cap_linear =
                math_audio_dsp::fast_math::fast_pow10(self.safety.safety_cap_db / 20.0);
            self.safety.safety_cap_min_scale =
                math_audio_dsp::fast_math::fast_pow10(-self.safety.safety_cap_db / 20.0);
        } else {
            self.safety.safety_cap_linear = 1.0;
            self.safety.safety_cap_min_scale = 0.0;
        }
    }

    /// Calculate ERB bands based on sample rate, FFT size, and frequency_resolution setting.
    ///
    /// Three modes are supported:
    /// - "erb": standard Glasberg & Moore (1990) ERB bands (~40-50 bands)
    ///   `erb_width = 24.7 * (4.37 * f/1000 + 1)` — approximately one critical band wide
    /// - "fine_erb": half-ERB width bands (~100 bands) for finer spatial resolution
    ///   Uses `0.5 * erb_width` so each band covers half a critical band
    /// - "per_bin": each FFT bin is its own band (freq_size bands)
    ///   Maximum resolution — each band is exactly 1 bin wide
    pub(super) fn calculate_erb_bands(&mut self) {
        self.steering.erb_bands.clear();
        let freq_per_bin = self.core.sample_rate as f32 / self.core.fft_size as f32;
        let freq_size = self.core.fft_size / 2; // Positive-frequency bins (Nyquist exclusive)

        match Self::canonical_frequency_resolution(&self.params.frequency_resolution) {
            "per_bin" => {
                // One band per FFT bin: band[k] starts at bin k
                for bin in 0..=freq_size {
                    self.steering.erb_bands.push(bin);
                }
            }
            "fine_erb" => {
                // Half-ERB width: ERB(f) * 0.5 per step
                // Glasberg & Moore (1990): ERB(f) = 24.7 * (4.37 * f/1000 + 1)
                let mut current_bin = 0;
                while current_bin < freq_size {
                    self.steering.erb_bands.push(current_bin);
                    let center_freq = current_bin as f32 * freq_per_bin;
                    let erb_width = 24.7 * (4.37 * center_freq / 1000.0 + 1.0);
                    // Half-bandwidth step keeps at least 1 bin minimum
                    let bins_width = (erb_width * 0.5 / freq_per_bin).max(1.0).round() as usize;
                    current_bin += bins_width;
                }
                // Ensure we cover up to Nyquist
                if *self.steering.erb_bands.last().unwrap() < freq_size {
                    self.steering.erb_bands.push(freq_size);
                }
            }
            // "erb" is the default; unknown values also fall through to standard ERB
            _ => {
                // Standard ERB: Glasberg & Moore (1990)
                // ERB(f) = 24.7 * (4.37 * f/1000 + 1)
                let mut current_bin = 0;
                while current_bin < freq_size {
                    self.steering.erb_bands.push(current_bin);
                    let center_freq = current_bin as f32 * freq_per_bin;
                    let erb_width = 24.7 * (4.37 * center_freq / 1000.0 + 1.0);
                    let bins_width = (erb_width / freq_per_bin).max(1.0).round() as usize;
                    current_bin += bins_width;
                }
                // Ensure we cover up to Nyquist
                if *self.steering.erb_bands.last().unwrap() < freq_size {
                    self.steering.erb_bands.push(freq_size);
                }
            }
        }

        // Resize per-band DOA state to match band count
        let num_bands = self.steering.erb_bands.len();
        self.steering.doa_angle.resize(num_bands, 0.0);
    }

    /// Recompute all cached bin indices that depend on sample_rate, fft_size,
    /// lfe_cutoff_hz, bandpass_hz, voice_freq_min_hz, or voice_freq_max_hz.
    ///
    /// Call this in `initialize()` and whenever any of those parameters change.
    pub(super) fn recache_bin_indices(&mut self) {
        if self.core.sample_rate == 0 || self.core.fft_size == 0 {
            return;
        }
        let freq_per_bin = self.core.sample_rate as f32 / self.core.fft_size as f32;
        self.cache.cached_freq_per_bin = freq_per_bin;

        self.cache.cached_lfe_cutoff_bin = ((self.params.lfe_cutoff_hz * self.core.fft_size as f32)
            / self.core.sample_rate as f32) as usize;
        self.cache.cached_bandpass_bin = ((self.params.bandpass_hz * self.core.fft_size as f32)
            / self.core.sample_rate as f32) as usize;

        let spectrum_size = self.core.fft_size / 2 + 1;
        let (voice_start_bin, voice_end_bin) = inclusive_voice_bin_range(
            self.dialogue.voice_freq_min_hz,
            self.dialogue.voice_freq_max_hz,
            freq_per_bin,
            spectrum_size,
        );
        self.cache.cached_voice_start_bin = voice_start_bin;
        self.cache.cached_voice_end_bin = voice_end_bin;

        self.recache_dialogue_weights();
    }

    /// Recompute the normalized dialogue sub-weights from the raw weight parameters.
    ///
    /// Call this whenever `dialogue_centroid_weight`, `dialogue_variance_weight`, or
    /// `dialogue_coherence_weight` changes.
    pub(super) fn recache_dialogue_weights(&mut self) {
        let w_sum = self.dialogue.dialogue_centroid_weight
            + self.dialogue.dialogue_variance_weight
            + self.dialogue.dialogue_coherence_weight;
        if w_sum > 1e-9 {
            self.dialogue.cached_dialogue_w_c = self.dialogue.dialogue_centroid_weight / w_sum;
            self.dialogue.cached_dialogue_w_v = self.dialogue.dialogue_variance_weight / w_sum;
            self.dialogue.cached_dialogue_w_coh = self.dialogue.dialogue_coherence_weight / w_sum;
        } else {
            self.dialogue.cached_dialogue_w_c = 0.333;
            self.dialogue.cached_dialogue_w_v = 0.333;
            self.dialogue.cached_dialogue_w_coh = 0.334;
        }
    }

    /// Recompute the cached sub-harmonic envelope coefficients from the current parameters.
    ///
    /// Call this in `initialize()` and whenever `subharmonic_freq_hz`,
    /// `subharmonic_attack_ms`, or `subharmonic_release_ms` changes.
    pub(super) fn recache_subharmonic_coeffs(&mut self) {
        if self.core.sample_rate == 0 {
            return;
        }
        let sr = self.core.sample_rate as f32;
        self.subharmonic.cached_subharmonic_phase_inc =
            2.0 * std::f32::consts::PI * self.subharmonic.subharmonic_freq_hz / sr;
        self.subharmonic.cached_subharmonic_attack_coeff = subharmonic_envelope_coeff(
            self.subharmonic.subharmonic_attack_ms,
            self.core.sample_rate,
        );
        self.subharmonic.cached_subharmonic_release_coeff = subharmonic_envelope_coeff(
            self.subharmonic.subharmonic_release_ms,
            self.core.sample_rate,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subharmonic_envelope_coeff_is_explicit_f32_and_finite() {
        let coeff = subharmonic_envelope_coeff(1.0, 384_000);
        let expected = 1.0 - (-1.0_f32 / (0.001 * 384_000.0)).exp();

        assert!(coeff.is_finite());
        assert!((coeff - expected).abs() < f32::EPSILON);
    }

    #[test]
    fn subharmonic_envelope_coeff_handles_zero_time_safely() {
        let coeff = subharmonic_envelope_coeff(0.0, 48_000);

        assert!(coeff.is_finite());
        assert!((0.0..=1.0).contains(&coeff));
    }

    #[test]
    fn voice_bin_range_excludes_nyquist_for_inclusive_iteration() {
        let spectrum_size = 1025;
        let freq_per_bin = 44_100.0 / 2048.0;
        let (_start, end) = inclusive_voice_bin_range(300.0, 50_000.0, freq_per_bin, spectrum_size);

        assert_eq!(end, spectrum_size - 2);
    }

    #[test]
    fn voice_bin_range_keeps_end_at_or_after_start() {
        let (start, end) = inclusive_voice_bin_range(3000.0, 300.0, 20.0, 128);

        assert_eq!(start, end);
    }
}
