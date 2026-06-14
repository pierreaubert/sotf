/// Cursor-backed FIFO for decoded samples.
///
/// The decoder consumes from the front on every emitted frame. Keeping a cursor
/// avoids a `Vec::drain(..n)` memmove in the hot path; compaction only happens
/// before growth when the consumed prefix is large enough to matter.
#[derive(Debug, Default)]
pub(super) struct SampleQueue {
    pub(super) data: Vec<f32>,
    pub(super) start: usize,
}

impl SampleQueue {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn len(&self) -> usize {
        self.data.len().saturating_sub(self.start)
    }

    pub(super) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(super) fn as_slice(&self) -> &[f32] {
        &self.data[self.start..]
    }

    pub(super) fn prefix(&self, len: usize) -> &[f32] {
        &self.as_slice()[..len]
    }

    pub(super) fn extend_from_slice(&mut self, samples: &[f32]) {
        self.compact_if_needed(samples.len());
        self.data.extend_from_slice(samples);
    }

    pub(super) fn consume(&mut self, len: usize) {
        debug_assert!(len <= self.len());
        self.start += len.min(self.len());
        if self.start == self.data.len() {
            self.clear();
        }
    }

    pub(super) fn clear(&mut self) {
        self.data.clear();
        self.start = 0;
    }

    pub(super) fn compact_if_needed(&mut self, incoming_len: usize) {
        if self.start == 0 {
            return;
        }
        if self.start == self.data.len() {
            self.clear();
            return;
        }

        let retained = self.len();
        let would_grow_past_consumed = retained + incoming_len > self.start;
        if self.start >= 8192 && would_grow_past_consumed {
            self.data.copy_within(self.start.., 0);
            self.data.truncate(retained);
            self.start = 0;
        }
    }
}
