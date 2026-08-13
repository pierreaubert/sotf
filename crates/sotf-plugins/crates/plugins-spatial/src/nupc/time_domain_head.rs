/// Direct time-domain FIR convolution for the first N taps of the IR.
///
/// Provides zero additional latency by processing samples immediately.
/// Used in combination with FFT-based partition levels for the IR tail.
use std::sync::Arc;

pub(super) struct TimeDomainHead {
    /// First N samples of the IR (reversed for direct convolution)
    pub(super) ir_taps: Arc<[f32]>,
    /// Circular input history buffer
    pub(super) history: Vec<f32>,
    /// Write position in history
    pub(super) pos: usize,
    /// Number of taps
    pub(super) n_taps: usize,
}

impl TimeDomainHead {
    pub(super) fn from_taps(ir_taps: Arc<[f32]>) -> Self {
        let n = ir_taps.len();
        Self {
            ir_taps,
            history: vec![0.0; n],
            pos: 0,
            n_taps: n,
        }
    }

    #[inline]
    pub(super) fn process_sample(&mut self, sample: f32) -> f32 {
        self.history[self.pos] = sample;
        let mut output = 0.0;
        let mut read = self.pos;
        for &tap in self.ir_taps.iter() {
            output += tap * self.history[read];
            if read == 0 {
                read = self.n_taps - 1;
            } else {
                read -= 1;
            }
        }
        self.pos = (self.pos + 1) % self.n_taps;
        output
    }

    pub(super) fn reset(&mut self) {
        self.history.fill(0.0);
        self.pos = 0;
    }
}
