// ============================================================================
// Head Position Smoother
// ============================================================================
//
// Applies exponential moving average (EMA) filtering to reduce jitter in
// head position data while maintaining responsiveness.

use crate::types::HeadPosition;

/// Smooths head position data using exponential moving average
#[derive(Debug)]
pub struct HeadPositionSmoother {
    /// Current smoothed position
    current: HeadPosition,

    /// Time constant for smoothing (seconds)
    time_constant: f32,

    /// Last update timestamp
    last_timestamp_ms: u64,

    /// Has been initialized with first sample
    initialized: bool,
}

impl HeadPositionSmoother {
    /// Create a new smoother with given time constant
    ///
    /// # Arguments
    /// * `time_constant` - Smoothing time constant in seconds.
    ///   - 0.05s = responsive but some jitter
    ///   - 0.1s = balanced (default)
    ///   - 0.2s = smooth but laggy
    pub fn new(time_constant: f32) -> Self {
        Self {
            current: HeadPosition::default(),
            time_constant: time_constant.max(0.001), // Prevent division by zero
            last_timestamp_ms: 0,
            initialized: false,
        }
    }

    /// Update with new position measurement
    ///
    /// Returns the smoothed position
    pub fn update(&mut self, new_pos: HeadPosition) -> HeadPosition {
        if !self.initialized {
            // First sample - accept directly
            self.current = new_pos;
            self.last_timestamp_ms = new_pos.timestamp_ms;
            self.initialized = true;
            return self.current;
        }

        // Calculate time delta
        let dt_ms = new_pos.timestamp_ms.saturating_sub(self.last_timestamp_ms);
        let dt_s = dt_ms as f32 / 1000.0;

        if dt_s <= 0.0 {
            // No time passed, return current
            return self.current;
        }

        // EMA coefficient: alpha = 1 - exp(-dt / tau)
        // Higher alpha = faster response to new values
        let alpha = 1.0 - (-dt_s / self.time_constant).exp();

        // Apply EMA to each component
        self.current = HeadPosition {
            x: self.current.x + alpha * (new_pos.x - self.current.x),
            y: self.current.y + alpha * (new_pos.y - self.current.y),
            z: self.current.z + alpha * (new_pos.z - self.current.z),
            yaw: self.current.yaw + alpha * (new_pos.yaw - self.current.yaw),
            pitch: self.current.pitch + alpha * (new_pos.pitch - self.current.pitch),
            roll: self.current.roll + alpha * (new_pos.roll - self.current.roll),
            timestamp_ms: new_pos.timestamp_ms,
            confidence: self.current.confidence + alpha * (new_pos.confidence - self.current.confidence),
        };

        self.last_timestamp_ms = new_pos.timestamp_ms;
        self.current
    }

    /// Get the current smoothed position without updating
    pub fn current(&self) -> HeadPosition {
        self.current
    }

    /// Reset the smoother state
    pub fn reset(&mut self) {
        self.current = HeadPosition::default();
        self.last_timestamp_ms = 0;
        self.initialized = false;
    }

    /// Set the smoothing time constant
    pub fn set_time_constant(&mut self, time_constant: f32) {
        self.time_constant = time_constant.max(0.001);
    }

    /// Check if smoother has been initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}

impl Default for HeadPositionSmoother {
    fn default() -> Self {
        Self::new(0.1) // 100ms time constant
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smoother_first_sample() {
        let mut smoother = HeadPositionSmoother::new(0.1);

        let pos = HeadPosition {
            x: 0.5,
            z: 0.3,
            timestamp_ms: 1000,
            confidence: 0.9,
            ..Default::default()
        };

        let result = smoother.update(pos);
        assert_eq!(result.x, 0.5);
        assert_eq!(result.z, 0.3);
    }

    #[test]
    fn test_smoother_convergence() {
        let mut smoother = HeadPositionSmoother::new(0.05);

        // Initialize at origin
        smoother.update(HeadPosition {
            timestamp_ms: 0,
            confidence: 1.0,
            ..Default::default()
        });

        // Move to x=1.0 over time
        let target = 1.0;
        let mut last_x = 0.0;

        for i in 1..=20 {
            let pos = HeadPosition {
                x: target,
                timestamp_ms: i * 33, // ~30fps
                confidence: 1.0,
                ..Default::default()
            };
            let result = smoother.update(pos);

            // Should be moving toward target
            assert!(result.x > last_x, "Should be increasing toward target");
            last_x = result.x;
        }

        // After 20 frames (~666ms), should be close to target
        assert!(last_x > 0.9, "Should have converged close to target");
    }

    #[test]
    fn test_smoother_jitter_reduction() {
        let mut smoother = HeadPositionSmoother::new(0.1);

        // Initialize
        smoother.update(HeadPosition {
            x: 0.5,
            timestamp_ms: 0,
            confidence: 1.0,
            ..Default::default()
        });

        // Apply jittery input around 0.5
        let jitter_values = [0.52, 0.48, 0.51, 0.49, 0.52, 0.48];
        let mut outputs = Vec::new();

        for (i, &x) in jitter_values.iter().enumerate() {
            let result = smoother.update(HeadPosition {
                x,
                timestamp_ms: (i + 1) as u64 * 33,
                confidence: 1.0,
                ..Default::default()
            });
            outputs.push(result.x);
        }

        // Output variance should be less than input variance
        let input_var = variance(&jitter_values);
        let output_var = variance(&outputs);
        assert!(
            output_var < input_var,
            "Output variance {} should be less than input variance {}",
            output_var,
            input_var
        );
    }

    fn variance(values: &[f32]) -> f32 {
        let mean = values.iter().sum::<f32>() / values.len() as f32;
        values.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / values.len() as f32
    }
}
