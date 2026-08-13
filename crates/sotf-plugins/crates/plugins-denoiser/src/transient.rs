//! Bounded-lookahead click detection and interpolation.
//!
//! The detector compares each delayed candidate with robust pre/post context.
//! Only short deviations whose surrounding medians return to the same local
//! trajectory are repaired. Multichannel decisions are linked in adjacent
//! channel pairs by default, while interpolation remains channel-specific.

/// Context on each side of the candidate and the plugin's fixed latency.
pub const LOOKAHEAD_SAMPLES: usize = 8;
const RING_FRAMES: usize = LOOKAHEAD_SAMPLES * 2 + 1;
const SCALE_FLOOR: f32 = 1.0e-5;
const CONTROL_SMOOTH_MS: f32 = 5.0;

pub struct TransientSuppressor {
    channels: usize,
    ring: Vec<f32>,
    write_frame: usize,
    frames_seen: usize,
    baselines: Vec<f32>,
    residuals: Vec<f32>,
    thresholds: Vec<f32>,
    returned: Vec<bool>,
    repair: Vec<bool>,
    clean_scale: Vec<f32>,
    clean_scale_primed: Vec<bool>,
    sensitivity_current: f32,
    sensitivity_target: f32,
    repair_mix_current: f32,
    repair_mix_target: f32,
    control_decay: f32,
    link_channels: bool,
}

impl TransientSuppressor {
    pub fn new(channels: usize, sample_rate: u32) -> Result<Self, String> {
        if channels == 0 {
            return Err("transient suppressor requires at least one channel".into());
        }
        if sample_rate == 0 {
            return Err("transient suppressor sample rate must be greater than zero".into());
        }
        let mut this = Self {
            channels,
            ring: vec![0.0; channels * RING_FRAMES],
            write_frame: 0,
            frames_seen: 0,
            baselines: vec![0.0; channels],
            residuals: vec![0.0; channels],
            thresholds: vec![SCALE_FLOOR; channels],
            returned: vec![false; channels],
            repair: vec![false; channels],
            clean_scale: vec![SCALE_FLOOR; channels],
            clean_scale_primed: vec![false; channels],
            sensitivity_current: 10.0,
            sensitivity_target: 10.0,
            repair_mix_current: 1.0,
            repair_mix_target: 1.0,
            control_decay: 0.0,
            link_channels: true,
        };
        this.set_sample_rate(sample_rate)?;
        Ok(this)
    }

    pub const fn latency_samples(&self) -> usize {
        LOOKAHEAD_SAMPLES
    }

    pub fn reset(&mut self) {
        self.ring.fill(0.0);
        self.write_frame = 0;
        self.frames_seen = 0;
        self.baselines.fill(0.0);
        self.residuals.fill(0.0);
        self.thresholds.fill(SCALE_FLOOR);
        self.returned.fill(false);
        self.repair.fill(false);
        self.clean_scale.fill(SCALE_FLOOR);
        self.clean_scale_primed.fill(false);
        self.sensitivity_current = self.sensitivity_target;
        self.repair_mix_current = self.repair_mix_target;
    }

    pub fn set_sample_rate(&mut self, sample_rate: u32) -> Result<(), String> {
        if sample_rate == 0 {
            return Err("transient suppressor sample rate must be greater than zero".into());
        }
        let smoothing_samples = sample_rate as f32 * CONTROL_SMOOTH_MS * 0.001;
        self.control_decay = (-1.0 / smoothing_samples.max(1.0)).exp();
        Ok(())
    }

    pub fn set_sensitivity(&mut self, sensitivity: f32) {
        self.sensitivity_target = canonical_sensitivity(sensitivity);
    }

    pub fn set_sensitivity_immediate(&mut self, sensitivity: f32) {
        self.sensitivity_target = canonical_sensitivity(sensitivity);
        self.sensitivity_current = self.sensitivity_target;
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.repair_mix_target = if enabled { 1.0 } else { 0.0 };
    }

    pub fn set_enabled_immediate(&mut self, enabled: bool) {
        self.repair_mix_target = if enabled { 1.0 } else { 0.0 };
        self.repair_mix_current = self.repair_mix_target;
    }

    pub fn set_link_channels(&mut self, linked: bool) {
        self.link_channels = linked;
    }

    /// Process an exact interleaved frame buffer in place.
    ///
    /// The first `LOOKAHEAD_SAMPLES` output frames are silence. Thereafter the
    /// output is the input delayed by that fixed amount, with only detected
    /// short clicks replaced. Invalid shapes are rejected without mutation.
    pub fn process(&mut self, buffer: &mut [f32]) -> Result<(), String> {
        if !buffer.len().is_multiple_of(self.channels) {
            return Err(format!(
                "transient buffer length {} is not divisible by {} channels",
                buffer.len(),
                self.channels
            ));
        }

        for frame in buffer.chunks_exact_mut(self.channels) {
            self.advance_controls();
            self.write_input_frame(frame);

            if self.frames_seen <= LOOKAHEAD_SAMPLES {
                frame.fill(0.0);
                continue;
            }

            let candidate_frame =
                (self.write_frame + RING_FRAMES - 1 - LOOKAHEAD_SAMPLES) % RING_FRAMES;
            if self.frames_seen < RING_FRAMES {
                for (ch, output) in frame.iter_mut().enumerate() {
                    *output = self.ring[candidate_frame * self.channels + ch];
                }
                continue;
            }

            self.analyze_candidate(candidate_frame);
            self.link_repair_decisions();
            for (ch, output) in frame.iter_mut().enumerate() {
                let dry = self.ring[candidate_frame * self.channels + ch];
                let wet = if self.repair[ch] {
                    self.baselines[ch]
                } else {
                    dry
                };
                *output = dry + (wet - dry) * self.repair_mix_current;
                self.update_clean_scale(ch);
            }
        }
        Ok(())
    }

    fn advance_controls(&mut self) {
        let one_minus = 1.0 - self.control_decay;
        self.sensitivity_current =
            self.sensitivity_current * self.control_decay + self.sensitivity_target * one_minus;
        self.repair_mix_current =
            self.repair_mix_current * self.control_decay + self.repair_mix_target * one_minus;
    }

    fn write_input_frame(&mut self, frame: &[f32]) {
        let previous_frame = (self.write_frame + RING_FRAMES - 1) % RING_FRAMES;
        for (ch, &input) in frame.iter().enumerate() {
            let sample = if input.is_finite() {
                input
            } else if self.frames_seen == 0 {
                0.0
            } else {
                self.ring[previous_frame * self.channels + ch]
            };
            self.ring[self.write_frame * self.channels + ch] = sample;
        }
        self.write_frame = (self.write_frame + 1) % RING_FRAMES;
        self.frames_seen = self.frames_seen.saturating_add(1);
    }

    fn analyze_candidate(&mut self, candidate_frame: usize) {
        for ch in 0..self.channels {
            let mut pre = [0.0_f32; LOOKAHEAD_SAMPLES];
            let mut post = [0.0_f32; LOOKAHEAD_SAMPLES];
            for i in 0..LOOKAHEAD_SAMPLES {
                pre[i] = self.sample(candidate_frame, ch, -(i as isize + 1));
                post[i] = self.sample(candidate_frame, ch, i as isize + 1);
            }
            let pre_median = median(pre);
            let post_median = median(post);
            let baseline = 0.5 * (pre_median + post_median);
            let candidate = self.sample(candidate_frame, ch, 0);
            let residual = (candidate - baseline).abs();

            let mut deviations = [0.0_f32; LOOKAHEAD_SAMPLES * 2];
            for i in 0..LOOKAHEAD_SAMPLES {
                deviations[i] = (pre[i] - pre_median).abs();
                deviations[LOOKAHEAD_SAMPLES + i] = (post[i] - post_median).abs();
            }
            let mut slopes = [0.0_f32; (LOOKAHEAD_SAMPLES - 1) * 2];
            for i in 0..LOOKAHEAD_SAMPLES - 1 {
                slopes[i] = (pre[i + 1] - pre[i]).abs();
                slopes[LOOKAHEAD_SAMPLES - 1 + i] = (post[i + 1] - post[i]).abs();
            }
            let local_scale = median_16(deviations)
                .mul_add(1.4826, SCALE_FLOOR)
                .max(median(slopes));
            let scale = if self.clean_scale_primed[ch] {
                local_scale.max(self.clean_scale[ch] * 0.25)
            } else {
                local_scale
            }
            .max(SCALE_FLOOR);
            let threshold = scale * self.sensitivity_current.max(1.0) * 2.0 + SCALE_FLOOR;
            let bridge = (post_median - pre_median).abs();
            let candidate_offset = candidate - baseline;
            let mut excursion_len = 1;
            for direction in [-1_isize, 1] {
                for distance in 1..=LOOKAHEAD_SAMPLES {
                    let neighbor =
                        self.sample(candidate_frame, ch, direction * distance as isize) - baseline;
                    if neighbor * candidate_offset > 0.0 && neighbor.abs() >= residual * 0.5 {
                        excursion_len += 1;
                    } else {
                        break;
                    }
                }
            }

            self.baselines[ch] = baseline;
            self.residuals[ch] = residual;
            self.thresholds[ch] = threshold;
            self.returned[ch] = bridge <= (residual * 0.5).max(scale * 6.0);
            self.repair[ch] = residual > threshold && self.returned[ch] && excursion_len <= 6;
        }
    }

    fn link_repair_decisions(&mut self) {
        if !self.link_channels || self.channels == 1 {
            return;
        }
        for first in (0..self.channels).step_by(2) {
            let end = (first + 2).min(self.channels);
            let linked = self.repair[first..end].iter().any(|&repair| repair);
            self.repair[first..end].fill(linked);
        }
    }

    fn update_clean_scale(&mut self, ch: usize) {
        if self.repair[ch] {
            return;
        }
        let residual = ((self.thresholds[ch] - SCALE_FLOOR)
            / (self.sensitivity_current.max(1.0) * 2.0))
            .max(SCALE_FLOOR);
        if !self.clean_scale_primed[ch] {
            self.clean_scale[ch] = residual;
            self.clean_scale_primed[ch] = true;
        } else if residual > self.clean_scale[ch] {
            self.clean_scale[ch] = residual;
        } else {
            self.clean_scale[ch] = self.clean_scale[ch] * 0.999 + residual * 0.001;
        }
    }

    fn sample(&self, candidate_frame: usize, ch: usize, offset: isize) -> f32 {
        let frame = (candidate_frame as isize + offset).rem_euclid(RING_FRAMES as isize) as usize;
        self.ring[frame * self.channels + ch]
    }
}

fn canonical_sensitivity(sensitivity: f32) -> f32 {
    if sensitivity.is_finite() {
        sensitivity.clamp(1.0, 100.0)
    } else {
        10.0
    }
}

fn median<const N: usize>(mut values: [f32; N]) -> f32 {
    values.sort_unstable_by(f32::total_cmp);
    if N.is_multiple_of(2) {
        0.5 * (values[N / 2 - 1] + values[N / 2])
    } else {
        values[N / 2]
    }
}

fn median_16(mut values: [f32; 16]) -> f32 {
    values.sort_unstable_by(f32::total_cmp);
    0.5 * (values[7] + values[8])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process_with_flush(suppressor: &mut TransientSuppressor, input: &[f32]) -> Vec<f32> {
        let mut stream = input.to_vec();
        stream.extend(std::iter::repeat_n(
            0.0,
            LOOKAHEAD_SAMPLES * suppressor.channels,
        ));
        suppressor.process(&mut stream).unwrap();
        stream
    }

    #[test]
    fn construction_contract_is_fallible() {
        assert!(TransientSuppressor::new(0, 48_000).is_err());
        assert!(TransientSuppressor::new(1, 0).is_err());
    }

    #[test]
    fn disabled_path_is_exactly_latency_matched() {
        let mut suppressor = TransientSuppressor::new(1, 48_000).unwrap();
        suppressor.set_enabled_immediate(false);
        let input: Vec<f32> = (0..64).map(|i| (i as f32 * 0.17).sin()).collect();
        let output = process_with_flush(&mut suppressor, &input);
        assert!(output[..LOOKAHEAD_SAMPLES].iter().all(|&x| x == 0.0));
        assert_eq!(
            &output[LOOKAHEAD_SAMPLES..LOOKAHEAD_SAMPLES + input.len()],
            input.as_slice()
        );
    }

    #[test]
    fn isolated_click_is_closer_to_clean_reference() {
        let clean: Vec<f32> = (0..256).map(|i| (i as f32 * 0.07).sin() * 0.25).collect();
        let mut corrupt = clean.clone();
        corrupt[120] += 3.0;
        let mut suppressor = TransientSuppressor::new(1, 48_000).unwrap();
        suppressor.set_sensitivity_immediate(3.0);
        let output = process_with_flush(&mut suppressor, &corrupt);
        let repaired = output[120 + LOOKAHEAD_SAMPLES];
        assert!((repaired - clean[120]).abs() < (corrupt[120] - clean[120]).abs() * 0.05);
    }

    #[test]
    fn short_click_region_is_interpolated_without_recovery_step() {
        let clean: Vec<f32> = (0..320).map(|i| (i as f32 * 0.04).sin() * 0.2).collect();
        let mut corrupt = clean.clone();
        for (offset, sample) in corrupt[150..155].iter_mut().enumerate() {
            *sample += if offset.is_multiple_of(2) { 3.0 } else { -3.0 };
        }
        let mut suppressor = TransientSuppressor::new(1, 48_000).unwrap();
        suppressor.set_sensitivity_immediate(2.0);
        let output = process_with_flush(&mut suppressor, &corrupt);
        let aligned = &output[LOOKAHEAD_SAMPLES..LOOKAHEAD_SAMPLES + clean.len()];
        for i in 150..155 {
            assert!(
                (aligned[i] - clean[i]).abs() < 0.1,
                "burst sample {i} was not reconstructed: clean={}, got={}",
                clean[i],
                aligned[i]
            );
        }
        for pair in aligned[147..159].windows(2) {
            assert!((pair[1] - pair[0]).abs() < 0.15);
        }
    }

    #[test]
    fn onset_and_step_are_preserved_after_new_and_reset() {
        let input = [vec![0.8; 64], {
            let mut step = vec![0.0; 32];
            step.extend(vec![0.8; 32]);
            step
        }];
        for signal in input {
            let mut suppressor = TransientSuppressor::new(1, 48_000).unwrap();
            for _ in 0..2 {
                let output = process_with_flush(&mut suppressor, &signal);
                assert_eq!(
                    &output[LOOKAHEAD_SAMPLES..LOOKAHEAD_SAMPLES + signal.len()],
                    signal.as_slice()
                );
                suppressor.reset();
            }
        }
    }

    #[test]
    fn clean_high_frequency_and_square_signals_are_not_repaired() {
        for input in [
            (0..256)
                .map(|i| (std::f32::consts::TAU * 12_000.0 * i as f32 / 48_000.0).sin() * 0.5)
                .collect::<Vec<_>>(),
            (0..256)
                .map(|i| if (i / 12) % 2 == 0 { -0.5 } else { 0.5 })
                .collect::<Vec<_>>(),
        ] {
            let mut suppressor = TransientSuppressor::new(1, 48_000).unwrap();
            suppressor.set_sensitivity_immediate(1.0);
            let output = process_with_flush(&mut suppressor, &input);
            assert_eq!(
                &output[LOOKAHEAD_SAMPLES..input.len()],
                &input[..input.len() - LOOKAHEAD_SAMPLES]
            );
        }
    }

    #[test]
    fn repeated_clicks_do_not_train_the_detector() {
        let mut input = vec![0.0; 96_000];
        for i in (64..input.len() - 64).step_by(19) {
            input[i] = if i % 2 == 0 { 4.0 } else { -4.0 };
        }
        let mut suppressor = TransientSuppressor::new(1, 48_000).unwrap();
        suppressor.set_sensitivity_immediate(2.0);
        let output = process_with_flush(&mut suppressor, &input);
        let max = output[64 + LOOKAHEAD_SAMPLES..input.len()]
            .iter()
            .copied()
            .map(f32::abs)
            .fold(0.0, f32::max);
        assert!(max < 0.05, "repeated click leaked at amplitude {max}");
    }

    #[test]
    fn linked_stereo_repairs_common_event_coherently() {
        let mut input = vec![0.0; 256 * 2];
        input[120 * 2] = 4.0;
        input[120 * 2 + 1] = 1.0;
        let mut suppressor = TransientSuppressor::new(2, 48_000).unwrap();
        suppressor.set_sensitivity_immediate(2.0);
        let output = process_with_flush(&mut suppressor, &input);
        let index = (120 + LOOKAHEAD_SAMPLES) * 2;
        assert!(output[index].abs() < 0.05);
        assert!(output[index + 1].abs() < 0.05);
    }

    #[test]
    fn independent_mode_does_not_modify_clean_partner() {
        let mut input = vec![0.25; 128 * 2];
        input[64 * 2] = 4.0;
        let mut suppressor = TransientSuppressor::new(2, 48_000).unwrap();
        suppressor.set_link_channels(false);
        suppressor.set_sensitivity_immediate(2.0);
        let output = process_with_flush(&mut suppressor, &input);
        let index = (64 + LOOKAHEAD_SAMPLES) * 2;
        assert!(output[index] < 1.0);
        assert_eq!(output[index + 1], 0.25);
    }

    #[test]
    fn non_finite_input_recovers_locally() {
        let mut input = vec![0.25; 128];
        input[40] = f32::NAN;
        input[80] = f32::INFINITY;
        let mut suppressor = TransientSuppressor::new(1, 48_000).unwrap();
        let output = process_with_flush(&mut suppressor, &input);
        assert!(output.iter().all(|sample| sample.is_finite()));
        assert_eq!(output[100 + LOOKAHEAD_SAMPLES], 0.25);
    }

    #[test]
    fn callback_partitioning_does_not_change_output() {
        let input: Vec<f32> = (0..511).map(|i| (i as f32 * 0.11).sin() * 0.2).collect();
        let mut whole = input.clone();
        let mut a = TransientSuppressor::new(1, 48_000).unwrap();
        a.process(&mut whole).unwrap();

        let mut chunked = input.clone();
        let mut b = TransientSuppressor::new(1, 48_000).unwrap();
        for chunk in chunked.chunks_mut(13) {
            b.process(chunk).unwrap();
        }
        assert_eq!(whole, chunked);
    }

    #[test]
    fn controls_move_smoothly_instead_of_stepping() {
        let mut suppressor = TransientSuppressor::new(1, 48_000).unwrap();
        suppressor.set_sensitivity_immediate(1.0);
        suppressor.set_sensitivity(100.0);
        suppressor.set_enabled(false);
        let mut one = [0.0];
        suppressor.process(&mut one).unwrap();
        assert!(suppressor.sensitivity_current > 1.0);
        assert!(suppressor.sensitivity_current < 100.0);
        assert!(suppressor.repair_mix_current > 0.0);
        assert!(suppressor.repair_mix_current < 1.0);
    }

    #[test]
    fn malformed_buffer_is_rejected_without_mutation() {
        let mut suppressor = TransientSuppressor::new(2, 48_000).unwrap();
        let mut buffer = vec![1.0; 3];
        let original = buffer.clone();
        assert!(suppressor.process(&mut buffer).is_err());
        assert_eq!(buffer, original);
    }
}
