/// Two-stage allpass diffuser for smearing transients before the FDN.
pub(super) struct AllpassDiffuser {
    pub(super) buffer: Vec<f32>,
    pub(super) write_pos: usize,
    pub(super) delay: usize,
    pub(super) feedback: f32,
}

impl AllpassDiffuser {
    pub(super) fn new(delay_samples: usize, feedback: f32) -> Self {
        Self {
            buffer: vec![0.0; delay_samples + 1],
            write_pos: 0,
            delay: delay_samples,
            feedback,
        }
    }

    #[inline]
    pub(super) fn process(&mut self, input: f32) -> f32 {
        let buf_len = self.buffer.len();
        let read_pos = (self.write_pos + buf_len - self.delay) % buf_len;
        let delayed = self.buffer[read_pos];
        // Schroeder allpass: y[n] = -g*x[n] + s[n-M], s[n] = x[n] + g*y[n]
        // where s is the buffer. DC gain = 1.0, |H(e^jw)| = 1 for all w.
        let output = -self.feedback * input + delayed;
        self.buffer[self.write_pos] = input + self.feedback * output;
        self.write_pos = (self.write_pos + 1) % buf_len;
        output
    }

    pub(super) fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.write_pos = 0;
    }
}
