//! Parameter vector utilities for handling different PEQ models
//!
//! This module provides utilities for working with parameter vectors that may have
//! different layouts depending on the PEQ model being used.

use crate::cli::PeqModel;
use crate::iir::BiquadFilterType;

/// Get the number of parameters per filter for a given PEQ model
pub fn params_per_filter(peq_model: PeqModel) -> usize {
    match peq_model {
        // Fixed filter types use 3 parameters: freq, Q, gain
        PeqModel::Pk | PeqModel::HpPk | PeqModel::HpPkLp | PeqModel::LsPk | PeqModel::LsPkHs => 3,
        // Free filter types use 4 parameters: type, freq, Q, gain
        PeqModel::FreePkFree | PeqModel::Free => 4,
    }
}

/// Get the number of filters from a parameter vector
pub fn num_filters(x: &[f64], peq_model: PeqModel) -> usize {
    x.len() / params_per_filter(peq_model)
}

/// Extract filter parameters for the i-th filter
pub fn get_filter_params(x: &[f64], i: usize, peq_model: PeqModel) -> FilterParams {
    let ppf = params_per_filter(peq_model);
    let offset = i * ppf;

    match peq_model {
        PeqModel::Pk | PeqModel::HpPk | PeqModel::HpPkLp | PeqModel::LsPk | PeqModel::LsPkHs => {
            // Fixed filter types: parameters are [freq, Q, gain]
            FilterParams {
                filter_type: None,
                freq: x[offset],
                q: x[offset + 1],
                gain: x[offset + 2],
            }
        }
        PeqModel::FreePkFree | PeqModel::Free => {
            // Free filter types: parameters are [type, freq, Q, gain]
            FilterParams {
                filter_type: Some(x[offset]),
                freq: x[offset + 1],
                q: x[offset + 2],
                gain: x[offset + 3],
            }
        }
    }
}

/// Set filter parameters for the i-th filter
pub fn set_filter_params(x: &mut [f64], i: usize, params: &FilterParams, peq_model: PeqModel) {
    let ppf = params_per_filter(peq_model);
    let offset = i * ppf;

    match peq_model {
        PeqModel::Pk | PeqModel::HpPk | PeqModel::HpPkLp | PeqModel::LsPk | PeqModel::LsPkHs => {
            // Fixed filter types: parameters are [freq, Q, gain]
            x[offset] = params.freq;
            x[offset + 1] = params.q;
            x[offset + 2] = params.gain;
        }
        PeqModel::FreePkFree | PeqModel::Free => {
            // Free filter types: parameters are [type, freq, Q, gain]
            x[offset] = params.filter_type.unwrap_or(0.0);
            x[offset + 1] = params.freq;
            x[offset + 2] = params.q;
            x[offset + 3] = params.gain;
        }
    }
}

/// Container for filter parameters
#[derive(Debug, Clone)]
pub struct FilterParams {
    /// Filter type (encoded as f64 for free filter models)
    pub filter_type: Option<f64>,
    /// Frequency (as log10 for optimization)
    pub freq: f64,
    /// Q factor
    pub q: f64,
    /// Gain in dB
    pub gain: f64,
}

/// Determine the filter type based on model and position
pub fn determine_filter_type(
    i: usize,
    num_filters: usize,
    peq_model: PeqModel,
    type_param: Option<f64>,
) -> BiquadFilterType {
    match peq_model {
        PeqModel::Pk => BiquadFilterType::Peak,
        PeqModel::HpPk => {
            if i == 0 {
                BiquadFilterType::HighpassVariableQ
            } else {
                BiquadFilterType::Peak
            }
        }
        PeqModel::HpPkLp => {
            if i == 0 {
                BiquadFilterType::HighpassVariableQ
            } else if i == num_filters - 1 {
                BiquadFilterType::Lowpass
            } else {
                BiquadFilterType::Peak
            }
        }
        PeqModel::LsPk => {
            if i == 0 {
                BiquadFilterType::Lowshelf
            } else {
                BiquadFilterType::Peak
            }
        }
        PeqModel::LsPkHs => {
            if i == 0 {
                BiquadFilterType::Lowshelf
            } else if i == num_filters - 1 {
                BiquadFilterType::Highshelf
            } else {
                BiquadFilterType::Peak
            }
        }
        PeqModel::FreePkFree => {
            // First and last filters are free, middle are peak
            if i == 0 || i == num_filters - 1 {
                decode_filter_type(type_param.unwrap_or(0.0))
            } else {
                BiquadFilterType::Peak
            }
        }
        PeqModel::Free => {
            // All filters are free
            decode_filter_type(type_param.unwrap_or(0.0))
        }
    }
}

/// Decode filter type from parameter value
/// Maps continuous parameter to discrete filter types for optimization
pub fn decode_filter_type(type_value: f64) -> BiquadFilterType {
    // Map [0, 1) -> Peak
    // Map [1, 2) -> Lowpass
    // Map [2, 3) -> Highpass
    // Map [3, 4) -> Lowshelf
    // Map [4, 5) -> Highshelf
    // Map [5, 6) -> HighpassVariableQ
    // Map [6, 7) -> Bandpass
    // Map [7, 8) -> Notch

    let idx = type_value.floor() as i32;
    match idx {
        0 => BiquadFilterType::Peak,
        1 => BiquadFilterType::Lowpass,
        2 => BiquadFilterType::Highpass,
        3 => BiquadFilterType::Lowshelf,
        4 => BiquadFilterType::Highshelf,
        5 => BiquadFilterType::HighpassVariableQ,
        6 => BiquadFilterType::Bandpass,
        7 => BiquadFilterType::Notch,
        _ => BiquadFilterType::Peak, // Default
    }
}

/// Encode filter type to parameter value
pub fn encode_filter_type(filter_type: BiquadFilterType) -> f64 {
    match filter_type {
        BiquadFilterType::Peak => 0.0,
        BiquadFilterType::Lowpass => 1.0,
        BiquadFilterType::Highpass => 2.0,
        BiquadFilterType::Lowshelf => 3.0,
        BiquadFilterType::Highshelf => 4.0,
        BiquadFilterType::HighpassVariableQ => 5.0,
        BiquadFilterType::Bandpass => 6.0,
        BiquadFilterType::Notch => 7.0,
    }
}

/// Get bounds for filter type parameter
pub fn filter_type_bounds() -> (f64, f64) {
    (0.0, 7.999) // Almost 8, but not quite to avoid edge cases
}
