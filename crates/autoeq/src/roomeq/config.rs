//! Configuration validation for room EQ.
//!
//! Performs comprehensive validation of RoomConfig before optimization.

use super::types::{OptimizerConfig, ProcessingMode, RoomConfig, SpeakerConfig, TargetShape};
use crate::{MeasurementRef, MeasurementSource};
use std::collections::HashMap;

/// Frequency (Hz) above which `ProcessingMode::PhaseLinear` tends to need an
/// impractical number of FIR taps. Crossing this with default FIR settings
/// produces quietly-degraded high-frequency response.
const PHASE_LINEAR_RECOMMENDED_MAX_FREQ_HZ: f64 = 2000.0;

/// Result of configuration validation
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether the configuration is valid
    pub is_valid: bool,
    /// Critical errors that prevent optimization
    pub errors: Vec<String>,
    /// Non-critical warnings that may affect results
    pub warnings: Vec<String>,
}

impl ValidationResult {
    /// Create a valid result with no errors or warnings
    pub fn valid() -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Add an error (marks result as invalid)
    pub fn add_error(&mut self, error: String) {
        self.errors.push(error);
        self.is_valid = false;
    }

    /// Add a warning (does not affect validity)
    pub fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }

    /// Merge another validation result into this one
    #[allow(dead_code)]
    pub fn merge(&mut self, other: ValidationResult) {
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
        self.is_valid = self.is_valid && other.is_valid;
    }

    /// Print validation results to stderr
    pub fn print_results(&self) {
        for warning in &self.warnings {
            eprintln!("Warning: {}", warning);
        }
        for error in &self.errors {
            eprintln!("Error: {}", error);
        }
    }
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self::valid()
    }
}

/// Validate a complete room configuration
pub fn validate_room_config(config: &RoomConfig) -> ValidationResult {
    let mut result = ValidationResult::valid();

    // Validate optimizer config
    validate_optimizer_config(&config.optimizer, &mut result);

    // I1 — warn when both target_curve and a non-trivial target_response
    // are set. `target_response` is baked into the measurement during
    // optimization; when both are present, `target_curve` is silently
    // ignored to prevent double-application. Users often don't realise
    // that setting both means only one takes effect.
    if config.target_curve.is_some()
        && let Some(ref tr) = config.optimizer.target_response
    {
        let has_shape = tr.shape != TargetShape::Flat
            || tr.slope_db_per_octave.abs() > 1e-6
            || tr.preference.bass_shelf_db.abs() > 1e-6
            || tr.preference.treble_shelf_db.abs() > 1e-6
            || tr.broadband_precorrection;
        if has_shape {
            result.add_warning(
                "Both target_curve and target_response are configured. \
                 target_response takes precedence — it is baked into the \
                 measurement before EQ optimization, and target_curve is \
                 ignored to avoid double-application. Set only one."
                    .to_string(),
            );
        }
    }

    // Validate speaker configurations
    validate_speakers(&config.speakers, &mut result);

    // Validate crossover references
    validate_crossovers(&config.speakers, config.crossovers.as_ref(), &mut result);

    // Cross-validate option interactions that depend on the speaker map
    // (multi-measurement weights, CEA2034 source detection).
    validate_cross_option_interactions(config, &mut result);

    result
}

/// Validate optimizer configuration parameters
fn validate_optimizer_config(opt: &OptimizerConfig, result: &mut ValidationResult) {
    if opt.num_filters == 0 {
        result.add_warning("num_filters is 0, no EQ will be applied".to_string());
    }

    if opt.min_freq >= opt.max_freq {
        result.add_error(format!(
            "min_freq ({}) must be less than max_freq ({})",
            opt.min_freq, opt.max_freq
        ));
    }

    if opt.min_freq <= 0.0 {
        result.add_error(format!("min_freq ({}) must be positive", opt.min_freq));
    }

    if opt.max_freq > 24000.0 {
        result.add_warning(format!(
            "max_freq ({}) is above Nyquist for 48kHz sample rate",
            opt.max_freq
        ));
    }

    if opt.min_q > opt.max_q {
        result.add_error(format!(
            "min_q ({}) must be less than or equal to max_q ({})",
            opt.min_q, opt.max_q
        ));
    }

    if opt.min_q <= 0.0 {
        result.add_error(format!("min_q ({}) must be positive", opt.min_q));
    }

    if !(1..=48).contains(&opt.smooth_n) {
        result.add_error(format!(
            "smooth_n ({}) must be in range [1..48]",
            opt.smooth_n
        ));
    }

    if opt.min_db > opt.max_db {
        result.add_error(format!(
            "min_db ({}) must be less than or equal to max_db ({})",
            opt.min_db, opt.max_db
        ));
    }

    if opt.max_iter == 0 {
        result.add_warning("max_iter is 0, optimization will not run".to_string());
    }

    // Validate algorithm choice — accept known library prefixes and bare names
    let valid_prefixes = ["nlopt:", "mh:", "autoeq:"];
    let valid_bare = ["cobyla", "de"];
    let algo = opt.algorithm.as_str();
    let is_known = valid_prefixes.iter().any(|p| algo.starts_with(p)) || valid_bare.contains(&algo);
    if !is_known {
        result.add_warning(format!(
            "Unknown algorithm '{}', may not be supported",
            opt.algorithm
        ));
    }

    // Validate loss type. Keep this in sync with the match arms in
    // `roomeq::eq::optimize_*` — they are the authoritative source.
    let valid_loss_types = ["flat", "score", "epa"];
    if !valid_loss_types.contains(&opt.loss_type.as_str()) {
        result.add_error(format!(
            "Unknown loss_type '{}', must be one of {:?}",
            opt.loss_type, valid_loss_types
        ));
    }

    // Validate PEQ model
    let valid_peq_models = ["pk", "ls-pk-hs", "free"];
    if !valid_peq_models.contains(&opt.peq_model.as_str()) {
        result.add_warning(format!(
            "Unknown peq_model '{}', may not be supported",
            opt.peq_model
        ));
    }

    // Validate CEA2034 correction config
    if let Some(ref cea) = opt.cea2034_correction
        && cea.enabled
    {
        if cea.num_filters == 0 || cea.num_filters > 20 {
            result.add_error(format!(
                "cea2034_correction.num_filters ({}) must be in range [1..20]",
                cea.num_filters
            ));
        }
        if cea.max_q <= 0.0 {
            result.add_error(format!(
                "cea2034_correction.max_q ({}) must be positive",
                cea.max_q
            ));
        }
        if cea.min_db >= 0.0 {
            result.add_warning(format!(
                "cea2034_correction.min_db ({}) is non-negative; speaker correction typically needs cuts",
                cea.min_db
            ));
        }
        if cea.max_db < cea.min_db {
            result.add_error(format!(
                "cea2034_correction.max_db ({}) must be >= min_db ({})",
                cea.max_db, cea.min_db
            ));
        }
        if cea.nearfield_threshold_m <= 0.0 {
            result.add_error(format!(
                "cea2034_correction.nearfield_threshold_m ({}) must be positive",
                cea.nearfield_threshold_m
            ));
        }
    }

    // I5 — PhaseLinear FIR at a wide frequency range silently under-resolves HF.
    // With default tap counts (≤4096), a linear-phase FIR designed for
    // [min_freq .. 20 kHz] lacks the resolution to represent high-frequency
    // room behaviour. The test suite caps max_freq for FIR modes
    // (roomeq_generated_data_test.rs), but production code does not.
    if opt.processing_mode == ProcessingMode::PhaseLinear
        && opt.max_freq > PHASE_LINEAR_RECOMMENDED_MAX_FREQ_HZ
    {
        result.add_warning(format!(
            "processing_mode=phase_linear with max_freq={:.0} Hz exceeds the recommended \
             ceiling of {:.0} Hz for reasonable FIR tap counts. Consider capping max_freq \
             or increasing fir.taps; the resulting correction will otherwise be accurate \
             only in the bass/low-mid range.",
            opt.max_freq, PHASE_LINEAR_RECOMMENDED_MAX_FREQ_HZ
        ));
    }

    // I2 — Schroeder split with a non-zero slope is inherently lossy: the low-
    // and high-frequency regions are optimized independently, so the slope
    // cannot be hit exactly across the crossover. The QA binary documents
    // this empirically in roomeq_qa_quality.rs. Warn the user so they know
    // the target slope will be approximated rather than matched.
    if opt.schroeder_split.as_ref().is_some_and(|s| s.enabled) {
        let has_slope = opt
            .target_response
            .as_ref()
            .map(|t| t.slope_db_per_octave.abs() > f64::EPSILON)
            .unwrap_or(false);
        if has_slope {
            result.add_warning(
                "schroeder_split is enabled together with a non-zero target slope \
                 (target_response.slope_db_per_octave). The modal and diffuse regions \
                 are optimized independently, so the requested slope will be \
                 approximated rather than matched exactly across the crossover."
                    .to_string(),
            );
        }
    }

    // Validate FIR config if processing_mode requires it
    if matches!(
        opt.processing_mode,
        ProcessingMode::PhaseLinear | ProcessingMode::Hybrid | ProcessingMode::MixedPhase
    ) && opt.fir.is_none()
    {
        result.add_warning(format!(
            "processing_mode={:?} requires FIR configuration; using defaults",
            opt.processing_mode
        ));
    }

    if let Some(ref fir) = opt.fir {
        if fir.taps == 0 {
            result.add_error("FIR taps must be greater than 0".to_string());
        }
        if fir.taps < 256 {
            result.add_warning(format!(
                "FIR taps ({}) is low, may result in poor frequency resolution",
                fir.taps
            ));
        }
        let valid_phases = ["linear", "minimum", "kirkeby"];
        if !valid_phases.contains(&fir.phase.to_lowercase().as_str()) {
            result.add_error(format!(
                "Unknown FIR phase '{}', must be one of {:?}",
                fir.phase, valid_phases
            ));
        }
    }

    // Validate mixed mode configuration
    if let Some(ref mixed_config) = opt.mixed_config {
        // mixed_config is only relevant when processing_mode == Hybrid
        if opt.processing_mode != ProcessingMode::Hybrid {
            result.add_warning(
                "mixed_config specified but processing_mode is not Hybrid, configuration will be ignored"
                    .to_string(),
            );
        }

        // Validate crossover frequency
        if mixed_config.crossover_freq <= 0.0 {
            result.add_error(format!(
                "mixed_config.crossover_freq ({}) must be positive",
                mixed_config.crossover_freq
            ));
        }
        if mixed_config.crossover_freq < opt.min_freq {
            result.add_warning(format!(
                "mixed_config.crossover_freq ({}) is below min_freq ({}), some frequencies may not be optimized",
                mixed_config.crossover_freq, opt.min_freq
            ));
        }
        if mixed_config.crossover_freq > opt.max_freq {
            result.add_warning(format!(
                "mixed_config.crossover_freq ({}) is above max_freq ({}), some frequencies may not be optimized",
                mixed_config.crossover_freq, opt.max_freq
            ));
        }

        // Validate crossover type
        let valid_crossover_types = ["LR24", "LR48", "LR4", "LR8"];
        if !valid_crossover_types
            .iter()
            .any(|&t| t.eq_ignore_ascii_case(&mixed_config.crossover_type))
        {
            result.add_error(format!(
                "Unknown mixed_config.crossover_type '{}', must be one of {:?}",
                mixed_config.crossover_type, valid_crossover_types
            ));
        }

        // Validate fir_band
        let valid_fir_bands = ["low", "high"];
        if !valid_fir_bands
            .iter()
            .any(|&b| b.eq_ignore_ascii_case(&mixed_config.fir_band))
        {
            result.add_error(format!(
                "Unknown mixed_config.fir_band '{}', must be 'low' or 'high'",
                mixed_config.fir_band
            ));
        }

        // Validate that each band (FIR and IIR) has a valid frequency range
        let fir_uses_low = mixed_config.fir_band.eq_ignore_ascii_case("low");
        let crossover = mixed_config.crossover_freq;

        if fir_uses_low {
            // FIR handles: min_freq to crossover_freq
            // IIR handles: crossover_freq to max_freq
            if crossover <= opt.min_freq {
                result.add_error(format!(
                    "In mixed mode with fir_band='low', crossover_freq ({}) must be greater than min_freq ({}) \
                    to give the FIR band a valid range",
                    crossover, opt.min_freq
                ));
            }
            if crossover >= opt.max_freq {
                result.add_error(format!(
                    "In mixed mode with fir_band='low', crossover_freq ({}) must be less than max_freq ({}) \
                    to give the IIR band a valid range",
                    crossover, opt.max_freq
                ));
            }
        } else {
            // FIR handles: crossover_freq to max_freq
            // IIR handles: min_freq to crossover_freq
            if crossover <= opt.min_freq {
                result.add_error(format!(
                    "In mixed mode with fir_band='high', crossover_freq ({}) must be greater than min_freq ({}) \
                    to give the IIR band a valid range",
                    crossover, opt.min_freq
                ));
            }
            if crossover >= opt.max_freq {
                result.add_error(format!(
                    "In mixed mode with fir_band='high', crossover_freq ({}) must be less than max_freq ({}) \
                    to give the FIR band a valid range",
                    crossover, opt.max_freq
                ));
            }
        }
    }
}

/// Validate speaker configurations
fn validate_speakers(speakers: &HashMap<String, SpeakerConfig>, result: &mut ValidationResult) {
    if speakers.is_empty() {
        result.add_error("No speakers configured".to_string());
        return;
    }

    for (name, config) in speakers {
        // Validate speaker model name if provided
        if let Some(speaker_name) = config.speaker_name()
            && !is_valid_speaker_name(speaker_name)
        {
            result.add_error(format!(
                "Speaker '{}' has invalid speaker_name '{}'. Only alphanumeric, spaces, and hyphens allowed.",
                name, speaker_name
            ));
        }

        match config {
            SpeakerConfig::Group(group) => {
                if group.measurements.is_empty() {
                    result.add_error(format!("Speaker group '{}' has no measurements", name));
                }
                if group.measurements.len() == 1 {
                    result.add_warning(format!(
                        "Speaker group '{}' has only 1 measurement, consider using Single config",
                        name
                    ));
                }
                if group.crossover.is_none() && group.measurements.len() > 1 {
                    result.add_error(format!(
                        "Speaker group '{}' has multiple drivers but no crossover specified",
                        name
                    ));
                }
            }
            SpeakerConfig::MultiSub(ms) => {
                if ms.subwoofers.is_empty() {
                    result.add_error(format!("Multi-sub '{}' has no subwoofers", name));
                }
                if ms.subwoofers.len() == 1 {
                    result.add_warning(format!(
                        "Multi-sub '{}' has only 1 subwoofer, consider using Single config",
                        name
                    ));
                }
            }
            SpeakerConfig::Dba(dba) => {
                if dba.front.is_empty() {
                    result.add_error(format!("DBA '{}' has no front speakers", name));
                }
                if dba.rear.is_empty() {
                    result.add_error(format!("DBA '{}' has no rear speakers", name));
                }
            }
            SpeakerConfig::Cardioid(cardioid) => {
                if cardioid.separation_meters <= 0.0 {
                    result.add_error(format!(
                        "Cardioid '{}' has invalid separation {:.2}m (must be > 0)",
                        name, cardioid.separation_meters
                    ));
                }
            }
            SpeakerConfig::Single(_) => {
                // Single speaker - minimal validation, path existence checked at load time
            }
        }
    }
}

/// Validate crossover references
fn validate_crossovers(
    speakers: &HashMap<String, SpeakerConfig>,
    crossovers: Option<&HashMap<String, super::types::CrossoverConfig>>,
    result: &mut ValidationResult,
) {
    for (name, config) in speakers {
        let SpeakerConfig::Group(group) = config else {
            continue;
        };
        let Some(ref crossover_ref) = group.crossover else {
            continue;
        };

        let Some(crossovers) = crossovers else {
            result.add_error(format!(
                "Speaker '{}' references crossover '{}' but no crossovers defined",
                name, crossover_ref
            ));
            continue;
        };

        if !crossovers.contains_key(crossover_ref) {
            result.add_error(format!(
                "Speaker '{}' references non-existent crossover '{}'",
                name, crossover_ref
            ));
            continue;
        }

        // Validate crossover config
        let crossover = &crossovers[crossover_ref];
        let num_drivers = group.measurements.len();
        let expected_freqs = num_drivers.saturating_sub(1);

        // Check frequency specification
        let has_single = crossover.frequency.is_some();
        let has_multiple = crossover.frequencies.is_some();
        let has_range = crossover.frequency_range.is_some();

        if has_single && num_drivers != 2 {
            result.add_warning(format!(
                "Crossover '{}' has single frequency but speaker '{}' has {} drivers",
                crossover_ref, name, num_drivers
            ));
        }

        if has_multiple
            && let Some(ref freqs) = crossover.frequencies
            && freqs.len() != expected_freqs
        {
            result.add_error(format!(
                "Crossover '{}' has {} frequencies but speaker '{}' needs {} for {} drivers",
                crossover_ref,
                freqs.len(),
                name,
                expected_freqs,
                num_drivers
            ));
        }

        if !has_single && !has_multiple && !has_range {
            // Will be auto-optimized
            result.add_warning(format!(
                "Crossover '{}' has no frequency specified, will be auto-optimized",
                crossover_ref
            ));
        }
    }
}

/// Validate interactions between optimizer options and the resolved speaker map.
///
/// Covers:
/// - B10: `multi_measurement.weights.len()` must match the number of
///   measurements on every `MeasurementSource::Multiple` in the speaker map.
/// - I4: `cea2034_correction.enabled` requires that at least one speaker
///   carries a CEA2034/spinorama-shaped source (speaker_name set, or a path
///   that contains "cea2034"/"spinorama"). Applying the 3-pass pipeline to
///   plain in-room responses silently produces garbage.
fn validate_cross_option_interactions(config: &RoomConfig, result: &mut ValidationResult) {
    validate_multi_measurement_weights(config, result);
    validate_cea2034_source_plausibility(config, result);
}

/// Collect all `MeasurementSource`s referenced by a speaker, so the validator
/// can inspect counts, paths, and speaker-name metadata uniformly.
fn collect_sources(speaker: &SpeakerConfig) -> Vec<&MeasurementSource> {
    match speaker {
        SpeakerConfig::Single(s) => vec![s],
        SpeakerConfig::Group(g) => g.measurements.iter().collect(),
        SpeakerConfig::MultiSub(m) => m.subwoofers.iter().collect(),
        SpeakerConfig::Cardioid(c) => vec![&c.front, &c.rear],
        SpeakerConfig::Dba(d) => d.front.iter().chain(d.rear.iter()).collect(),
    }
}

fn validate_multi_measurement_weights(config: &RoomConfig, result: &mut ValidationResult) {
    let Some(mm) = config.optimizer.multi_measurement.as_ref() else {
        return;
    };
    let Some(weights) = mm.weights.as_ref() else {
        return;
    };

    for (channel, speaker) in &config.speakers {
        for source in collect_sources(speaker) {
            let count = match source {
                MeasurementSource::Multiple(m) => m.measurements.len(),
                MeasurementSource::InMemoryMultiple(curves) => curves.len(),
                _ => continue,
            };
            if count != weights.len() {
                result.add_error(format!(
                    "Channel '{}': multi_measurement.weights has {} entries but the channel \
                     has {} measurements. The lengths must match; `optimize_channel_eq_multi` \
                     would otherwise index out of bounds.",
                    channel,
                    weights.len(),
                    count,
                ));
            }
        }
    }
}

/// Return true if a measurement path/metadata plausibly points at CEA2034
/// (spinorama) data. The check is heuristic on purpose — the validator's job
/// is to flag the common misuse "`cea2034_correction.enabled=true` applied to
/// plain in-room measurements", not to guarantee correctness.
fn source_is_cea2034_shaped(source: &MeasurementSource) -> bool {
    // A named speaker is the strongest signal: spinorama fetches set it, and
    // the 3-pass pipeline uses that name as a cache key.
    if source.speaker_name().is_some() {
        return true;
    }
    let path_hints = |path: &std::path::Path| {
        let lower = path.to_string_lossy().to_lowercase();
        lower.contains("cea2034") || lower.contains("spinorama") || lower.contains("cea-2034")
    };
    let ref_hint = |r: &MeasurementRef| match r {
        MeasurementRef::Path(p) => path_hints(p),
        MeasurementRef::Named { path, name } => {
            path_hints(path)
                || name
                    .as_deref()
                    .map(|n| {
                        n.to_lowercase().contains("cea2034")
                            || n.to_lowercase().contains("spinorama")
                    })
                    .unwrap_or(false)
        }
        MeasurementRef::Inline(_) => false,
    };
    match source {
        MeasurementSource::Single(s) => ref_hint(&s.measurement),
        MeasurementSource::Multiple(m) => m.measurements.iter().any(ref_hint),
        MeasurementSource::InMemory(_) | MeasurementSource::InMemoryMultiple(_) => false,
    }
}

fn validate_cea2034_source_plausibility(config: &RoomConfig, result: &mut ValidationResult) {
    let enabled = config
        .optimizer
        .cea2034_correction
        .as_ref()
        .is_some_and(|c| c.enabled);
    if !enabled {
        return;
    }

    let any_plausible = config
        .speakers
        .values()
        .flat_map(collect_sources)
        .any(source_is_cea2034_shaped);

    if !any_plausible {
        result.add_warning(
            "cea2034_correction is enabled but no speaker looks like a CEA2034/spinorama \
             source (no speaker_name set, no path/name hint of 'cea2034' or 'spinorama'). \
             The 3-pass correction pipeline assumes spinorama-shaped data; applying it to \
             plain in-room responses will produce incorrect results. \
             Either disable cea2034_correction or provide a speaker_name so the pipeline \
             can fetch the matching spinorama data."
                .to_string(),
        );
    }
}

/// Check if a speaker name is valid (alphanumeric, spaces, hyphens)
fn is_valid_speaker_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    name.chars()
        .all(|c| c.is_alphanumeric() || c == ' ' || c == '-')
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::roomeq::types::*;
    use crate::{MeasurementRef, MeasurementSingle, MeasurementSource};
    use std::path::PathBuf;

    #[test]
    fn test_validation_result_default_is_valid() {
        let result = ValidationResult::default();
        assert!(result.is_valid);
        assert!(result.errors.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_validation_result_add_error_invalidates() {
        let mut result = ValidationResult::valid();
        result.add_error("Test error".to_string());
        assert!(!result.is_valid);
        assert_eq!(result.errors.len(), 1);
    }

    #[test]
    fn test_validation_result_add_warning_keeps_valid() {
        let mut result = ValidationResult::valid();
        result.add_warning("Test warning".to_string());
        assert!(result.is_valid);
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn test_validate_empty_speakers() {
        let config = RoomConfig {
            version: default_config_version(),
            system: None,
            speakers: HashMap::new(),
            crossovers: None,
            target_curve: None,
            optimizer: OptimizerConfig::default(),
            recording_config: None,
            cea2034_cache: None,
        };

        let result = validate_room_config(&config);
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("No speakers")));
    }

    #[test]
    fn test_validate_min_freq_greater_than_max() {
        let mut speakers = HashMap::new();
        speakers.insert(
            "left".to_string(),
            SpeakerConfig::Single(MeasurementSource::Single(MeasurementSingle {
                measurement: MeasurementRef::Path(PathBuf::from("test.csv")),
                speaker_name: None,
            })),
        );

        let mut optimizer = OptimizerConfig::default();
        optimizer.min_freq = 20000.0;
        optimizer.max_freq = 20.0;

        let config = RoomConfig {
            version: default_config_version(),
            system: None,
            speakers,
            crossovers: None,
            target_curve: None,
            optimizer,
            recording_config: None,
            cea2034_cache: None,
        };

        let result = validate_room_config(&config);
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("min_freq")));
    }

    #[test]
    fn test_validate_crossover_reference() {
        let mut speakers = HashMap::new();
        speakers.insert(
            "left".to_string(),
            SpeakerConfig::Group(SpeakerGroup {
                name: "Test".to_string(),
                speaker_name: None,
                measurements: vec![
                    MeasurementSource::Single(MeasurementSingle {
                        measurement: MeasurementRef::Path(PathBuf::from("woofer.csv")),
                        speaker_name: None,
                    }),
                    MeasurementSource::Single(MeasurementSingle {
                        measurement: MeasurementRef::Path(PathBuf::from("tweeter.csv")),
                        speaker_name: None,
                    }),
                ],
                crossover: Some("nonexistent".to_string()),
            }),
        );

        let config = RoomConfig {
            version: default_config_version(),
            system: None,
            speakers,
            crossovers: Some(HashMap::new()), // Empty crossovers
            target_curve: None,
            optimizer: OptimizerConfig::default(),
            recording_config: None,
            cea2034_cache: None,
        };

        let result = validate_room_config(&config);
        assert!(!result.is_valid);
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("non-existent crossover"))
        );
    }

    #[test]
    fn test_validate_speaker_name() {
        let mut speakers = HashMap::new();
        speakers.insert(
            "left".to_string(),
            SpeakerConfig::Single(MeasurementSource::Single(MeasurementSingle {
                measurement: MeasurementRef::Path(PathBuf::from("left.csv")),
                speaker_name: Some("Invalid @ Name".to_string()),
            })),
        );

        let config = RoomConfig {
            version: default_config_version(),
            system: None,
            speakers,
            crossovers: None,
            target_curve: None,
            optimizer: OptimizerConfig::default(),
            recording_config: None,
            cea2034_cache: None,
        };

        let result = validate_room_config(&config);
        assert!(!result.is_valid);
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("invalid speaker_name"))
        );
    }

    // ========================================================================
    // Group 4: Algorithm validation
    // ========================================================================

    /// Helper to create a minimal valid RoomConfig with a given algorithm
    fn config_with_algorithm(algo: &str) -> RoomConfig {
        let mut speakers = HashMap::new();
        speakers.insert(
            "left".to_string(),
            SpeakerConfig::Single(MeasurementSource::Single(MeasurementSingle {
                measurement: MeasurementRef::Path(PathBuf::from("test.csv")),
                speaker_name: None,
            })),
        );
        let mut optimizer = OptimizerConfig::default();
        optimizer.algorithm = algo.to_string();
        RoomConfig {
            version: default_config_version(),
            system: None,
            speakers,
            crossovers: None,
            target_curve: None,
            optimizer,
            recording_config: None,
            cea2034_cache: None,
        }
    }

    #[test]
    fn test_all_algorithm_prefixes_accepted() {
        // Bug #7: mh:firefly was flagged as unknown. All prefixed algorithms
        // (mh:*, nlopt:*, autoeq:*) and bare names (cobyla, de) must be valid.
        let valid_algos = [
            "mh:firefly",
            "mh:pso",
            "nlopt:cobyla",
            "nlopt:isres",
            "autoeq:de",
            "cobyla",
            "de",
        ];
        for algo in &valid_algos {
            let config = config_with_algorithm(algo);
            let result = validate_room_config(&config);
            let has_algo_warning = result
                .warnings
                .iter()
                .any(|w| w.contains("Unknown algorithm"));
            assert!(
                !has_algo_warning,
                "Algorithm '{}' should be accepted without warning, but got: {:?}",
                algo, result.warnings
            );
        }
    }

    #[test]
    fn test_unknown_algorithm_warns_not_errors() {
        // An unrecognized algorithm should produce a warning, not an error.
        // The config should still be valid (algo might be a plugin).
        let config = config_with_algorithm("bogus_algo");
        let result = validate_room_config(&config);
        assert!(
            result.is_valid,
            "Unknown algorithm should warn, not error. Errors: {:?}",
            result.errors
        );
        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.contains("Unknown algorithm")),
            "Unknown algorithm should produce a warning, but warnings: {:?}",
            result.warnings
        );
    }
}
