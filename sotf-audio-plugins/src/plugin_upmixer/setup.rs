// ============================================================================
// Setup and Configuration Management
// ============================================================================

use super::UpmixerPlugin;
use crate::plugin::{Plugin, PluginResult};
use crate::speaker_config::{calculate_panning_gain, get_speaker_config};

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
        self.output_accumulator = vec![vec![0.0; self.fft_size * 3]; self.num_output_channels];
        self.output_block = vec![0.0; self.fft_size * self.num_output_channels];

        self.recalculate_panning_gains();
        self.reset();

        Ok(())
    }

    /// Recalculate panning gains for current speaker configuration
    pub(super) fn recalculate_panning_gains(&mut self) {
        const LEFT_AZIMUTH: f32 = 30.0;
        const RIGHT_AZIMUTH: f32 = -30.0;

        self.panning_gains_left.clear();
        self.panning_gains_right.clear();

        for speaker in self.speaker_config.speakers.iter() {
            if speaker.is_lfe {
                self.panning_gains_left.push(0.5);
                self.panning_gains_right.push(0.5);
            } else {
                let left_gain =
                    calculate_panning_gain(LEFT_AZIMUTH, 0.0, speaker.azimuth, speaker.elevation);
                let right_gain =
                    calculate_panning_gain(RIGHT_AZIMUTH, 0.0, speaker.azimuth, speaker.elevation);
                self.panning_gains_left.push(left_gain);
                self.panning_gains_right.push(right_gain);
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
