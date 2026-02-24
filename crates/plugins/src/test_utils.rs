//! Testing utilities for audio plugins.
use crate::plugin::{Plugin, ProcessContext};
use std::f32::consts::PI;

/// A simple stateful signal generator for testing.
/// Uses deterministic logic matching math-dsp.
pub struct SignalGen {
    sample_rate: f64,
    phase: f64,
    frequency: f64,
    amplitude: f32,
    gen_type: SignalType,
    // State for noise generators
    seed: u64,
    // State for pink noise (Voss-McCartney)
    pink_state: [f32; 7],
    // State for sweep
    sweep_t: f64,
    sweep_f_end: f64,
    sweep_duration: f64,
}

enum SignalType {
    Sine,
    WhiteNoise,
    PinkNoise,
    Impulse,
    Step,
    LogSweep,
}

impl SignalGen {
    pub fn new_sine(sample_rate: f64, frequency: f64, amplitude: f32) -> Self {
        Self::new(sample_rate, amplitude, SignalType::Sine, frequency)
    }

    pub fn new_white_noise(amplitude: f32) -> Self {
        Self::new(0.0, amplitude, SignalType::WhiteNoise, 0.0)
    }

    pub fn new_pink_noise(amplitude: f32) -> Self {
        Self::new(0.0, amplitude, SignalType::PinkNoise, 0.0)
    }

    pub fn new_impulse() -> Self {
        Self::new(0.0, 1.0, SignalType::Impulse, 0.0)
    }

    pub fn new_step() -> Self {
        Self::new(0.0, 1.0, SignalType::Step, 0.0)
    }

    pub fn new_log_sweep(sample_rate: f64, f_start: f64, f_end: f64, duration: f64, amplitude: f32) -> Self {
        let mut signal_gen = Self::new(sample_rate, amplitude, SignalType::LogSweep, f_start);
        signal_gen.sweep_f_end = f_end;
        signal_gen.sweep_duration = duration;
        signal_gen
    }

    fn new(sample_rate: f64, amplitude: f32, gen_type: SignalType, frequency: f64) -> Self {
        Self {
            sample_rate,
            phase: 0.0,
            frequency,
            amplitude,
            gen_type,
            seed: 1234567890,
            pink_state: [0.0; 7],
            sweep_t: 0.0,
            sweep_f_end: 0.0,
            sweep_duration: 0.0,
        }
    }

    /// Clip a sample to prevent overflow in PCM conversion
    #[inline]
    fn clip(x: f32) -> f32 {
        x.clamp(-0.999_999, 0.999_999)
    }

    fn next_white(&mut self) -> f32 {
        // Simple LCG random number generator for deterministic output
        // LCG constants from Numerical Recipes
        self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let random_u32 = (self.seed & 0xFFFFFFFF) as u32;
        (random_u32 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    pub fn generate(&mut self, num_samples: usize) -> Vec<f32> {
        let mut buffer = Vec::with_capacity(num_samples);
        for _ in 0..num_samples {
            let sample = match self.gen_type {
                SignalType::Sine => {
                    let s = (self.phase * 2.0 * PI as f64).sin() as f32 * self.amplitude;
                    self.phase = (self.phase + self.frequency / self.sample_rate) % 1.0;
                    Self::clip(s)
                }
                SignalType::WhiteNoise => {
                    Self::clip(self.next_white() * self.amplitude)
                }
                SignalType::PinkNoise => {
                    let white = self.next_white();
                    let b = &mut self.pink_state;
                    b[0] = 0.99886 * b[0] + white * 0.0555179;
                    b[1] = 0.99332 * b[1] + white * 0.0750759;
                    b[2] = 0.96900 * b[2] + white * 0.153_852;
                    b[3] = 0.86650 * b[3] + white * 0.3104856;
                    b[4] = 0.55000 * b[4] + white * 0.5329522;
                    b[5] = -0.7616 * b[5] - white * 0.0168980;

                    let pink = b[0] + b[1] + b[2] + b[3] + b[4] + b[5] + b[6] + white * 0.5362;
                    b[6] = white * 0.115926;

                    const PINK_NORM: f32 = 1.0 / 1.744;
                    Self::clip(self.amplitude * pink * PINK_NORM)
                }
                SignalType::Impulse => {
                    if self.phase == 0.0 {
                        self.phase = 1.0;
                        Self::clip(self.amplitude)
                    } else {
                        0.0
                    }
                }
                SignalType::Step => Self::clip(self.amplitude),
                SignalType::LogSweep => {
                    if self.sweep_t >= self.sweep_duration {
                        0.0
                    } else {
                        let k = (self.sweep_f_end / self.frequency).ln() / self.sweep_duration;
                        let coefficient = 2.0 * PI as f64 * self.frequency / k;
                        let phase = coefficient * ((k * self.sweep_t).exp() - 1.0);
                        let s = (phase.sin() as f32) * self.amplitude;
                        self.sweep_t += 1.0 / self.sample_rate;
                        Self::clip(s)
                    }
                }
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
