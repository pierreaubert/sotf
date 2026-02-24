//! Testing utilities for audio plugins.
use crate::plugin::{Plugin, ProcessContext};
use std::f32::consts::PI;

/// A simple signal generator for testing.
pub struct SignalGenerator {
    sample_rate: f64,
    phase: f64,
    frequency: f64,
    amplitude: f32,
    gen_type: SignalType,
}

enum SignalType {
    Sine,
    WhiteNoise,
    Impulse,
    Step,
}

impl SignalGenerator {
    pub fn new_sine(sample_rate: f64, frequency: f64, amplitude: f32) -> Self {
        Self {
            sample_rate,
            phase: 0.0,
            frequency,
            amplitude,
            gen_type: SignalType::Sine,
        }
    }

    pub fn new_white_noise(amplitude: f32) -> Self {
        Self {
            sample_rate: 0.0, // Not needed for noise
            phase: 0.0,
            frequency: 0.0,
            amplitude,
            gen_type: SignalType::WhiteNoise,
        }
    }

    pub fn new_impulse() -> Self {
        Self {
            sample_rate: 0.0,
            phase: 0.0,
            frequency: 0.0,
            amplitude: 1.0,
            gen_type: SignalType::Impulse,
        }
    }

    pub fn new_step() -> Self {
        Self {
            sample_rate: 0.0,
            phase: 0.0,
            frequency: 0.0,
            amplitude: 1.0,
            gen_type: SignalType::Step,
        }
    }

    pub fn generate(&mut self, num_samples: usize) -> Vec<f32> {
        let mut buffer = Vec::with_capacity(num_samples);
        for _ in 0..num_samples {
            let sample = match self.gen_type {
                SignalType::Sine => {
                    let s = (self.phase * 2.0 * PI as f64).sin() as f32 * self.amplitude;
                    self.phase = (self.phase + self.frequency / self.sample_rate) % 1.0;
                    s
                }
                SignalType::WhiteNoise => {
                    (rand::random::<f32>() * 2.0 - 1.0) * self.amplitude
                }
                SignalType::Impulse => {
                    if self.phase == 0.0 {
                        self.phase = 1.0;
                        self.amplitude
                    } else {
                        0.0
                    }
                }
                SignalType::Step => self.amplitude,
            };
            buffer.push(sample);
        }
        buffer
    }
}

/// Utilities for comparing audio buffers.
pub struct BufferComparison;

impl BufferComparison {
    pub fn compare_rms(buf1: &[f32], buf2: &[f32], threshold: f32) -> bool {
        if buf1.len() != buf2.len() {
            return false;
        }
        if buf1.is_empty() {
            return true;
        }

        let mut sum_sq_diff = 0.0;
        for (s1, s2) in buf1.iter().zip(buf2.iter()) {
            let diff = s1 - s2;
            sum_sq_diff += diff * diff;
        }
        let rms_diff = (sum_sq_diff / buf1.len() as f32).sqrt();
        rms_diff < threshold
    }

    pub fn compare_bit_accurate(buf1: &[f32], buf2: &[f32]) -> bool {
        buf1 == buf2
    }
}

/// A harness for testing plugins with varied buffer sizes.
pub fn test_varied_buffer_sizes<P: Plugin>(
    plugin: &mut P,
    sample_rate: f64,
    input: &[f32],
    expected_output: &[f32],
) {
    let buffer_sizes = [1, 16, 32, 64, 128, 256, 512, 1024, 13, 127]; // Includes non-power-of-two
    let num_channels_in = plugin.input_channels();
    let num_channels_out = plugin.output_channels();
    let total_frames = input.len() / num_channels_in;

    for &block_size in &buffer_sizes {
        let mut output = vec![0.0; expected_output.len()];
        let mut frames_processed = 0;

        while frames_processed < total_frames {
            let num_frames = (block_size).min(total_frames - frames_processed);
            let ctx = ProcessContext {
                sample_rate: sample_rate as u32,
                num_frames,
            };

            let in_slice = &input[frames_processed * num_channels_in..(frames_processed + num_frames) * num_channels_in];
            let out_slice = &mut output[frames_processed * num_channels_out..(frames_processed + num_frames) * num_channels_out];

            plugin.process(in_slice, out_slice, &ctx).unwrap();
            frames_processed += num_frames;
        }

        assert!(
            BufferComparison::compare_rms(&output, expected_output, 1e-5),
            "Failed for block size {}",
            block_size
        );
    }
}
