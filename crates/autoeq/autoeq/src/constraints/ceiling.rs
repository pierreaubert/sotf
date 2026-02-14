use super::super::cli::PeqModel;
use super::super::x2peq::x2spl;
use ndarray::Array1;

/// Data needed by the nonlinear ceiling constraint callback.
#[derive(Clone)]
pub struct CeilingConstraintData {
    /// Frequency points for evaluation (Hz)
    pub freqs: Array1<f64>,
    /// Sample rate in Hz
    pub srate: f64,
    /// Maximum allowed SPL in dB
    pub max_db: f64,
    /// PEQ model that defines the filter structure
    pub peq_model: PeqModel,
}

/// Inequality constraint: combined response must not exceed max_db.
/// Returns fc(x) = max_i (peq_spl\[i\] - max_db). Feasible when <= 0.
pub fn constraint_ceiling(
    x: &[f64],
    _grad: Option<&mut [f64]>,
    data: &mut CeilingConstraintData,
) -> f64 {
    let peq_spl = x2spl(&data.freqs, x, data.srate, data.peq_model);

    viol_ceiling_from_spl(&peq_spl, data.max_db, data.peq_model)
}

/// Compute ceiling constraint violation from frequency response
///
/// Calculates the maximum excess over the allowed SPL ceiling.
///
/// # Arguments
/// * `peq_spl` - Frequency response in dB SPL
/// * `max_db` - Maximum allowed SPL ceiling
/// * `peq_model` - PEQ model that defines the filter structure (unused, kept for API compatibility)
///
/// # Returns
/// Maximum violation amount (0.0 if no violation)
pub fn viol_ceiling_from_spl(peq_spl: &Array1<f64>, max_db: f64, _peq_model: PeqModel) -> f64 {
    let mut max_excess = 0.0_f64;
    for &v in peq_spl.iter() {
        let excess = (v - max_db).max(0.0);
        if excess > max_excess {
            max_excess = excess;
        }
    }
    max_excess
}
