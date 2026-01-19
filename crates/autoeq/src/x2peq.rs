//! AutoEQ - A library for audio equalization and filter optimization
//!
//! Copyright (C) 2025 Pierre Aubert pierre(at)spinorama(dot)org
//!
//! This program is free software: you can redistribute it and/or modify
//! it under the terms of the GNU General Public License as published by
//! the Free Software Foundation, either version 3 of the License, or
//! (at your option) any later version.
//!
//! This program is distributed in the hope that it will be useful,
//! but WITHOUT ANY WARRANTY; without even the implied warranty of
//! MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//! GNU General Public License for more details.
//!
//! You should have received a copy of the GNU General Public License
//! along with this program.  If not, see <https://www.gnu.org/licenses/>.

use crate::cli::PeqModel;
use crate::iir::{Biquad, Peq};
use crate::param_utils::{self, FilterParams};
use ndarray::Array1;

/// Convert parameter vector to Peq structure
///
/// # Arguments
/// * `x` - Parameter vector with filter parameters (layout depends on PeqModel)
/// * `srate` - Sample rate in Hz
/// * `peq_model` - PEQ model that defines the filter structure
///
/// # Returns
/// A Peq structure containing the filters
pub fn x2peq(x: &[f64], srate: f64, peq_model: PeqModel) -> Peq {
    let num_filters = param_utils::num_filters(x, peq_model);
    let mut peq = Vec::with_capacity(num_filters);

    for i in 0..num_filters {
        let params = param_utils::get_filter_params(x, i, peq_model);
        let freq = 10f64.powf(params.freq);
        let q = params.q;
        let gain = params.gain;

        let ftype =
            param_utils::determine_filter_type(i, num_filters, peq_model, params.filter_type);

        let filter = Biquad::new(ftype, freq, srate, q, gain);
        peq.push((1.0, filter));
    }

    peq
}

/// Convert Peq structure back to parameter vector
///
/// # Arguments
/// * `peq` - Peq structure containing the filters
/// * `peq_model` - PEQ model that defines the parameter layout
///
/// # Returns
/// Parameter vector with appropriate layout for the PEQ model
pub fn peq2x(peq: &Peq, peq_model: PeqModel) -> Vec<f64> {
    let ppf = param_utils::params_per_filter(peq_model);
    let mut x = Vec::with_capacity(peq.len() * ppf);

    for (_weight, filter) in peq {
        let params = FilterParams {
            filter_type: if ppf == 4 {
                Some(param_utils::encode_filter_type(filter.filter_type))
            } else {
                None
            },
            freq: filter.freq.log10(),
            q: filter.q,
            gain: filter.db_gain,
        };

        // Add parameters based on model
        match peq_model {
            PeqModel::Pk
            | PeqModel::HpPk
            | PeqModel::HpPkLp
            | PeqModel::LsPk
            | PeqModel::LsPkHs => {
                x.push(params.freq);
                x.push(params.q);
                x.push(params.gain);
            }
            PeqModel::FreePkFree | PeqModel::Free => {
                x.push(params.filter_type.unwrap_or(0.0));
                x.push(params.freq);
                x.push(params.q);
                x.push(params.gain);
            }
        }
    }

    x
}

/// Convert parameter vector to parametric EQ frequency response
///
/// Computes the combined frequency response of all filters specified in the parameter vector.
/// This is a compatibility function that uses x2peq and compute_peq_response internally.
///
/// # Arguments
/// * `freqs` - Frequency points for evaluation (Hz)
/// * `x` - Parameter vector with filter parameters (layout depends on PeqModel)
/// * `srate` - Sample rate in Hz
/// * `peq_model` - PEQ model that defines the filter structure
///
/// # Returns
/// Frequency response in dB SPL at the specified frequency points
pub fn x2spl(freqs: &Array1<f64>, x: &[f64], srate: f64, peq_model: PeqModel) -> Array1<f64> {
    let peq = x2peq(x, srate, peq_model);
    crate::iir::compute_peq_response(freqs, &peq, srate)
}

/// Compute the combined PEQ response from parameter vector
///
/// This is an alias for x2spl, provided for compatibility
pub fn compute_peq_response_from_x(
    freqs: &Array1<f64>,
    x: &[f64],
    sample_rate: f64,
    peq_model: PeqModel,
) -> Array1<f64> {
    x2spl(freqs, x, sample_rate, peq_model)
}

/// Build a vector of sorted filter rows from optimization parameters
///
/// # Arguments
/// * `x` - Slice of optimization parameters (layout depends on PeqModel)
/// * `peq_model` - PEQ model that defines the filter structure
///
/// # Returns
/// * Vector of FilterRow structs sorted by frequency
///
/// # Details
/// Converts the flat parameter vector into a vector of FilterRow structs,
/// sorts them by frequency, and marks filters according to the PEQ model.
pub fn build_sorted_filters(x: &[f64], peq_model: PeqModel) -> Vec<crate::iir::FilterRow> {
    let num_filters = param_utils::num_filters(x, peq_model);
    let mut rows: Vec<crate::iir::FilterRow> = Vec::with_capacity(num_filters);

    for i in 0..num_filters {
        let params = param_utils::get_filter_params(x, i, peq_model);
        let freq = 10f64.powf(params.freq);
        let q = params.q;
        let gain = params.gain;

        let ftype =
            param_utils::determine_filter_type(i, num_filters, peq_model, params.filter_type);

        rows.push(crate::iir::FilterRow {
            freq,
            q,
            gain,
            kind: ftype.short_name(),
        });
    }

    rows.sort_by(|a, b| {
        a.freq
            .partial_cmp(&b.freq)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // For highpass and lowpass filters, set gain to 0 for display
    for row in &mut rows {
        if row.kind == "HPQ" || row.kind == "LP" || row.kind == "HP" {
            row.gain = 0.0;
        }
    }

    rows
}

/// Print a formatted table of the parametric EQ filters.
///
/// The filters are printed with any non-Peak filters marked according to the PEQ model,
/// with all filters sorted by frequency.
pub fn peq_print_from_x(x: &[f64], peq_model: PeqModel) {
    let peq = x2peq(x, crate::iir::SRATE, peq_model);
    crate::iir::peq_print(&peq);
}
