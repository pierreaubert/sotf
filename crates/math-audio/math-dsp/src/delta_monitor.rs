// ============================================================================
// Delta Monitor — "Solo what's being removed/added"
// ============================================================================
//
// Computes the difference between the dry (input) and wet (processed) signal.
// Essential for spectral compressors and denoisers where you want to hear
// exactly what the processor is removing.
//
// HARD RULES:
// - No allocations
// - Zero-cost when disabled

/// Monitors the delta (difference) between input and output of a processor.
///
/// When enabled, replaces the output with `output - input`, letting the user
/// hear only what the processor changed.
#[derive(Debug, Clone, Copy)]
pub struct DeltaMonitor {
    enabled: bool,
}

impl Default for DeltaMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl DeltaMonitor {
    pub fn new() -> Self {
        Self { enabled: false }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    #[inline]
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// If enabled, replaces `output[i]` with `output[i] - input[i]` (the delta).
    /// If disabled, does nothing (zero cost — branch predicted away).
    ///
    /// Both slices must have the same length (total samples = frames * channels).
    #[inline]
    pub fn apply_if_enabled(&self, input: &[f32], output: &mut [f32]) {
        if !self.enabled {
            return;
        }
        debug_assert_eq!(input.len(), output.len());
        for (out, &inp) in output.iter_mut().zip(input.iter()) {
            *out -= inp;
        }
    }

    /// Convenience: capture the dry signal before processing, then call
    /// `apply_after` on the wet signal. This is for in-place processors
    /// that overwrite the input buffer.
    ///
    /// `dry_copy` must be filled by the caller before processing.
    #[inline]
    pub fn apply_from_copy(&self, dry_copy: &[f32], wet_buffer: &mut [f32]) {
        self.apply_if_enabled(dry_copy, wet_buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disabled_is_noop() {
        let monitor = DeltaMonitor::new();
        let input = [1.0, 2.0, 3.0];
        let mut output = [1.5, 2.5, 3.5];
        let output_copy = output;
        monitor.apply_if_enabled(&input, &mut output);
        assert_eq!(output, output_copy);
    }

    #[test]
    fn test_enabled_computes_delta() {
        let mut monitor = DeltaMonitor::new();
        monitor.set_enabled(true);
        let input = [1.0, 2.0, 3.0];
        let mut output = [1.5, 2.5, 3.5];
        monitor.apply_if_enabled(&input, &mut output);
        assert!((output[0] - 0.5).abs() < 1e-6);
        assert!((output[1] - 0.5).abs() < 1e-6);
        assert!((output[2] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_delta_of_identical_signals_is_zero() {
        let mut monitor = DeltaMonitor::new();
        monitor.set_enabled(true);
        let input = [1.0, -0.5, 0.3];
        let mut output = [1.0, -0.5, 0.3];
        monitor.apply_if_enabled(&input, &mut output);
        for &s in &output {
            assert!(s.abs() < 1e-6);
        }
    }

    #[test]
    fn test_default() {
        let monitor = DeltaMonitor::default();
        assert!(!monitor.enabled());
    }
}
