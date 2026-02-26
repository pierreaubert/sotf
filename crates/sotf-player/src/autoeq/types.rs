//! AutoEQ Types
//!
//! Shared type definitions for speaker EQ optimization.

use serde::{Deserialize, Serialize};
use std::str::FromStr;

// ============================================================================
// Crossover Types
// ============================================================================

/// Crossover filter type for multi-driver speakers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CrossoverType {
    /// Butterworth 12 dB/octave (2nd order)
    Butterworth12,
    /// Linkwitz-Riley 12 dB/octave (2nd order)
    LR12,
    /// Linkwitz-Riley 24 dB/octave (4th order) - most common
    #[default]
    LR24,
    /// Linkwitz-Riley 48 dB/octave (8th order)
    LR48,
}

impl CrossoverType {
    /// Convert to plugin-compatible string
    pub fn to_plugin_string(&self) -> &'static str {
        match self {
            CrossoverType::Butterworth12 => "Butterworth12",
            CrossoverType::LR12 => "LR12",
            CrossoverType::LR24 => "LR24",
            CrossoverType::LR48 => "LR48",
        }
    }
}

impl FromStr for CrossoverType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "butterworth12" | "bw12" | "butterworth2" => Ok(CrossoverType::Butterworth12),
            "lr12" | "linkwitzriley12" | "linkwitzriley2" => Ok(CrossoverType::LR12),
            "lr24" | "linkwitzriley24" | "linkwitzriley4" => Ok(CrossoverType::LR24),
            "lr48" | "linkwitzriley48" | "linkwitzriley8" => Ok(CrossoverType::LR48),
            _ => Err(format!("Unknown crossover type: {}", s)),
        }
    }
}

// ============================================================================
// Speaker Configuration
// ============================================================================

/// Speaker configuration type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SpeakerConfigType {
    /// Single measurement (simple speaker)
    #[default]
    Single,
    /// Multiple drivers with crossover
    MultiDriver,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crossover_type_conversion() {
        let ct = CrossoverType::LR24;
        assert_eq!(ct.to_plugin_string(), "LR24");
        assert_eq!(
            "lr24".parse::<CrossoverType>().ok(),
            Some(CrossoverType::LR24)
        );
    }
}
