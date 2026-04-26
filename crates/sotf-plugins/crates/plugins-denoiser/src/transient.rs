/// Adaptive slew-rate limiter for click and pop repair.
pub struct TransientSuppressor {
    channels: usize,
    last_samples: Vec<f32>,
    slope_envelope: Vec<f32>,
    sensitivity: f32,
    decay: f32,
    one_minus_decay: f32,
}

impl TransientSuppressor {
    pub fn new(channels: usize) -> Self {
        Self {
            channels,
            last_samples: vec![0.0; channels],
            slope_envelope: vec![0.0; channels],
            sensitivity: 10.0,
            decay: 0.99,
            one_minus_decay: 0.01,
        }
    }

    pub fn reset(&mut self) {
        self.last_samples.fill(0.0);
        self.slope_envelope.fill(0.0);
    }

    pub fn set_sensitivity(&mut self, sensitivity: f32) {
        self.sensitivity = sensitivity.max(1.0);
    }

    pub fn process(&mut self, buffer: &mut [f32]) {
        if self.channels == 0 {
            return;
        }

        for frame in buffer.chunks_mut(self.channels) {
            for (ch, sample) in frame.iter_mut().enumerate() {
                let last = self.last_samples[ch];
                let delta = *sample - last;
                let abs_delta = delta.abs();

                if self.slope_envelope[ch] == 0.0 {
                    self.slope_envelope[ch] = abs_delta + 1e-6;
                }

                let threshold = self.slope_envelope[ch] * self.sensitivity + 1e-5;
                let processed_sample;

                if abs_delta > threshold {
                    let sign = if delta >= 0.0 { 1.0 } else { -1.0 };
                    processed_sample = last + sign * threshold;
                    *sample = processed_sample;
                } else {
                    processed_sample = *sample;
                    if abs_delta > self.slope_envelope[ch] {
                        self.slope_envelope[ch] = abs_delta;
                    } else {
                        self.slope_envelope[ch] =
                            self.slope_envelope[ch] * self.decay + abs_delta * self.one_minus_decay;
                    }
                }

                self.last_samples[ch] = processed_sample;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suppresses_large_slew_spike() {
        let mut suppressor = TransientSuppressor::new(1);
        suppressor.set_sensitivity(5.0);

        let mut buffer = Vec::new();
        buffer.extend(std::iter::repeat_n(0.0, 10));
        for i in 0..100 {
            buffer.push((i as f32 * 0.1).sin() * 0.5);
        }

        let click_idx = buffer.len();
        buffer.push(2.0);
        buffer.extend(std::iter::repeat_n(0.0, 10));

        suppressor.process(&mut buffer);
        assert!(buffer[click_idx] < 1.0);
    }
}
