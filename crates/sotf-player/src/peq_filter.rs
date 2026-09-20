//! Canonical PEQ filter record shared by the headphone and Spinorama EQ
//! flows.
//!
//! This is the 4-field JSON-shaped filter record that autoeq emits on its
//! per-channel DSP chain output. The shape matches what
//! `math_audio_iir_fir::BiquadFilterType::long_name()` produces for
//! `filter_type`, with the `freq` / `db_gain` field names used by the
//! autoeq JSON format.
//!
//! Previously, `headphone_eq_types::HeadphoneEqBiquad` and
//! `spinorama_eq_types::SpinoramaBiquad` were two separate struct
//! definitions with identical fields, identical derives, and identical
//! serde representation — duplicate code with no type-system benefit.
//! They are now both type aliases for this one struct.
//!
//! `room_eq_types::EqFilterConfig` uses a different naming convention
//! (`frequency` / `gain_db` instead of `freq` / `db_gain`) and is not
//! aliased here; it needs its own migration.

use serde::{Deserialize, Serialize};

/// A single PEQ filter record in the shape autoeq emits.
///
/// Fields follow the `freq` / `db_gain` convention used by autoeq's
/// optimizer JSON output and by `math_audio_iir_fir::Biquad`. The
/// `filter_type` is a string produced by
/// `BiquadFilterType::long_name()` (`"Peak"`, `"Lowshelf"`, …).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeqFilter {
    /// Filter type in long-name form (e.g. `"Peak"`, `"Lowshelf"`).
    pub filter_type: String,
    /// Centre frequency in Hz.
    pub freq: f64,
    /// Quality factor.
    pub q: f64,
    /// Gain in dB (used by Peak, Lowshelf, Highshelf).
    pub db_gain: f64,
}

impl PeqFilter {
    /// Decode optimizer names and conventional PEQ abbreviations consistently
    /// for playback and export. Unknown legacy names retain the Peak default.
    pub fn biquad_filter_type(&self) -> math_audio_iir_fir::BiquadFilterType {
        use math_audio_iir_fir::BiquadFilterType::*;
        match self.filter_type.to_ascii_lowercase().as_str() {
            "ls" | "lowshelf" => Lowshelf,
            "hs" | "highshelf" => Highshelf,
            "lp" | "lowpass" => Lowpass,
            "hp" | "highpass" => Highpass,
            "bp" | "bandpass" => Bandpass,
            "no" | "notch" => Notch,
            "ap" | "allpass" => AllPass,
            _ => Peak,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optimizer_filter_names_preserve_response_types() {
        use math_audio_iir_fir::BiquadFilterType::*;
        for filter_type in [
            Peak, Lowshelf, Highshelf, Lowpass, Highpass, Bandpass, Notch, AllPass,
        ] {
            for name in [
                filter_type.long_name().to_string(),
                filter_type.long_name().to_lowercase(),
            ] {
                let filter = PeqFilter {
                    filter_type: name,
                    freq: 1000.0,
                    q: 0.7,
                    db_gain: 3.0,
                };
                assert_eq!(filter.biquad_filter_type(), filter_type);
            }
        }
    }
}
