use super::audio_sample::AudioSample;

pub(super) struct DelayBuffer<T: AudioSample> {
    pub(super) buffer: Vec<T>,
    pub(super) pos: usize,
    pub(super) delay: usize,
    pub(super) channels: usize,
}

impl<T: AudioSample> DelayBuffer<T> {
    pub(super) fn new(max_delay_samples: usize, channels: usize) -> Self {
        let max_delay = max_delay_samples.max(1);
        Self {
            buffer: vec![T::default(); max_delay * channels],
            pos: 0,
            delay: max_delay,
            channels,
        }
    }

    #[inline]
    pub(super) fn process_frame(&mut self, input: &[T], output: &mut [T]) {
        debug_assert_eq!(input.len(), self.channels);
        debug_assert_eq!(output.len(), self.channels);

        let base = self.pos * self.channels;
        let buf_slice = &mut self.buffer[base..base + self.channels];
        output[..self.channels].copy_from_slice(buf_slice);
        buf_slice.copy_from_slice(&input[..self.channels]);
        self.pos = (self.pos + 1) % self.delay;
    }

    #[cfg(test)]
    pub(super) fn delay(&self) -> usize {
        self.delay
    }
}
