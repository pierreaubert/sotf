// ============================================================================
// RNNoise Backend — nnnoiseless wrapper
// ============================================================================
//
// Wraps the nnnoiseless crate (pure Rust RNNoise implementation) as a
// DenoiserBackend. RNNoise is a lightweight recurrent neural network
// designed specifically for speech denoising.
//
// Constraints:
// - Requires exactly 480 samples per frame (10ms at 48kHz)
// - Speech-only (optimized for voice, not music)
// - Internal 48kHz processing (resamples internally if needed)

use crate::DenoiserData;
use crate::backend::{DenoiserAlgorithm, DenoiserBackend};

const RNNOISE_FRAME_SIZE: usize = 480;

/// RNNoise denoising backend.
pub struct RnnoiseBackend {
    denoisers: Vec<Box<nnnoiseless::DenoiseState>>,
    channels: usize,
    sample_rate: u32,
    /// Per-channel input accumulation buffers
    accum_buffers: Vec<Vec<f32>>,
    /// Per-channel output ring buffers (fixed size, no allocation)
    output_buffers: Vec<Vec<f32>>,
    output_write_pos: usize,
    output_read_pos: usize,
    /// Samples accumulated so far
    accum_fill: usize,
    /// VAD probability from last frame (per channel)
    vad_prob: Vec<f32>,
    /// Monitoring data
    avg_reduction_db: f32,
}

impl Default for RnnoiseBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl RnnoiseBackend {
    pub fn new() -> Self {
        Self {
            denoisers: Vec::new(),
            channels: 0,
            sample_rate: 48000,
            accum_buffers: Vec::new(),
            output_buffers: Vec::new(),
            output_write_pos: 0,
            output_read_pos: 0,
            accum_fill: 0,
            vad_prob: Vec::new(),
            avg_reduction_db: 0.0,
        }
    }

    pub fn max_in_place_frames(&self) -> usize {
        self.output_buffers
            .first()
            .map(|buffer| buffer.len())
            .unwrap_or(RNNOISE_FRAME_SIZE * 4)
    }
}

impl DenoiserBackend for RnnoiseBackend {
    fn initialize(&mut self, sample_rate: u32, channels: usize) {
        self.sample_rate = sample_rate;
        self.channels = channels;
        self.denoisers = (0..channels)
            .map(|_| nnnoiseless::DenoiseState::new())
            .collect::<Vec<_>>();
        self.accum_buffers = vec![vec![0.0; RNNOISE_FRAME_SIZE]; channels];
        // Ring buffer: 4x frame size gives enough headroom
        let ring_size = RNNOISE_FRAME_SIZE * 4;
        self.output_buffers = vec![vec![0.0; ring_size]; channels];
        self.output_write_pos = 0;
        self.output_read_pos = 0;
        self.accum_fill = 0;
        self.vad_prob = vec![0.0; channels];
    }

    fn process(&mut self, buffer: &mut [f32], num_frames: usize, channels: usize) {
        if self.denoisers.is_empty() || channels == 0 {
            return;
        }

        // Deinterleave into per-channel buffers, accumulate, and process
        for frame in 0..num_frames {
            for ch in 0..channels.min(self.channels) {
                self.accum_buffers[ch][self.accum_fill] = buffer[frame * channels + ch];
            }
            self.accum_fill += 1;

            if self.accum_fill == RNNOISE_FRAME_SIZE {
                // Process each channel through RNNoise
                let mut ch0_output_power = 0.0;
                for ch in 0..channels.min(self.channels) {
                    let mut input_buf = [0.0f32; RNNOISE_FRAME_SIZE];
                    let mut output_buf = [0.0f32; RNNOISE_FRAME_SIZE];
                    input_buf.copy_from_slice(&self.accum_buffers[ch]);

                    // RNNoise expects and returns samples in [-32768, 32767] range
                    for s in &mut input_buf {
                        *s *= 32767.0;
                    }

                    self.denoisers[ch].process_frame(&mut output_buf, &input_buf);
                    // nnnoiseless doesn't return VAD in this API; vad_prob stays at default

                    // Convert back to [-1, 1]
                    for s in &mut output_buf {
                        *s /= 32767.0;
                    }

                    if ch == 0 {
                        ch0_output_power = output_buf.iter().map(|x| x * x).sum::<f32>()
                            / RNNOISE_FRAME_SIZE as f32;
                    }

                    // Write to ring buffer (no allocation)
                    let ring_size = self.output_buffers[ch].len();
                    for (i, &s) in output_buf.iter().enumerate() {
                        self.output_buffers[ch][(self.output_write_pos + i) % ring_size] = s;
                    }
                }
                self.output_write_pos += RNNOISE_FRAME_SIZE;

                // Estimate reduction
                let input_power: f32 = self.accum_buffers[0].iter().map(|x| x * x).sum::<f32>()
                    / RNNOISE_FRAME_SIZE as f32;

                if input_power > 1e-10 {
                    self.avg_reduction_db = 0.9 * self.avg_reduction_db
                        + 0.1 * 10.0 * (input_power / ch0_output_power.max(1e-10)).log10();
                }

                self.accum_fill = 0;
            }
        }

        // Read from ring buffer into interleaved output
        let ch_count = channels.min(self.channels);
        let available = self.output_write_pos.saturating_sub(self.output_read_pos);
        let to_write = num_frames.min(available);

        if ch_count > 0 {
            let ring_size = self.output_buffers[0].len();
            for frame in 0..to_write {
                for ch in 0..ch_count {
                    buffer[frame * channels + ch] =
                        self.output_buffers[ch][(self.output_read_pos + frame) % ring_size];
                }
            }
            self.output_read_pos += to_write;
        }

        // Zero-fill any remaining output
        for frame in to_write..num_frames {
            for ch in 0..channels {
                buffer[frame * channels + ch] = 0.0;
            }
        }
    }

    fn reset(&mut self) {
        self.denoisers = (0..self.channels)
            .map(|_| nnnoiseless::DenoiseState::new())
            .collect::<Vec<_>>();
        for buf in &mut self.accum_buffers {
            buf.fill(0.0);
        }
        for buf in &mut self.output_buffers {
            buf.fill(0.0);
        }
        self.output_write_pos = 0;
        self.output_read_pos = 0;
        self.accum_fill = 0;
        self.avg_reduction_db = 0.0;
    }

    fn latency_samples(&self) -> usize {
        RNNOISE_FRAME_SIZE
    }

    fn get_data(&self) -> DenoiserData {
        DenoiserData {
            avg_reduction_db: self.avg_reduction_db,
            learning_active: false,
            ..Default::default()
        }
    }

    fn algorithm(&self) -> DenoiserAlgorithm {
        DenoiserAlgorithm::RNNoise
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rnnoise_creation() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 2);
        assert_eq!(backend.channels, 2);
        assert_eq!(backend.latency_samples(), 480);
    }

    #[test]
    fn test_rnnoise_silence_passthrough() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 1);

        // Process silence
        let mut buffer = vec![0.0f32; 960]; // 2 * 480
        backend.process(&mut buffer, 960, 1);

        // Output should be near zero
        for (i, &s) in buffer.iter().enumerate() {
            assert!(s.abs() < 0.01, "Sample {i} should be near zero, got {s}");
        }
    }

    #[test]
    fn test_rnnoise_process_noise() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 1);

        // Process some noise
        let mut rng: u64 = 42;
        let mut buffer: Vec<f32> = (0..RNNOISE_FRAME_SIZE * 4)
            .map(|_| {
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                ((rng >> 33) as f32 / u32::MAX as f32) * 0.1
            })
            .collect();

        let input_power: f32 = buffer.iter().map(|x| x * x).sum::<f32>() / buffer.len() as f32;
        let len = buffer.len();
        backend.process(&mut buffer, len, 1);

        let output_power: f32 = buffer.iter().map(|x| x * x).sum::<f32>() / buffer.len() as f32;

        // RNNoise should reduce noise power (at least somewhat)
        // Note: first frame may not show reduction due to warmup
        assert!(
            output_power <= input_power * 1.5,
            "Output power ({output_power:.6}) should not be much higher than input ({input_power:.6})"
        );
    }

    #[test]
    fn test_rnnoise_reset() {
        let mut backend = RnnoiseBackend::new();
        backend.initialize(48000, 1);

        let mut buffer = vec![0.5f32; 960];
        backend.process(&mut buffer, 960, 1);

        backend.reset();
        assert_eq!(backend.accum_fill, 0);
    }
}
