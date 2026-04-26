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
        Self {
            foreground: Pbfdaf::new(block_size, echo_tail_samples, fg_mu, 1e-6),
            background: Pbfdaf::new(block_size, echo_tail_samples, bg_mu, 1e-6),
            power_fg: 1.0,
            power_bg: 1.0,
            power_alpha: 0.95,
            transfer_count: 0,
            transfer_threshold: 5,
            block_size,
            output_buf: vec![0.0; block_size],
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

        // Run both filters
        let error_fg = self.foreground.process(mic, reference);
        let pow_fg: f32 = error_fg.iter().map(|x| x * x).sum::<f32>() / b as f32;
        // Copy foreground output before background borrows self
        self.output_buf[..b].copy_from_slice(error_fg);

        let error_bg = self.background.process(mic, reference);
        let pow_bg: f32 = error_bg.iter().map(|x| x * x).sum::<f32>() / b as f32;

        // Smooth power estimates
        self.power_fg = self.power_alpha * self.power_fg + (1.0 - self.power_alpha) * pow_fg;
        self.power_bg = self.power_alpha * self.power_bg + (1.0 - self.power_alpha) * pow_bg;

        // Check if background is consistently better
        if self.power_bg < self.power_fg * 0.95 {
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
        self.foreground.copy_adaptive_state_from(&self.background);
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
    }

    /// Get the processing block size.
    pub fn block_size(&self) -> usize {
        self.block_size
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
}
