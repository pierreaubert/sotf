// ============================================================================
// Setup and Configuration Management
// ============================================================================

use super::UpmixerPlugin;
use crate::plugin::{Plugin, PluginResult};
use crate::speaker_config::{
    calculate_panning_gain, calculate_panning_gain_with_wraparound, get_speaker_config,
};

impl UpmixerPlugin {
    /// Change speaker configuration at runtime
    pub(super) fn change_speaker_config(&mut self, config_id: &str) -> PluginResult<()> {
        let new_config = get_speaker_config(config_id)
            .ok_or_else(|| format!("Invalid speaker config: {}", config_id))?;

        if new_config.total_channels == self.num_output_channels {
            // Same channel count, just update config and panning gains
            self.speaker_config = new_config;
            self.recalculate_panning_gains();
            return Ok(());
        }

        // Different channel count - need to reallocate buffers
        self.speaker_config = new_config;
        self.num_output_channels = new_config.total_channels;

        // Reallocate output buffers
        // Note: time_out_channels are now real (f32) for RealFFT inverse output
        self.time_out_channels = vec![vec![0.0; self.fft_size]; self.num_output_channels];
        // Also reallocate HR output buffers which depend on channel count
        self.hr_time_out_channels = vec![vec![0.0; self.hr_fft_size]; self.num_output_channels];
        let accumulator_frames = self.fft_size * 4;
        debug_assert!(accumulator_frames.is_power_of_two());
        self.output_accumulator = vec![0.0; accumulator_frames * self.num_output_channels];
        self.output_accumulator_mask = accumulator_frames - 1;
        self.output_block = vec![0.0; self.fft_size * self.num_output_channels];
        self.blended_decorrelation_filters.clear();

        self.recalculate_panning_gains();
        self.reset();

        Ok(())
    }

    /// Recalculate panning gains for current speaker configuration
    pub(super) fn recalculate_panning_gains(&mut self) {
        const LEFT_AZIMUTH: f32 = 30.0;
        const RIGHT_AZIMUTH: f32 = -30.0;
        // Attenuation for rear speakers receiving wrapped-around sources
        // This maintains front-back separation while ensuring rear speakers get audio
        const WRAP_ATTENUATION: f32 = 0.7;

        self.panning_gains_left.clear();
        self.panning_gains_right.clear();

        for speaker in self.speaker_config.speakers.iter() {
            if speaker.is_lfe {
                self.panning_gains_left.push(0.5);
                self.panning_gains_right.push(0.5);
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

                self.panning_gains_left.push(left_gain);
                self.panning_gains_right.push(right_gain);
            }
        }

        // Cache per-speaker flags to avoid string/float comparisons in hot path
        self.cached_is_front.clear();
        self.cached_is_height.clear();
        self.cached_is_center.clear();
        self.cached_hr_active_channels.clear();
        for (i, speaker) in self.speaker_config.speakers.iter().enumerate() {
            let is_front = speaker.azimuth.abs() < 80.0;
            let is_height = speaker.elevation > 10.0;
            let is_center = speaker.label == "C";
            self.cached_is_front.push(is_front);
            self.cached_is_height.push(is_height);
            self.cached_is_center.push(is_center);
            // HR path processes front, non-LFE, non-height channels
            if !speaker.is_lfe && !is_height && speaker.azimuth.abs() < 80.0 {
                self.cached_hr_active_channels.push(i);
            }
        }

        // Normalize gains using energy-preserving normalization
        // For each source (left and right), normalize so sum of squared gains = 1
        let left_energy: f32 = self.panning_gains_left.iter().map(|g| g * g).sum();
        let right_energy: f32 = self.panning_gains_right.iter().map(|g| g * g).sum();

        if left_energy > 0.0 {
            let left_scale = 1.0 / left_energy.sqrt();
            for i in 0..self.num_output_channels {
                self.panning_gains_left[i] *= left_scale;
            }
        }

        if right_energy > 0.0 {
            let right_scale = 1.0 / right_energy.sqrt();
            for i in 0..self.num_output_channels {
                self.panning_gains_right[i] *= right_scale;
            }
        }
    }

    /// Precompute per-bin frequency weights for height mask (hf_ratio^0.7).
    ///
    /// These depend only on sample_rate, bandpass_hz, height_hf_cap_hz and fft_size,
    /// all of which are constant between initialize() calls (or parameter changes).
    pub(super) fn precompute_height_freq_weights(&mut self) {
        let spectrum_size = self.fft_size / 2 + 1;
        self.height_freq_weights.resize(spectrum_size, 0.0);

        let nyquist = self.sample_rate as f32 / 2.0;
        let hf_start = self.bandpass_hz.max(self.lfe_cutoff_hz);
        let hf_end = self.height_hf_cap_hz.min(nyquist);
        let freq_per_bin = self.sample_rate as f32 / self.fft_size as f32;

        for i in 0..spectrum_size {
            let freq = i as f32 * freq_per_bin;
            let hf_ratio = if freq <= hf_start {
                0.0
            } else if freq >= hf_end {
                1.0
            } else {
                (freq - hf_start) / (hf_end - hf_start)
            };
            self.height_freq_weights[i] = hf_ratio.powf(0.7);
        }
    }

    /// Update cached safety_cap linear values from safety_cap_db
    pub(super) fn update_safety_cap_cache(&mut self) {
        if self.safety_cap_db > 0.0 {
            self.safety_cap_linear = 10.0_f32.powf(self.safety_cap_db / 20.0);
            self.safety_cap_min_scale = 10.0_f32.powf(-self.safety_cap_db / 20.0);
        } else {
            self.safety_cap_linear = 1.0;
            self.safety_cap_min_scale = 0.0;
        }
    }

    /// Calculate ERB bands based on sample rate and FFT size
    pub(super) fn calculate_erb_bands(&mut self) {
        self.erb_bands.clear();
        let freq_per_bin = self.sample_rate as f32 / self.fft_size as f32;

        // Glasberg and Moore (1990) ERB scale
        // ERB(f) = 24.7 * (4.37 * f / 1000 + 1)
        // We want bands to be roughly 1 ERB wide

        let mut current_bin = 0;
        while current_bin < self.fft_size / 2 {
            self.erb_bands.push(current_bin);

            let center_freq = current_bin as f32 * freq_per_bin;
            let erb_width = 24.7 * (4.37 * center_freq / 1000.0 + 1.0);
            let bins_width = (erb_width / freq_per_bin).max(1.0).round() as usize;

            current_bin += bins_width;
        }
        // Ensure we cover the full spectrum up to Nyquist
        if *self.erb_bands.last().unwrap() < self.fft_size / 2 {
            self.erb_bands.push(self.fft_size / 2);
        }
    }
}
