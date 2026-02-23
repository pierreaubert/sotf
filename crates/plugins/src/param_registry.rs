// ============================================================================
// Common Parameter Registry
// ============================================================================
//
// This module provides pre-defined parameter definitions for common audio
// plugin parameters. Using these ensures consistency across plugins and
// reduces code duplication.
//
// # Example
// ```rust,ignore
// use sotf_plugins::param_registry::CommonParams;
// use sotf_plugins::Parameter;
// use super::parameters::ParameterImportance;
//
// impl MyPlugin {
//     fn parameters(&self) -> Vec<Parameter> {
//         vec![
//             CommonParams::gain(),
//             CommonParams::threshold(),
//             CommonParams::ratio(),
//         ]
//     }
// }
// ```

use super::parameters::{Parameter, ParameterImportance};

/// Common parameter definitions for audio plugins
pub struct CommonParams;

impl CommonParams {
    /// Gain parameter (-60 to +60 dB)
    pub fn gain() -> Parameter {
        Parameter::new_float("gain", "Gain", 0.0, -60.0, 60.0)
            .with_unit("dB")
            .with_group("Gain")
            .with_importance(ParameterImportance::Critical)
            .with_description("Input/output gain adjustment")
            .build()
    }

    /// Input gain parameter (-60 to +60 dB)
    pub fn input_gain() -> Parameter {
        Parameter::new_float("input_gain", "Input Gain", 0.0, -60.0, 60.0)
            .with_unit("dB")
            .with_group("Gain")
            .with_importance(ParameterImportance::Critical)
            .with_description("Input stage gain")
            .build()
    }

    /// Output gain parameter (-60 to +60 dB)
    pub fn output_gain() -> Parameter {
        Parameter::new_float("output_gain", "Output Gain", 0.0, -60.0, 60.0)
            .with_unit("dB")
            .with_group("Gain")
            .with_importance(ParameterImportance::Critical)
            .with_description("Output stage gain")
            .build()
    }

    /// Makeup gain parameter (0 to +24 dB)
    pub fn makeup_gain() -> Parameter {
        Parameter::new_float("makeup_gain", "Makeup Gain", 0.0, 0.0, 24.0)
            .with_unit("dB")
            .with_group("Gain")
            .with_importance(ParameterImportance::Useful)
            .with_description("Gain compensation after processing")
            .build()
    }

    /// Threshold parameter (-60 to 0 dB)
    pub fn threshold() -> Parameter {
        Parameter::new_float("threshold", "Threshold", -20.0, -60.0, 0.0)
            .with_unit("dB")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::Critical)
            .with_description("Level at which processing activates")
            .build()
    }

    /// Ratio parameter (1:1 to 20:1)
    pub fn ratio() -> Parameter {
        Parameter::new_float("ratio", "Ratio", 4.0, 1.0, 20.0)
            .with_unit(":1")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::Critical)
            .with_description("Compression ratio")
            .build()
    }

    /// Attack time parameter (0.01 to 100 ms)
    pub fn attack() -> Parameter {
        Parameter::new_float("attack", "Attack", 10.0, 0.01, 100.0)
            .with_unit("ms")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::Useful)
            .with_logarithmic(true)
            .with_description("Time to reach full compression")
            .build()
    }

    /// Release time parameter (10 to 1000 ms)
    pub fn release() -> Parameter {
        Parameter::new_float("release", "Release", 100.0, 10.0, 1000.0)
            .with_unit("ms")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::Useful)
            .with_logarithmic(true)
            .with_description("Time to return to normal after signal drops")
            .build()
    }

    /// Knee parameter (0 to 24 dB)
    pub fn knee() -> Parameter {
        Parameter::new_float("knee", "Knee", 6.0, 0.0, 24.0)
            .with_unit("dB")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::FineTuning)
            .with_description("Transition zone around threshold")
            .build()
    }

    /// Bypass parameter
    pub fn bypass() -> Parameter {
        Parameter::new_bool("bypass", "Bypass", false)
            .with_group("General")
            .with_importance(ParameterImportance::Critical)
            .with_description("Enable/disable processing")
            .build()
    }

    /// Mix/Dry-Wet parameter (0% to 100%)
    pub fn mix() -> Parameter {
        Parameter::new_float("mix", "Mix", 100.0, 0.0, 100.0)
            .with_unit("%")
            .with_group("Mix")
            .with_importance(ParameterImportance::Critical)
            .with_description("Blend between dry and wet signals")
            .build()
    }

    /// Dry/Wet parameter (0.0 to 1.0)
    pub fn dry_wet() -> Parameter {
        Parameter::new_float("dry_wet", "Dry/Wet", 1.0, 0.0, 1.0)
            .with_unit("")
            .with_group("Mix")
            .with_importance(ParameterImportance::Critical)
            .with_description("Blend between dry and wet signals")
            .build()
    }

    /// Frequency parameter (20 Hz to 20 kHz)
    pub fn frequency() -> Parameter {
        Parameter::new_float("frequency", "Frequency", 1000.0, 20.0, 20000.0)
            .with_unit("Hz")
            .with_group("Filter")
            .with_importance(ParameterImportance::Critical)
            .with_logarithmic(true)
            .with_description("Center or cutoff frequency")
            .build()
    }

    /// Q factor parameter (0.1 to 20)
    pub fn q() -> Parameter {
        Parameter::new_float("q", "Q", 1.0, 0.1, 20.0)
            .with_unit("")
            .with_group("Filter")
            .with_importance(ParameterImportance::Useful)
            .with_logarithmic(true)
            .with_description("Resonance/bandwidth")
            .build()
    }

    /// Gain in dB for EQ filters (-24 to +24 dB)
    pub fn gain_db() -> Parameter {
        Parameter::new_float("gain_db", "Gain", 0.0, -24.0, 24.0)
            .with_unit("dB")
            .with_group("Filter")
            .with_importance(ParameterImportance::Critical)
            .with_description("Filter gain in dB")
            .build()
    }

    /// Bandwidth in octaves (0.1 to 4.0)
    pub fn bandwidth() -> Parameter {
        Parameter::new_float("bandwidth", "Bandwidth", 1.0, 0.1, 4.0)
            .with_unit("oct")
            .with_group("Filter")
            .with_importance(ParameterImportance::FineTuning)
            .with_description("Bandwidth in octaves")
            .build()
    }

    /// Master gain for multi-band crossovers
    pub fn master_gain() -> Parameter {
        Parameter::new_float("master_gain", "Master", 0.0, -24.0, 24.0)
            .with_unit("dB")
            .with_group("Master")
            .with_importance(ParameterImportance::Critical)
            .with_description("Overall output level")
            .build()
    }

    /// Solo parameter for channels
    pub fn solo() -> Parameter {
        Parameter::new_bool("solo", "Solo", false)
            .with_group("Channel")
            .with_importance(ParameterImportance::Critical)
            .with_description("Solo this channel")
            .build()
    }

    /// Mute parameter for channels
    pub fn mute() -> Parameter {
        Parameter::new_bool("mute", "Mute", false)
            .with_group("Channel")
            .with_importance(ParameterImportance::Critical)
            .with_description("Mute this channel")
            .build()
    }

    /// Pan parameter (-1.0 to 1.0)
    pub fn pan() -> Parameter {
        Parameter::new_float("pan", "Pan", 0.0, -1.0, 1.0)
            .with_unit("")
            .with_group("Channel")
            .with_importance(ParameterImportance::Useful)
            .with_description("Stereo position (-1 = left, 0 = center, 1 = right)")
            .build()
    }

    /// Width parameter (0.0 to 1.0)
    pub fn width() -> Parameter {
        Parameter::new_float("width", "Width", 1.0, 0.0, 1.0)
            .with_unit("")
            .with_group("Stereo")
            .with_importance(ParameterImportance::Useful)
            .with_description("Stereo width (0 = mono, 1 = stereo)")
            .build()
    }

    /// Center parameter (0.0 to 1.0)
    pub fn center() -> Parameter {
        Parameter::new_float("center", "Center", 0.5, 0.0, 1.0)
            .with_unit("")
            .with_group("Stereo")
            .with_importance(ParameterImportance::FineTuning)
            .with_description("Center channel level")
            .build()
    }

    /// LFE gain parameter (0.0 to 2.0)
    pub fn lfe_gain() -> Parameter {
        Parameter::new_float("lfe_gain", "LFE Gain", 1.0, 0.0, 2.0)
            .with_unit("")
            .with_group("LFE")
            .with_importance(ParameterImportance::Useful)
            .with_description("LFE channel gain")
            .build()
    }

    /// LFE crossover frequency (20 to 120 Hz)
    pub fn lfe_crossover() -> Parameter {
        Parameter::new_float("lfe_crossover", "LFE Crossover", 80.0, 20.0, 120.0)
            .with_unit("Hz")
            .with_group("LFE")
            .with_importance(ParameterImportance::Useful)
            .with_description("LFE low-pass crossover frequency")
            .build()
    }

    /// High-pass frequency (20 to 500 Hz)
    pub fn highpass() -> Parameter {
        Parameter::new_float("highpass", "High-Pass", 20.0, 20.0, 500.0)
            .with_unit("Hz")
            .with_group("Filter")
            .with_importance(ParameterImportance::Useful)
            .with_logarithmic(true)
            .with_description("High-pass cutoff frequency")
            .build()
    }

    /// Low-pass frequency (1 kHz to 20 kHz)
    pub fn lowpass() -> Parameter {
        Parameter::new_float("lowpass", "Low-Pass", 20000.0, 1000.0, 20000.0)
            .with_unit("Hz")
            .with_group("Filter")
            .with_importance(ParameterImportance::Useful)
            .with_logarithmic(true)
            .with_description("Low-pass cutoff frequency")
            .build()
    }
}

/// EQ-specific parameter definitions
pub struct EqParams;

impl EqParams {
    /// Low shelf frequency (20 to 500 Hz)
    pub fn low_shelf_freq() -> Parameter {
        Parameter::new_float("low_shelf_freq", "Low Freq", 100.0, 20.0, 500.0)
            .with_unit("Hz")
            .with_group("Low Shelf")
            .with_importance(ParameterImportance::Useful)
            .with_logarithmic(true)
            .with_description("Low shelf center frequency")
            .build()
    }

    /// Low shelf gain (-24 to +24 dB)
    pub fn low_shelf_gain() -> Parameter {
        Parameter::new_float("low_shelf_gain", "Low Gain", 0.0, -24.0, 24.0)
            .with_unit("dB")
            .with_group("Low Shelf")
            .with_importance(ParameterImportance::Critical)
            .with_description("Low shelf gain")
            .build()
    }

    /// High shelf frequency (2 kHz to 20 kHz)
    pub fn high_shelf_freq() -> Parameter {
        Parameter::new_float("high_shelf_freq", "High Freq", 8000.0, 2000.0, 20000.0)
            .with_unit("Hz")
            .with_group("High Shelf")
            .with_importance(ParameterImportance::Useful)
            .with_logarithmic(true)
            .with_description("High shelf center frequency")
            .build()
    }

    /// High shelf gain (-24 to +24 dB)
    pub fn high_shelf_gain() -> Parameter {
        Parameter::new_float("high_shelf_gain", "High Gain", 0.0, -24.0, 24.0)
            .with_unit("dB")
            .with_group("High Shelf")
            .with_importance(ParameterImportance::Critical)
            .with_description("High shelf gain")
            .build()
    }

    /// Peaking filter frequency (20 Hz to 20 kHz)
    pub fn peak_freq() -> Parameter {
        Parameter::new_float("peak_freq", "Freq", 1000.0, 20.0, 20000.0)
            .with_unit("Hz")
            .with_group("Peak")
            .with_importance(ParameterImportance::Critical)
            .with_logarithmic(true)
            .with_description("Peaking filter center frequency")
            .build()
    }

    /// Peaking filter Q (0.1 to 20)
    pub fn peak_q() -> Parameter {
        Parameter::new_float("peak_q", "Q", 1.0, 0.1, 20.0)
            .with_unit("")
            .with_group("Peak")
            .with_importance(ParameterImportance::Useful)
            .with_logarithmic(true)
            .with_description("Peaking filter Q factor")
            .build()
    }

    /// Peaking filter gain (-24 to +24 dB)
    pub fn peak_gain() -> Parameter {
        Parameter::new_float("peak_gain", "Gain", 0.0, -24.0, 24.0)
            .with_unit("dB")
            .with_group("Peak")
            .with_importance(ParameterImportance::Critical)
            .with_description("Peaking filter gain")
            .build()
    }
}

/// Dynamics-specific parameter definitions
pub struct DynamicsParams;

impl DynamicsParams {
    /// Range parameter (0 to 60 dB)
    pub fn range() -> Parameter {
        Parameter::new_float("range", "Range", 60.0, 0.0, 60.0)
            .with_unit("dB")
            .with_group("Gate/Expander")
            .with_importance(ParameterImportance::Useful)
            .with_description("Maximum attenuation")
            .build()
    }

    /// Hysteresis parameter (0 to 6 dB)
    pub fn hysteresis() -> Parameter {
        Parameter::new_float("hysteresis", "Hysteresis", 2.0, 0.0, 6.0)
            .with_unit("dB")
            .with_group("Gate/Expander")
            .with_importance(ParameterImportance::FineTuning)
            .with_description("Threshold difference for attack/release")
            .build()
    }

    /// Hold time parameter (0 to 500 ms)
    pub fn hold() -> Parameter {
        Parameter::new_float("hold", "Hold", 50.0, 0.0, 500.0)
            .with_unit("ms")
            .with_group("Gate/Expander")
            .with_importance(ParameterImportance::FineTuning)
            .with_description("Time to hold after signal drops below threshold")
            .build()
    }

    /// Lookahead parameter (0 to 20 ms)
    pub fn lookahead() -> Parameter {
        Parameter::new_float("lookahead", "Lookahead", 0.0, 0.0, 20.0)
            .with_unit("ms")
            .with_group("Dynamics")
            .with_importance(ParameterImportance::FineTuning)
            .with_description("Lookahead time for attack")
            .build()
    }

    /// Auto make-up gain
    pub fn auto_gain() -> Parameter {
        Parameter::new_bool("auto_gain", "Auto Gain", false)
            .with_group("Gain")
            .with_importance(ParameterImportance::Useful)
            .with_description("Automatically compensate for gain reduction")
            .build()
    }

    /// Detector source (input/output/sidechain)
    pub fn detector_source() -> Parameter {
        Parameter::new_int("detector_source", "Detector", 0, 0, 2)
            .with_unit("")
            .with_group("Detector")
            .with_importance(ParameterImportance::FineTuning)
            .with_description("Detector source (0=Input, 1=Output, 2=Sidechain)")
            .build()
    }
}
