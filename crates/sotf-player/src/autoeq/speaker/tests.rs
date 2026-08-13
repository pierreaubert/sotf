use super::super::types::SpeakerConfigType;
use super::callback_config::CallbackConfig;
use super::load::load_measurement_as_driver;
use super::optimize::optimize_dba;
use super::optimize::optimize_multidriver;
use super::optimize::optimize_multisub;
use super::run::run_speaker_optimization;
use super::speaker_optimization_config::SpeakerOptimizationConfig;
use super::speaker_optimization_config_ext::SpeakerOptimizationConfigExt;
use super::speaker_optimization_progress::SpeakerOptimizationProgress;
use super::speaker_optimization_result::SpeakerOptimizationResult;
use super::speaker_optimization_result::generate_dummy_result;
use super::types::MeasurementInput;
use super::types::OptimizationStage;
use super::types::PreviewCurves;
use super::types::SpeakerConfigTypeExt;
use std::io::Write;
use tempfile::NamedTempFile;

// ============================================================================
// Configuration Type Tests
// ============================================================================

#[test]
fn test_speaker_config_type_default() {
    let config = SpeakerOptimizationConfig::default();
    assert!(matches!(config.config_type, SpeakerConfigType::Single));
    assert!(config.main_measurement.is_none());
    assert!(config.driver_measurements.is_empty());
}

#[test]
fn test_speaker_config_type_ext_default() {
    let config = SpeakerOptimizationConfigExt::default();
    assert!(matches!(config.config_type, SpeakerConfigTypeExt::Single));
    assert!(config.main_measurement.is_none());
    assert!(config.driver_measurements.is_empty());
    assert!(config.front_measurements.is_empty());
    assert!(config.rear_measurements.is_empty());
}

#[test]
fn test_callback_config_default() {
    let config = CallbackConfig::default();
    assert_eq!(config.interval, 25);
    assert!(config.include_biquads);
    assert!(config.include_filter_response);
}

// ============================================================================
// MeasurementInput Validation Tests
// ============================================================================

#[test]
fn test_load_measurement_spinorama_not_supported() {
    let input = MeasurementInput::Spinorama {
        speaker: "Test".to_string(),
        version: "asr".to_string(),
        measurement: "CEA2034".to_string(),
        curve_name: "LW".to_string(),
    };
    let result = load_measurement_as_driver(&input);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("not supported"));
}

#[test]
fn test_load_measurement_csv_file_not_found() {
    let input = MeasurementInput::CsvFile(std::path::PathBuf::from("/nonexistent/path/driver.csv"));
    let result = load_measurement_as_driver(&input);
    assert!(result.is_err());
}

#[test]
fn test_load_measurement_from_curve() {
    let curve = autoeq::Curve {
        freq: ndarray::Array1::from_vec(vec![20.0, 100.0, 1000.0, 10000.0]),
        spl: ndarray::Array1::from_vec(vec![80.0, 85.0, 90.0, 85.0]),
        phase: None,
        ..Default::default()
    };
    let input = MeasurementInput::Curve(curve);
    let result = load_measurement_as_driver(&input);
    assert!(result.is_ok());
    let driver = result.unwrap();
    assert_eq!(driver.freq.len(), 4);
    assert_eq!(driver.spl.len(), 4);
}

#[test]
fn test_load_measurement_from_curve_with_phase() {
    let curve = autoeq::Curve {
        freq: ndarray::Array1::from_vec(vec![20.0, 100.0, 1000.0]),
        spl: ndarray::Array1::from_vec(vec![80.0, 85.0, 90.0]),
        phase: Some(ndarray::Array1::from_vec(vec![0.0, 45.0, 90.0])),
        ..Default::default()
    };
    let input = MeasurementInput::Curve(curve);
    let result = load_measurement_as_driver(&input);
    assert!(result.is_ok());
    let driver = result.unwrap();
    assert!(driver.phase.is_some());
}

// ============================================================================
// Multi-driver Optimization Validation Tests
// ============================================================================

#[test]
fn test_multidriver_requires_two_drivers() {
    let config = SpeakerOptimizationConfig {
        config_type: SpeakerConfigType::MultiDriver,
        driver_measurements: vec![MeasurementInput::Spinorama {
            speaker: "Test".to_string(),
            version: "asr".to_string(),
            measurement: "CEA2034".to_string(),
            curve_name: "LW".to_string(),
        }],
        ..Default::default()
    };
    let result = optimize_multidriver(&config.driver_measurements, &config, None);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("at least 2 drivers"));
}

#[test]
fn test_multidriver_empty_measurements() {
    let config = SpeakerOptimizationConfig {
        config_type: SpeakerConfigType::MultiDriver,
        driver_measurements: vec![],
        ..Default::default()
    };
    let result = optimize_multidriver(&config.driver_measurements, &config, None);
    assert!(result.is_err());
}

// ============================================================================
// Multi-sub Optimization Validation Tests
// ============================================================================

#[test]
fn test_multisub_requires_measurements() {
    let config = SpeakerOptimizationConfigExt {
        config_type: SpeakerConfigTypeExt::MultiSub,
        driver_measurements: vec![],
        ..Default::default()
    };
    let result = optimize_multisub(&config.driver_measurements, &config, None);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("at least 1 subwoofer"));
}

// ============================================================================
// DBA Optimization Validation Tests
// ============================================================================

#[test]
fn test_dba_requires_front_and_rear() {
    let config = SpeakerOptimizationConfigExt {
        config_type: SpeakerConfigTypeExt::Dba,
        front_measurements: vec![],
        rear_measurements: vec![],
        ..Default::default()
    };
    let result = optimize_dba(&config, None);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("front and rear"));
}

#[test]
fn test_dba_requires_front() {
    let curve = autoeq::Curve {
        freq: ndarray::Array1::from_vec(vec![20.0, 100.0]),
        spl: ndarray::Array1::from_vec(vec![80.0, 85.0]),
        phase: None,
        ..Default::default()
    };
    let config = SpeakerOptimizationConfigExt {
        config_type: SpeakerConfigTypeExt::Dba,
        front_measurements: vec![],
        rear_measurements: vec![MeasurementInput::Curve(curve)],
        ..Default::default()
    };
    let result = optimize_dba(&config, None);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("front and rear"));
}

#[test]
fn test_dba_requires_rear() {
    let curve = autoeq::Curve {
        freq: ndarray::Array1::from_vec(vec![20.0, 100.0]),
        spl: ndarray::Array1::from_vec(vec![80.0, 85.0]),
        phase: None,
        ..Default::default()
    };
    let config = SpeakerOptimizationConfigExt {
        config_type: SpeakerConfigTypeExt::Dba,
        front_measurements: vec![MeasurementInput::Curve(curve)],
        rear_measurements: vec![],
        ..Default::default()
    };
    let result = optimize_dba(&config, None);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("front and rear"));
}

// ============================================================================
// Optimization Progress Tests
// ============================================================================

#[test]
fn test_optimization_progress_from_update() {
    let update = autoeq::ProgressUpdate {
        iteration: 100,
        loss: 0.5,
        score: Some(85.0),
        convergence: 0.001,
        params: vec![1.0, 2.0, 3.0],
        biquads: vec![],
        filter_response: vec![0.0; 10],
        max_iterations: 1000,
    };
    let progress = SpeakerOptimizationProgress::from(&update);
    assert_eq!(progress.iteration, 100);
    assert!((progress.loss - 0.5).abs() < 0.001);
    assert_eq!(progress.score, Some(85.0));
    assert!((progress.convergence - 0.001).abs() < 0.0001);
    assert_eq!(progress.current_params.len(), 3);
    assert_eq!(progress.max_iterations, 1000);
    assert!(matches!(progress.stage, OptimizationStage::Eq));
}

#[test]
fn test_optimization_stage_default() {
    let stage = OptimizationStage::default();
    assert!(matches!(stage, OptimizationStage::Eq));
}

// ============================================================================
// Result Structure Tests
// ============================================================================

#[test]
fn test_dummy_result_generation() {
    let result = generate_dummy_result();
    assert_eq!(result.frequencies.len(), 200);
    assert_eq!(result.input_curve.len(), 200);
    assert_eq!(result.normalized_curve.len(), 200);
    assert_eq!(result.input_curve, result.normalized_curve);
    assert_eq!(result.target_curve.len(), 200);
    assert!(!result.optimization_history.is_empty());
    assert!(result.initial_loss > result.final_loss);
}

#[test]
fn test_speaker_opt_result_from_autoeq() {
    // Create a minimal SpeakerOptResult to test conversion
    let curves = autoeq::VisualizationCurves {
        frequencies: vec![20.0, 100.0, 1000.0],
        input_curve: vec![80.0, 85.0, 90.0],
        target_curve: vec![85.0, 85.0, 85.0],
        deviation_curve: vec![5.0, 0.0, -5.0],
        filter_response: vec![0.0, 0.0, 0.0],
        error_curve: vec![5.0, 0.0, -5.0],
        corrected_curve: vec![85.0, 85.0, 85.0],
        individual_filter_responses: vec![],
    };
    let mut measurement_file = NamedTempFile::new().expect("create measurement fixture");
    writeln!(measurement_file, "frequency,spl\n20,80\n100,85\n1000,90")
        .expect("write measurement fixture");
    let measurement_path = measurement_file.path().to_path_buf();
    let measurement =
        autoeq::read::read_record_from_csv(&measurement_path).expect("load measurement fixture");
    let lineage = autoeq::OptimizationLineage {
        input: measurement.clone(),
        normalized_input: measurement.clone(),
        corrected_output: measurement,
    };
    let optimization_run = autoeq::optim::run_descriptor::OptimizationRunDescriptor {
        schema: "autoeq.optimization-run".to_string(),
        schema_version: 1,
        objective: "test".to_string(),
        parameter_bounds: vec![],
        constraints: vec![],
        seed: None,
        backend: "test".to_string(),
        backend_version: "test".to_string(),
        stopping_reason: "test".to_string(),
        platform: autoeq::optim::run_descriptor::OptimizerExecutionPlatform {
            operating_system: std::env::consts::OS.to_string(),
            architecture: std::env::consts::ARCH.to_string(),
            compiler: "test".to_string(),
        },
    };
    let autoeq_result = autoeq::SpeakerOptResult {
        biquads: vec![],
        curves,
        spin_data: None,
        history: vec![(0, 1.0), (100, 0.1)],
        initial_loss: 1.0,
        final_loss: 0.1,
        optimization_run,
        lineage,
    };
    let result = SpeakerOptimizationResult::from(autoeq_result);
    assert_eq!(result.frequencies.len(), 3);
    assert_eq!(result.normalized_curve.len(), 3);
    assert_eq!(result.input_curve, result.normalized_curve);
    assert!((result.initial_loss - 1.0).abs() < 0.001);
    assert!((result.final_loss - 0.1).abs() < 0.001);
}

// ============================================================================
// Preview Curves Tests
// ============================================================================

#[test]
fn test_preview_curves_struct() {
    let preview = PreviewCurves {
        frequencies: vec![20.0, 100.0, 1000.0],
        input_curve: vec![80.0, 85.0, 90.0],
        target_curve: vec![85.0, 85.0, 85.0],
        deviation_curve: vec![5.0, 0.0, -5.0],
    };
    assert_eq!(preview.frequencies.len(), 3);
    assert_eq!(preview.input_curve.len(), 3);
    assert_eq!(preview.target_curve.len(), 3);
    assert_eq!(preview.deviation_curve.len(), 3);
}

// ============================================================================
// Backward Compatibility Tests
// ============================================================================

#[test]
fn test_run_speaker_optimization_dummy() {
    let args = autoeq::Args::speaker_defaults();
    let result = run_speaker_optimization("Dummy Speaker", &args);
    assert!(result.is_ok());
    let opt_result = result.unwrap();
    assert!(!opt_result.frequencies.is_empty());
}
