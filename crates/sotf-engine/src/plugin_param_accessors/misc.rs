use crate::plugins::PluginSettings;
use sotf_plugins::param_specs::{self};
use sotf_plugins::{SpectralTiltCorrection, TiltReferenceFreq};

pub(super) fn spectral_tilt_to_index(stc: &SpectralTiltCorrection) -> f64 {
    match stc {
        SpectralTiltCorrection::None => 0.0,
        SpectralTiltCorrection::ThreeDbPerOctave => 1.0,
        SpectralTiltCorrection::SixDbPerOctave => 2.0,
        SpectralTiltCorrection::Pink => 3.0,
        SpectralTiltCorrection::Custom(_) => 3.0,
    }
}

pub(super) fn tilt_reference_to_index(trf: &TiltReferenceFreq) -> f64 {
    match trf {
        TiltReferenceFreq::Standard => 0.0,
        TiltReferenceFreq::OneKilohertz => 1.0,
        TiltReferenceFreq::TwoKilohertz => 2.0,
        TiltReferenceFreq::MinFreq => 3.0,
    }
}

#[inline]
pub(super) fn b2f(b: bool) -> f64 {
    if b { 1.0 } else { 0.0 }
}

#[inline]
pub(super) fn f2b(f: f64) -> bool {
    f > 0.5
}

impl PluginSettings {
    /// Get the engine parameter key and value string for zero-dropout updates.
    ///
    /// Returns `None` for structural params, file paths, out-of-range indices,
    /// and plugins with no editable params. For plugins where PARAMS ordering
    /// matches the GPUI param index, this replaces the manual per-plugin mapping.
    pub fn engine_param_at(&self, idx: usize) -> Option<(String, String)> {
        let specs = self.param_specs();
        let spec = specs.get(idx)?;
        if spec.update_mode == param_specs::UpdateMode::Structural {
            return None;
        }
        if matches!(spec.param_type, param_specs::ParamType::FilePath) {
            return None;
        }
        let value = self.param_value_string(idx)?;
        Some((spec.engine_key.to_string(), value))
    }

    /// Format the current value of parameter at `index` as a string for engine communication.
    ///
    /// Unlike `param_value()` which returns f64, this returns the raw string value
    /// suitable for JSON serialization to the plugin engine. String-typed choices
    /// (speaker_config, crossover_type) are returned as their string values.
    pub fn param_value_string(&self, index: usize) -> Option<String> {
        let specs = self.param_specs();
        let spec = specs.get(index)?;

        match spec.param_type {
            param_specs::ParamType::FilePath => {
                const CONVOLUTION_IR_FILE_IDX: usize =
                    param_specs::index_of(param_specs::convolution::PARAMS, "ir_file");
                const BINAURAL_SOFA_FILE_IDX: usize =
                    param_specs::index_of(param_specs::binaural::PARAMS, "sofa_file");
                const AB_PATH_A_CONFIG_IDX: usize =
                    param_specs::index_of(param_specs::ab_compare::PARAMS, "path_a_config");
                const AB_PATH_B_CONFIG_IDX: usize =
                    param_specs::index_of(param_specs::ab_compare::PARAMS, "path_b_config");

                // Return the file path string directly. Indices are derived from
                // PARAMS so param reordering fails fast instead of drifting.
                match self {
                    Self::Convolution { ir_file, .. } if index == CONVOLUTION_IR_FILE_IDX => {
                        Some(ir_file.clone())
                    }
                    Self::BinauralDecoder { sofa_file, .. } if index == BINAURAL_SOFA_FILE_IDX => {
                        Some(sofa_file.clone())
                    }
                    Self::ABCompare { path_a_file, .. } if index == AB_PATH_A_CONFIG_IDX => {
                        Some(path_a_file.clone())
                    }
                    Self::ABCompare { path_b_file, .. } if index == AB_PATH_B_CONFIG_IDX => {
                        Some(path_b_file.clone())
                    }
                    _ => None,
                }
            }
            param_specs::ParamType::Bool { .. } => {
                self.param_value(index).map(|v| format!("{}", f2b(v)))
            }
            param_specs::ParamType::Choice { .. } => {
                const UPMIXER_SPEAKER_CONFIG_IDX: usize =
                    param_specs::index_of(param_specs::upmixer::PARAMS, "speaker_config");
                const AAE_SPEAKER_CONFIG_IDX: usize =
                    param_specs::index_of(param_specs::aae::PARAMS, "speaker_config");
                const AAE_ROOM_PRESET_IDX: usize =
                    param_specs::index_of(param_specs::aae::PARAMS, "room_preset");
                const AMBISONICS_TARGET_LAYOUT_IDX: usize =
                    param_specs::index_of(param_specs::ambisonics::PARAMS, "target_layout");
                const BAND_SPLIT_CROSSOVER_TYPE_IDX: usize =
                    param_specs::index_of(param_specs::band_split::PARAMS, "crossover_type");
                const CROSSFEED_MODE_IDX: usize =
                    param_specs::index_of(param_specs::crossfeed::PARAMS, "crossfeed_mode");
                const CROSSFEED_PRESET_IDX: usize =
                    param_specs::index_of(param_specs::crossfeed::PARAMS, "crossfeed_preset");
                const COMPRESSOR_SIDECHAIN_HPF_ORDER_IDX: usize =
                    param_specs::index_of(param_specs::compressor::PARAMS, "sidechain_hpf_order");
                const COMPRESSOR_DETECTION_MODE_IDX: usize =
                    param_specs::index_of(param_specs::compressor::PARAMS, "detection_mode");
                const EXPANDER_DETECTION_MODE_IDX: usize =
                    param_specs::index_of(param_specs::expander::PARAMS, "detection_mode");
                const MULTIBAND_EXPANDER_DETECTION_MODE_IDX: usize = param_specs::index_of(
                    param_specs::multiband_expander::GLOBAL_PARAMS,
                    "detection_mode",
                );

                // String-typed choices need special handling
                match self {
                    Self::Upmixer { speaker_config, .. } if index == UPMIXER_SPEAKER_CONFIG_IDX => {
                        Some(speaker_config.clone())
                    }
                    Self::AAE { speaker_config, .. } if index == AAE_SPEAKER_CONFIG_IDX => {
                        Some(speaker_config.clone())
                    }
                    Self::AAE { room_preset, .. } if index == AAE_ROOM_PRESET_IDX => {
                        Some(room_preset.clone())
                    }
                    Self::AmbisonicsDecoder { target_layout, .. }
                        if index == AMBISONICS_TARGET_LAYOUT_IDX =>
                    {
                        Some(target_layout.clone())
                    }
                    Self::BandSplit { crossover_type, .. }
                        if index == BAND_SPLIT_CROSSOVER_TYPE_IDX =>
                    {
                        Some(crossover_type.clone())
                    }
                    Self::Crossfeed { mode, .. } if index == CROSSFEED_MODE_IDX => Some(format!(
                        "{}",
                        serde_json::to_value(mode).unwrap_or_default()
                    )),
                    Self::Crossfeed { preset, .. } if index == CROSSFEED_PRESET_IDX => Some(
                        format!("{}", serde_json::to_value(preset).unwrap_or_default()),
                    ),
                    Self::Compressor {
                        sidechain_hpf_order,
                        ..
                    } if index == COMPRESSOR_SIDECHAIN_HPF_ORDER_IDX => {
                        Some(sidechain_hpf_order.clone())
                    }
                    Self::Compressor { detection_mode, .. }
                        if index == COMPRESSOR_DETECTION_MODE_IDX =>
                    {
                        Some(detection_mode.clone())
                    }
                    Self::Expander { detection_mode, .. }
                        if index == EXPANDER_DETECTION_MODE_IDX =>
                    {
                        Some(detection_mode.clone())
                    }
                    Self::MultibandExpander { detection_mode, .. }
                        if index == MULTIBAND_EXPANDER_DETECTION_MODE_IDX =>
                    {
                        Some(detection_mode.clone())
                    }
                    _ => {
                        // Numeric choice: format as integer
                        self.param_value(index).map(|v| format!("{}", v as i64))
                    }
                }
            }
            param_specs::ParamType::Int { .. } => {
                self.param_value(index).map(|v| format!("{}", v as i64))
            }
            param_specs::ParamType::Float { .. } => {
                self.param_value(index).map(|v| spec.engine_value_string(v))
            }
        }
    }
}
