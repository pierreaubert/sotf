//! Configuration validation for room EQ.
//!
//! Performs comprehensive validation of RoomConfig before optimization.

use super::types::{OptimizerConfig, RoomConfig, SpeakerConfig, TiltType};
use std::collections::HashMap;

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

    // Warn when both target_curve and target_tilt are set (non-flat)
    if config.target_curve.is_some()
        && let Some(ref tilt) = config.optimizer.target_tilt
        && tilt.tilt_type != TiltType::Flat
    {
        result.add_warning(
            "Both target_curve and target_tilt are configured. \
             target_tilt will be baked into the measurement and target_curve \
             will be ignored to avoid double-application."
                .to_string(),
        );
    }

    // Validate speaker configurations
    validate_speakers(&config.speakers, &mut result);

    // Validate crossover references
    validate_crossovers(&config.speakers, config.crossovers.as_ref(), &mut result);

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
    let is_known = valid_prefixes.iter().any(|p| algo.starts_with(p))
        || valid_bare.contains(&algo);
    if !is_known {
        result.add_warning(format!(
            "Unknown algorithm '{}', may not be supported",
            opt.algorithm
        ));
    }

    // Validate loss type
    let valid_loss_types = ["flat", "score"];
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

    // Validate mode
    let valid_modes = ["iir", "fir", "mixed", "mixed_phase"];
    if !valid_modes.contains(&opt.mode.as_str()) {
        result.add_error(format!(
            "Unknown mode '{}', must be one of {:?}",
            opt.mode, valid_modes
        ));
    }

    // Validate FIR config if mode requires it
    if (opt.mode == "fir" || opt.mode == "mixed") && opt.fir.is_none() {
        result.add_warning(format!(
            "mode '{}' specified but no FIR configuration provided, using defaults",
            opt.mode
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
        // mixed_config is only relevant when mode == "mixed"
        if opt.mode != "mixed" {
            result.add_warning(
                "mixed_config specified but mode is not 'mixed', configuration will be ignored"
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
            result.warnings.iter().any(|w| w.contains("Unknown algorithm")),
            "Unknown algorithm should produce a warning, but warnings: {:?}",
            result.warnings
        );
    }
}
