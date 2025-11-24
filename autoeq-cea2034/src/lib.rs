#![doc = include_str!("../README.md")]

use std::collections::HashMap;
use std::error::Error;

use ndarray::concatenate;
use ndarray::s;
use ndarray::{Array1, Array2, Axis};
use serde::{Deserialize, Serialize};

/// A struct to hold frequency and SPL data.
/// Re-exported from the main autoeq crate for compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Curve {
    /// Frequency points in Hz
    pub freq: Array1<f64>,
    /// Sound Pressure Level in dB
    pub spl: Array1<f64>,
}

/// Convert SPL values to pressure values
///
/// # Arguments
/// * `spl` - Array of SPL values
///
/// # Returns
/// * Array of pressure values
///
/// # Formula
/// pressure = 10^((spl-105)/20)
fn spl2pressure(spl: &Array1<f64>) -> Array1<f64> {
    // 10^((spl-105)/20)
    spl.mapv(|v| 10f64.powf((v - 105.0) / 20.0))
}

/// Convert pressure values to SPL values
///
/// # Arguments
/// * `p` - Array of pressure values
///
/// # Returns
/// * Array of SPL values
///
/// # Formula
/// spl = 20*log10(p) + 105
fn pressure2spl(p: &Array1<f64>) -> Array1<f64> {
    // 20*log10(p) + 105
    p.mapv(|v| 20.0 * v.log10() + 105.0)
}

/// Convert SPL values to squared pressure values
///
/// # Arguments
/// * `spl` - 2D array of SPL values
///
/// # Returns
/// * 2D array of squared pressure values
///
/// # Details
/// Computes pressure values from SPL and then squares them using vectorized operations
fn spl2pressure2(spl: &Array2<f64>) -> Array2<f64> {
    // Vectorized: 10^((spl-105)/20) then square
    spl.mapv(|v| {
        let p = 10f64.powf((v - 105.0) / 20.0);
        p * p
    })
}

/// Compute the CEA2034 spinorama from SPL data (internal implementation)
///
/// # Arguments
/// * `spl` - 2D array of SPL measurements
/// * `idx` - Indices for grouping measurements
/// * `weights` - Weights for computing weighted averages
///
/// # Returns
/// * 2D array representing the CEA2034 spinorama
///
/// # Details
/// Computes various CEA2034 curves including On Axis, Listening Window,
/// Early Reflections, Sound Power, and Predicted In-Room response.
fn cea2034_array(spl: &Array2<f64>, idx: &[Vec<usize>], weights: &Array1<f64>) -> Array2<f64> {
    let len_spl = spl.shape()[1];
    let p2 = spl2pressure2(spl);
    let idx_sp = idx.len() - 1;
    let idx_lw = 0usize;
    let idx_er = 1usize;
    let idx_pir = idx_sp + 1;

    let mut cea = Array2::<f64>::zeros((idx.len() + 1, len_spl));

    for (i, idx_val) in idx.iter().enumerate().take(idx_sp) {
        let curve = apply_rms(&p2, idx_val);
        cea.row_mut(i).assign(&curve);
    }

    // ER: indices 2..=6 per original logic - vectorized
    let er_rows = cea.slice(s![2..=6, ..]);
    let er_pressures = er_rows.mapv(|v| {
        let p = 10f64.powf((v - 105.0) / 20.0);
        p * p
    });
    let er_p2_sum = er_pressures.sum_axis(Axis(0));
    let er_p = er_p2_sum.mapv(|v| (v / 5.0).sqrt());
    let er_spl = pressure2spl(&er_p);
    cea.row_mut(idx_er).assign(&er_spl);

    // SP weighted
    let sp_curve = apply_weighted_rms(&p2, &idx[idx_sp], weights);
    cea.row_mut(idx_sp).assign(&sp_curve);

    // PIR - vectorized computation
    let lw_p = spl2pressure(&cea.row(idx_lw).to_owned());
    let er_p = spl2pressure(&cea.row(idx_er).to_owned());
    let sp_p = spl2pressure(&cea.row(idx_sp).to_owned());

    let lw2 = lw_p.mapv(|v| v * v);
    let er2 = er_p.mapv(|v| v * v);
    let sp2 = sp_p.mapv(|v| v * v);

    let pir = (lw2.mapv(|v| 0.12 * v) + er2.mapv(|v| 0.44 * v) + sp2.mapv(|v| 0.44 * v))
        .mapv(|v| v.sqrt());
    let pir_spl = pressure2spl(&pir);
    cea.row_mut(idx_pir).assign(&pir_spl);

    cea
}

/// Apply RMS averaging to pressure squared values
///
/// # Arguments
/// * `p2` - 2D array of squared pressure values
/// * `idx` - Indices of rows to include in RMS calculation
///
/// # Returns
/// * Array of SPL values after RMS averaging
///
/// # Formula
/// rms = sqrt(sum(p2\[idx\]) / len) then converted to SPL
fn apply_rms(p2: &Array2<f64>, idx: &[usize]) -> Array1<f64> {
    // Vectorized sum using select and sum_axis
    let selected_rows = p2.select(Axis(0), idx);
    let sum_rows = selected_rows.sum_axis(Axis(0));
    let len_idx = idx.len() as f64;
    let r = sum_rows.mapv(|v| (v / len_idx).sqrt());
    pressure2spl(&r)
}

/// Apply weighted RMS averaging to pressure squared values
///
/// # Arguments
/// * `p2` - 2D array of squared pressure values
/// * `idx` - Indices of rows to include in weighted RMS calculation
/// * `weights` - Weights for each row
///
/// # Returns
/// * Array of SPL values after weighted RMS averaging
///
/// # Formula
/// weighted_rms = sqrt(sum(p2\[idx\] * weights\[idx\]) / sum(weights)) then converted to SPL
fn apply_weighted_rms(p2: &Array2<f64>, idx: &[usize], weights: &Array1<f64>) -> Array1<f64> {
    // Vectorized weighted sum
    let selected_rows = p2.select(Axis(0), idx);
    let selected_weights = weights.select(Axis(0), idx);
    let sum_w = selected_weights.sum();

    // Broadcast weights to match row dimensions and compute weighted sum
    let weighted_rows = &selected_rows * &selected_weights.insert_axis(Axis(1));
    let acc = weighted_rows.sum_axis(Axis(0));
    let r = acc.mapv(|v| (v / sum_w).sqrt());
    pressure2spl(&r)
}

/// Compute Mean Absolute Deviation (MAD) for a slice of SPL values
///
/// # Arguments
/// * `spl` - Array of SPL values
/// * `imin` - Start index (inclusive)
/// * `imax` - End index (exclusive)
///
/// # Returns
/// * Mean absolute deviation value
///
/// # Formula
/// mad = mean(|x - mean(x)|)
fn mad(spl: &Array1<f64>, imin: usize, imax: usize) -> f64 {
    let slice = spl.slice(s![imin..imax]).to_owned();
    let m = slice.mean().unwrap_or(0.0);
    let diffs = slice.mapv(|v| (v - m).abs());
    diffs.mean().unwrap_or(f64::NAN)
}

/// Compute the coefficient of determination (R-squared) between two arrays
///
/// # Arguments
/// * `x` - First array of values
/// * `y` - Second array of values
///
/// # Returns
/// * R-squared value (Pearson correlation coefficient squared)
fn r_squared(x: &Array1<f64>, y: &Array1<f64>) -> f64 {
    // Vectorized Pearson correlation squared
    let n = x.len() as f64;
    if n == 0.0 {
        return f64::NAN;
    }
    let mx = x.mean().unwrap_or(0.0);
    let my = y.mean().unwrap_or(0.0);

    // Vectorized computation of deviations
    let dx = x.mapv(|v| v - mx);
    let dy = y.mapv(|v| v - my);

    let num = (&dx * &dy).sum();
    let sxx = (&dx * &dx).sum();
    let syy = (&dy * &dy).sum();

    if sxx == 0.0 || syy == 0.0 {
        return f64::NAN;
    }
    let r = num / (sxx.sqrt() * syy.sqrt());
    r * r
}

// ---------------- Pure Rust API below ----------------

/// Compute the CEA2034 spinorama from SPL data
///
/// # Arguments
/// * `spl` - 2D array of SPL measurements
/// * `idx` - Indices for grouping measurements
/// * `weights` - Weights for computing weighted averages
///
/// # Returns
/// * 2D array representing the CEA2034 spinorama
pub fn cea2034(spl: &Array2<f64>, idx: &[Vec<usize>], weights: &Array1<f64>) -> Array2<f64> {
    cea2034_array(spl, idx, weights)
}

/// Generate octave band frequencies
///
/// # Arguments
/// * `count` - Number of bands per octave
///
/// # Returns
/// * Vector of tuples representing (low, center, high) frequencies for each band
///
/// # Panics
/// * If count is less than 2
pub fn octave(count: usize) -> Vec<(f64, f64, f64)> {
    assert!(count >= 2, "count (N) must be >= 2");
    let reference = 1290.0_f64;
    let p = 2.0_f64.powf(1.0 / count as f64);
    let p_band = 2.0_f64.powf(1.0 / (2.0 * count as f64));
    let o_iter: i32 = (count as i32 * 10 + 1) / 2;
    let mut centers: Vec<f64> = Vec::with_capacity((o_iter as usize) * 2 + 1);
    for i in (1..=o_iter).rev() {
        centers.push(reference / p.powi(i));
    }
    centers.push(reference);
    for i in 1..=o_iter {
        let center = reference * p.powi(i);
        if (center / p_band) <= 20000.0 {
            centers.push(reference * p.powi(i));
        }
    }
    centers
        .into_iter()
        .map(|c| (c / p_band, c, c * p_band))
        .collect()
}

/// Compute octave band intervals for a given frequency array
///
/// # Arguments
/// * `count` - Number of bands per octave
/// * `freq` - Array of frequencies
///
/// # Returns
/// * Vector of tuples representing (start_index, end_index) for each band
pub fn octave_intervals(count: usize, freq: &Array1<f64>) -> Vec<(usize, usize)> {
    let bands = octave(count);

    // Python logic: band_min_freq = max(100, min_freq)
    let min_freq = freq[0];
    let band_min_freq = 100.0_f64.max(min_freq);

    let mut out = Vec::new();
    for (low, center, high) in bands.into_iter() {
        if center < band_min_freq || center > 12000.0 {
            continue; // skip bands outside desired range
        }
        // Match Python: dfu.loc[(dfu.Freq >= band_min) & (dfu.Freq <= band_max)]
        // Python uses inclusive bounds on both ends
        let imin = freq.iter().position(|&f| f >= low).unwrap_or(freq.len());
        let imax = freq.iter().position(|&f| f > high).unwrap_or(freq.len());
        out.push((imin, imax));
    }
    out
}

/// Compute the Narrow Band Deviation (NBD) metric
///
/// # Arguments
/// * `intervals` - Vector of (start_index, end_index) tuples for frequency bands
/// * `spl` - SPL measurements
///
/// # Returns
/// * NBD value as f64
pub fn nbd(intervals: &[(usize, usize)], spl: &Array1<f64>) -> f64 {
    let mut sum = 0.0;
    let mut cnt = 0.0;
    for &(imin, imax) in intervals.iter() {
        let v = mad(spl, imin, imax);
        if v.is_finite() {
            sum += v;
            cnt += 1.0;
        }
    }
    if cnt == 0.0 { f64::NAN } else { sum / cnt }
}

/// Compute the Low Frequency Extension (LFX) metric
///
/// # Arguments
/// * `freq` - Frequency array
/// * `lw` - Listening window SPL measurements
/// * `sp` - Sound power SPL measurements
///
/// # Returns
/// * LFX value as f64 (log10 of the frequency)
pub fn lfx(freq: &Array1<f64>, lw: &Array1<f64>, sp: &Array1<f64>) -> f64 {
    // Match Python behavior:
    // LW reference is mean(LW) over [300 Hz, 10 kHz], inclusive on both ends.
    // Implemented by indices: [first f >= 300] .. [first f > 10000]
    let lw_min = freq.iter().position(|&f| f >= 300.0).unwrap_or(freq.len());
    let lw_max = freq.iter().position(|&f| f > 10000.0).unwrap_or(freq.len());
    if lw_min >= lw_max {
        return (300.0_f64).log10();
    }
    let lw_ref = lw.slice(s![lw_min..lw_max]).mean().unwrap_or(0.0) - 6.0;
    // Collect indices where freq <= 300 Hz AND SP <= (LW_ref)
    let mut indices: Vec<usize> = Vec::new();
    for (i, (&f, &spv)) in freq.iter().zip(sp.iter()).enumerate() {
        if f <= 300.0 && spv <= lw_ref {
            indices.push(i);
        }
    }
    if indices.is_empty() {
        // No frequency bin meets the -6 dB criterion → fall back to lowest frequency
        return freq[0].log10();
    }

    // Identify the first contiguous group of indices (as in Python implementation)
    let mut last_idx = indices[0];
    for &idx in indices.iter().skip(1) {
        if idx == last_idx + 1 {
            last_idx = idx;
        } else {
            break; // stop at the end of the first consecutive block
        }
    }

    // Use the next frequency bin (pos + 1) to align with Python behavior
    let next_idx = last_idx + 1;
    if next_idx < freq.len() {
        freq[next_idx].log10()
    } else {
        // Some measurements might end at/below 300 Hz, use default per Python
        (300.0_f64).log10()
    }
}

/// Compute the Smoothness Metric (SM)
///
/// # Arguments
/// * `freq` - Frequency array
/// * `spl` - SPL measurements
///
/// # Returns
/// * SM value as f64 (R-squared value)
pub fn sm(freq: &Array1<f64>, spl: &Array1<f64>) -> f64 {
    let f_min = freq.iter().position(|&f| f > 100.0).unwrap_or(freq.len());
    let f_max = freq
        .iter()
        .position(|&f| f >= 16000.0)
        .unwrap_or(freq.len());
    if f_min >= f_max {
        return f64::NAN;
    }
    let x: Array1<f64> = freq.slice(s![f_min..f_max]).mapv(|v| v.log10());
    let y: Array1<f64> = spl.slice(s![f_min..f_max]).to_owned();
    r_squared(&x, &y)
}

/// Metrics computed for the CEA2034 preference score
#[derive(Debug, Clone)]
pub struct ScoreMetrics {
    /// Narrow Band Deviation for on-axis response
    pub nbd_on: f64,
    /// Narrow Band Deviation for predicted in-room response
    pub nbd_pir: f64,
    /// Low Frequency Extension metric
    pub lfx: f64,
    /// Smoothness Metric for predicted in-room response
    pub sm_pir: f64,
    /// Overall preference score
    pub pref_score: f64,
}

/// Compute all CEA2034 metrics and preference score
///
/// # Arguments
/// * `freq` - Frequency array
/// * `intervals` - Octave band intervals
/// * `on` - On-axis SPL measurements
/// * `lw` - Listening window SPL measurements
/// * `sp` - Sound power SPL measurements
/// * `pir` - Predicted in-room SPL measurements
///
/// # Returns
/// * ScoreMetrics struct containing all computed metrics
pub fn score(
    freq: &Array1<f64>,
    intervals: &[(usize, usize)],
    on: &Array1<f64>,
    lw: &Array1<f64>,
    sp: &Array1<f64>,
    pir: &Array1<f64>,
) -> ScoreMetrics {
    let nbd_on = nbd(intervals, on);
    let nbd_pir = nbd(intervals, pir);
    let sm_pir = sm(freq, pir);
    let lfx_val = lfx(freq, lw, sp);
    let pref = 12.69 - 2.49 * nbd_on - 2.99 * nbd_pir - 4.31 * lfx_val + 2.32 * sm_pir;
    ScoreMetrics {
        nbd_on,
        nbd_pir,
        lfx: lfx_val,
        sm_pir,
        pref_score: pref,
    }
}

/// Compute CEA2034 metrics and preference score for a PEQ filter
///
/// # Arguments
/// * `freq` - Frequency array
/// * `idx` - Indices for grouping measurements
/// * `intervals` - Octave band intervals
/// * `weights` - Weights for computing weighted averages
/// * `spl_h` - Horizontal SPL measurements
/// * `spl_v` - Vertical SPL measurements
/// * `peq` - PEQ filter response
///
/// # Returns
/// * Tuple containing (spinorama data, ScoreMetrics)
///
/// # Panics
/// * If peq length doesn't match SPL columns
pub fn score_peq(
    freq: &Array1<f64>,
    idx: &[Vec<usize>],
    intervals: &[(usize, usize)],
    weights: &Array1<f64>,
    spl_h: &Array2<f64>,
    spl_v: &Array2<f64>,
    peq: &Array1<f64>,
) -> (Array2<f64>, ScoreMetrics) {
    assert_eq!(
        peq.len(),
        spl_h.shape()[1],
        "peq length must match SPL columns"
    );
    assert_eq!(
        peq.len(),
        spl_v.shape()[1],
        "peq length must match SPL columns"
    );

    // add PEQ to each row using broadcasting
    let peq_broadcast = peq.view().insert_axis(Axis(0));
    let spl_h_peq = spl_h + &peq_broadcast;
    let spl_v_peq = spl_v + &peq_broadcast;

    let spl_full =
        concatenate(Axis(0), &[spl_h_peq.view(), spl_v_peq.view()]).expect("concatenate failed");
    let spin_nd = cea2034_array(&spl_full, idx, weights);

    // Prepare rows for scoring
    let on = spl_h_peq.row(17).to_owned();
    let lw = spin_nd.row(0).to_owned();
    let sp_row = spin_nd.row(spin_nd.shape()[0] - 2).to_owned();
    let pir = spin_nd.row(spin_nd.shape()[0] - 1).to_owned();

    let metrics = score(freq, intervals, &on, &lw, &sp_row, &pir);
    (spin_nd, metrics)
}

/// Compute approximate CEA2034 metrics and preference score for a PEQ filter
///
/// This is a simplified version of score_peq that works directly with pre-computed
/// LW, SP, and PIR curves rather than computing them from raw measurements.
///
/// # Arguments
/// * `freq` - Frequency array
/// * `intervals` - Octave band intervals
/// * `lw` - Listening window SPL measurements
/// * `sp` - Sound power SPL measurements
/// * `pir` - Predicted in-room SPL measurements
/// * `on` - On-axis SPL measurements
/// * `peq` - PEQ filter response
///
/// # Returns
/// * ScoreMetrics struct containing all computed metrics
pub fn score_peq_approx(
    freq: &Array1<f64>,
    intervals: &[(usize, usize)],
    lw: &Array1<f64>,
    sp: &Array1<f64>,
    pir: &Array1<f64>,
    on: &Array1<f64>,
    peq: &Array1<f64>,
) -> ScoreMetrics {
    let on2 = on + peq;
    let lw2 = lw + peq;
    let sp2 = sp + peq;
    let pir2 = pir + peq;
    score(freq, intervals, &on2, &lw2, &sp2, &pir2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn octave_count_2_includes_reference_center() {
        let bands = octave(2);
        // find the center equal to 1290
        assert!(bands.iter().any(|&(_l, c, _h)| (c - 1290.0).abs() < 1e-9));
    }

    #[test]
    fn nbd_simple_mean_of_mads() {
        let spl = Array1::from(vec![0.0, 1.0, 2.0, 1.0, 0.0]);
        // two intervals: [0..3) and [2..5)
        let intervals = vec![(0, 3), (2, 5)];
        let v = nbd(&intervals, &spl);
        assert!(v.is_finite());
    }

    #[test]
    fn score_peq_approx_matches_score_when_peq_zero() {
        // Simple synthetic data
        let freq = Array1::from(vec![100.0, 1000.0, 10000.0]);
        let intervals = vec![(0, 3)];
        let on = Array1::from(vec![80.0, 85.0, 82.0]);
        let lw = Array1::from(vec![81.0, 84.0, 83.0]);
        let sp = Array1::from(vec![79.0, 83.0, 81.0]);
        let pir = Array1::from(vec![80.5, 84.0, 82.0]);
        let zero = Array1::zeros(freq.len());

        let m1 = score(&freq, &intervals, &on, &lw, &sp, &pir);
        let m2 = score_peq_approx(&freq, &intervals, &lw, &sp, &pir, &on, &zero);

        assert!((m1.nbd_on - m2.nbd_on).abs() < 1e-12);
        assert!((m1.nbd_pir - m2.nbd_pir).abs() < 1e-12);
        assert!((m1.lfx - m2.lfx).abs() < 1e-12);
        assert!((m1.sm_pir - m2.sm_pir).abs() < 1e-12);
        assert!((m1.pref_score - m2.pref_score).abs() < 1e-12);
    }

    #[test]
    fn lfx_next_bin_after_first_block() {
        // Frequencies spanning below and above 300 and up to 12k
        let freq = Array1::from(vec![
            50.0, 100.0, 200.0, 300.0, 500.0, 1000.0, 5000.0, 10000.0, 12000.0,
        ]);
        // LW constant 80 dB; LW_ref = 80 - 6 = 74
        let lw = Array1::from(vec![80.0; 9]);
        // SP <= LW_ref for first two bins only (50, 100). First block ends at index 1.
        // Next bin is index 2 -> 200 Hz
        let sp = Array1::from(vec![70.0, 73.0, 75.0, 76.0, 80.0, 80.0, 80.0, 80.0, 80.0]);
        let val = lfx(&freq, &lw, &sp);
        assert!((val - 200.0_f64.log10()).abs() < 1e-12);
    }

    #[test]
    fn lfx_no_indices_falls_back_to_first_freq() {
        let freq = Array1::from(vec![
            50.0, 100.0, 200.0, 300.0, 500.0, 1000.0, 5000.0, 10000.0, 12000.0,
        ]);
        let lw = Array1::from(vec![80.0; 9]);
        // All SP > LW_ref (74) for <= 300
        let sp = Array1::from(vec![75.0, 80.0, 80.0, 80.0, 80.0, 80.0, 80.0, 80.0, 80.0]);
        let val = lfx(&freq, &lw, &sp);
        assert!((val - 50.0_f64.log10()).abs() < 1e-12);
    }

    #[test]
    fn lfx_next_index_oob_defaults_to_300() {
        let freq = Array1::from(vec![100.0, 200.0, 300.0]);
        let lw = Array1::from(vec![80.0, 80.0, 80.0]);
        // All SP <= LW_ref (74) for <= 300 => indices [0,1,2]; next index OOB
        let sp = Array1::from(vec![70.0, 70.0, 70.0]);
        let val = lfx(&freq, &lw, &sp);
        assert!((val - 300.0_f64.log10()).abs() < 1e-12);
    }
}

/// Compute Predicted In-Room (PIR) response from LW, ER, and SP measurements
///
/// # Arguments
/// * `lw` - Listening window SPL measurements
/// * `er` - Early reflections SPL measurements
/// * `sp` - Sound power SPL measurements
///
/// # Returns
/// * PIR SPL measurements
pub fn compute_pir_from_lw_er_sp(
    lw: &Array1<f64>,
    er: &Array1<f64>,
    sp: &Array1<f64>,
) -> Array1<f64> {
    let lw_p = spl2pressure(lw);
    let er_p = spl2pressure(er);
    let sp_p = spl2pressure(sp);
    let lw2 = lw_p.mapv(|v| v * v);
    let er2 = er_p.mapv(|v| v * v);
    let sp2 = sp_p.mapv(|v| v * v);
    let pir_p2 = lw2.mapv(|v| 0.12 * v) + &er2.mapv(|v| 0.44 * v) + &sp2.mapv(|v| 0.44 * v);
    let pir_p = pir_p2.mapv(|v| v.sqrt());
    pressure2spl(&pir_p)
}

/// Compute CEA2034 metrics for speaker performance evaluation
///
/// # Arguments
/// * `freq` - Frequency grid for computation
/// * `cea_plot_data` - Cached plot data (may be updated if fetched)
/// * `peq` - Optional PEQ response to apply to metrics
///
/// # Returns
/// * Result containing ScoreMetrics or an error
///
/// # Details
/// Computes CEA2034 metrics including preference score, Narrow Band Deviation (NBD),
/// Low Frequency Extension (LFX), and Smoothness Metric (SM) for various curves.
pub async fn compute_cea2034_metrics(
    freq: &Array1<f64>,
    cea2034_data: &HashMap<String, Curve>,
    peq: Option<&Array1<f64>>,
) -> Result<ScoreMetrics, Box<dyn Error>> {
    let on = &cea2034_data.get("On Axis").unwrap().spl;
    let lw = &cea2034_data.get("Listening Window").unwrap().spl;
    let sp = &cea2034_data.get("Sound Power").unwrap().spl;
    let pir = &cea2034_data.get("Estimated In-Room Response").unwrap().spl;

    // 1/2 octave intervals for band metrics
    let intervals = octave_intervals(2, freq);

    // Use provided PEQ or assume zero PEQ
    let peq_arr = peq.cloned().unwrap_or_else(|| Array1::zeros(freq.len()));

    Ok(score_peq_approx(
        freq, &intervals, lw, sp, pir, on, &peq_arr,
    ))
}

#[cfg(test)]
mod pir_helpers_tests {
    use super::{compute_pir_from_lw_er_sp, pressure2spl, spl2pressure};
    use crate::Curve;
    use ndarray::Array1;
    use std::collections::HashMap;

    // Helpers to encode f64 arrays into the Plotly-typed array base64 format used in read.rs
    fn _le_f64_bytes(vals: &[f64]) -> Vec<u8> {
        let mut out = Vec::with_capacity(vals.len() * 8);
        for v in vals {
            out.extend_from_slice(&v.to_bits().to_le_bytes());
        }
        out
    }

    fn _base64_encode(bytes: &[u8]) -> String {
        let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        let mut i = 0usize;
        while i < bytes.len() {
            let b0 = bytes[i] as u32;
            let b1 = if i + 1 < bytes.len() {
                bytes[i + 1] as u32
            } else {
                0
            };
            let b2 = if i + 2 < bytes.len() {
                bytes[i + 2] as u32
            } else {
                0
            };

            let idx0 = (b0 >> 2) & 0x3F;
            let idx1 = ((b0 & 0x03) << 4) | ((b1 >> 4) & 0x0F);
            let idx2 = ((b1 & 0x0F) << 2) | ((b2 >> 6) & 0x03);
            let idx3 = b2 & 0x3F;

            out.push(alphabet[idx0 as usize] as char);
            out.push(alphabet[idx1 as usize] as char);
            if i + 1 < bytes.len() {
                out.push(alphabet[idx2 as usize] as char);
            } else {
                out.push('=');
            }
            if i + 2 < bytes.len() {
                out.push(alphabet[idx3 as usize] as char);
            } else {
                out.push('=');
            }

            i += 3;
        }
        out
    }

    #[test]
    fn spl_pressure_roundtrip_is_identity() {
        let spl = Array1::from(vec![60.0, 80.0, 100.0]);
        let p = spl2pressure(&spl);
        let spl2 = pressure2spl(&p);
        for (a, b) in spl.iter().zip(spl2.iter()) {
            assert!((a - b).abs() < 1e-12);
        }
    }

    #[test]
    fn pir_equals_input_when_all_equal() {
        let lw = Array1::from(vec![80.0, 80.0, 80.0]);
        let er = Array1::from(vec![80.0, 80.0, 80.0]);
        let sp = Array1::from(vec![80.0, 80.0, 80.0]);
        let pir = compute_pir_from_lw_er_sp(&lw, &er, &sp);
        for v in pir.iter() {
            assert!((*v - 80.0).abs() < 1e-12);
        }
    }

    #[test]
    fn pir_reflects_er_sp_weighting() {
        // ER and SP have higher weights than LW (0.44 each vs 0.12)
        let lw = Array1::from(vec![70.0, 70.0, 70.0]);
        let er = Array1::from(vec![80.0, 80.0, 80.0]);
        let sp = Array1::from(vec![80.0, 80.0, 80.0]);
        let pir = compute_pir_from_lw_er_sp(&lw, &er, &sp);
        for v in pir.iter() {
            assert!(*v > 75.0 && *v < 81.0);
        }
    }

    #[tokio::test]
    async fn metrics_with_precomputed_curves() {
        use super::{compute_cea2034_metrics, octave_intervals, score};

        // Simple two-point dataset
        let freq = Array1::from(vec![100.0, 1000.0]);
        let on_vals = Array1::from(vec![80.0_f64, 85.0_f64]);
        let lw_vals = Array1::from(vec![81.0_f64, 84.0_f64]);
        let er_vals = Array1::from(vec![79.0_f64, 83.0_f64]);
        let sp_vals = Array1::from(vec![78.0_f64, 82.0_f64]);

        // Precompute PIR from LW/ER/SP
        let pir_vals = compute_pir_from_lw_er_sp(&lw_vals, &er_vals, &sp_vals);

        // Build CEA2034 data map expected by the helper
        let mut cea2034_data: HashMap<String, Curve> = HashMap::new();
        cea2034_data.insert(
            "On Axis".to_string(),
            Curve {
                freq: freq.clone(),
                spl: on_vals.clone(),
            },
        );
        cea2034_data.insert(
            "Listening Window".to_string(),
            Curve {
                freq: freq.clone(),
                spl: lw_vals.clone(),
            },
        );
        cea2034_data.insert(
            "Sound Power".to_string(),
            Curve {
                freq: freq.clone(),
                spl: sp_vals.clone(),
            },
        );
        cea2034_data.insert(
            "Estimated In-Room Response".to_string(),
            Curve {
                freq: freq.clone(),
                spl: pir_vals.clone(),
            },
        );

        // Compute using the async helper
        let got = compute_cea2034_metrics(&freq, &cea2034_data, None)
            .await
            .expect("metrics");

        // Build expected
        let intervals = octave_intervals(2, &freq);
        let expected = score(&freq, &intervals, &on_vals, &lw_vals, &sp_vals, &pir_vals);

        assert!((got.nbd_on - expected.nbd_on).abs() < 1e-12);
        assert!((got.nbd_pir - expected.nbd_pir).abs() < 1e-12);
        assert!((got.lfx - expected.lfx).abs() < 1e-12);
        if got.sm_pir.is_nan() && expected.sm_pir.is_nan() {
            // ok
        } else {
            assert!((got.sm_pir - expected.sm_pir).abs() < 1e-12);
        }
        if got.pref_score.is_nan() && expected.pref_score.is_nan() {
            // ok
        } else {
            assert!((got.pref_score - expected.pref_score).abs() < 1e-12);
        }
    }

    #[tokio::test]
    async fn metrics_with_precomputed_curves_and_peq_matches_approx() {
        use super::{compute_cea2034_metrics, octave_intervals, score_peq_approx};

        // Simple two-point dataset
        let freq = Array1::from(vec![100.0, 1000.0]);
        let on_vals = Array1::from(vec![80.0_f64, 85.0_f64]);
        let lw_vals = Array1::from(vec![81.0_f64, 84.0_f64]);
        let er_vals = Array1::from(vec![79.0_f64, 83.0_f64]);
        let sp_vals = Array1::from(vec![78.0_f64, 82.0_f64]);

        // Precompute PIR from LW/ER/SP
        let pir_vals = compute_pir_from_lw_er_sp(&lw_vals, &er_vals, &sp_vals);

        // Build CEA2034 data map expected by the helper
        let mut cea2034_data: HashMap<String, Curve> = HashMap::new();
        cea2034_data.insert(
            "On Axis".to_string(),
            Curve {
                freq: freq.clone(),
                spl: on_vals.clone(),
            },
        );
        cea2034_data.insert(
            "Listening Window".to_string(),
            Curve {
                freq: freq.clone(),
                spl: lw_vals.clone(),
            },
        );
        cea2034_data.insert(
            "Sound Power".to_string(),
            Curve {
                freq: freq.clone(),
                spl: sp_vals.clone(),
            },
        );
        cea2034_data.insert(
            "Estimated In-Room Response".to_string(),
            Curve {
                freq: freq.clone(),
                spl: pir_vals.clone(),
            },
        );

        // A simple PEQ response
        let peq = Array1::from(vec![1.0_f64, -1.0_f64]);

        // Compute using the async helper with PEQ
        let got = compute_cea2034_metrics(&freq, &cea2034_data, Some(&peq))
            .await
            .expect("metrics with peq");

        // Build expected using the approximation helper
        let intervals = octave_intervals(2, &freq);
        let expected = score_peq_approx(
            &freq, &intervals, &lw_vals, &sp_vals, &pir_vals, &on_vals, &peq,
        );

        assert!((got.nbd_on - expected.nbd_on).abs() < 1e-12);
        assert!((got.nbd_pir - expected.nbd_pir).abs() < 1e-12);
        assert!((got.lfx - expected.lfx).abs() < 1e-12);
        if got.sm_pir.is_nan() && expected.sm_pir.is_nan() {
            // ok
        } else {
            assert!((got.sm_pir - expected.sm_pir).abs() < 1e-12);
        }
        if got.pref_score.is_nan() && expected.pref_score.is_nan() {
            // ok
        } else {
            assert!((got.pref_score - expected.pref_score).abs() < 1e-12);
        }
    }
}
