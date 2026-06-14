use super::audio_sample::AudioSample;
use super::audio_sample::ensure_len;

pub(super) struct NodeBuffer<T: AudioSample> {
    pub(super) data: Vec<T>,
    pub(super) actual_len: usize,
    pub(super) num_channels: usize,
}

impl<T: AudioSample> NodeBuffer<T> {
    pub(super) fn new(num_frames: usize, num_channels: usize) -> Self {
        Self {
            data: vec![T::default(); num_frames * num_channels],
            actual_len: 0,
            num_channels,
        }
    }
    pub(super) fn write(&mut self, data: &[T]) {
        ensure_len(&mut self.data, data.len());
        self.data[..data.len()].copy_from_slice(data);
        self.actual_len = data.len();
    }
    pub(super) fn read(&self) -> &[T] {
        if self.actual_len == 0 {
            &[]
        } else {
            &self.data[..self.actual_len]
        }
    }
    pub(super) fn clear(&mut self) {
        self.actual_len = 0;
    }
    pub(super) fn ensure_capacity(&mut self, num_frames: usize) {
        let required = num_frames * self.num_channels;
        ensure_len(&mut self.data, required);
    }
}
