//! EQ Plugin Parameters
//!
//! Defines all parameters for the 4-band parametric EQ using nih-plug's parameter system.

use nih_plug::prelude::*;
use std::sync::Arc;

/// Number of EQ bands
pub const NUM_BANDS: usize = 4;

/// Filter types available for each band
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum FilterType {
    #[id = "peak"]
    #[name = "Peak"]
    Peak,

    #[id = "lowshelf"]
    #[name = "Low Shelf"]
    LowShelf,

    #[id = "highshelf"]
    #[name = "High Shelf"]
    HighShelf,

    #[id = "lowpass"]
    #[name = "Low Pass"]
    LowPass,

    #[id = "highpass"]
    #[name = "High Pass"]
    HighPass,
}

impl Default for FilterType {
    fn default() -> Self {
        Self::Peak
    }
}

/// Parameters for a single EQ band
#[derive(Params)]
pub struct EqBandParams {
    /// Filter type (peak, shelf, pass, etc.)
    #[id = "type"]
    pub filter_type: EnumParam<FilterType>,

    /// Center/corner frequency in Hz (20-20000, logarithmic)
    #[id = "freq"]
    pub frequency: FloatParam,

    /// Q factor / bandwidth (0.1-10)
    #[id = "q"]
    pub q: FloatParam,

    /// Gain in dB (-24 to +24)
    #[id = "gain"]
    pub gain_db: FloatParam,

    /// Band enabled/bypassed
    #[id = "enabled"]
    pub enabled: BoolParam,
}

impl EqBandParams {
    /// Create a new EQ band with default parameters for the given band index
    pub fn new(band_index: usize) -> Self {
        // Default frequencies for 4 bands: 100Hz, 500Hz, 2kHz, 8kHz
        let default_freq = match band_index {
            0 => 100.0,
            1 => 500.0,
            2 => 2000.0,
            3 => 8000.0,
            _ => 1000.0,
        };

        Self {
            filter_type: EnumParam::new("Type", FilterType::Peak),

            frequency: FloatParam::new(
                "Frequency",
                default_freq,
                FloatRange::Skewed {
                    min: 20.0,
                    max: 20000.0,
                    factor: FloatRange::skew_factor(-2.0), // Logarithmic
                },
            )
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_hz_then_khz(0)),

            q: FloatParam::new(
                "Q",
                1.0,
                FloatRange::Skewed {
                    min: 0.1,
                    max: 10.0,
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            .with_step_size(0.01),

            gain_db: FloatParam::new(
                "Gain",
                0.0,
                FloatRange::Linear {
                    min: -24.0,
                    max: 24.0,
                },
            )
            .with_unit(" dB")
            .with_step_size(0.1),

            enabled: BoolParam::new("Enabled", true),
        }
    }
}

impl Default for EqBandParams {
    fn default() -> Self {
        Self::new(0)
    }
}

/// Main EQ plugin parameters
#[derive(Params)]
pub struct SotfEqParams {
    /// Band 1 parameters
    #[nested(id_prefix = "band1", group = "Band 1")]
    pub band1: EqBandParams,

    /// Band 2 parameters
    #[nested(id_prefix = "band2", group = "Band 2")]
    pub band2: EqBandParams,

    /// Band 3 parameters
    #[nested(id_prefix = "band3", group = "Band 3")]
    pub band3: EqBandParams,

    /// Band 4 parameters
    #[nested(id_prefix = "band4", group = "Band 4")]
    pub band4: EqBandParams,

    /// Output gain in dB
    #[id = "output_gain"]
    pub output_gain: FloatParam,
}

impl SotfEqParams {
    /// Get band parameters by index (0-3)
    pub fn band(&self, index: usize) -> &EqBandParams {
        match index {
            0 => &self.band1,
            1 => &self.band2,
            2 => &self.band3,
            _ => &self.band4,
        }
    }
}

impl Default for SotfEqParams {
    fn default() -> Self {
        Self {
            band1: EqBandParams::new(0),
            band2: EqBandParams::new(1),
            band3: EqBandParams::new(2),
            band4: EqBandParams::new(3),
            output_gain: FloatParam::new(
                "Output Gain",
                0.0,
                FloatRange::Linear {
                    min: -24.0,
                    max: 24.0,
                },
            )
            .with_unit(" dB")
            .with_step_size(0.1),
        }
    }
}

/// Create the plugin parameters wrapped in Arc for thread-safe sharing
pub fn create_params() -> Arc<SotfEqParams> {
    Arc::new(SotfEqParams::default())
}
