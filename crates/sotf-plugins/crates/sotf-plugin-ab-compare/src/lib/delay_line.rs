/// Minimal fixed-delay ring buffer for aligning two processing paths.
pub(super) struct DelayLine {
    pub(super) buffer: Vec<f32>,
    pub(super) pos: usize,
    pub(super) len: usize,
}

impl DelayLine {
    pub(super) fn new() -> Self {
        Self {
            buffer: Vec::new(),
            pos: 0,
            len: 0,
        }
    }

    /// Set delay in frames (interleaved samples = frames * channels).
    /// Allocates only when size changes.
    pub(super) fn set_delay(&mut self, frames: usize, channels: usize) {
        let new_len = frames * channels;
        if new_len != self.len {
            self.buffer.resize(new_len, 0.0);
            self.buffer.fill(0.0);
            self.pos = 0;
            self.len = new_len;
        }
    }

    /// Swap each sample in `data` with the delayed version. No-op when len == 0.
    #[inline]
    pub(super) fn process(&mut self, data: &mut [f32]) {
        if self.len == 0 {
            return;
        }
        for sample in data.iter_mut() {
            std::mem::swap(&mut self.buffer[self.pos], sample);
            self.pos += 1;
            if self.pos >= self.len {
                self.pos = 0;
            }
        }
    }

    pub(super) fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.pos = 0;
    }
}
