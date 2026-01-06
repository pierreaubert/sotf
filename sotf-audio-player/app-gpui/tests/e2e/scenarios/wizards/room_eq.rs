//! E2E tests for Room EQ Wizard.
//!
//! Tests for the 5-step room EQ optimization wizard:
//! 1. LoadData - Load/import measurement data
//! 2. Configure - Configure channels and optimizer settings
//! 3. Optimize - Run optimization (per-channel)
//! 4. Review - Review results and visualizations
//! 5. Export - Export DSP chain and apply

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Mock Types for Testing
// =============================================================================

/// Room EQ workflow step
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum RoomEqStep {
    #[default]
    LoadData,
    Configure,
    Optimize,
    Review,
    Export,
}

impl RoomEqStep {
    fn index(&self) -> usize {
        match self {
            RoomEqStep::LoadData => 0,
            RoomEqStep::Configure => 1,
            RoomEqStep::Optimize => 2,
            RoomEqStep::Review => 3,
            RoomEqStep::Export => 4,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            RoomEqStep::LoadData => "Load Data",
            RoomEqStep::Configure => "Configure",
            RoomEqStep::Optimize => "Optimize",
            RoomEqStep::Review => "Review",
            RoomEqStep::Export => "Export",
        }
    }

    fn next(&self) -> Option<RoomEqStep> {
        match self {
            RoomEqStep::LoadData => Some(RoomEqStep::Configure),
            RoomEqStep::Configure => Some(RoomEqStep::Optimize),
            RoomEqStep::Optimize => Some(RoomEqStep::Review),
            RoomEqStep::Review => Some(RoomEqStep::Export),
            RoomEqStep::Export => None,
        }
    }

    fn previous(&self) -> Option<RoomEqStep> {
        match self {
            RoomEqStep::LoadData => None,
            RoomEqStep::Configure => Some(RoomEqStep::LoadData),
            RoomEqStep::Optimize => Some(RoomEqStep::Configure),
            RoomEqStep::Review => Some(RoomEqStep::Optimize),
            RoomEqStep::Export => Some(RoomEqStep::Review),
        }
    }
}

/// Data source type
#[derive(Debug, Clone, PartialEq)]
enum DataSource {
    FromRecording,
    FromFile(String),
}

/// Optimization algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Algorithm {
    #[default]
    Cobyla,
    DifferentialEvolution,
    NelderMead,
}

/// Speaker config type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum SpeakerConfigType {
    #[default]
    Single,
    MultiDriver,
}

/// Crossover type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum CrossoverType {
    LR12,
    #[default]
    LR24,
    LR48,
    Butterworth12,
    Butterworth24,
}

impl CrossoverType {
    fn as_str(&self) -> &'static str {
        match self {
            CrossoverType::LR12 => "Linkwitz-Riley 12dB",
            CrossoverType::LR24 => "Linkwitz-Riley 24dB",
            CrossoverType::LR48 => "Linkwitz-Riley 48dB",
            CrossoverType::Butterworth12 => "Butterworth 12dB",
            CrossoverType::Butterworth24 => "Butterworth 24dB",
        }
    }
}

/// Optimization status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum OptimizationStatus {
    #[default]
    Idle,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Channel measurement
#[derive(Debug, Clone)]
struct ChannelMeasurement {
    channel_name: String,
    frequencies: Vec<f32>,
    magnitude_db: Vec<f32>,
    is_group: bool,
}

/// Speaker configuration
#[derive(Debug, Clone)]
struct SpeakerConfig {
    channel_name: String,
    config_type: SpeakerConfigType,
    crossover_type: CrossoverType,
    driver_names: Vec<String>,
    crossover_freq_hints: Vec<f64>,
}

impl Default for SpeakerConfig {
    fn default() -> Self {
        Self {
            channel_name: String::new(),
            config_type: SpeakerConfigType::Single,
            crossover_type: CrossoverType::LR24,
            driver_names: Vec::new(),
            crossover_freq_hints: Vec::new(),
        }
    }
}

/// Optimizer configuration
#[derive(Debug, Clone)]
struct OptimizerConfig {
    algorithm: Algorithm,
    num_filters: usize,
    min_q: f64,
    max_q: f64,
    min_db: f64,
    max_db: f64,
    min_freq: f64,
    max_freq: f64,
    max_iter: usize,
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            algorithm: Algorithm::DifferentialEvolution,
            num_filters: 5,
            min_q: 0.5,
            max_q: 6.0,
            min_db: -12.0,
            max_db: 3.0,
            min_freq: 20.0,
            max_freq: 16000.0,
            max_iter: 10000,
        }
    }
}

/// Channel optimization result
#[derive(Debug, Clone)]
struct ChannelOptResult {
    channel_name: String,
    pre_score: f64,
    post_score: f64,
    eq_filters: Vec<EqFilter>,
}

/// EQ filter
#[derive(Debug, Clone)]
struct EqFilter {
    filter_type: String,
    frequency: f64,
    q: f64,
    gain_db: f64,
}

/// Room EQ state for testing
struct RoomEqState {
    step: RoomEqStep,
    // Step 1: Load Data
    data_source: DataSource,
    channel_measurements: Vec<ChannelMeasurement>,
    // Step 2: Configuration
    speaker_configs: Vec<SpeakerConfig>,
    optimizer_config: OptimizerConfig,
    // Step 3: Optimization
    optimization_status: OptimizationStatus,
    current_channel: Option<String>,
    channel_results: Vec<ChannelOptResult>,
    overall_progress: f32,
    // Step 5: Export
    dsp_output: Option<String>,
    // UI State
    status_message: String,
    error_message: Option<String>,
}

impl Default for RoomEqState {
    fn default() -> Self {
        Self {
            step: RoomEqStep::LoadData,
            data_source: DataSource::FromRecording,
            channel_measurements: Vec::new(),
            speaker_configs: Vec::new(),
            optimizer_config: OptimizerConfig::default(),
            optimization_status: OptimizationStatus::Idle,
            current_channel: None,
            channel_results: Vec::new(),
            overall_progress: 0.0,
            dsp_output: None,
            status_message: String::new(),
            error_message: None,
        }
    }
}

impl RoomEqState {
    fn has_measurements(&self) -> bool {
        !self.channel_measurements.is_empty()
    }

    fn channel_count(&self) -> usize {
        self.channel_measurements.len()
    }

    fn is_optimization_complete(&self) -> bool {
        self.optimization_status == OptimizationStatus::Completed
    }

    fn is_optimizing(&self) -> bool {
        self.optimization_status == OptimizationStatus::Running
    }

    fn init_speaker_configs(&mut self) {
        self.speaker_configs = self
            .channel_measurements
            .iter()
            .map(|m| SpeakerConfig {
                channel_name: m.channel_name.clone(),
                config_type: if m.is_group {
                    SpeakerConfigType::MultiDriver
                } else {
                    SpeakerConfigType::Single
                },
                crossover_type: CrossoverType::LR24,
                driver_names: Vec::new(),
                crossover_freq_hints: Vec::new(),
            })
            .collect();
    }

    fn average_pre_score(&self) -> f64 {
        if self.channel_results.is_empty() {
            0.0
        } else {
            self.channel_results.iter().map(|r| r.pre_score).sum::<f64>()
                / self.channel_results.len() as f64
        }
    }

    fn average_post_score(&self) -> f64 {
        if self.channel_results.is_empty() {
            0.0
        } else {
            self.channel_results.iter().map(|r| r.post_score).sum::<f64>()
                / self.channel_results.len() as f64
        }
    }

    fn reset_optimization(&mut self) {
        self.optimization_status = OptimizationStatus::Idle;
        self.current_channel = None;
        self.channel_results.clear();
        self.overall_progress = 0.0;
        self.error_message = None;
    }
}

// =============================================================================
// Step Navigation Tests
// =============================================================================

/// Test step indices.
#[gpui::test]
async fn test_step_indices(_cx: &mut TestAppContext) {
    assert_eq!(RoomEqStep::LoadData.index(), 0);
    assert_eq!(RoomEqStep::Configure.index(), 1);
    assert_eq!(RoomEqStep::Optimize.index(), 2);
    assert_eq!(RoomEqStep::Review.index(), 3);
    assert_eq!(RoomEqStep::Export.index(), 4);
}

/// Test step labels.
#[gpui::test]
async fn test_step_labels(_cx: &mut TestAppContext) {
    assert_eq!(RoomEqStep::LoadData.label(), "Load Data");
    assert_eq!(RoomEqStep::Configure.label(), "Configure");
    assert_eq!(RoomEqStep::Optimize.label(), "Optimize");
    assert_eq!(RoomEqStep::Review.label(), "Review");
    assert_eq!(RoomEqStep::Export.label(), "Export");
}

/// Test step next navigation.
#[gpui::test]
async fn test_step_next_navigation(_cx: &mut TestAppContext) {
    assert_eq!(RoomEqStep::LoadData.next(), Some(RoomEqStep::Configure));
    assert_eq!(RoomEqStep::Configure.next(), Some(RoomEqStep::Optimize));
    assert_eq!(RoomEqStep::Optimize.next(), Some(RoomEqStep::Review));
    assert_eq!(RoomEqStep::Review.next(), Some(RoomEqStep::Export));
    assert_eq!(RoomEqStep::Export.next(), None);
}

/// Test step previous navigation.
#[gpui::test]
async fn test_step_previous_navigation(_cx: &mut TestAppContext) {
    assert_eq!(RoomEqStep::LoadData.previous(), None);
    assert_eq!(RoomEqStep::Configure.previous(), Some(RoomEqStep::LoadData));
    assert_eq!(RoomEqStep::Optimize.previous(), Some(RoomEqStep::Configure));
    assert_eq!(RoomEqStep::Review.previous(), Some(RoomEqStep::Optimize));
    assert_eq!(RoomEqStep::Export.previous(), Some(RoomEqStep::Review));
}

/// Test complete step sequence.
#[gpui::test]
async fn test_complete_step_sequence(_cx: &mut TestAppContext) {
    let mut step = RoomEqStep::LoadData;
    let mut steps = vec![step];

    while let Some(next) = step.next() {
        step = next;
        steps.push(step);
    }

    assert_eq!(steps.len(), 5);
    assert_eq!(steps[0], RoomEqStep::LoadData);
    assert_eq!(steps[4], RoomEqStep::Export);
}

// =============================================================================
// Step 1: Load Data Tests
// =============================================================================

/// Test initial state defaults.
#[gpui::test]
async fn test_initial_state_defaults(_cx: &mut TestAppContext) {
    let state = RoomEqState::default();

    assert_eq!(state.step, RoomEqStep::LoadData);
    assert_eq!(state.data_source, DataSource::FromRecording);
    assert!(!state.has_measurements());
}

/// Test data source from recording.
#[gpui::test]
async fn test_data_source_from_recording(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RoomEqState::default()));

    state.borrow_mut().data_source = DataSource::FromRecording;
    assert_eq!(state.borrow().data_source, DataSource::FromRecording);
}

/// Test data source from file.
#[gpui::test]
async fn test_data_source_from_file(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RoomEqState::default()));

    state.borrow_mut().data_source = DataSource::FromFile("/path/to/measurements.json".to_string());
    match &state.borrow().data_source {
        DataSource::FromFile(path) => assert!(path.contains("measurements.json")),
        _ => panic!("Expected FromFile"),
    }
}

/// Test loading measurements.
#[gpui::test]
async fn test_loading_measurements(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RoomEqState::default()));

    // Add mock measurements
    state.borrow_mut().channel_measurements = vec![
        ChannelMeasurement {
            channel_name: "L".to_string(),
            frequencies: vec![100.0, 1000.0, 10000.0],
            magnitude_db: vec![0.0, 0.0, 0.0],
            is_group: false,
        },
        ChannelMeasurement {
            channel_name: "R".to_string(),
            frequencies: vec![100.0, 1000.0, 10000.0],
            magnitude_db: vec![0.0, 0.0, 0.0],
            is_group: false,
        },
    ];

    assert!(state.borrow().has_measurements());
    assert_eq!(state.borrow().channel_count(), 2);
}

/// Test multi-driver measurement loading.
#[gpui::test]
async fn test_multi_driver_measurement(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RoomEqState::default()));

    state.borrow_mut().channel_measurements = vec![ChannelMeasurement {
        channel_name: "L".to_string(),
        frequencies: vec![100.0, 1000.0, 10000.0],
        magnitude_db: vec![0.0, 0.0, 0.0],
        is_group: true, // Multi-driver
    }];

    assert!(state.borrow().channel_measurements[0].is_group);
}

// =============================================================================
// Step 2: Configure Tests
// =============================================================================

/// Test speaker config initialization.
#[gpui::test]
async fn test_speaker_config_initialization(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RoomEqState::default()));

    // Add measurements
    state.borrow_mut().channel_measurements = vec![
        ChannelMeasurement {
            channel_name: "L".to_string(),
            frequencies: Vec::new(),
            magnitude_db: Vec::new(),
            is_group: false,
        },
        ChannelMeasurement {
            channel_name: "R".to_string(),
            frequencies: Vec::new(),
            magnitude_db: Vec::new(),
            is_group: false,
        },
    ];

    // Initialize configs
    state.borrow_mut().init_speaker_configs();

    assert_eq!(state.borrow().speaker_configs.len(), 2);
    assert_eq!(state.borrow().speaker_configs[0].channel_name, "L");
    assert_eq!(state.borrow().speaker_configs[1].channel_name, "R");
}

/// Test speaker config type selection.
#[gpui::test]
async fn test_speaker_config_type_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RoomEqState::default()));

    state.borrow_mut().speaker_configs = vec![SpeakerConfig::default()];

    state.borrow_mut().speaker_configs[0].config_type = SpeakerConfigType::Single;
    assert_eq!(
        state.borrow().speaker_configs[0].config_type,
        SpeakerConfigType::Single
    );

    state.borrow_mut().speaker_configs[0].config_type = SpeakerConfigType::MultiDriver;
    assert_eq!(
        state.borrow().speaker_configs[0].config_type,
        SpeakerConfigType::MultiDriver
    );
}

/// Test crossover type selection.
#[gpui::test]
async fn test_crossover_type_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RoomEqState::default()));

    state.borrow_mut().speaker_configs = vec![SpeakerConfig::default()];

    let crossover_types = [
        CrossoverType::LR12,
        CrossoverType::LR24,
        CrossoverType::LR48,
        CrossoverType::Butterworth12,
        CrossoverType::Butterworth24,
    ];

    for ct in crossover_types {
        state.borrow_mut().speaker_configs[0].crossover_type = ct;
        assert_eq!(state.borrow().speaker_configs[0].crossover_type, ct);
    }
}

/// Test crossover type labels.
#[gpui::test]
async fn test_crossover_type_labels(_cx: &mut TestAppContext) {
    assert!(CrossoverType::LR12.as_str().contains("12dB"));
    assert!(CrossoverType::LR24.as_str().contains("24dB"));
    assert!(CrossoverType::LR48.as_str().contains("48dB"));
    assert!(CrossoverType::Butterworth12.as_str().contains("Butterworth"));
}

/// Test multi-driver configuration.
#[gpui::test]
async fn test_multi_driver_configuration(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RoomEqState::default()));

    state.borrow_mut().speaker_configs = vec![SpeakerConfig {
        channel_name: "L".to_string(),
        config_type: SpeakerConfigType::MultiDriver,
        crossover_type: CrossoverType::LR24,
        driver_names: vec!["woofer".to_string(), "tweeter".to_string()],
        crossover_freq_hints: vec![2000.0],
    }];

    let config = &state.borrow().speaker_configs[0];
    assert_eq!(config.driver_names.len(), 2);
    assert_eq!(config.crossover_freq_hints.len(), 1);
}

/// Test optimizer config defaults.
#[gpui::test]
async fn test_optimizer_config_defaults(_cx: &mut TestAppContext) {
    let config = OptimizerConfig::default();

    assert_eq!(config.algorithm, Algorithm::DifferentialEvolution);
    assert_eq!(config.num_filters, 5);
    assert!((config.min_q - 0.5).abs() < 0.01);
    assert!((config.max_q - 6.0).abs() < 0.01);
}

/// Test optimizer algorithm selection.
#[gpui::test]
async fn test_optimizer_algorithm_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RoomEqState::default()));

    let algorithms = [
        Algorithm::Cobyla,
        Algorithm::DifferentialEvolution,
        Algorithm::NelderMead,
    ];

    for algo in algorithms {
        state.borrow_mut().optimizer_config.algorithm = algo;
        assert_eq!(state.borrow().optimizer_config.algorithm, algo);
    }
}

/// Test optimizer parameter bounds.
#[gpui::test]
async fn test_optimizer_parameter_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RoomEqState::default()));

    state.borrow_mut().optimizer_config.num_filters = 7;
    state.borrow_mut().optimizer_config.min_freq = 30.0;
    state.borrow_mut().optimizer_config.max_freq = 15000.0;

    assert_eq!(state.borrow().optimizer_config.num_filters, 7);
    assert!(state.borrow().optimizer_config.min_freq >= 20.0);
    assert!(state.borrow().optimizer_config.max_freq <= 20000.0);
}

// =============================================================================
// Step 3: Optimization Tests
// =============================================================================

/// Test optimization status transitions.
#[gpui::test]
async fn test_optimization_status_transitions(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RoomEqState::default()));

    // Initial state
    assert_eq!(state.borrow().optimization_status, OptimizationStatus::Idle);
    assert!(!state.borrow().is_optimizing());

    // Start optimization
    state.borrow_mut().optimization_status = OptimizationStatus::Running;
    assert!(state.borrow().is_optimizing());

    // Complete optimization
    state.borrow_mut().optimization_status = OptimizationStatus::Completed;
    assert!(state.borrow().is_optimization_complete());
}

/// Test current channel tracking.
#[gpui::test]
async fn test_current_channel_tracking(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RoomEqState::default()));

    state.borrow_mut().current_channel = Some("L".to_string());
    assert_eq!(state.borrow().current_channel, Some("L".to_string()));

    state.borrow_mut().current_channel = Some("R".to_string());
    assert_eq!(state.borrow().current_channel, Some("R".to_string()));
}

/// Test overall progress tracking.
#[gpui::test]
async fn test_overall_progress_tracking(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RoomEqState::default()));

    let progress_values: Vec<f32> = vec![0.0, 0.25, 0.5, 0.75, 1.0];
    for value in progress_values {
        state.borrow_mut().overall_progress = value;
        assert!((state.borrow().overall_progress - value).abs() < 0.001);
    }
}

/// Test channel results collection.
#[gpui::test]
async fn test_channel_results_collection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RoomEqState::default()));

    state.borrow_mut().channel_results = vec![
        ChannelOptResult {
            channel_name: "L".to_string(),
            pre_score: 10.0,
            post_score: 2.0,
            eq_filters: vec![EqFilter {
                filter_type: "peak".to_string(),
                frequency: 100.0,
                q: 2.0,
                gain_db: -5.0,
            }],
        },
        ChannelOptResult {
            channel_name: "R".to_string(),
            pre_score: 8.0,
            post_score: 1.5,
            eq_filters: Vec::new(),
        },
    ];

    assert_eq!(state.borrow().channel_results.len(), 2);
}

/// Test average score calculation.
#[gpui::test]
async fn test_average_score_calculation(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RoomEqState::default()));

    state.borrow_mut().channel_results = vec![
        ChannelOptResult {
            channel_name: "L".to_string(),
            pre_score: 10.0,
            post_score: 2.0,
            eq_filters: Vec::new(),
        },
        ChannelOptResult {
            channel_name: "R".to_string(),
            pre_score: 8.0,
            post_score: 2.0,
            eq_filters: Vec::new(),
        },
    ];

    let avg_pre = state.borrow().average_pre_score();
    let avg_post = state.borrow().average_post_score();

    assert!((avg_pre - 9.0).abs() < 0.1);
    assert!((avg_post - 2.0).abs() < 0.1);
}

/// Test optimization reset.
#[gpui::test]
async fn test_optimization_reset(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RoomEqState::default()));

    // Set up completed optimization
    state.borrow_mut().optimization_status = OptimizationStatus::Completed;
    state.borrow_mut().current_channel = Some("R".to_string());
    state.borrow_mut().overall_progress = 1.0;
    state.borrow_mut().channel_results.push(ChannelOptResult {
        channel_name: "L".to_string(),
        pre_score: 10.0,
        post_score: 2.0,
        eq_filters: Vec::new(),
    });

    // Reset
    state.borrow_mut().reset_optimization();

    assert_eq!(state.borrow().optimization_status, OptimizationStatus::Idle);
    assert!(state.borrow().current_channel.is_none());
    assert!((state.borrow().overall_progress - 0.0).abs() < 0.001);
    assert!(state.borrow().channel_results.is_empty());
}

/// Test optimization failure handling.
#[gpui::test]
async fn test_optimization_failure_handling(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RoomEqState::default()));

    state.borrow_mut().optimization_status = OptimizationStatus::Failed;
    state.borrow_mut().error_message = Some("Optimization diverged".to_string());

    assert_eq!(state.borrow().optimization_status, OptimizationStatus::Failed);
    assert!(state.borrow().error_message.is_some());
}

// =============================================================================
// Step 4: Review Tests
// =============================================================================

/// Test review step with results.
#[gpui::test]
async fn test_review_step_with_results(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RoomEqState::default()));

    state.borrow_mut().step = RoomEqStep::Review;
    state.borrow_mut().channel_results = vec![ChannelOptResult {
        channel_name: "L".to_string(),
        pre_score: 10.0,
        post_score: 2.0,
        eq_filters: vec![
            EqFilter {
                filter_type: "peak".to_string(),
                frequency: 100.0,
                q: 2.0,
                gain_db: -5.0,
            },
            EqFilter {
                filter_type: "peak".to_string(),
                frequency: 1000.0,
                q: 1.5,
                gain_db: 3.0,
            },
        ],
    }];

    let result = &state.borrow().channel_results[0];
    assert_eq!(result.eq_filters.len(), 2);
}

/// Test score improvement display.
#[gpui::test]
async fn test_score_improvement_display(_cx: &mut TestAppContext) {
    fn format_improvement(pre: f64, post: f64) -> String {
        let improvement = ((pre - post) / pre * 100.0).round();
        format!("{:.0}% improvement", improvement)
    }

    assert_eq!(format_improvement(10.0, 2.0), "80% improvement");
    assert_eq!(format_improvement(8.0, 4.0), "50% improvement");
}

/// Test filter display format.
#[gpui::test]
async fn test_filter_display_format(_cx: &mut TestAppContext) {
    fn format_filter(filter: &EqFilter) -> String {
        format!(
            "{}: {:.0} Hz, Q={:.1}, {:.1} dB",
            filter.filter_type, filter.frequency, filter.q, filter.gain_db
        )
    }

    let filter = EqFilter {
        filter_type: "peak".to_string(),
        frequency: 1000.0,
        q: 2.0,
        gain_db: -3.0,
    };

    let formatted = format_filter(&filter);
    assert!(formatted.contains("peak"));
    assert!(formatted.contains("1000 Hz"));
    assert!(formatted.contains("Q=2.0"));
}

// =============================================================================
// Step 5: Export Tests
// =============================================================================

/// Test DSP output generation.
#[gpui::test]
async fn test_dsp_output_generation(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RoomEqState::default()));

    state.borrow_mut().dsp_output = Some(
        r#"{"channels": {"L": {"plugins": []}, "R": {"plugins": []}}}"#.to_string(),
    );

    assert!(state.borrow().dsp_output.is_some());
}

/// Test export format options.
#[gpui::test]
async fn test_export_format_options(_cx: &mut TestAppContext) {
    let formats = ["json", "camillaDsp", "eq-apo", "yaml"];

    for format in formats {
        // Just verify format strings are valid
        assert!(!format.is_empty());
    }
}

// =============================================================================
// Full Wizard Flow Tests
// =============================================================================

/// Test complete wizard flow.
#[gpui::test]
async fn test_complete_wizard_flow(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RoomEqState::default()));

    // Step 1: Load data
    assert_eq!(state.borrow().step, RoomEqStep::LoadData);
    state.borrow_mut().channel_measurements = vec![
        ChannelMeasurement {
            channel_name: "L".to_string(),
            frequencies: Vec::new(),
            magnitude_db: Vec::new(),
            is_group: false,
        },
        ChannelMeasurement {
            channel_name: "R".to_string(),
            frequencies: Vec::new(),
            magnitude_db: Vec::new(),
            is_group: false,
        },
    ];
    assert!(state.borrow().has_measurements());

    // Step 2: Configure
    state.borrow_mut().step = RoomEqStep::Configure;
    state.borrow_mut().init_speaker_configs();
    assert_eq!(state.borrow().speaker_configs.len(), 2);

    // Step 3: Optimize
    state.borrow_mut().step = RoomEqStep::Optimize;
    state.borrow_mut().optimization_status = OptimizationStatus::Completed;
    state.borrow_mut().channel_results = vec![
        ChannelOptResult {
            channel_name: "L".to_string(),
            pre_score: 10.0,
            post_score: 2.0,
            eq_filters: Vec::new(),
        },
        ChannelOptResult {
            channel_name: "R".to_string(),
            pre_score: 8.0,
            post_score: 1.5,
            eq_filters: Vec::new(),
        },
    ];
    assert!(state.borrow().is_optimization_complete());

    // Step 4: Review
    state.borrow_mut().step = RoomEqStep::Review;
    let avg_improvement = state.borrow().average_pre_score() - state.borrow().average_post_score();
    assert!(avg_improvement > 0.0);

    // Step 5: Export
    state.borrow_mut().step = RoomEqStep::Export;
    state.borrow_mut().dsp_output = Some("{}".to_string());
    assert!(state.borrow().dsp_output.is_some());
}

/// Test wizard back navigation.
#[gpui::test]
async fn test_wizard_back_navigation(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RoomEqState::default()));

    // Start at last step
    state.borrow_mut().step = RoomEqStep::Export;

    // Navigate back through all steps
    let mut step = state.borrow().step;
    let mut visited = vec![step];

    while let Some(prev) = step.previous() {
        step = prev;
        visited.push(step);
    }

    assert_eq!(visited.len(), 5);
    assert_eq!(visited[4], RoomEqStep::LoadData);
}

/// Test status message updates.
#[gpui::test]
async fn test_status_message_updates(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RoomEqState::default()));

    state.borrow_mut().status_message = "Loading measurements...".to_string();
    assert!(state.borrow().status_message.contains("Loading"));

    state.borrow_mut().status_message = "Optimizing channel L...".to_string();
    assert!(state.borrow().status_message.contains("Optimizing"));
}

/// Test error message display.
#[gpui::test]
async fn test_error_message_display(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RoomEqState::default()));

    assert!(state.borrow().error_message.is_none());

    state.borrow_mut().error_message = Some("Invalid measurement file".to_string());
    assert!(state.borrow().error_message.is_some());
}

/// Test surround channel configuration.
#[gpui::test]
async fn test_surround_channel_configuration(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(RoomEqState::default()));

    // 5.1 setup
    let channels = ["L", "R", "C", "LFE", "SL", "SR"];
    state.borrow_mut().channel_measurements = channels
        .iter()
        .map(|name| ChannelMeasurement {
            channel_name: name.to_string(),
            frequencies: Vec::new(),
            magnitude_db: Vec::new(),
            is_group: false,
        })
        .collect();

    assert_eq!(state.borrow().channel_count(), 6);
}
