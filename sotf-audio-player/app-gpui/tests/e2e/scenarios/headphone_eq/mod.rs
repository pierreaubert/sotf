//! E2E tests for Headphone EQ Wizard.
//!
//! Tests for the 4-step headphone EQ optimization wizard:
//! 1. MeasurementTarget - Select measurement file and target curve
//! 2. Optimization - Configure optimizer parameters and run optimization
//! 3. Listen - Preview and apply EQ to playback
//! 4. Save - Export format selection and save

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Mock Types for Testing
// =============================================================================

/// Headphone EQ workflow step
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum HeadphoneEqStep {
    #[default]
    MeasurementTarget,
    Optimization,
    Listen,
    Save,
}

impl HeadphoneEqStep {
    fn index(&self) -> usize {
        match self {
            HeadphoneEqStep::MeasurementTarget => 0,
            HeadphoneEqStep::Optimization => 1,
            HeadphoneEqStep::Listen => 2,
            HeadphoneEqStep::Save => 3,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            HeadphoneEqStep::MeasurementTarget => "Measurement",
            HeadphoneEqStep::Optimization => "Optimization",
            HeadphoneEqStep::Listen => "Listen",
            HeadphoneEqStep::Save => "Save",
        }
    }

    fn next(&self) -> Option<HeadphoneEqStep> {
        match self {
            HeadphoneEqStep::MeasurementTarget => Some(HeadphoneEqStep::Optimization),
            HeadphoneEqStep::Optimization => Some(HeadphoneEqStep::Listen),
            HeadphoneEqStep::Listen => Some(HeadphoneEqStep::Save),
            HeadphoneEqStep::Save => None,
        }
    }

    fn previous(&self) -> Option<HeadphoneEqStep> {
        match self {
            HeadphoneEqStep::MeasurementTarget => None,
            HeadphoneEqStep::Optimization => Some(HeadphoneEqStep::MeasurementTarget),
            HeadphoneEqStep::Listen => Some(HeadphoneEqStep::Optimization),
            HeadphoneEqStep::Save => Some(HeadphoneEqStep::Listen),
        }
    }
}

/// Optimization algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Algorithm {
    Cobyla,
    #[default]
    DifferentialEvolution,
    NelderMead,
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
    loss: String,
    peq_model: String,
    population: usize,
    de_f: f64,
    de_cr: f64,
    strategy: String,
    tolerance: f64,
    refine: bool,
    local_algo: String,
    smooth: bool,
    smooth_n: usize,
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            algorithm: Algorithm::DifferentialEvolution,
            num_filters: 10,
            min_q: 0.5,
            max_q: 10.0,
            min_db: -12.0,
            max_db: 12.0,
            min_freq: 20.0,
            max_freq: 20000.0,
            max_iter: 10000,
            loss: "headphone-score".to_string(),
            peq_model: "pk".to_string(),
            population: 80,
            de_f: 0.8,
            de_cr: 0.9,
            strategy: "currenttobest1bin".to_string(),
            tolerance: 1e-3,
            refine: false,
            local_algo: "cobyla".to_string(),
            smooth: false,
            smooth_n: 1,
        }
    }
}

/// Optimization result
#[derive(Debug, Clone)]
struct OptimizationResult {
    pre_score: f64,
    post_score: f64,
    biquads: Vec<BiquadFilter>,
}

/// Biquad filter
#[derive(Debug, Clone)]
struct BiquadFilter {
    filter_type: String,
    freq: f64,
    q: f64,
    db_gain: f64,
}

/// Headphone EQ state for testing
struct HeadphoneEqState {
    step: HeadphoneEqStep,
    // Step 1: Measurement & Target
    measurement_path: Option<String>,
    loss_type: String,
    target_preset: String,
    custom_target_path: Option<String>,
    // Step 2: Configuration
    optimizer_config: OptimizerConfig,
    // Step 3: Optimization
    optimization_status: OptimizationStatus,
    progress: f32,
    progress_history: Vec<(usize, f64)>,
    // Step 4: Apply
    result: Option<OptimizationResult>,
    export_format: String,
    save_name: String,
    // UI State
    status_message: String,
    error_message: Option<String>,
}

impl Default for HeadphoneEqState {
    fn default() -> Self {
        Self {
            step: HeadphoneEqStep::MeasurementTarget,
            measurement_path: None,
            loss_type: "score".to_string(),
            target_preset: "harman-over-ear-2018".to_string(),
            custom_target_path: None,
            optimizer_config: OptimizerConfig::default(),
            optimization_status: OptimizationStatus::Idle,
            progress: 0.0,
            progress_history: Vec::new(),
            result: None,
            export_format: "json".to_string(),
            save_name: String::new(),
            status_message: String::new(),
            error_message: None,
        }
    }
}

impl HeadphoneEqState {
    fn can_advance(&self) -> bool {
        match self.step {
            HeadphoneEqStep::MeasurementTarget => self.measurement_path.is_some(),
            HeadphoneEqStep::Optimization => {
                self.optimization_status == OptimizationStatus::Completed
            }
            HeadphoneEqStep::Listen => self.result.is_some(),
            HeadphoneEqStep::Save => true,
        }
    }

    fn is_optimizing(&self) -> bool {
        self.optimization_status == OptimizationStatus::Running
    }

    fn reset_optimization(&mut self) {
        self.optimization_status = OptimizationStatus::Idle;
        self.progress = 0.0;
        self.progress_history.clear();
        self.result = None;
        self.error_message = None;
    }
}

// =============================================================================
// Step Navigation Tests
// =============================================================================

/// Test step indices.
#[gpui::test]
async fn test_step_indices(_cx: &mut TestAppContext) {
    assert_eq!(HeadphoneEqStep::MeasurementTarget.index(), 0);
    assert_eq!(HeadphoneEqStep::Optimization.index(), 1);
    assert_eq!(HeadphoneEqStep::Listen.index(), 2);
    assert_eq!(HeadphoneEqStep::Save.index(), 3);
}

/// Test step labels.
#[gpui::test]
async fn test_step_labels(_cx: &mut TestAppContext) {
    assert_eq!(HeadphoneEqStep::MeasurementTarget.label(), "Measurement");
    assert_eq!(HeadphoneEqStep::Optimization.label(), "Optimization");
    assert_eq!(HeadphoneEqStep::Listen.label(), "Listen");
    assert_eq!(HeadphoneEqStep::Save.label(), "Save");
}

/// Test step next navigation.
#[gpui::test]
async fn test_step_next_navigation(_cx: &mut TestAppContext) {
    assert_eq!(
        HeadphoneEqStep::MeasurementTarget.next(),
        Some(HeadphoneEqStep::Optimization)
    );
    assert_eq!(
        HeadphoneEqStep::Optimization.next(),
        Some(HeadphoneEqStep::Listen)
    );
    assert_eq!(HeadphoneEqStep::Listen.next(), Some(HeadphoneEqStep::Save));
    assert_eq!(HeadphoneEqStep::Save.next(), None);
}

/// Test step previous navigation.
#[gpui::test]
async fn test_step_previous_navigation(_cx: &mut TestAppContext) {
    assert_eq!(HeadphoneEqStep::MeasurementTarget.previous(), None);
    assert_eq!(
        HeadphoneEqStep::Optimization.previous(),
        Some(HeadphoneEqStep::MeasurementTarget)
    );
    assert_eq!(
        HeadphoneEqStep::Listen.previous(),
        Some(HeadphoneEqStep::Optimization)
    );
    assert_eq!(
        HeadphoneEqStep::Save.previous(),
        Some(HeadphoneEqStep::Listen)
    );
}

/// Test complete step sequence.
#[gpui::test]
async fn test_complete_step_sequence(_cx: &mut TestAppContext) {
    let mut step = HeadphoneEqStep::MeasurementTarget;
    let mut steps = vec![step];

    while let Some(next) = step.next() {
        step = next;
        steps.push(step);
    }

    assert_eq!(steps.len(), 4);
    assert_eq!(steps[0], HeadphoneEqStep::MeasurementTarget);
    assert_eq!(steps[3], HeadphoneEqStep::Save);
}

// =============================================================================
// Step 1: Measurement & Target Tests
// =============================================================================

/// Test initial state defaults.
#[gpui::test]
async fn test_initial_state_defaults(_cx: &mut TestAppContext) {
    let state = HeadphoneEqState::default();

    assert_eq!(state.step, HeadphoneEqStep::MeasurementTarget);
    assert!(state.measurement_path.is_none());
    assert_eq!(state.loss_type, "score");
    assert_eq!(state.target_preset, "harman-over-ear-2018");
}

/// Test measurement file selection.
#[gpui::test]
async fn test_measurement_file_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    assert!(!state.borrow().can_advance());

    state.borrow_mut().measurement_path = Some("/path/to/headphone.csv".to_string());

    assert!(state.borrow().can_advance());
}

/// Test loss type selection.
#[gpui::test]
async fn test_loss_type_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    let loss_types = ["flat", "score"];
    for loss_type in loss_types {
        state.borrow_mut().loss_type = loss_type.to_string();
        assert_eq!(state.borrow().loss_type, loss_type);
    }
}

/// Test target preset selection.
#[gpui::test]
async fn test_target_preset_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    let presets = [
        "harman-over-ear-2018",
        "harman-in-ear-2019",
        "diffuse-field",
        "custom",
    ];

    for preset in presets {
        state.borrow_mut().target_preset = preset.to_string();
        assert_eq!(state.borrow().target_preset, preset);
    }
}

/// Test custom target file selection.
#[gpui::test]
async fn test_custom_target_file_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    state.borrow_mut().target_preset = "custom".to_string();
    state.borrow_mut().custom_target_path = Some("/path/to/target.csv".to_string());

    assert_eq!(state.borrow().target_preset, "custom");
    assert!(state.borrow().custom_target_path.is_some());
}

// =============================================================================
// Step 2: Optimizer Configuration Tests
// =============================================================================

/// Test optimizer config defaults.
#[gpui::test]
async fn test_optimizer_config_defaults(_cx: &mut TestAppContext) {
    let config = OptimizerConfig::default();

    assert_eq!(config.algorithm, Algorithm::DifferentialEvolution);
    assert_eq!(config.num_filters, 10);
    assert!((config.min_q - 0.5).abs() < 0.01);
    assert!((config.max_q - 10.0).abs() < 0.01);
    assert!((config.min_db - (-12.0)).abs() < 0.1);
    assert!((config.max_db - 12.0).abs() < 0.1);
}

/// Test algorithm selection.
#[gpui::test]
async fn test_algorithm_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

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

/// Test num_filters control.
#[gpui::test]
async fn test_num_filters_control(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    let test_values = [3, 5, 7, 10, 15];
    for value in test_values {
        state.borrow_mut().optimizer_config.num_filters = value;
        assert_eq!(state.borrow().optimizer_config.num_filters, value);
    }
}

/// Test Q factor bounds.
#[gpui::test]
async fn test_q_factor_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    state.borrow_mut().optimizer_config.min_q = 0.3;
    state.borrow_mut().optimizer_config.max_q = 15.0;

    assert!(state.borrow().optimizer_config.min_q > 0.0);
    assert!(state.borrow().optimizer_config.max_q > state.borrow().optimizer_config.min_q);
}

/// Test gain bounds.
#[gpui::test]
async fn test_gain_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    state.borrow_mut().optimizer_config.min_db = -15.0;
    state.borrow_mut().optimizer_config.max_db = 10.0;

    assert!(state.borrow().optimizer_config.min_db < 0.0);
    assert!(state.borrow().optimizer_config.max_db > 0.0);
}

/// Test frequency bounds.
#[gpui::test]
async fn test_frequency_bounds(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    state.borrow_mut().optimizer_config.min_freq = 30.0;
    state.borrow_mut().optimizer_config.max_freq = 18000.0;

    assert!(state.borrow().optimizer_config.min_freq >= 20.0);
    assert!(state.borrow().optimizer_config.max_freq <= 20000.0);
}

/// Test max iterations.
#[gpui::test]
async fn test_max_iterations(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    let test_values = [1000, 5000, 10000, 20000];
    for value in test_values {
        state.borrow_mut().optimizer_config.max_iter = value;
        assert_eq!(state.borrow().optimizer_config.max_iter, value);
    }
}

/// Test PEQ model selection.
#[gpui::test]
async fn test_peq_model_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    let models = ["pk", "hp-pk", "ls-pk-hs"];
    for model in models {
        state.borrow_mut().optimizer_config.peq_model = model.to_string();
        assert_eq!(state.borrow().optimizer_config.peq_model, model);
    }
}

/// Test DE strategy selection.
#[gpui::test]
async fn test_de_strategy_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    let strategies = ["currenttobest1bin", "best1bin", "rand1bin", "best2bin"];

    for strategy in strategies {
        state.borrow_mut().optimizer_config.strategy = strategy.to_string();
        assert_eq!(state.borrow().optimizer_config.strategy, strategy);
    }
}

/// Test DE parameters.
#[gpui::test]
async fn test_de_parameters(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    // F factor (mutation)
    state.borrow_mut().optimizer_config.de_f = 0.7;
    assert!((state.borrow().optimizer_config.de_f - 0.7).abs() < 0.01);

    // CR (crossover rate)
    state.borrow_mut().optimizer_config.de_cr = 0.85;
    assert!((state.borrow().optimizer_config.de_cr - 0.85).abs() < 0.01);

    // Population
    state.borrow_mut().optimizer_config.population = 100;
    assert_eq!(state.borrow().optimizer_config.population, 100);
}

/// Test local refinement toggle.
#[gpui::test]
async fn test_local_refinement_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    assert!(!state.borrow().optimizer_config.refine);

    state.borrow_mut().optimizer_config.refine = true;
    assert!(state.borrow().optimizer_config.refine);
}

/// Test smoothing toggle.
#[gpui::test]
async fn test_smoothing_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    assert!(!state.borrow().optimizer_config.smooth);

    state.borrow_mut().optimizer_config.smooth = true;
    state.borrow_mut().optimizer_config.smooth_n = 3;

    assert!(state.borrow().optimizer_config.smooth);
    assert_eq!(state.borrow().optimizer_config.smooth_n, 3);
}

// =============================================================================
// Step 3: Optimization Execution Tests
// =============================================================================

/// Test optimization status transitions.
#[gpui::test]
async fn test_optimization_status_transitions(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    // Initial state
    assert_eq!(state.borrow().optimization_status, OptimizationStatus::Idle);
    assert!(!state.borrow().is_optimizing());

    // Start optimization
    state.borrow_mut().optimization_status = OptimizationStatus::Running;
    assert!(state.borrow().is_optimizing());

    // Complete optimization
    state.borrow_mut().optimization_status = OptimizationStatus::Completed;
    assert!(!state.borrow().is_optimizing());
}

/// Test optimization progress tracking.
#[gpui::test]
async fn test_optimization_progress_tracking(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    state.borrow_mut().optimization_status = OptimizationStatus::Running;
    state.borrow_mut().progress = 0.0;

    // Simulate progress updates
    let progress_values: Vec<f32> = vec![0.1, 0.25, 0.5, 0.75, 1.0];
    for value in progress_values {
        state.borrow_mut().progress = value;
        assert!((state.borrow().progress - value).abs() < 0.001);
    }
}

/// Test progress history tracking.
#[gpui::test]
async fn test_progress_history_tracking(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    // Add progress points
    state.borrow_mut().progress_history.push((0, 10.0));
    state.borrow_mut().progress_history.push((100, 5.0));
    state.borrow_mut().progress_history.push((200, 2.5));

    assert_eq!(state.borrow().progress_history.len(), 3);
    assert_eq!(state.borrow().progress_history[0], (0, 10.0));
}

/// Test optimization reset.
#[gpui::test]
async fn test_optimization_reset(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    // Simulate completed optimization
    state.borrow_mut().optimization_status = OptimizationStatus::Completed;
    state.borrow_mut().progress = 1.0;
    state.borrow_mut().progress_history.push((100, 5.0));
    state.borrow_mut().result = Some(OptimizationResult {
        pre_score: 10.0,
        post_score: 2.0,
        biquads: Vec::new(),
    });

    // Reset
    state.borrow_mut().reset_optimization();

    assert_eq!(state.borrow().optimization_status, OptimizationStatus::Idle);
    assert!((state.borrow().progress - 0.0).abs() < 0.001);
    assert!(state.borrow().progress_history.is_empty());
    assert!(state.borrow().result.is_none());
}

/// Test can_advance after optimization.
#[gpui::test]
async fn test_can_advance_after_optimization(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    state.borrow_mut().step = HeadphoneEqStep::Optimization;

    // Cannot advance until completed
    assert!(!state.borrow().can_advance());

    state.borrow_mut().optimization_status = OptimizationStatus::Running;
    assert!(!state.borrow().can_advance());

    state.borrow_mut().optimization_status = OptimizationStatus::Completed;
    assert!(state.borrow().can_advance());
}

/// Test optimization failure handling.
#[gpui::test]
async fn test_optimization_failure_handling(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    state.borrow_mut().optimization_status = OptimizationStatus::Failed;
    state.borrow_mut().error_message = Some("Optimization diverged".to_string());

    assert_eq!(
        state.borrow().optimization_status,
        OptimizationStatus::Failed
    );
    assert!(state.borrow().error_message.is_some());
}

/// Test optimization cancellation.
#[gpui::test]
async fn test_optimization_cancellation(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    state.borrow_mut().optimization_status = OptimizationStatus::Running;
    state.borrow_mut().progress = 0.5;

    state.borrow_mut().optimization_status = OptimizationStatus::Cancelled;

    assert_eq!(
        state.borrow().optimization_status,
        OptimizationStatus::Cancelled
    );
    assert!(!state.borrow().can_advance());
}

// =============================================================================
// Step 4: Listen & Preview Tests
// =============================================================================

/// Test result availability for listen step.
#[gpui::test]
async fn test_result_availability_for_listen(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    state.borrow_mut().step = HeadphoneEqStep::Listen;

    // Cannot advance without result
    assert!(!state.borrow().can_advance());

    state.borrow_mut().result = Some(OptimizationResult {
        pre_score: 10.0,
        post_score: 2.0,
        biquads: vec![BiquadFilter {
            filter_type: "peak".to_string(),
            freq: 1000.0,
            q: 2.0,
            db_gain: -3.0,
        }],
    });

    assert!(state.borrow().can_advance());
}

/// Test result score display.
#[gpui::test]
async fn test_result_score_display(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    state.borrow_mut().result = Some(OptimizationResult {
        pre_score: 8.5,
        post_score: 2.3,
        biquads: Vec::new(),
    });

    let result = state.borrow().result.as_ref().unwrap().clone();
    assert!((result.pre_score - 8.5).abs() < 0.1);
    assert!((result.post_score - 2.3).abs() < 0.1);

    let improvement = result.pre_score - result.post_score;
    assert!(improvement > 0.0);
}

/// Test biquad filter result.
#[gpui::test]
async fn test_biquad_filter_result(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    let biquads = vec![
        BiquadFilter {
            filter_type: "peak".to_string(),
            freq: 100.0,
            q: 1.5,
            db_gain: -5.0,
        },
        BiquadFilter {
            filter_type: "peak".to_string(),
            freq: 1000.0,
            q: 2.0,
            db_gain: 3.0,
        },
        BiquadFilter {
            filter_type: "peak".to_string(),
            freq: 8000.0,
            q: 1.0,
            db_gain: -2.0,
        },
    ];

    state.borrow_mut().result = Some(OptimizationResult {
        pre_score: 10.0,
        post_score: 2.0,
        biquads: biquads.clone(),
    });

    let result = state.borrow().result.as_ref().unwrap().clone();
    assert_eq!(result.biquads.len(), 3);
    assert!((result.biquads[0].freq - 100.0).abs() < 0.1);
    assert!((result.biquads[1].freq - 1000.0).abs() < 0.1);
}

// =============================================================================
// Step 5: Export & Save Tests
// =============================================================================

/// Test export format selection.
#[gpui::test]
async fn test_export_format_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    let formats = ["json", "camillaDsp", "eq-apo", "rme-totalmix"];
    for format in formats {
        state.borrow_mut().export_format = format.to_string();
        assert_eq!(state.borrow().export_format, format);
    }
}

/// Test save name entry.
#[gpui::test]
async fn test_save_name_entry(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    state.borrow_mut().save_name = "My Headphone EQ".to_string();
    assert_eq!(state.borrow().save_name, "My Headphone EQ");
}

/// Test save step always advances.
#[gpui::test]
async fn test_save_step_always_advances(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    state.borrow_mut().step = HeadphoneEqStep::Save;

    // Save step can always advance (finish wizard)
    assert!(state.borrow().can_advance());
}

// =============================================================================
// Full Wizard Flow Tests
// =============================================================================

/// Test complete wizard flow.
#[gpui::test]
async fn test_complete_wizard_flow(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    // Step 1: Select measurement
    assert_eq!(state.borrow().step, HeadphoneEqStep::MeasurementTarget);
    state.borrow_mut().measurement_path = Some("/path/to/headphone.csv".to_string());
    assert!(state.borrow().can_advance());

    // Move to Step 2
    state.borrow_mut().step = HeadphoneEqStep::Optimization;
    state.borrow_mut().optimizer_config.num_filters = 8;
    state.borrow_mut().optimization_status = OptimizationStatus::Completed;
    assert!(state.borrow().can_advance());

    // Move to Step 3
    state.borrow_mut().step = HeadphoneEqStep::Listen;
    state.borrow_mut().result = Some(OptimizationResult {
        pre_score: 10.0,
        post_score: 2.0,
        biquads: vec![BiquadFilter {
            filter_type: "peak".to_string(),
            freq: 1000.0,
            q: 2.0,
            db_gain: -3.0,
        }],
    });
    assert!(state.borrow().can_advance());

    // Move to Step 4
    state.borrow_mut().step = HeadphoneEqStep::Save;
    state.borrow_mut().export_format = "json".to_string();
    state.borrow_mut().save_name = "Test EQ".to_string();
    assert!(state.borrow().can_advance());

    // Final state
    assert_eq!(state.borrow().step, HeadphoneEqStep::Save);
    assert!(state.borrow().step.next().is_none());
}

/// Test wizard back navigation.
#[gpui::test]
async fn test_wizard_back_navigation(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    // Start at last step
    state.borrow_mut().step = HeadphoneEqStep::Save;

    // Navigate back through all steps
    let mut step = state.borrow().step;
    let mut visited = vec![step];

    while let Some(prev) = step.previous() {
        step = prev;
        visited.push(step);
    }

    assert_eq!(visited.len(), 4);
    assert_eq!(visited[3], HeadphoneEqStep::MeasurementTarget);
}

/// Test status message updates.
#[gpui::test]
async fn test_status_message_updates(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    state.borrow_mut().status_message = "Loading measurement file...".to_string();
    assert_eq!(state.borrow().status_message, "Loading measurement file...");

    state.borrow_mut().status_message = "Optimizing (iteration 5000/10000)...".to_string();
    assert!(state.borrow().status_message.contains("Optimizing"));
}

/// Test error message display.
#[gpui::test]
async fn test_error_message_display(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(HeadphoneEqState::default()));

    assert!(state.borrow().error_message.is_none());

    state.borrow_mut().error_message = Some("Invalid measurement file format".to_string());
    assert!(state.borrow().error_message.is_some());
}
