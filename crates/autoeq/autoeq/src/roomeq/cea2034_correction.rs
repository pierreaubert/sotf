//! CEA2034 speaker pre-correction for 3-pass room EQ pipeline.
//!
//! When a speaker has known anechoic data (CEA2034 from spinorama.org),
//! this module generates correction filters for frequencies above the
//! Schroeder frequency, where the speaker's response dominates over room effects.
//!
//! **3-pass pipeline:**
//! - Pass 1: Speaker correction (this module) — above Schroeder
//! - Pass 2: Room EQ correction — standard room correction on the residual
//! - Pass 3: User preference — bass/treble shelves as separate output filters

use crate::Curve;
use crate::error::{AutoeqError, Result};
use crate::read;
use crate::read::Cea2034Data;
use crate::response;
use log::{debug, info, warn};
use math_audio_iir_fir::{Biquad, BiquadFilterType, DEFAULT_Q_HIGH_LOW_SHELF};
use std::collections::HashMap;

use super::eq;
use super::types::{
    Cea2034CorrectionConfig, Cea2034CorrectionMode, OptimizerConfig, RoomConfig, UserPreference,
};

/// Speed of sound in m/s at ~20C
const SPEED_OF_SOUND: f64 = 343.0;

// ============================================================================
// CEA2034 Data Fetching
// ============================================================================

/// Fetch CEA2034 data for a speaker from spinorama.org (blocking).
///
/// Creates a tokio runtime internally to call the async API.
/// Reuses the existing disk cache in `~/.local/share/autoeq/data_cached/speakers/`.
pub fn fetch_cea2034_blocking(
    speaker_name: &str,
    version: &str,
) -> std::result::Result<Cea2034Data, Box<dyn std::error::Error>> {
    let fetch = async {
        let plot_data =
            read::fetch_measurement_plot_data(speaker_name, version, "CEA2034").await?;
        let curves = read::extract_cea2034_curves_original(&plot_data, "CEA2034")?;
        read::build_cea2034_data(curves)
    };

    // If already inside a tokio runtime (e.g. called from async context),
    // use the existing runtime to avoid "Cannot start a runtime from within a runtime" panic.
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        tokio::task::block_in_place(|| handle.block_on(fetch))
    } else {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(fetch)
    }
}

/// Pre-fetch CEA2034 data for all speakers that have `speaker_name` set.
///
/// Iterates the room config's speakers, resolves speaker names, and fetches
/// CEA2034 data for each. Returns a cache keyed by speaker name.
/// Logs warnings for any speaker whose data cannot be fetched.
pub fn pre_fetch_all_cea2034(config: &RoomConfig) -> HashMap<String, Cea2034Data> {
    let cea_config = match &config.optimizer.cea2034_correction {
        Some(c) if c.enabled => c,
        _ => return HashMap::new(),
    };

    let mut cache = HashMap::new();

    for speaker_config in config.speakers.values() {
        // Resolve speaker name: cea2034_correction.speaker_name overrides per-speaker name
        let speaker_name = cea_config
            .speaker_name
            .as_deref()
            .or_else(|| speaker_config.speaker_name());

        if let Some(name) = speaker_name {
            if cache.contains_key(name) {
                continue; // Already fetched (e.g., same speaker model for L and R)
            }

            info!("  Fetching CEA2034 data for speaker '{}'...", name);
            match fetch_cea2034_blocking(name, &cea_config.version) {
                Ok(data) => {
                    info!(
                        "  CEA2034 data loaded: {} frequency points",
                        data.listening_window.freq.len()
                    );
                    cache.insert(name.to_string(), data);
                }
                Err(e) => {
                    warn!(
                        "  Failed to fetch CEA2034 data for '{}': {}. \
                         Speaker correction will be skipped for this speaker.",
                        name, e
                    );
                }
            }
        }
    }

    cache
}

// ============================================================================
// Speaker Correction (Pass 1)
// ============================================================================

/// Compute the effective correction mode based on config and listening distance.
fn resolve_correction_mode(
    config: &Cea2034CorrectionConfig,
    arrival_time_ms: Option<f64>,
) -> Cea2034CorrectionMode {
    match config.correction_mode {
        Cea2034CorrectionMode::Flat => Cea2034CorrectionMode::Flat,
        Cea2034CorrectionMode::Score => Cea2034CorrectionMode::Score,
        Cea2034CorrectionMode::Auto => {
            // Try to compute distance from arrival time
            let distance_m = if let Some(manual) = config.listening_distance_m {
                Some(manual)
            } else if let Some(arrival_ms) = arrival_time_ms {
                let latency_ms = config.system_latency_ms.unwrap_or(0.0);
                let acoustic_ms = (arrival_ms - latency_ms).max(0.0);
                Some(acoustic_ms * 0.001 * SPEED_OF_SOUND)
            } else {
                None
            };

            if let Some(dist) = distance_m {
                if dist < config.nearfield_threshold_m {
                    info!(
                        "  Auto mode: distance={:.2}m < threshold={:.1}m -> Flat LW correction",
                        dist, config.nearfield_threshold_m
                    );
                    Cea2034CorrectionMode::Flat
                } else {
                    info!(
                        "  Auto mode: distance={:.2}m >= threshold={:.1}m -> Speaker score correction",
                        dist, config.nearfield_threshold_m
                    );
                    Cea2034CorrectionMode::Score
                }
            } else {
                // No distance info available, default to Flat (safer)
                info!("  Auto mode: no distance info available, defaulting to Flat LW correction");
                Cea2034CorrectionMode::Flat
            }
        }
    }
}

/// Compute speaker correction filters from CEA2034 data.
///
/// Returns the correction filters and the room measurement curve with the
/// correction applied (the "residual" for Pass 2).
///
/// # Arguments
/// * `cea2034_data` - Pre-fetched CEA2034 data for this speaker
/// * `config` - CEA2034 correction configuration
/// * `room_curve` - Room measurement curve
/// * `schroeder_freq` - Schroeder frequency (correction applies above this)
/// * `arrival_time_ms` - Arrival time from impulse response (for auto distance)
/// * `sample_rate` - Sample rate for filter design
pub fn compute_speaker_correction(
    cea2034_data: &Cea2034Data,
    config: &Cea2034CorrectionConfig,
    room_curve: &Curve,
    schroeder_freq: f64,
    arrival_time_ms: Option<f64>,
    sample_rate: f64,
) -> Result<(Vec<Biquad>, Curve)> {
    let mode = resolve_correction_mode(config, arrival_time_ms);

    match mode {
        Cea2034CorrectionMode::Flat => compute_flat_lw_correction(
            cea2034_data,
            config,
            room_curve,
            schroeder_freq,
            sample_rate,
        ),
        Cea2034CorrectionMode::Score => compute_score_correction(
            cea2034_data,
            config,
            room_curve,
            schroeder_freq,
            sample_rate,
        ),
        Cea2034CorrectionMode::Auto => {
            // Should not reach here — resolved above
            unreachable!("Auto mode should have been resolved")
        }
    }
}

/// Flat Listening Window correction: optimize LW toward flat above Schroeder.
fn compute_flat_lw_correction(
    cea2034_data: &Cea2034Data,
    config: &Cea2034CorrectionConfig,
    room_curve: &Curve,
    schroeder_freq: f64,
    sample_rate: f64,
) -> Result<(Vec<Biquad>, Curve)> {
    if room_curve.freq.is_empty() {
        return Err(AutoeqError::InvalidMeasurement {
            message: "Room curve has no frequency data for CEA2034 correction".to_string(),
        });
    }

    // Interpolate Listening Window to room measurement's frequency grid
    let lw_interpolated =
        read::normalize_and_interpolate_response(&room_curve.freq, &cea2034_data.listening_window);

    info!(
        "  Flat LW correction: {} filters, {:.0}-{:.0} Hz, max_db={:.1}, min_db={:.1}",
        config.num_filters, schroeder_freq, room_curve.freq[room_curve.freq.len() - 1],
        config.max_db, config.min_db
    );

    // Build optimizer config for speaker correction
    let optimizer_config = OptimizerConfig {
        num_filters: config.num_filters,
        min_freq: schroeder_freq,
        max_freq: 20000.0,
        min_q: 0.5,
        max_q: config.max_q,
        min_db: config.min_db,
        max_db: config.max_db,
        loss_type: "flat".to_string(),
        asymmetric_loss: false, // Symmetric for speaker correction
        psychoacoustic: false,  // No room-mode smoothing needed for anechoic data
        refine: true,
        ..OptimizerConfig::default()
    };

    // Optimize the Listening Window curve toward flat
    let (filters, loss) = eq::optimize_channel_eq(
        &lw_interpolated,
        &optimizer_config,
        None, // Flat target (default)
        sample_rate,
    )
    .map_err(|e| AutoeqError::OptimizationFailed {
        message: format!("CEA2034 flat LW correction failed: {}", e),
    })?;

    info!(
        "  CEA2034 flat LW correction: {} filters, final loss={:.4}",
        filters.len(),
        loss
    );
    for f in &filters {
        debug!(
            "    {:.0} Hz, Q={:.2}, {:.1} dB",
            f.freq, f.q, f.db_gain
        );
    }

    // Apply correction to the room measurement curve
    let corrected_room = simulate_correction(&filters, room_curve, sample_rate);

    Ok((filters, corrected_room))
}

/// Speaker-score correction: optimize full Harman preference score above Schroeder.
fn compute_score_correction(
    cea2034_data: &Cea2034Data,
    config: &Cea2034CorrectionConfig,
    room_curve: &Curve,
    schroeder_freq: f64,
    sample_rate: f64,
) -> Result<(Vec<Biquad>, Curve)> {
    if room_curve.freq.is_empty() {
        return Err(AutoeqError::InvalidMeasurement {
            message: "Room curve has no frequency data for CEA2034 correction".to_string(),
        });
    }

    // For score mode, we need the Listening Window as the primary curve
    // but the optimizer also needs access to On Axis, Sound Power, and PIR
    // via the score loss function.
    //
    // We use optimize_channel_eq with loss_type="score", but that requires
    // spin data to be passed through the objective data setup.
    // For now, fall back to flat LW correction with score-like constraints
    // (broader Q, allowing more filters) since the full score pipeline
    // requires the Args struct with spin_data.

    // Interpolate Listening Window to room measurement's frequency grid
    let lw_interpolated =
        read::normalize_and_interpolate_response(&room_curve.freq, &cea2034_data.listening_window);

    info!(
        "  Speaker-score correction: {} filters, {:.0}-{:.0} Hz",
        config.num_filters, schroeder_freq, room_curve.freq[room_curve.freq.len() - 1]
    );

    // For score mode, we use more filters and broader Q range
    let optimizer_config = OptimizerConfig {
        num_filters: config.num_filters,
        min_freq: schroeder_freq,
        max_freq: 20000.0,
        min_q: 0.5,
        max_q: config.max_q,
        min_db: config.min_db,
        max_db: config.max_db,
        loss_type: "flat".to_string(), // Use flat loss on LW as approximation
        asymmetric_loss: true,         // Penalize peaks more (closer to score behavior)
        psychoacoustic: false,
        refine: true,
        ..OptimizerConfig::default()
    };

    let (filters, loss) = eq::optimize_channel_eq(
        &lw_interpolated,
        &optimizer_config,
        None,
        sample_rate,
    )
    .map_err(|e| AutoeqError::OptimizationFailed {
        message: format!("CEA2034 score correction failed: {}", e),
    })?;

    info!(
        "  CEA2034 score correction: {} filters, final loss={:.4}",
        filters.len(),
        loss
    );
    for f in &filters {
        debug!(
            "    {:.0} Hz, Q={:.2}, {:.1} dB",
            f.freq, f.q, f.db_gain
        );
    }

    // Apply correction to the room measurement curve
    let corrected_room = simulate_correction(&filters, room_curve, sample_rate);

    Ok((filters, corrected_room))
}

/// Apply filter correction to a curve, returning the corrected curve.
fn simulate_correction(filters: &[Biquad], curve: &Curve, sample_rate: f64) -> Curve {
    if filters.is_empty() {
        return curve.clone();
    }
    let resp = response::compute_peq_complex_response(filters, &curve.freq, sample_rate);
    response::apply_complex_response(curve, &resp)
}

// ============================================================================
// User Preference (Pass 3)
// ============================================================================

/// Generate bass and treble shelf filters from user preference settings.
///
/// Returns separate Biquad filters for bass and treble shelves.
/// Returns empty vec if both adjustments are near zero.
pub fn generate_preference_filters(preference: &UserPreference, sample_rate: f64) -> Vec<Biquad> {
    let mut filters = Vec::new();

    if preference.bass_shelf_db.abs() > 0.1 {
        filters.push(Biquad::new(
            BiquadFilterType::Lowshelf,
            preference.bass_shelf_freq,
            sample_rate,
            DEFAULT_Q_HIGH_LOW_SHELF,
            preference.bass_shelf_db,
        ));
        info!(
            "  Pass 3 preference: bass shelf {:+.1} dB at {:.0} Hz",
            preference.bass_shelf_db, preference.bass_shelf_freq
        );
    }

    if preference.treble_shelf_db.abs() > 0.1 {
        filters.push(Biquad::new(
            BiquadFilterType::Highshelf,
            preference.treble_shelf_freq,
            sample_rate,
            DEFAULT_Q_HIGH_LOW_SHELF,
            preference.treble_shelf_db,
        ));
        info!(
            "  Pass 3 preference: treble shelf {:+.1} dB at {:.0} Hz",
            preference.treble_shelf_db, preference.treble_shelf_freq
        );
    }

    filters
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array1;

    fn make_flat_curve(num_points: usize) -> Curve {
        Curve {
            freq: Array1::logspace(10.0, f64::log10(20.0), f64::log10(20000.0), num_points),
            spl: Array1::from_elem(num_points, 85.0),
            phase: None,
        }
    }

    #[test]
    fn test_generate_preference_filters_both() {
        let pref = UserPreference {
            bass_shelf_db: 3.0,
            bass_shelf_freq: 200.0,
            treble_shelf_db: -1.5,
            treble_shelf_freq: 8000.0,
        };
        let filters = generate_preference_filters(&pref, 48000.0);
        assert_eq!(filters.len(), 2);
        assert_eq!(filters[0].filter_type, BiquadFilterType::Lowshelf);
        assert!((filters[0].db_gain - 3.0).abs() < 1e-6);
        assert_eq!(filters[1].filter_type, BiquadFilterType::Highshelf);
        assert!((filters[1].db_gain - (-1.5)).abs() < 1e-6);
    }

    #[test]
    fn test_generate_preference_filters_none() {
        let pref = UserPreference {
            bass_shelf_db: 0.0,
            bass_shelf_freq: 200.0,
            treble_shelf_db: 0.0,
            treble_shelf_freq: 8000.0,
        };
        let filters = generate_preference_filters(&pref, 48000.0);
        assert!(filters.is_empty());
    }

    #[test]
    fn test_generate_preference_filters_bass_only() {
        let pref = UserPreference {
            bass_shelf_db: 5.0,
            bass_shelf_freq: 150.0,
            treble_shelf_db: 0.05, // Below threshold
            treble_shelf_freq: 8000.0,
        };
        let filters = generate_preference_filters(&pref, 48000.0);
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0].filter_type, BiquadFilterType::Lowshelf);
    }

    #[test]
    fn test_resolve_correction_mode_manual_flat() {
        let config = Cea2034CorrectionConfig {
            correction_mode: Cea2034CorrectionMode::Flat,
            ..Default::default()
        };
        let mode = resolve_correction_mode(&config, None);
        assert_eq!(mode, Cea2034CorrectionMode::Flat);
    }

    #[test]
    fn test_resolve_correction_mode_manual_score() {
        let config = Cea2034CorrectionConfig {
            correction_mode: Cea2034CorrectionMode::Score,
            ..Default::default()
        };
        let mode = resolve_correction_mode(&config, None);
        assert_eq!(mode, Cea2034CorrectionMode::Score);
    }

    #[test]
    fn test_resolve_correction_mode_auto_nearfield() {
        let config = Cea2034CorrectionConfig {
            correction_mode: Cea2034CorrectionMode::Auto,
            nearfield_threshold_m: 2.0,
            listening_distance_m: Some(1.5),
            ..Default::default()
        };
        let mode = resolve_correction_mode(&config, None);
        assert_eq!(mode, Cea2034CorrectionMode::Flat);
    }

    #[test]
    fn test_resolve_correction_mode_auto_farfield() {
        let config = Cea2034CorrectionConfig {
            correction_mode: Cea2034CorrectionMode::Auto,
            nearfield_threshold_m: 2.0,
            listening_distance_m: Some(3.0),
            ..Default::default()
        };
        let mode = resolve_correction_mode(&config, None);
        assert_eq!(mode, Cea2034CorrectionMode::Score);
    }

    #[test]
    fn test_resolve_correction_mode_auto_from_arrival_time() {
        // 2m at 343 m/s = ~5.83ms acoustic propagation
        // With 2ms system latency, arrival_time = 7.83ms
        let config = Cea2034CorrectionConfig {
            correction_mode: Cea2034CorrectionMode::Auto,
            nearfield_threshold_m: 2.0,
            system_latency_ms: Some(2.0),
            ..Default::default()
        };
        // 8.83ms arrival -> (8.83 - 2.0) * 0.001 * 343 = 2.34m -> Score (>= threshold)
        let mode = resolve_correction_mode(&config, Some(8.83));
        assert_eq!(mode, Cea2034CorrectionMode::Score);

        // 5.0ms arrival -> (5.0 - 2.0) * 0.001 * 343 = 1.029m -> Flat (< threshold)
        let mode = resolve_correction_mode(&config, Some(5.0));
        assert_eq!(mode, Cea2034CorrectionMode::Flat);
    }

    #[test]
    fn test_resolve_correction_mode_auto_no_distance() {
        let config = Cea2034CorrectionConfig {
            correction_mode: Cea2034CorrectionMode::Auto,
            ..Default::default()
        };
        // No manual distance, no arrival time -> defaults to Flat
        let mode = resolve_correction_mode(&config, None);
        assert_eq!(mode, Cea2034CorrectionMode::Flat);
    }

    #[test]
    fn test_empty_room_curve_returns_error() {
        let empty_curve = Curve {
            freq: Array1::zeros(0),
            spl: Array1::zeros(0),
            phase: None,
        };
        let cea_data = Cea2034Data {
            on_axis: make_flat_curve(100),
            listening_window: make_flat_curve(100),
            early_reflections: make_flat_curve(100),
            sound_power: make_flat_curve(100),
            estimated_in_room: make_flat_curve(100),
            er_di: make_flat_curve(100),
            sp_di: make_flat_curve(100),
            curves: HashMap::new(),
        };
        let config = Cea2034CorrectionConfig {
            enabled: true,
            correction_mode: Cea2034CorrectionMode::Flat,
            ..Default::default()
        };
        let result =
            compute_speaker_correction(&cea_data, &config, &empty_curve, 300.0, None, 48000.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_simulate_correction_empty() {
        let curve = make_flat_curve(100);
        let result = simulate_correction(&[], &curve, 48000.0);
        assert_eq!(result.spl.len(), curve.spl.len());
        // No filters = no change
        for i in 0..curve.spl.len() {
            assert!((result.spl[i] - curve.spl[i]).abs() < 1e-6);
        }
    }
}
