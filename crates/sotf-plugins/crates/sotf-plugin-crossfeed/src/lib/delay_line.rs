/// A mono delay line supporting fractional delays up to 1 ms at the active sample rate.
pub(super) struct DelayLine {
    pub(super) buffer: Vec<f32>,
    pub(super) write_pos: usize,
    pub(super) delay_samples: f32,
    pub(super) capacity: usize,
}

impl DelayLine {
    pub(super) fn new(delay_ms: f32, sample_rate: u32) -> Self {
        let capacity = Self::capacity_for_sample_rate(sample_rate);
        let delay_samples = Self::delay_samples(delay_ms, sample_rate, capacity);
        Self {
            buffer: vec![0.0; capacity],
            write_pos: 0,
            delay_samples,
            capacity,
        }
    }

    pub(super) fn set_delay(&mut self, delay_ms: f32, sample_rate: u32) {
        let capacity = Self::capacity_for_sample_rate(sample_rate);
        if capacity != self.capacity {
            self.buffer.resize(capacity, 0.0);
            self.capacity = capacity;
            self.write_pos %= self.capacity;
        }
        self.delay_samples = Self::delay_samples(delay_ms, sample_rate, self.capacity);
    }

    pub(super) fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.write_pos = 0;
    }

    pub(super) fn capacity_for_sample_rate(sample_rate: u32) -> usize {
        (sample_rate as f32 * 0.001).ceil() as usize + 2
    }

    pub(super) fn delay_samples(delay_ms: f32, sample_rate: u32, capacity: usize) -> f32 {
        ((delay_ms / 1000.0) * sample_rate as f32)
            .max(0.0)
            .min(capacity as f32 - 2.0)
    }

    #[inline]
    pub(super) fn process(&mut self, sample: f32) -> f32 {
        if self.delay_samples <= f32::EPSILON {
            return sample;
        }
        self.buffer[self.write_pos] = sample;
        let int_delay = self.delay_samples.floor() as usize;
        let fract = self.delay_samples - int_delay as f32;
        let read_pos_base = (self.write_pos + self.capacity - int_delay) % self.capacity;
        let read_pos_next = (read_pos_base + self.capacity - 1) % self.capacity;
        let out = self.buffer[read_pos_base] * (1.0 - fract) + self.buffer[read_pos_next] * fract;
        self.write_pos = (self.write_pos + 1) % self.capacity;
        out
    }
}
