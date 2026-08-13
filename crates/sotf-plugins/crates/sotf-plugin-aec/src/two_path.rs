// ============================================================================
// Two-Path AEC — Foreground/Background Filter Management
// ============================================================================
//
// Maintains two parallel adaptive filters:
// - Background: always adapts aggressively (explores)
// - Foreground: used for output, cautiously updated (exploits)
//
// The background filter is periodically transferred to the foreground
// when it demonstrates consistently better performance.

use crate::pbfdaf::Pbfdaf;

/// Two-path acoustic echo canceller.
#[derive(Debug)]
pub struct TwoPathAec {
    foreground: Pbfdaf,
    background: Pbfdaf,
    /// Smoothed foreground residual power
    power_fg: f32,
    /// Smoothed background residual power
    power_bg: f32,
    /// Power smoothing factor
    power_alpha: f32,
    /// Number of consecutive frames where background is better
    transfer_count: usize,
    /// Threshold for transfer (number of consecutive better frames)
    transfer_threshold: usize,
    block_size: usize,
    /// Pre-allocated output buffer to avoid returning borrows
    output_buf: Vec<f32>,
    double_talk: bool,
    double_talk_blocks: usize,
    reference_fft_count: u64,
    background_adaptation_scale: f32,
}

impl TwoPathAec {
    /// Create a new two-path AEC.
    ///
    /// # Arguments
    /// * `block_size` - Processing block size
    /// * `echo_tail_samples` - Echo path length in samples
    /// * `fg_mu` - Foreground step size (conservative, e.g. 0.3)
    /// * `bg_mu` - Background step size (aggressive, e.g. 0.7)
    pub fn new(block_size: usize, echo_tail_samples: usize, fg_mu: f32, bg_mu: f32) -> Self {
        Self::new_with_sample_rate(block_size, echo_tail_samples, fg_mu, bg_mu, 48_000)
    }

    pub fn new_with_sample_rate(
        block_size: usize,
        echo_tail_samples: usize,
        fg_mu: f32,
        bg_mu: f32,
        sample_rate: u32,
    ) -> Self {
        let block_seconds = block_size as f32 / sample_rate.max(1) as f32;
        let power_alpha = (-block_seconds / 0.100).exp();
        Self {
            foreground: Pbfdaf::new(block_size, echo_tail_samples, fg_mu, 1e-6),
            background: Pbfdaf::new(block_size, echo_tail_samples, bg_mu, 1e-6),
            power_fg: 1.0,
            power_bg: 1.0,
            power_alpha,
            transfer_count: 0,
            // Require 25 consecutive blocks (≈ 133 ms at 48 kHz / 256) where
            // background has at least 1 dB advantage (power ratio < 0.794).
            // The old value of 5 blocks (≈ 27 ms) triggered rapid foreground /
            // background ping-pong on non-stationary signals.
            transfer_threshold: (0.133 / block_seconds).ceil().max(1.0) as usize,
            block_size,
            output_buf: vec![0.0; block_size],
            double_talk: false,
            double_talk_blocks: 0,
            reference_fft_count: 0,
            background_adaptation_scale: 1.0,
        }
    }

    /// Process one block through both filters.
    ///
    /// # Arguments
    /// * `mic` - Microphone input
    /// * `reference` - Far-end reference signal
    ///
    /// # Returns
    /// Error signal from foreground filter (echo-cancelled audio)
    pub fn process(&mut self, mic: &[f32], reference: &[f32]) -> &[f32] {
        let b = self.block_size;

        // The foreground is deliberately stable: only the background adapts.
        // Analyze the shared reference/FDL once in the background, then use it
        // to evaluate the foreground output without a second reference FFT.
        self.background_adaptation_scale = if self.double_talk { 0.05 } else { 1.0 };
        let error_bg = self.background.process_with_adaptation(
            mic,
            reference,
            self.background_adaptation_scale,
        );
        self.reference_fft_count += 1;
        let pow_bg: f32 = error_bg.iter().map(|x| x * x).sum::<f32>() / b as f32;
        let error_fg = self
            .foreground
            .process_with_shared_reference(mic, &self.background, 0.0);
        let pow_fg: f32 = error_fg.iter().map(|x| x * x).sum::<f32>() / b as f32;
        self.output_buf[..b].copy_from_slice(error_fg);

        // Smooth power estimates
        self.power_fg = self.power_alpha * self.power_fg + (1.0 - self.power_alpha) * pow_fg;
        self.power_bg = self.power_alpha * self.power_bg + (1.0 - self.power_alpha) * pow_bg;

        let echo_power = self
            .background
            .last_echo_estimate_freq()
            .iter()
            .map(|x| x.norm_sqr())
            .sum::<f32>()
            / (self.background.last_echo_estimate_freq().len()
                * self.background.last_echo_estimate_freq().len()) as f32;
        let mic_power = mic.iter().map(|x| x * x).sum::<f32>() / b as f32;
        self.double_talk =
            echo_power > 1e-8 && pow_fg > echo_power * 0.20 && pow_fg > mic_power * 0.08;
        if self.double_talk {
            self.double_talk_blocks += 1;
        }

        // Check if background is consistently better by at least 1 dB
        // (power ratio 10^(-1/10) ≈ 0.794).  The old 5% margin (0.95) was too
        // loose and triggered transfers on routine fluctuations.
        if self.power_bg < self.power_fg * 0.794 {
            self.transfer_count += 1;
        } else {
            self.transfer_count = 0;
        }

        // Transfer background to foreground if sustained improvement
        if self.transfer_count >= self.transfer_threshold {
            self.transfer_bg_to_fg();
            self.transfer_count = 0;
        }

        &self.output_buf[..b]
    }

    /// Transfer background filter weights to foreground.
    fn transfer_bg_to_fg(&mut self) {
        self.foreground.copy_weights_from(&self.background);
        self.power_fg = self.power_bg;
    }

    /// Access foreground filter's last error spectrum (for post-filter).
    pub fn last_error_freq(&self) -> &[rustfft::num_complex::Complex<f32>] {
        self.foreground.last_error_freq()
    }

    /// Access foreground filter's last echo estimate spectrum (for post-filter).
    pub fn last_echo_estimate_freq(&self) -> &[rustfft::num_complex::Complex<f32>] {
        self.foreground.last_echo_estimate_freq()
    }

    /// Reset both filters.
    pub fn reset(&mut self) {
        self.foreground.reset();
        self.background.reset();
        self.power_fg = 1.0;
        self.power_bg = 1.0;
        self.transfer_count = 0;
        self.double_talk = false;
        self.double_talk_blocks = 0;
        self.background_adaptation_scale = 1.0;
    }

    /// Get the processing block size.
    #[cfg(test)]
    pub fn block_size(&self) -> usize {
        self.block_size
    }

    /// Get the transfer threshold (minimum consecutive better-background frames
    /// required before promoting background → foreground).
    #[cfg(test)]
    pub fn transfer_threshold(&self) -> usize {
        self.transfer_threshold
    }

    /// Get the current consecutive-better-background counter.
    #[cfg(test)]
    pub fn transfer_count(&self) -> usize {
        self.transfer_count
    }

    /// Sum of squared foreground filter weights (useful for testing leakage decay).
    #[cfg(test)]
    pub fn foreground_weight_energy(&self) -> f32 {
        self.foreground.adaptive_state_energy()
    }

    #[cfg(test)]
    pub fn background_weight_energy(&self) -> f32 {
        self.background.adaptive_state_energy()
    }

    #[cfg(test)]
    pub fn reference_fft_count(&self) -> u64 {
        self.reference_fft_count
    }

    #[cfg(test)]
    pub fn double_talk_blocks(&self) -> usize {
        self.double_talk_blocks
    }

    #[cfg(test)]
    pub fn power_alpha(&self) -> f32 {
        self.power_alpha
    }

    #[cfg(test)]
    pub fn background_adaptation_scale(&self) -> f32 {
        self.background_adaptation_scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_two_path_creation() {
        let aec = TwoPathAec::new(256, 4800, 0.3, 0.7);
        assert_eq!(aec.block_size(), 256);
    }

    #[test]
    fn test_two_path_reset() {
        let mut aec = TwoPathAec::new(256, 2400, 0.3, 0.7);
        let mic = vec![0.1; 256];
        let reference = vec![0.2; 256];
        let _ = aec.process(&mic, &reference);

        aec.reset();
        assert_eq!(aec.transfer_count, 0);
    }

    #[test]
    fn test_two_path_convergence() {
        let block_size = 256;
        let delay = 80;
        let mut aec = TwoPathAec::new(block_size, 512, 0.3, 0.7);

        let mut ref_history = Vec::new();
        let num_blocks = 100;

        for block_idx in 0..num_blocks {
            let reference: Vec<f32> = (0..block_size)
                .map(|i| {
                    let t = (block_idx * block_size + i) as f32;
                    (t * 0.15).sin() * 0.4
                })
                .collect();
            ref_history.extend_from_slice(&reference);

            let mic: Vec<f32> = (0..block_size)
                .map(|i| {
                    let gi = block_idx * block_size + i;
                    if gi >= delay && gi - delay < ref_history.len() {
                        ref_history[gi - delay] * 0.5
                    } else {
                        0.0
                    }
                })
                .collect();

            let error = aec.process(&mic, &reference);
            assert_eq!(error.len(), block_size);
        }
    }

    #[test]
    fn test_transfer_copies_background_state_to_foreground() {
        let block_size = 256;
        let mut aec = TwoPathAec::new(block_size, 512, 0.3, 0.7);

        for block_idx in 0..8 {
            let reference: Vec<f32> = (0..block_size)
                .map(|i| {
                    let t = (block_idx * block_size + i) as f32;
                    (t * 0.13).sin() * 0.4
                })
                .collect();
            let mic: Vec<f32> = reference.iter().map(|sample| sample * 0.5).collect();
            let _ = aec.process(&mic, &reference);
        }

        let background_energy = aec.background.adaptive_state_energy();
        assert!(background_energy > 0.0);

        aec.foreground.reset();
        assert_eq!(aec.foreground.adaptive_state_energy(), 0.0);

        aec.transfer_bg_to_fg();
        let foreground_energy = aec.foreground.adaptive_state_energy();
        assert!(
            (foreground_energy - background_energy).abs() < background_energy * 1e-5,
            "foreground should receive background state: foreground={foreground_energy}, background={background_energy}"
        );
    }

    #[test]
    fn reference_fft_and_fdl_analysis_is_shared_by_both_paths() {
        let mut aec = TwoPathAec::new_with_sample_rate(32, 128, 0.3, 0.7, 48_000);
        let mic = vec![0.2; 32];
        let reference = vec![0.3; 32];
        let before = aec.reference_fft_count();
        let _ = aec.process(&mic, &reference);
        assert_eq!(aec.reference_fft_count() - before, 1);
    }

    #[test]
    fn double_talk_freezes_foreground_and_recovers_afterwards() {
        let block_size = 64;
        let mut aec = TwoPathAec::new_with_sample_rate(block_size, 256, 0.3, 0.7, 48_000);
        for block in 0..180 {
            let reference: Vec<f32> = (0..block_size)
                .map(|i| (((block * block_size + i) as f32) * 0.131).sin() * 0.5)
                .collect();
            let mic: Vec<f32> = reference.iter().map(|x| x * 0.45).collect();
            let _ = aec.process(&mic, &reference);
        }
        let before = aec.foreground_weight_energy();
        for block in 0..80 {
            let reference: Vec<f32> = (0..block_size)
                .map(|i| (((block * block_size + i) as f32) * 0.131).sin() * 0.5)
                .collect();
            let mic: Vec<f32> = reference
                .iter()
                .enumerate()
                .map(|(i, echo)| {
                    echo * 0.45 + (((block * block_size + i) as f32) * 0.077).sin() * 0.5
                })
                .collect();
            let _ = aec.process(&mic, &reference);
        }
        let after = aec.foreground_weight_energy();
        assert!(
            aec.double_talk_blocks() > 0,
            "fixture must exercise the adaptation gate"
        );
        assert_eq!(aec.background_adaptation_scale(), 0.05);
        assert!((after - before).abs() < before.max(1e-6) * 0.08);
    }

    #[test]
    fn abrupt_echo_path_change_recovers_after_double_talk() {
        let block_size = 32;
        let total_blocks = 700;
        let mut aec = TwoPathAec::new_with_sample_rate(block_size, 160, 0.3, 0.7, 48_000);
        let reference: Vec<f32> = (0..total_blocks * block_size)
            .map(|n| {
                let n = n as u32;
                (((n.wrapping_mul(1_103_515_245).wrapping_add(12_345) >> 8) & 0xffff) as f32
                    / 32_768.0)
                    - 1.0
            })
            .collect();
        let mut late_mic = 0.0;
        let mut late_error = 0.0;
        for block in 0..total_blocks {
            let delay = if block < 250 { 13 } else { 71 };
            let mut mic = vec![0.0; block_size];
            for (i, sample) in mic.iter_mut().enumerate() {
                let n = block * block_size + i;
                if n >= delay {
                    *sample = reference[n - delay] * 0.5;
                }
                if (250..310).contains(&block) {
                    *sample += (n as f32 * 0.119).sin() * 0.35;
                }
            }
            let start = block * block_size;
            let error = aec.process(&mic, &reference[start..start + block_size]);
            if block >= 600 {
                late_mic += mic.iter().map(|x| x * x).sum::<f32>();
                late_error += error.iter().map(|x| x * x).sum::<f32>();
            }
        }
        let erle = 10.0 * (late_mic / late_error.max(1e-20)).log10();
        assert!(erle > 5.0, "path-change recovery ERLE was {erle:.1} dB");
    }

    #[test]
    fn time_constants_are_sample_rate_derived() {
        let low = TwoPathAec::new_with_sample_rate(256, 512, 0.3, 0.7, 48_000);
        let high = TwoPathAec::new_with_sample_rate(256, 512, 0.3, 0.7, 96_000);
        assert!(high.power_alpha() > low.power_alpha());
        assert!(high.transfer_threshold() > low.transfer_threshold());
    }
}
