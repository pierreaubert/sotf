// ============================================================================
// Lookahead Buffer — Shared circular delay for dynamics plugins
// ============================================================================

/// A per-channel circular delay buffer for lookahead processing.
///
/// Audio is pushed into the buffer and comes out delayed by `delay` samples.
/// The delay can be changed at any time (up to `max_delay`).
#[derive(Debug, Clone)]
pub struct LookaheadBuffer {
    buffer: Vec<f32>,
    pos: usize,
    max_delay: usize,
    delay: usize,
    channels: usize,
}

impl LookaheadBuffer {
    /// Create a new lookahead buffer.
    ///
    /// * `max_delay_samples` — maximum delay in frames (not samples)
    /// * `channels` — number of interleaved channels
    pub fn new(max_delay_samples: usize, channels: usize) -> Self {
        let max_delay = max_delay_samples.max(1);
        Self {
            buffer: vec![0.0; max_delay * channels],
            pos: 0,
            max_delay,
            delay: max_delay,
            channels,
        }
    }

    /// Create from a delay time in milliseconds.
    pub fn from_ms(delay_ms: f32, sample_rate: u32, channels: usize) -> Self {
        let samples = (delay_ms * 0.001 * sample_rate as f32).round() as usize;
        Self::new(samples, channels)
    }

    /// Set the active delay in frames. Clamped to [1, max_delay].
    /// Resets buffer to prevent stale data from wrong circular positions.
    pub fn set_delay(&mut self, delay_samples: usize) {
        let new_delay = delay_samples.clamp(1, self.max_delay);
        if new_delay != self.delay {
            self.delay = new_delay;
            self.buffer.fill(0.0);
            self.pos = 0;
        }
    }

    /// Set delay from milliseconds.
    pub fn set_delay_ms(&mut self, delay_ms: f32, sample_rate: u32) {
        let samples = (delay_ms * 0.001 * sample_rate as f32).round() as usize;
        self.set_delay(samples);
    }

    /// Push one frame of interleaved samples and return the delayed frame.
    ///
    /// `input` must have exactly `channels` elements.
    /// `output` must have exactly `channels` elements and will be filled with the delayed samples.
    #[inline]
    pub fn process_frame(&mut self, input: &[f32], output: &mut [f32]) {
        debug_assert_eq!(input.len(), self.channels);
        debug_assert_eq!(output.len(), self.channels);

        let base = self.pos * self.channels;
        let buf_slice = &mut self.buffer[base..base + self.channels];
        output[..self.channels].copy_from_slice(buf_slice);
        buf_slice.copy_from_slice(&input[..self.channels]);
        self.pos = (self.pos + 1) % self.delay;
    }

    /// Push a single sample (mono or per-channel usage) and return the delayed sample.
    #[inline]
    pub fn push(&mut self, sample: f32) -> f32 {
        assert_eq!(self.channels, 1, "use process_frame for multi-channel");
        let delayed = self.buffer[self.pos];
        self.buffer[self.pos] = sample;
        self.pos = (self.pos + 1) % self.delay;
        delayed
    }

    /// Reset the buffer to silence.
    pub fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.pos = 0;
    }

    /// Resize for a new max delay (e.g., when sample rate changes).
    pub fn resize(&mut self, max_delay_samples: usize, channels: usize) {
        self.max_delay = max_delay_samples.max(1);
        self.channels = channels;
        self.delay = self.delay.min(self.max_delay);
        self.buffer.resize(self.max_delay * self.channels, 0.0);
        self.buffer.fill(0.0);
        self.pos = 0;
    }

    /// Current delay in frames.
    pub fn delay(&self) -> usize {
        self.delay
    }

    /// Maximum delay in frames.
    pub fn max_delay(&self) -> usize {
        self.max_delay
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mono_delay() {
        let mut buf = LookaheadBuffer::new(3, 1);
        // Buffer is 3 frames of silence initially
        assert_eq!(buf.push(1.0), 0.0); // frame 0: read silence, write 1.0
        assert_eq!(buf.push(2.0), 0.0); // frame 1: read silence, write 2.0
        assert_eq!(buf.push(3.0), 0.0); // frame 2: read silence, write 3.0
        assert_eq!(buf.push(4.0), 1.0); // frame 3: wraps, reads 1.0
        assert_eq!(buf.push(5.0), 2.0); // frame 4: reads 2.0
        assert_eq!(buf.push(6.0), 3.0); // frame 5: reads 3.0
    }

    #[test]
    fn test_stereo_delay() {
        let mut buf = LookaheadBuffer::new(2, 2);
        let mut out = [0.0f32; 2];

        buf.process_frame(&[1.0, 10.0], &mut out);
        assert_eq!(out, [0.0, 0.0]);

        buf.process_frame(&[2.0, 20.0], &mut out);
        assert_eq!(out, [0.0, 0.0]);

        buf.process_frame(&[3.0, 30.0], &mut out);
        assert_eq!(out, [1.0, 10.0]);

        buf.process_frame(&[4.0, 40.0], &mut out);
        assert_eq!(out, [2.0, 20.0]);
    }

    #[test]
    fn test_from_ms() {
        let buf = LookaheadBuffer::from_ms(5.0, 48000, 2);
        assert_eq!(buf.max_delay(), 240); // 5ms * 48000 = 240 samples
        assert_eq!(buf.delay(), 240);
    }

    #[test]
    fn test_reset() {
        let mut buf = LookaheadBuffer::new(2, 1);
        buf.push(1.0);
        buf.push(2.0);
        buf.reset();
        assert_eq!(buf.push(99.0), 0.0);
        assert_eq!(buf.push(100.0), 0.0);
    }

    #[test]
    fn test_set_delay_shorter() {
        let mut buf = LookaheadBuffer::new(10, 1);
        buf.set_delay(2);
        assert_eq!(buf.delay(), 2);
        // Now only 2-frame delay
        assert_eq!(buf.push(1.0), 0.0);
        assert_eq!(buf.push(2.0), 0.0);
        assert_eq!(buf.push(3.0), 1.0);
    }
}
