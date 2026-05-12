//! EQ filter configuration and APO format parsing

use math_audio_iir_fir::{Biquad, BiquadFilterType};
use serde::{Deserialize, Serialize};

/// Configuration for a single EQ filter (biquad)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EQFilter {
    pub filter_type: BiquadFilterType,
    pub frequency: f64,
    pub q: f64,
    pub gain_db: f64,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub solo: bool,
}

impl EQFilter {
    pub fn new(filter_type: BiquadFilterType, frequency: f64, q: f64, gain_db: f64) -> Self {
        Self {
            filter_type,
            frequency,
            q,
            gain_db,
            muted: false,
            solo: false,
        }
    }

    pub fn to_biquad(&self, sample_rate: f64) -> Biquad {
        let frequency = if self.frequency.is_finite() && self.frequency > 0.0 {
            let nyquist = sample_rate * 0.5;
            self.frequency.min(nyquist * 0.999)
        } else {
            log::warn!(
                "Invalid EQ frequency {}; falling back to 1000 Hz",
                self.frequency
            );
            1000.0
        };
        let q = if self.q.is_finite() && self.q > 0.0 {
            self.q
        } else {
            log::warn!("Invalid EQ Q {}; falling back to 0.707", self.q);
            0.707
        };
        let gain_db = if self.gain_db.is_finite() {
            self.gain_db
        } else {
            log::warn!("Invalid EQ gain {}; falling back to 0 dB", self.gain_db);
            0.0
        };

        Biquad::new(self.filter_type, frequency, sample_rate, q, gain_db)
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.frequency.is_finite() || self.frequency <= 0.0 {
            return Err(format!("Invalid EQ frequency: {}", self.frequency));
        }
        if !self.q.is_finite() || self.q <= 0.0 {
            return Err(format!("Invalid EQ Q: {}", self.q));
        }
        if !self.gain_db.is_finite() {
            return Err(format!("Invalid EQ gain: {}", self.gain_db));
        }
        Ok(())
    }

    /// Parse a single APO filter line
    /// Format: "Filter N: ON FILTERTYPE Fc FREQ Hz Gain GAIN dB Q QVAL"
    /// Example: "Filter 1: ON PK Fc 100 Hz Gain -2.0 dB Q 1.41"
    pub fn from_apo_line(line: &str) -> Result<Self, String> {
        let line = line.trim();

        // Skip if filter is OFF
        if line.contains("OFF") {
            return Err("Filter is disabled".to_string());
        }

        // Parse filter type
        let filter_type = if line.contains(" PK ") || line.contains(" PEQ ") {
            BiquadFilterType::Peak
        } else if line.contains(" LSC ") || line.contains(" LOW_SHELF ") || line.contains(" LS ") {
            BiquadFilterType::Lowshelf
        } else if line.contains(" HSC ") || line.contains(" HIGH_SHELF ") || line.contains(" HS ") {
            BiquadFilterType::Highshelf
        } else if line.contains(" LP ") || line.contains(" LPQ ") {
            BiquadFilterType::Lowpass
        } else if line.contains(" HP ") || line.contains(" HPQ ") {
            BiquadFilterType::Highpass
        } else if line.contains(" NO ") || line.contains(" NOTCH ") {
            BiquadFilterType::Notch
        } else if line.contains(" BP ") {
            BiquadFilterType::Bandpass
        } else {
            return Err(format!("Unknown filter type in line: {}", line));
        };

        // Parse frequency (look for "Fc" followed by number)
        let frequency = line
            .split_whitespace()
            .skip_while(|&s| s != "Fc")
            .nth(1)
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or_else(|| format!("Could not parse frequency from line: {}", line))?;

        // Parse gain (look for "Gain" followed by number)
        let gain_db = line
            .split_whitespace()
            .skip_while(|&s| s != "Gain")
            .nth(1)
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0); // Default to 0 dB if not found (for LP/HP/BP/NO filters)

        // Parse Q (look for "Q" followed by number)
        let q = line
            .split_whitespace()
            .skip_while(|&s| s != "Q")
            .nth(1)
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.707); // Default Q value

        let filter = Self::new(filter_type, frequency, q, gain_db);
        filter.validate()?;
        Ok(filter)
    }

    /// Parse APO format file and return a vector of EQ filters
    /// Format:
    /// ```text
    /// Preamp: -6.0 dB
    /// Filter 1: ON PK Fc 100 Hz Gain -2.0 dB Q 1.41
    /// Filter 2: ON LSC Fc 105 Hz Gain 4.1 dB Q 0.71
    /// ```
    pub fn from_apo_file(path: &std::path::Path) -> Result<Vec<Self>, String> {
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;

        let mut filters = Vec::new();

        for line in content.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }

            // Skip preamp lines for now
            if line.to_lowercase().starts_with("preamp:") {
                continue;
            }

            // Try to parse as filter line
            if line.to_lowercase().contains("filter") && line.contains(':') {
                match Self::from_apo_line(line) {
                    Ok(filter) => filters.push(filter),
                    Err(e) => log::warn!("Skipping line '{}': {}", line, e),
                }
            }
        }

        if filters.is_empty() {
            Err("No valid filters found in APO file".to_string())
        } else {
            Ok(filters)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apo_parser_rejects_invalid_frequency_and_q() {
        assert!(EQFilter::from_apo_line("Filter 1: ON PK Fc 0 Hz Gain 1 dB Q 1").is_err());
        assert!(EQFilter::from_apo_line("Filter 1: ON PK Fc 100 Hz Gain 1 dB Q 0").is_err());
        assert!(EQFilter::from_apo_line("Filter 1: ON PK Fc NaN Hz Gain 1 dB Q 1").is_err());
    }

    #[test]
    fn to_biquad_sanitizes_invalid_direct_values() {
        let filter = EQFilter::new(BiquadFilterType::Peak, -10.0, 0.0, f64::NAN);
        let biquad = filter.to_biquad(48_000.0);

        assert!(biquad.freq.is_finite());
        assert!(biquad.freq > 0.0);
        assert!(biquad.q.is_finite());
        assert!(biquad.q > 0.0);
        assert_eq!(biquad.db_gain, 0.0);
    }
}
