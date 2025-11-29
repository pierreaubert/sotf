//! Linear scale implementation

use super::{Scale, generate_linear_ticks};

/// A linear scale maps a continuous domain to a continuous range using linear interpolation
///
/// # Example
///
/// ```
/// use d3rs::scale::{LinearScale, Scale};
///
/// let scale = LinearScale::new()
///     .domain(0.0, 100.0)
///     .range(0.0, 500.0);
///
/// assert_eq!(scale.scale(0.0), 0.0);
/// assert_eq!(scale.scale(50.0), 250.0);
/// assert_eq!(scale.scale(100.0), 500.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearScale {
    domain_min: f64,
    domain_max: f64,
    range_min: f64,
    range_max: f64,
}

impl Default for LinearScale {
    fn default() -> Self {
        Self::new()
    }
}

impl LinearScale {
    /// Create a new linear scale with default domain [0, 1] and range [0, 1]
    ///
    /// # Example
    ///
    /// ```
    /// use d3rs::scale::LinearScale;
    ///
    /// let scale = LinearScale::new();
    /// ```
    pub fn new() -> Self {
        Self {
            domain_min: 0.0,
            domain_max: 1.0,
            range_min: 0.0,
            range_max: 1.0,
        }
    }

    /// Set the domain (input extent)
    ///
    /// # Example
    ///
    /// ```
    /// use d3rs::scale::LinearScale;
    ///
    /// let scale = LinearScale::new().domain(0.0, 100.0);
    /// ```
    pub fn domain(mut self, min: f64, max: f64) -> Self {
        self.domain_min = min;
        self.domain_max = max;
        self
    }

    /// Set the range (output extent)
    ///
    /// # Example
    ///
    /// ```
    /// use d3rs::scale::LinearScale;
    ///
    /// let scale = LinearScale::new().range(0.0, 500.0);
    /// ```
    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.range_min = min;
        self.range_max = max;
        self
    }

    /// Convenience method to set range from 0.0 to max (for normalized coordinates)
    ///
    /// # Example
    ///
    /// ```
    /// use d3rs::scale::LinearScale;
    ///
    /// let scale = LinearScale::new().range_normalized(1.0);
    /// ```
    pub fn range_normalized(self, max: f64) -> Self {
        self.range(0.0, max)
    }

    /// Clamp values to the domain
    ///
    /// When enabled, values outside the domain will be clamped to the domain extent.
    /// When disabled (default), extrapolation occurs for out-of-domain values.
    pub fn clamp(self, _enabled: bool) -> Self {
        // TODO: Implement clamping flag
        self
    }
}

impl Scale<f64, f64> for LinearScale {
    fn scale(&self, value: f64) -> f64 {
        let t = (value - self.domain_min) / (self.domain_max - self.domain_min);
        self.range_min + t * (self.range_max - self.range_min)
    }

    fn invert(&self, value: f64) -> Option<f64> {
        let t = (value - self.range_min) / (self.range_max - self.range_min);
        Some(self.domain_min + t * (self.domain_max - self.domain_min))
    }

    fn ticks(&self, count: usize) -> Vec<f64> {
        generate_linear_ticks(self.domain_min, self.domain_max, count)
    }

    fn domain(&self) -> (f64, f64) {
        (self.domain_min, self.domain_max)
    }

    fn range(&self) -> (f64, f64) {
        (self.range_min, self.range_max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_linear_scale_basic() {
        let scale = LinearScale::new()
            .domain(0.0, 100.0)
            .range(0.0, 500.0);

        assert_relative_eq!(scale.scale(0.0), 0.0);
        assert_relative_eq!(scale.scale(50.0), 250.0);
        assert_relative_eq!(scale.scale(100.0), 500.0);
    }

    #[test]
    fn test_linear_scale_inverted_range() {
        let scale = LinearScale::new()
            .domain(0.0, 100.0)
            .range(500.0, 0.0); // Inverted

        assert_relative_eq!(scale.scale(0.0), 500.0);
        assert_relative_eq!(scale.scale(50.0), 250.0);
        assert_relative_eq!(scale.scale(100.0), 0.0);
    }

    #[test]
    fn test_linear_scale_invert() {
        let scale = LinearScale::new()
            .domain(0.0, 100.0)
            .range(0.0, 500.0);

        assert_relative_eq!(scale.invert(0.0).unwrap(), 0.0);
        assert_relative_eq!(scale.invert(250.0).unwrap(), 50.0);
        assert_relative_eq!(scale.invert(500.0).unwrap(), 100.0);
    }

    #[test]
    fn test_linear_scale_extrapolation() {
        let scale = LinearScale::new()
            .domain(0.0, 100.0)
            .range(0.0, 500.0);

        // Values outside domain should extrapolate
        assert_relative_eq!(scale.scale(-50.0), -250.0);
        assert_relative_eq!(scale.scale(150.0), 750.0);
    }

    #[test]
    fn test_linear_scale_negative_domain() {
        let scale = LinearScale::new()
            .domain(-100.0, 100.0)
            .range(0.0, 1.0);

        assert_relative_eq!(scale.scale(-100.0), 0.0);
        assert_relative_eq!(scale.scale(0.0), 0.5);
        assert_relative_eq!(scale.scale(100.0), 1.0);
    }

    #[test]
    fn test_linear_scale_normalized() {
        let scale = LinearScale::new()
            .domain(0.0, 100.0)
            .range_normalized(1.0);

        assert_relative_eq!(scale.scale(0.0), 0.0);
        assert_relative_eq!(scale.scale(50.0), 0.5);
        assert_relative_eq!(scale.scale(100.0), 1.0);
    }

    #[test]
    fn test_linear_scale_roundtrip() {
        let scale = LinearScale::new()
            .domain(0.0, 100.0)
            .range(0.0, 500.0);

        for value in [0.0, 25.0, 50.0, 75.0, 100.0] {
            let scaled = scale.scale(value);
            let inverted = scale.invert(scaled).unwrap();
            assert_relative_eq!(inverted, value, epsilon = 1e-10);
        }
    }
}
