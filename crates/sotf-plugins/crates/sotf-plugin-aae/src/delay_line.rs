//! Interpolated delay line with allpass fractional-sample interpolation.
//!
//! Used by the FDN and early reflection generator. Supports time-variant
//! modulation of the delay length (Griesinger's key LARES innovation).

/// Circular buffer delay line with allpass interpolation for fractional delays.
///
/// Allpass interpolation preserves frequency response flatness (unlike linear
/// interpolation which rolls off high frequencies). This is critical for the FDN
/// where the delay line is in a feedback loop — any spectral coloring accumulates.
pub struct DelayLine {
    buffer: Vec<f32>,
    write_pos: usize,
    length: usize,
}

impl DelayLine {
    /// Create a delay line that supports reading up to `max_delay_samples` of delay.
    ///
    /// The internal buffer is sized `max_delay_samples + 2`:
    /// - +1 slot because `write_pos` always points to the *next* write position,
    ///   so the oldest readable sample lives at `write_pos` (one slot ahead).
    /// - +1 slot because `read_allpass` reads `int_delay` and `int_delay + 1`
    ///   simultaneously; without the extra slot the interpolation would clamp early.
    ///
    /// Callers can pass the exact maximum delay they need — no over-allocation required.
    pub fn new(max_delay_samples: usize) -> Self {
        let length = max_delay_samples + 2;
        Self {
            buffer: vec![0.0; length],
            write_pos: 0,
            length,
        }
    }

    /// Write one sample into the delay line and advance the write pointer.
    #[inline]
    pub fn push(&mut self, sample: f32) {
        self.buffer[self.write_pos] = sample;
        self.write_pos += 1;
        if self.write_pos >= self.length {
            self.write_pos = 0;
        }
    }

    /// Read at integer delay (no interpolation). `delay` is in samples.
    #[inline]
    pub fn read(&self, delay: usize) -> f32 {
        let delay = delay.min(self.max_delay_samples());
        let pos = if self.write_pos > delay {
            self.write_pos - delay - 1
        } else {
            self.length + self.write_pos - delay - 1
        };
        self.buffer[pos]
    }

    /// Maximum delay that can be read while still allowing interpolation to read `delay + 1`.
    #[inline]
    pub fn max_delay_samples(&self) -> usize {
        self.length.saturating_sub(2)
    }

    /// Read at fractional delay using allpass interpolation.
    ///
    /// `delay_samples` is the total delay including fractional part.
    /// The allpass interpolator: `y[n] = a*(x[n] - y[n-1]) + x[n-1]`
    /// where `a = (1 - frac) / (1 + frac)`.
    ///
    /// `state` is the one-sample allpass filter state, maintained by the caller
    /// to ensure continuity across calls (important for modulated delays).
    #[inline]
    pub fn read_allpass(&self, delay_samples: f32, state: &mut f32) -> f32 {
        let delay_samples = delay_samples.clamp(0.0, self.max_delay_samples() as f32);
        let int_delay = delay_samples as usize;
        let frac = delay_samples - int_delay as f32;

        let s0 = self.read(int_delay);
        let s1 = self.read(int_delay + 1);

        // Allpass coefficient
        let a = (1.0 - frac) / (1.0 + frac);

        let output = a * (s0 - *state) + s1;
        *state = output;
        output
    }

    /// Read at fractional delay using linear interpolation.
    /// Simpler but introduces low-pass filtering at high frequencies.
    #[inline]
    pub fn read_linear(&self, delay_samples: f32) -> f32 {
        let delay_samples = delay_samples.clamp(0.0, self.max_delay_samples() as f32);
        let int_delay = delay_samples as usize;
        let frac = delay_samples - int_delay as f32;
        let s0 = self.read(int_delay);
        let s1 = self.read(int_delay + 1);
        s0 + frac * (s1 - s0)
    }

    /// Clear the buffer.
    pub fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.write_pos = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `DelayLine::new(N)` must support reading exactly N samples of delay.
    #[test]
    fn test_max_delay_is_exactly_n() {
        // new(10) should be able to read at delay=10 without clamping.
        let mut dl = DelayLine::new(10);
        // Push 11 unique values so positions 0..10 all have distinct data
        for i in 1..=11 {
            dl.push(i as f32);
        }
        // The most-recently pushed value is 11.0 (delay=0).
        // Delay 10 should give 1.0.
        let v = dl.read(10);
        assert!(
            (v - 1.0).abs() < 1e-6,
            "delay=10 should give 1.0 (oldest), got {v}. max_delay_samples={}",
            dl.max_delay_samples()
        );
        assert_eq!(
            dl.max_delay_samples(),
            10,
            "max_delay_samples() should equal the value passed to new()"
        );
    }

    #[test]
    fn test_push_read_basic() {
        let mut dl = DelayLine::new(10);
        dl.push(1.0);
        dl.push(2.0);
        dl.push(3.0);
        // delay=0 reads the most recent sample
        assert!((dl.read(0) - 3.0).abs() < 1e-6);
        assert!((dl.read(1) - 2.0).abs() < 1e-6);
        assert!((dl.read(2) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_wrap_around() {
        let mut dl = DelayLine::new(4);
        for i in 0..10 {
            dl.push(i as f32);
        }
        assert!((dl.read(0) - 9.0).abs() < 1e-6);
        assert!((dl.read(1) - 8.0).abs() < 1e-6);
        assert!((dl.read(3) - 6.0).abs() < 1e-6);
    }

    #[test]
    fn test_linear_interpolation_midpoint() {
        let mut dl = DelayLine::new(10);
        dl.push(0.0);
        dl.push(1.0);
        // Between sample at delay=0 (1.0) and delay=1 (0.0): midpoint = 0.5
        let val = dl.read_linear(0.5);
        assert!((val - 0.5).abs() < 1e-6, "got {val}");
    }

    #[test]
    fn test_allpass_interpolation_at_integer() {
        let mut dl = DelayLine::new(10);
        dl.push(0.0);
        dl.push(0.0);
        dl.push(1.0);
        let mut state = 0.0;
        // At integer delay frac=0, allpass coeff = 1.0, reduces to pass-through
        let val = dl.read_allpass(1.0, &mut state);
        // Should be close to the sample at delay=1 = 0.0 (since state starts at 0)
        // Actually: a=1.0, output = 1*(read(1)-0) + read(2) = 0 + 0 = 0.0
        // read(1)=0.0 (pushed second), read(2)=0.0 (pushed first)
        // That's correct for delay=1 into [0, 0, 1]
        assert!(val.abs() < 1e-6, "got {val}");
    }

    #[test]
    fn test_reset() {
        let mut dl = DelayLine::new(10);
        dl.push(5.0);
        dl.push(10.0);
        dl.reset();
        assert!((dl.read(0)).abs() < 1e-6);
    }
}
