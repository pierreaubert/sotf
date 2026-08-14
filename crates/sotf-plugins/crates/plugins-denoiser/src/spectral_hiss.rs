use math_audio_dsp::stft::{RealFftProcessor, generate_hann_window};

pub const SPECTRAL_HISS_FFT_SIZE: usize = 1024;
const HOP_SIZE: usize = SPECTRAL_HISS_FFT_SIZE / 4;
const NUM_BINS: usize = SPECTRAL_HISS_FFT_SIZE / 2 + 1;
const MIN_HISTORY_SLOTS: usize = 8;
const MIN_HISTORY_SECONDS: f32 = 0.512;
const GAIN_ATTACK_SECONDS: f32 = 0.015;
const GAIN_RELEASE_SECONDS: f32 = 0.050;
const BYPASS_SECONDS: f32 = 0.005;

/// Higher-latency stationary-hiss reducer using WOLA and a bounded
/// minimum-statistics noise estimate.
struct BypassFade {
    mix: f32,
    target: f32,
    step: f32,
}

pub struct SpectralHissReducer {
    channels: usize,
    sample_rate: u32,
    cutoff_hz: f32,
    threshold_linear: f32,
    strength: f32,
    bypass: BypassFade,
    hops_per_slot: usize,
    gain_attack: f32,
    gain_release: f32,
    fft: Vec<RealFftProcessor>,
    window: Vec<f32>,
    input: Vec<Vec<f32>>,
    input_write: usize,
    input_fill: usize,
    output: Vec<f32>,
    output_mask: usize,
    output_read: usize,
    output_write: usize,
    output_fill: usize,
    latency_fill: usize,
    dry_delay: Vec<f32>,
    dry_pos: usize,
    power: Vec<Vec<f32>>,
    smoothed_power: Vec<Vec<f32>>,
    smoothed_gain: Vec<Vec<f32>>,
    high_band_noise: Vec<f32>,
    current_min: Vec<Vec<f32>>,
    minimum_history: Vec<Vec<f32>>,
    history_slot: usize,
    hops_in_slot: usize,
}

impl SpectralHissReducer {
    pub fn new(channels: usize) -> Self {
        let output_frames = (SPECTRAL_HISS_FFT_SIZE * 4).next_power_of_two();
        Self {
            channels,
            sample_rate: 48_000,
            cutoff_hz: 4_000.0,
            threshold_linear: 10.0_f32.powf(-30.0 / 20.0),
            strength: 0.5,
            bypass: BypassFade {
                mix: 1.0,
                target: 1.0,
                step: 1.0,
            },
            hops_per_slot: 12,
            gain_attack: 0.35,
            gain_release: 0.9,
            fft: (0..channels)
                .map(|_| RealFftProcessor::new_bidirectional(SPECTRAL_HISS_FFT_SIZE))
                .collect(),
            window: generate_hann_window(SPECTRAL_HISS_FFT_SIZE),
            input: vec![vec![0.0; SPECTRAL_HISS_FFT_SIZE]; channels],
            input_write: 0,
            // Prime the causal analysis window with zero-valued history. The
            // first hop is therefore available after HOP_SIZE input frames.
            input_fill: SPECTRAL_HISS_FFT_SIZE - HOP_SIZE,
            output: vec![0.0; output_frames * channels],
            output_mask: output_frames - 1,
            output_read: 0,
            output_write: 0,
            output_fill: 0,
            latency_fill: 0,
            dry_delay: vec![0.0; SPECTRAL_HISS_FFT_SIZE * channels],
            dry_pos: 0,
            power: vec![vec![0.0; NUM_BINS]; channels],
            smoothed_power: vec![vec![0.0; NUM_BINS]; channels],
            smoothed_gain: vec![vec![1.0; NUM_BINS]; channels],
            high_band_noise: vec![0.0; channels],
            current_min: vec![vec![f32::INFINITY; NUM_BINS]; channels],
            minimum_history: vec![vec![f32::INFINITY; MIN_HISTORY_SLOTS * NUM_BINS]; channels],
            history_slot: 0,
            hops_in_slot: 0,
        }
    }

    pub fn initialize(&mut self, sample_rate: u32) -> Result<(), String> {
        if sample_rate == 0 {
            return Err("sample rate must be nonzero".into());
        }
        self.sample_rate = sample_rate;
        self.hops_per_slot = ((MIN_HISTORY_SECONDS * sample_rate as f32
            / (MIN_HISTORY_SLOTS * HOP_SIZE) as f32)
            .round() as usize)
            .max(1);
        let hop_seconds = HOP_SIZE as f32 / sample_rate as f32;
        self.gain_attack = (-hop_seconds / GAIN_ATTACK_SECONDS).exp();
        self.gain_release = (-hop_seconds / GAIN_RELEASE_SECONDS).exp();
        self.bypass.step = 1.0 / (BYPASS_SECONDS * sample_rate as f32).max(1.0);
        self.reset();
        Ok(())
    }

    pub fn set_params(&mut self, cutoff_hz: f32, threshold_db: f32, strength: f32) {
        self.cutoff_hz = cutoff_hz.clamp(20.0, self.sample_rate as f32 * 0.45);
        self.threshold_linear = 10.0_f32.powf(threshold_db.clamp(-120.0, 0.0) / 20.0);
        self.strength = strength.clamp(0.0, 1.0);
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.bypass.target = if enabled { 1.0 } else { 0.0 };
    }

    pub const fn latency_samples(&self) -> usize {
        SPECTRAL_HISS_FFT_SIZE
    }

    pub fn reset(&mut self) {
        for channel in &mut self.input {
            channel.fill(0.0);
        }
        self.input_write = 0;
        self.input_fill = SPECTRAL_HISS_FFT_SIZE - HOP_SIZE;
        self.output.fill(0.0);
        self.output_read = 0;
        self.output_write = 0;
        self.output_fill = 0;
        self.latency_fill = 0;
        self.dry_delay.fill(0.0);
        self.dry_pos = 0;
        for channel in &mut self.power {
            channel.fill(0.0);
        }
        for channel in &mut self.smoothed_power {
            channel.fill(0.0);
        }
        for channel in &mut self.smoothed_gain {
            channel.fill(1.0);
        }
        for channel in &mut self.current_min {
            channel.fill(f32::INFINITY);
        }
        for channel in &mut self.minimum_history {
            channel.fill(f32::INFINITY);
        }
        self.history_slot = 0;
        self.hops_in_slot = 0;
        self.bypass.mix = self.bypass.target;
        self.high_band_noise.fill(0.0);
    }

    fn process_hop(&mut self) {
        let scale = 1.0 / (SPECTRAL_HISS_FFT_SIZE as f32 * 1.5);
        let cutoff_bin =
            ((self.cutoff_hz * SPECTRAL_HISS_FFT_SIZE as f32 / self.sample_rate as f32).ceil()
                as usize)
                .min(NUM_BINS - 1);
        for ch in 0..self.channels {
            for i in 0..SPECTRAL_HISS_FFT_SIZE {
                let source = (self.input_write + i) & (SPECTRAL_HISS_FFT_SIZE - 1);
                self.fft[ch].time_buffer[i] = self.input[ch][source] * self.window[i];
            }
            self.fft[ch].forward();
            // First pass updates every bin before tonality is classified, so
            // protection cannot depend on FFT scan order.
            for bin in 0..NUM_BINS {
                let value = self.fft[ch].freq_buffer[bin];
                let power = value.re * value.re + value.im * value.im;
                self.power[ch][bin] = power;
                let previous = self.smoothed_power[ch][bin];
                let smoothed = if previous == 0.0 {
                    power
                } else {
                    0.8 * previous + 0.2 * power
                };
                self.smoothed_power[ch][bin] = smoothed;
                self.current_min[ch][bin] = self.current_min[ch][bin].min(smoothed);
            }

            let mut aggregate_noise = 0.0;
            for bin in cutoff_bin..NUM_BINS {
                let mut noise = self.current_min[ch][bin];
                for slot in 0..MIN_HISTORY_SLOTS {
                    noise = noise.min(self.minimum_history[ch][slot * NUM_BINS + bin]);
                }
                if noise.is_finite() {
                    aggregate_noise += noise;
                }
            }
            // Parseval with an unnormalised real FFT: doubled one-sided bin
            // energy is N² * mean(x² w²); periodic Hann mean(w²)=3/8.
            let window_energy = 0.375;
            let noise_rms = (aggregate_noise / window_energy).sqrt() * (2.0_f32).sqrt()
                / SPECTRAL_HISS_FFT_SIZE as f32;
            self.high_band_noise[ch] = noise_rms;

            // Second pass applies classification and always advances the gain
            // smoother, including threshold/strength release back to unity.
            for bin in cutoff_bin..NUM_BINS {
                let power = self.power[ch][bin];
                let mut noise = self.current_min[ch][bin];
                for slot in 0..MIN_HISTORY_SLOTS {
                    noise = noise.min(self.minimum_history[ch][slot * NUM_BINS + bin]);
                }

                let mut target_gain = 1.0;
                if self.strength > 0.0
                    && noise.is_finite()
                    && noise > 0.0
                    && noise_rms <= self.threshold_linear
                {
                    let wiener = (1.0 - noise / power.max(1.0e-20)).clamp(0.0, 1.0);
                    let floor = 1.0 - self.strength;
                    // Persistent narrowband peaks are wanted programme,
                    // not broadband hiss. Preserve bins whose local peak
                    // is strongly tonal relative to their neighbours.
                    // Preserve the full three-bin Hann main lobe around a
                    // local tonal peak. A bin-centred sinusoid has each
                    // adjacent-bin power at one quarter of the peak.
                    let mut tonal = false;
                    for candidate in bin.saturating_sub(1)..=(bin + 1).min(NUM_BINS - 1) {
                        if candidate > 0 && candidate + 1 < NUM_BINS {
                            let centre = self.smoothed_power[ch][candidate];
                            let neighbour = 0.5
                                * (self.smoothed_power[ch][candidate - 1]
                                    + self.smoothed_power[ch][candidate + 1]);
                            tonal |= centre > 3.0 * neighbour.max(1.0e-20);
                        }
                    }
                    target_gain = if tonal {
                        1.0
                    } else {
                        floor + (1.0 - floor) * wiener.sqrt()
                    };
                    // Slow release avoids isolated high-gain time/frequency
                    // holes (the usual source of musical-noise chirps),
                    // while attenuation can engage promptly.
                }
                let previous_gain = self.smoothed_gain[ch][bin];
                let coefficient = if target_gain < previous_gain {
                    self.gain_attack
                } else {
                    self.gain_release
                };
                let gain = coefficient * previous_gain + (1.0 - coefficient) * target_gain;
                self.smoothed_gain[ch][bin] = gain;
                self.fft[ch].freq_buffer[bin] *= gain;
            }
            self.fft[ch].inverse();
            for i in 0..SPECTRAL_HISS_FFT_SIZE {
                let frame = (self.output_write + i) & self.output_mask;
                self.output[frame * self.channels + ch] +=
                    self.fft[ch].time_buffer[i] * self.window[i] * scale;
            }
        }
        self.output_write = (self.output_write + HOP_SIZE) & self.output_mask;
        self.output_fill += HOP_SIZE;
        self.hops_in_slot += 1;
        if self.hops_in_slot == self.hops_per_slot {
            for ch in 0..self.channels {
                let start = self.history_slot * NUM_BINS;
                self.minimum_history[ch][start..start + NUM_BINS]
                    .copy_from_slice(&self.current_min[ch]);
                self.current_min[ch].fill(f32::INFINITY);
            }
            self.history_slot = (self.history_slot + 1) % MIN_HISTORY_SLOTS;
            self.hops_in_slot = 0;
        }
    }

    pub fn process(&mut self, buffer: &mut [f32]) {
        if self.channels == 0 {
            return;
        }
        let frames = buffer.len() / self.channels;
        debug_assert_eq!(buffer.len(), frames * self.channels);

        for frame in 0..frames {
            let base = frame * self.channels;
            // Capture every input sample before writing any output. This is
            // essential for callbacks larger than the FFT: analysis/control
            // timing must not depend on the host's partitioning.
            for ch in 0..self.channels {
                let sample = buffer[base + ch];
                self.input[ch][self.input_write] = if sample.is_finite() { sample } else { 0.0 };
            }
            self.input_write = (self.input_write + 1) & (SPECTRAL_HISS_FFT_SIZE - 1);
            self.input_fill += 1;
            if self.input_fill == SPECTRAL_HISS_FFT_SIZE {
                self.process_hop();
                self.input_fill = SPECTRAL_HISS_FFT_SIZE - HOP_SIZE;
            }

            let startup = self.latency_fill < HOP_SIZE;
            debug_assert!(startup || self.output_fill > 0);
            for ch in 0..self.channels {
                let dry_index = self.dry_pos + ch;
                let dry = self.dry_delay[dry_index];
                self.dry_delay[dry_index] =
                    self.input[ch][(self.input_write + SPECTRAL_HISS_FFT_SIZE - 1)
                        & (SPECTRAL_HISS_FFT_SIZE - 1)];
                let wet = if startup {
                    0.0
                } else {
                    self.output[self.output_read * self.channels + ch]
                };
                buffer[base + ch] = dry + (wet - dry) * self.bypass.mix;
                if !startup {
                    self.output[self.output_read * self.channels + ch] = 0.0;
                }
            }
            self.dry_pos += self.channels;
            if self.dry_pos == self.dry_delay.len() {
                self.dry_pos = 0;
            }
            if startup {
                self.latency_fill += 1;
            } else {
                self.output_read = (self.output_read + 1) & self.output_mask;
                self.output_fill -= 1;
            }
            if self.bypass.mix < self.bypass.target {
                self.bypass.mix = (self.bypass.mix + self.bypass.step).min(self.bypass.target);
            } else if self.bypass.mix > self.bypass.target {
                self.bypass.mix = (self.bypass.mix - self.bypass.step).max(self.bypass.target);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(input: &[f32], partitions: &[usize], enabled: bool, strength: f32) -> Vec<f32> {
        let mut reducer = SpectralHissReducer::new(1);
        reducer.initialize(48_000).unwrap();
        reducer.set_params(4_000.0, -30.0, strength);
        reducer.set_enabled(enabled);
        let mut output = Vec::with_capacity(input.len());
        let mut offset = 0;
        let mut partition = 0;
        while offset < input.len() {
            let count = partitions[partition % partitions.len()].min(input.len() - offset);
            let mut block = input[offset..offset + count].to_vec();
            reducer.process(&mut block);
            output.extend(block);
            offset += count;
            partition += 1;
        }
        output
    }

    #[test]
    fn unity_spectral_path_has_exact_reported_impulse_latency() {
        let mut input = vec![0.0; 4096];
        input[0] = 1.0;
        for partitions in [&[1][..], &[64], &[511], &[512], &[1024], &[73, 997, 5, 256]] {
            let output = render(&input, partitions, true, 0.0);
            let peak = output
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))
                .unwrap();
            assert_eq!(peak.0, SPECTRAL_HISS_FFT_SIZE, "{partitions:?}");
            assert!((peak.1 - 1.0).abs() < 2.0e-5, "{partitions:?}: {peak:?}");
        }
    }

    #[test]
    fn callback_partition_does_not_change_samples() {
        let input: Vec<f32> = (0..8192)
            .map(|i| {
                let t = i as f32 / 48_000.0;
                0.2 * (2.0 * std::f32::consts::PI * 600.0 * t).sin()
                    + 0.03 * ((i * 7919 % 1021) as f32 / 510.5 - 1.0)
            })
            .collect();
        assert_eq!(
            render(&input, &[8192], true, 0.65),
            render(&input, &[1, 64, 511, 73, 997], true, 0.65)
        );
    }

    #[test]
    fn bypass_is_exactly_the_reported_delayed_dry_signal() {
        let input: Vec<f32> = (0..4096).map(|i| (i as f32 * 0.017).sin()).collect();
        let output = render(&input, &[1, 64, 511, 1024], false, 1.0);
        assert_eq!(
            &output[..SPECTRAL_HISS_FFT_SIZE],
            vec![0.0; SPECTRAL_HISS_FFT_SIZE]
        );
        assert_eq!(
            &output[SPECTRAL_HISS_FFT_SIZE..],
            &input[..input.len() - SPECTRAL_HISS_FFT_SIZE]
        );
    }

    #[test]
    fn reset_matches_a_fresh_instance_and_recovers_from_non_finite_input() {
        let mut poison = Vec::with_capacity(4096);
        for _ in 0..1024 {
            poison.extend([f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.25]);
        }
        let mut reducer = SpectralHissReducer::new(1);
        reducer.initialize(48_000).unwrap();
        reducer.process(&mut poison);
        reducer.reset();

        let input: Vec<f32> = (0..4096).map(|i| (i as f32 * 0.031).sin()).collect();
        let mut reset_output = input.clone();
        reducer.process(&mut reset_output);
        let fresh_output = render(&input, &[input.len()], true, 0.5);
        assert_eq!(reset_output, fresh_output);
        assert!(reset_output.iter().all(|sample| sample.is_finite()));
    }

    #[test]
    fn stereo_channels_remain_independent() {
        let frames = 4096;
        let mut stereo = vec![0.0; frames * 2];
        for frame in 0..frames {
            stereo[frame * 2] = (frame as f32 * 0.07).sin();
        }
        let mut reducer = SpectralHissReducer::new(2);
        reducer.initialize(48_000).unwrap();
        reducer.process(&mut stereo);
        assert!(stereo.chunks_exact(2).all(|frame| frame[1] == 0.0));
        assert!(stereo.chunks_exact(2).any(|frame| frame[0] != 0.0));
    }

    #[test]
    fn unity_wola_reconstructs_broadband_signal() {
        let input: Vec<f32> = (0..8192)
            .map(|i| ((i * 3571 % 2053) as f32 / 1026.5 - 1.0) * 0.2)
            .collect();
        let output = render(&input, &[1, 64, 511, 997], true, 0.0);
        let mut maximum_error = 0.0_f32;
        for i in SPECTRAL_HISS_FFT_SIZE..input.len() {
            maximum_error =
                maximum_error.max((output[i] - input[i - SPECTRAL_HISS_FFT_SIZE]).abs());
        }
        assert!(maximum_error < 2.0e-5, "WOLA error {maximum_error}");
    }
}
