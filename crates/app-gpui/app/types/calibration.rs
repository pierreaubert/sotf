/// Parsed microphone calibration data
#[derive(Debug, Clone, Default)]
pub struct CalibrationData {
    /// Frequency points in Hz
    pub frequencies: Vec<f64>,
    /// SPL deviation in dB (positive = mic reads louder)
    pub spl_db: Vec<f64>,
}

impl CalibrationData {
    /// Parse a calibration file from its contents
    pub fn parse(content: &str) -> Option<Self> {
        let mut frequencies = Vec::new();
        let mut spl_db = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            // Skip empty lines, comments, and headers
            if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
                continue;
            }
            // Skip header lines containing text like "frequency" or "hz"
            let lower = line.to_lowercase();
            if lower.contains("frequency") || lower.contains("spl") || lower.contains("hz") {
                continue;
            }

            // Split by comma, tab, or whitespace
            let parts: Vec<&str> = line
                .split([',', '\t', ' '])
                .filter(|s| !s.is_empty())
                .collect();

            if parts.len() >= 2
                && let (Ok(freq), Ok(spl)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>())
            {
                // Validate reasonable frequency range (1 Hz to 100 kHz)
                if freq > 0.0 && freq <= 100000.0 && spl.is_finite() {
                    frequencies.push(freq);
                    spl_db.push(spl);
                }
            }
        }

        if frequencies.is_empty() {
            None
        } else {
            Some(Self {
                frequencies,
                spl_db,
            })
        }
    }

    /// Check if calibration data is valid
    pub fn is_valid(&self) -> bool {
        !self.frequencies.is_empty() && self.frequencies.len() == self.spl_db.len()
    }
}
