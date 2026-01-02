//! EQ Plugin Parameters
//!
//! Defines all parameters for the 4-band parametric EQ using plinth-plugin's parameter system.

use plinth_derive::ParameterKind;
use plinth_plugin::{
    BoolParameter, FloatFormatter, FloatParameter, LinearFloatRange, LogFloatRange,
    Parameter, ParameterId, ParameterMap, Parameters,
};
use std::sync::Arc;

/// Number of EQ bands
pub const NUM_BANDS: usize = 4;

/// Filter types available for each band
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterType {
    #[default]
    Peak,
    LowShelf,
    HighShelf,
    LowPass,
    HighPass,
}

impl FilterType {
    pub fn name(&self) -> &'static str {
        match self {
            FilterType::Peak => "Peak",
            FilterType::LowShelf => "Low Shelf",
            FilterType::HighShelf => "High Shelf",
            FilterType::LowPass => "Low Pass",
            FilterType::HighPass => "High Pass",
        }
    }

    pub fn from_index(index: usize) -> Self {
        match index {
            0 => FilterType::Peak,
            1 => FilterType::LowShelf,
            2 => FilterType::HighShelf,
            3 => FilterType::LowPass,
            4 => FilterType::HighPass,
            _ => FilterType::Peak,
        }
    }

    pub fn to_index(&self) -> usize {
        match self {
            FilterType::Peak => 0,
            FilterType::LowShelf => 1,
            FilterType::HighShelf => 2,
            FilterType::LowPass => 3,
            FilterType::HighPass => 4,
        }
    }
}

/// Parameter identifiers for the EQ plugin
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ParameterKind)]
pub enum EqParameter {
    // Band 1
    Band1Enabled,
    Band1Type,
    Band1Frequency,
    Band1Q,
    Band1Gain,

    // Band 2
    Band2Enabled,
    Band2Type,
    Band2Frequency,
    Band2Q,
    Band2Gain,

    // Band 3
    Band3Enabled,
    Band3Type,
    Band3Frequency,
    Band3Q,
    Band3Gain,

    // Band 4
    Band4Enabled,
    Band4Type,
    Band4Frequency,
    Band4Q,
    Band4Gain,

    // Output
    OutputGain,
}

impl EqParameter {
    /// Get band parameters by index
    pub fn band_enabled(band: usize) -> Self {
        match band {
            0 => EqParameter::Band1Enabled,
            1 => EqParameter::Band2Enabled,
            2 => EqParameter::Band3Enabled,
            _ => EqParameter::Band4Enabled,
        }
    }

    pub fn band_type(band: usize) -> Self {
        match band {
            0 => EqParameter::Band1Type,
            1 => EqParameter::Band2Type,
            2 => EqParameter::Band3Type,
            _ => EqParameter::Band4Type,
        }
    }

    pub fn band_frequency(band: usize) -> Self {
        match band {
            0 => EqParameter::Band1Frequency,
            1 => EqParameter::Band2Frequency,
            2 => EqParameter::Band3Frequency,
            _ => EqParameter::Band4Frequency,
        }
    }

    pub fn band_q(band: usize) -> Self {
        match band {
            0 => EqParameter::Band1Q,
            1 => EqParameter::Band2Q,
            2 => EqParameter::Band3Q,
            _ => EqParameter::Band4Q,
        }
    }

    pub fn band_gain(band: usize) -> Self {
        match band {
            0 => EqParameter::Band1Gain,
            1 => EqParameter::Band2Gain,
            2 => EqParameter::Band3Gain,
            _ => EqParameter::Band4Gain,
        }
    }
}

/// EQ plugin parameters container
pub struct EqParameters {
    map: ParameterMap,
}

impl Default for EqParameters {
    fn default() -> Self {
        let mut map = ParameterMap::new();

        // Default frequencies for 4 bands: 100Hz, 500Hz, 2kHz, 8kHz
        let default_freqs = [100.0, 500.0, 2000.0, 8000.0];

        // Shared ranges (wrapped in Arc for reuse)
        // LogFloatRange::new takes (min, max, k) where k is the logarithmic exponent (k > 1.0)
        let type_range = Arc::new(LinearFloatRange::new(0.0, 4.0));
        let freq_range = Arc::new(LogFloatRange::new(20.0, 20000.0, 10.0));
        let q_range = Arc::new(LogFloatRange::new(0.1, 10.0, 10.0));
        let gain_range = Arc::new(LinearFloatRange::new(-24.0, 24.0));

        // Shared formatters
        let type_formatter = Arc::new(FloatFormatter::new(0, ""));
        let freq_formatter = Arc::new(FloatFormatter::new(0, " Hz"));
        let q_formatter = Arc::new(FloatFormatter::new(2, ""));
        let gain_formatter = Arc::new(FloatFormatter::new(1, " dB"));

        // Add parameters for each band
        for band in 0..NUM_BANDS {
            let prefix = format!("Band {}", band + 1);

            // Enabled
            map.add(
                BoolParameter::new(EqParameter::band_enabled(band), format!("{} Enabled", prefix))
                    .with_default_value(true),
            );

            // Filter type (stored as float, 0-4 for 5 types)
            map.add(
                FloatParameter::new(
                    EqParameter::band_type(band),
                    format!("{} Type", prefix),
                    type_range.clone(),
                )
                .with_default_value(0.0)
                .with_formatter(type_formatter.clone()),
            );

            // Frequency (20Hz - 20kHz, logarithmic)
            map.add(
                FloatParameter::new(
                    EqParameter::band_frequency(band),
                    format!("{} Frequency", prefix),
                    freq_range.clone(),
                )
                .with_default_value(default_freqs[band])
                .with_formatter(freq_formatter.clone()),
            );

            // Q (0.1 - 10, logarithmic)
            map.add(
                FloatParameter::new(
                    EqParameter::band_q(band),
                    format!("{} Q", prefix),
                    q_range.clone(),
                )
                .with_default_value(1.0)
                .with_formatter(q_formatter.clone()),
            );

            // Gain (-24 to +24 dB)
            map.add(
                FloatParameter::new(
                    EqParameter::band_gain(band),
                    format!("{} Gain", prefix),
                    gain_range.clone(),
                )
                .with_default_value(0.0)
                .with_formatter(gain_formatter.clone()),
            );
        }

        // Output gain
        map.add(
            FloatParameter::new(EqParameter::OutputGain, "Output Gain", gain_range)
                .with_default_value(0.0)
                .with_formatter(gain_formatter),
        );

        Self { map }
    }
}

impl EqParameters {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Get band filter type
    pub fn band_filter_type(&self, band: usize) -> FilterType {
        let type_value: f64 = self.map.value::<FloatParameter>(EqParameter::band_type(band));
        FilterType::from_index(type_value.round() as usize)
    }

    /// Get band frequency
    pub fn band_frequency(&self, band: usize) -> f64 {
        self.map.value::<FloatParameter>(EqParameter::band_frequency(band))
    }

    /// Get band Q
    pub fn band_q(&self, band: usize) -> f64 {
        self.map.value::<FloatParameter>(EqParameter::band_q(band))
    }

    /// Get band gain in dB
    pub fn band_gain(&self, band: usize) -> f64 {
        self.map.value::<FloatParameter>(EqParameter::band_gain(band))
    }

    /// Get band enabled
    pub fn band_enabled(&self, band: usize) -> bool {
        self.map.value::<BoolParameter>(EqParameter::band_enabled(band))
    }

    /// Get output gain in dB
    pub fn output_gain(&self) -> f64 {
        self.map.value::<FloatParameter>(EqParameter::OutputGain)
    }
}

impl Parameters for EqParameters {
    fn ids(&self) -> &[ParameterId] {
        self.map.ids()
    }

    fn get(&self, id: impl Into<ParameterId>) -> Option<&dyn Parameter> {
        self.map.get(id)
    }
}
