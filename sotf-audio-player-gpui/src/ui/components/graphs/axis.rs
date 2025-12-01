//! Axis management for frequency/SPL graphs
//!
//! Provides logarithmic frequency axis and linear SPL axis calculations.

/// Frequency axis configuration
#[derive(Debug, Clone, Copy)]
pub struct FrequencyAxis {
    pub min_hz: f64,
    pub max_hz: f64,
}

impl Default for FrequencyAxis {
    fn default() -> Self {
        Self {
            min_hz: 20.0,
            max_hz: 20000.0,
        }
    }
}

impl FrequencyAxis {
    /// Create a new frequency axis with custom range
    pub fn new(min_hz: f64, max_hz: f64) -> Self {
        Self { min_hz, max_hz }
    }

    /// Convert frequency to normalized position (0.0 to 1.0) using logarithmic scale
    pub fn freq_to_normalized(&self, freq: f64) -> f64 {
        let log_min = self.min_hz.ln();
        let log_max = self.max_hz.ln();
        let log_freq = freq.clamp(self.min_hz, self.max_hz).ln();
        ((log_freq - log_min) / (log_max - log_min)).clamp(0.0, 1.0)
    }

    /// Convert normalized position (0.0 to 1.0) back to frequency
    pub fn normalized_to_freq(&self, normalized: f64) -> f64 {
        let log_min = self.min_hz.ln();
        let log_max = self.max_hz.ln();
        let log_freq = log_min + normalized.clamp(0.0, 1.0) * (log_max - log_min);
        log_freq.exp()
    }

    /// Get the standard frequency tick values for audio graphs
    /// Returns: 20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000
    pub fn tick_frequencies(&self) -> Vec<f64> {
        vec![
            20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0,
        ]
        .into_iter()
        .filter(|&f| f >= self.min_hz && f <= self.max_hz)
        .collect()
    }

    /// Get normalized positions for tick marks
    pub fn tick_positions(&self) -> Vec<(f64, f64)> {
        self.tick_frequencies()
            .into_iter()
            .map(|freq| (freq, self.freq_to_normalized(freq)))
            .collect()
    }
}

/// SPL (dB) axis configuration
#[derive(Debug, Clone, Copy)]
pub struct SplAxis {
    pub min_db: f64,
    pub max_db: f64,
}

impl Default for SplAxis {
    fn default() -> Self {
        Self {
            min_db: -24.0,
            max_db: 24.0,
        }
    }
}

impl SplAxis {
    /// Create a new SPL axis with custom range
    pub fn new(min_db: f64, max_db: f64) -> Self {
        Self { min_db, max_db }
    }

    /// Convert dB value to normalized position (0.0 to 1.0) using linear scale
    /// Note: 0.0 is at the top (max_db), 1.0 is at the bottom (min_db)
    pub fn db_to_normalized(&self, db: f64) -> f64 {
        let db_clamped = db.clamp(self.min_db, self.max_db);
        ((self.max_db - db_clamped) / (self.max_db - self.min_db)).clamp(0.0, 1.0)
    }

    /// Convert normalized position (0.0 to 1.0) back to dB value
    pub fn normalized_to_db(&self, normalized: f64) -> f64 {
        self.max_db - normalized.clamp(0.0, 1.0) * (self.max_db - self.min_db)
    }

    /// Get the standard dB tick values based on range
    pub fn tick_values(&self) -> Vec<f64> {
        let range = self.max_db - self.min_db;
        let step = if range <= 24.0 {
            6.0
        } else if range <= 48.0 {
            12.0
        } else {
            24.0
        };

        let mut ticks = Vec::new();
        let mut db = (self.min_db / step).ceil() * step;
        while db <= self.max_db {
            ticks.push(db);
            db += step;
        }
        ticks
    }

    /// Get normalized positions for tick marks
    pub fn tick_positions(&self) -> Vec<(f64, f64)> {
        self.tick_values()
            .into_iter()
            .map(|db| (db, self.db_to_normalized(db)))
            .collect()
    }

    /// Get the normalized position for 0 dB reference line
    pub fn zero_db_position(&self) -> Option<f64> {
        if self.min_db <= 0.0 && self.max_db >= 0.0 {
            Some(self.db_to_normalized(0.0))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_freq_axis_log_scale() {
        let axis = FrequencyAxis::default();

        // 20 Hz should be at 0%
        assert!((axis.freq_to_normalized(20.0) - 0.0).abs() < 0.001);

        // 20 kHz should be at 100%
        assert!((axis.freq_to_normalized(20000.0) - 1.0).abs() < 0.001);

        // 632 Hz (geometric mean) should be near 50%
        let mid_freq = (20.0_f64 * 20000.0).sqrt();
        let mid_pos = axis.freq_to_normalized(mid_freq);
        assert!((mid_pos - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_spl_axis_linear_scale() {
        let axis = SplAxis::default();

        // +24 dB should be at 0% (top)
        assert!((axis.db_to_normalized(24.0) - 0.0).abs() < 0.001);

        // -24 dB should be at 100% (bottom)
        assert!((axis.db_to_normalized(-24.0) - 1.0).abs() < 0.001);

        // 0 dB should be at 50%
        assert!((axis.db_to_normalized(0.0) - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_roundtrip_conversions() {
        let freq_axis = FrequencyAxis::default();
        let spl_axis = SplAxis::default();

        // Frequency roundtrip
        for freq in [20.0, 100.0, 1000.0, 10000.0, 20000.0] {
            let normalized = freq_axis.freq_to_normalized(freq);
            let back = freq_axis.normalized_to_freq(normalized);
            assert!((freq - back).abs() < 0.1);
        }

        // SPL roundtrip
        for db in [-24.0, -12.0, 0.0, 12.0, 24.0] {
            let normalized = spl_axis.db_to_normalized(db);
            let back = spl_axis.normalized_to_db(normalized);
            assert!((db - back).abs() < 0.001);
        }
    }
}
