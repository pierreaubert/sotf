// ============================================================================
// Room EQ Screen Types
// ============================================================================

use serde::{Deserialize, Serialize};

use super::recording::{RecordingResult, RecordingState};
use autoeq::roomeq::{
    CrossoverConfig as BackendCrossoverConfig,
    ExcursionProtectionConfig as BackendExcursionProtectionConfig, FirConfig as BackendFirConfig,
    HighFreqFilterConfig, HighpassType, LowFreqFilterConfig, MeasurementSource,
    MixedPhaseSerdeConfig as BackendMixedPhaseConfig, MultiMeasurementConfig,
    MultiMeasurementStrategy, MultiSeatConfig as BackendMultiSeatConfig, MultiSeatStrategy,
    OptimizerConfig as BackendOptimizerConfig,
    PhaseAlignmentConfig as BackendPhaseAlignmentConfig,
    PreRingingSerdeConfig as BackendPreRingingConfig,
    ProcessingMode as BackendProcessingMode, RoomConfig,
    SchroederSplitConfig as BackendSchroederSplitConfig, SpeakerConfig, SpeakerGroup,
    TargetTiltConfig as BackendTargetTiltConfig, TiltType,
};
use std::collections::HashMap;

/// Wrapper for InteractiveChartState that implements Debug
#[derive(Clone)]
pub struct InteractiveChartStateWrapper(pub gpui_px::interaction::InteractiveChartState);

impl std::fmt::Debug for InteractiveChartStateWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InteractiveChartStateWrapper")
            .field("is_zoomed", &self.0.is_zoomed())
            .finish()
    }
}

impl InteractiveChartStateWrapper {
    pub fn new(x_min: f64, x_max: f64, y_min: f64, y_max: f64) -> Self {
        Self(gpui_px::interaction::InteractiveChartState::new(
            x_min, x_max, y_min, y_max,
        ))
    }

    pub fn with_log_x(mut self, is_log: bool) -> Self {
        self.0 = self.0.with_log_x(is_log);
        self
    }

    pub fn with_size(mut self, width: f32, height: f32) -> Self {
        self.0 = self.0.with_size(width, height);
        self
    }

    pub fn inner(&self) -> &gpui_px::interaction::InteractiveChartState {
        &self.0
    }
}

// Domain types shared via sotf-player crate — single source of truth for all apps.
pub use sotf_audio_player::room_eq_types::{
    BroadbandTargetMatchingConfig, ChannelDspChain, ChannelMeasurement, ChannelOptResult,
    CustomTargetCurve, DspChainMetadata, DspChainOutput, DspPluginConfig, DriverDspChain,
    EqFilterConfig, ExcursionProtectionConfig, GroupDelayOptConfig, MixedModeUiConfig,
    MixedPhaseUiConfig, MultiMeasurementUiConfig, MultiSeatConfig, MultiSpeakerMode,
    PhaseAlignmentConfig, PreRingingConfig, RecordingConfiguration, RoomEqDataSource,
    RoomEqFirConfig, RoomEqMeasurementsFile, RoomEqOptimizationMode, RoomEqSpeakerConfig,
    RoomEqStep, SchroederSplitConfig, SpeakerConfigType, SubOptimizerUiConfig,
    ChannelMatchingUiConfig, TargetCurveControlPoint, TargetTiltConfig, VoGConfig,
};
pub type CrossoverType = sotf_audio_player::room_eq_types::RoomEqCrossoverType;
pub use sotf_audio_player::room_eq_types::AutoEqField;
pub use sotf_audio_player::room_eq_types::OptimizationStatus;
pub use sotf_audio_player::room_eq_types::RoomEqAlgorithm;

fn default_tolerance() -> f64 {
    1e-5
}
fn default_atolerance() -> f64 {
    1e-5
}
fn default_smooth_n() -> usize {
    6
}
fn default_strategy() -> String {
    "lshade".to_string()
}

/// Optimizer configuration for Room EQ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEqOptimizerConfig {
    /// Optimization mode
    #[serde(default)]
    pub mode: RoomEqOptimizationMode,
    /// FIR configuration
    #[serde(default)]
    pub fir: RoomEqFirConfig,
    /// Multi-speaker optimization mode
    pub multi_speaker_mode: MultiSpeakerMode,
    /// Optimization algorithm (e.g., "autoeq:de", "nlopt:cobyla", "nlopt:neldermead")
    pub algorithm: String,
    /// DE mutation strategy (e.g., "currenttobest1bin", "lshade", "best1bin")
    #[serde(default = "default_strategy")]
    pub strategy: String,
    /// Number of PEQ filters per channel
    pub num_filters: usize,
    /// Minimum Q factor
    pub min_q: f64,
    /// Maximum Q factor
    pub max_q: f64,
    /// Minimum gain in dB
    pub min_db: f64,
    /// Maximum gain in dB
    pub max_db: f64,
    /// Minimum frequency in Hz
    pub min_freq: f64,
    /// Maximum frequency in Hz
    pub max_freq: f64,
    /// Maximum number of iterations
    pub max_iter: usize,
    /// PEQ model (e.g., "pk", "ls-pk-hs")
    pub peq_model: String,
    /// Population size for evolutionary algorithms
    pub population: usize,
    /// Enable local refinement after global optimization
    pub refine: bool,
    /// Local algorithm for refinement
    pub local_algo: String,
    /// Loss function type (e.g., "flat", "score")
    pub loss_type: String,
    /// Enable psychoacoustic smoothing
    pub psychoacoustic: bool,
    /// Enable curve smoothing
    #[serde(default)]
    pub smooth: bool,
    /// Curve smoothing window size (1/N octave)
    #[serde(default = "default_smooth_n")]
    pub smooth_n: usize,
    /// Enable asymmetric loss (penalize peaks more than dips)
    pub asymmetric_loss: bool,
    /// Convergence tolerance (relative)
    #[serde(default = "default_tolerance")]
    pub tolerance: f64,
    /// Convergence tolerance (absolute)
    #[serde(default = "default_atolerance")]
    pub atolerance: f64,
    /// Target curve (e.g., "flat", "harman")
    pub target_curve: String,
    /// System type (e.g., "stereo", "multichannel")
    pub system_type: String,

    // --- v2 fields ---
    /// Allow inter-speaker delay optimization
    #[serde(default)]
    pub allow_delay: bool,
    /// Random seed for reproducible results (None for random)
    #[serde(default)]
    pub seed: Option<u64>,
    /// Group delay optimization
    #[serde(default)]
    pub gd_opt: GroupDelayOptConfig,
    /// Voice of God (timbre matching)
    #[serde(default)]
    pub vog: VoGConfig,
    /// Broadband target matching
    #[serde(default)]
    pub broadband_target_matching: BroadbandTargetMatchingConfig,
    /// Mixed mode configuration (when mode == Mixed)
    #[serde(default)]
    pub mixed_config: MixedModeUiConfig,
    /// Mixed-phase configuration (when mode == MixedPhase)
    #[serde(default)]
    pub mixed_phase: MixedPhaseUiConfig,

    // --- Advanced Room Correction (Scenario B) ---
    #[serde(default)]
    pub target_tilt: TargetTiltConfig,
    #[serde(default)]
    pub excursion_protection: ExcursionProtectionConfig,
    #[serde(default)]
    pub schroeder_split: SchroederSplitConfig,

    // --- Advanced System Optimization (Scenario A) ---
    #[serde(default)]
    pub phase_alignment: PhaseAlignmentConfig,
    #[serde(default)]
    pub multi_seat: MultiSeatConfig,
    #[serde(default)]
    pub multi_measurement: MultiMeasurementUiConfig,

    // --- Subwoofer & Channel Matching ---
    /// Subwoofer-specific optimizer overrides
    #[serde(default)]
    pub sub_config: SubOptimizerUiConfig,
    /// Inter-channel consistency correction
    #[serde(default)]
    pub channel_matching: ChannelMatchingUiConfig,

    /// True when settings were imported from a backend config file (recordings.json).
    /// When set, `apply_smart_defaults()` skips overriding feature toggles.
    #[serde(default)]
    pub imported_from_file: bool,
}

impl Default for RoomEqOptimizerConfig {
    fn default() -> Self {
        Self {
            mode: RoomEqOptimizationMode::default(),
            fir: RoomEqFirConfig::default(),
            multi_speaker_mode: MultiSpeakerMode::Combined,
            algorithm: "autoeq:de".to_string(),
            strategy: "lshade".to_string(),
            num_filters: 7,
            min_q: 0.5,
            max_q: 6.0,
            min_db: -12.0,
            max_db: 4.0,
            min_freq: 20.0,
            max_freq: 1600.0,
            max_iter: 50000,
            peq_model: "pk".to_string(),
            population: 50,
            refine: false,
            local_algo: "cobyla".to_string(),
            loss_type: "flat".to_string(),
            psychoacoustic: true,
            smooth: false,
            smooth_n: 6,
            asymmetric_loss: true,
            tolerance: 1e-5,
            atolerance: 1e-5,
            target_curve: "flat".to_string(),
            system_type: "stereo".to_string(),
            allow_delay: false,
            seed: None,
            gd_opt: GroupDelayOptConfig::default(),
            vog: VoGConfig::default(),
            broadband_target_matching: BroadbandTargetMatchingConfig::default(),
            mixed_config: MixedModeUiConfig::default(),
            mixed_phase: MixedPhaseUiConfig::default(),
            target_tilt: TargetTiltConfig::default(),
            excursion_protection: ExcursionProtectionConfig::default(),
            schroeder_split: SchroederSplitConfig::default(),
            phase_alignment: PhaseAlignmentConfig::default(),
            multi_seat: MultiSeatConfig::default(),
            multi_measurement: MultiMeasurementUiConfig::default(),
            sub_config: SubOptimizerUiConfig::default(),
            channel_matching: ChannelMatchingUiConfig::default(),
            imported_from_file: false,
        }
    }
}

impl RoomEqOptimizerConfig {
    /// Import optimizer parameters and feature toggles from a backend `OptimizerConfig`.
    ///
    /// This is used when loading a RoomConfig JSON file so that the GPUI
    /// uses the same optimizer settings as the roomeq CLI.
    /// Sets `imported_from_file = true` so that `apply_smart_defaults()` will
    /// not override the imported feature toggle state.
    pub fn import_from_backend(&mut self, backend: &BackendOptimizerConfig) {
        // Core optimizer parameters
        self.algorithm = backend.algorithm.clone();
        self.strategy = backend.strategy.clone();
        self.num_filters = backend.num_filters;
        self.min_q = backend.min_q;
        self.max_q = backend.max_q;
        self.min_db = backend.min_db;
        self.max_db = backend.max_db;
        self.min_freq = backend.min_freq;
        self.max_freq = backend.max_freq;
        self.max_iter = backend.max_iter;
        self.population = backend.population;
        self.peq_model = backend.peq_model.clone();
        self.loss_type = backend.loss_type.clone();
        self.psychoacoustic = backend.psychoacoustic;
        self.asymmetric_loss = backend.asymmetric_loss;
        self.tolerance = backend.tolerance;
        self.atolerance = backend.atolerance;
        self.refine = backend.refine;
        self.local_algo = backend.local_algo.clone();
        self.seed = backend.seed;

        // FIR configuration
        if let Some(ref fir) = backend.fir {
            self.fir.taps = fir.taps;
            self.fir.phase = fir.phase.clone();
            self.fir.correct_excess_phase = fir.correct_excess_phase;
            self.fir.phase_smoothing = fir.phase_smoothing;
            self.fir.pre_ringing = fir.pre_ringing.as_ref().map(|pr| PreRingingConfig {
                threshold_db: pr.threshold_db,
                max_time_s: pr.max_time_s,
            });
        }

        // Mixed-phase configuration
        if let Some(ref mp) = backend.mixed_phase {
            self.mixed_phase = MixedPhaseUiConfig {
                max_fir_length_ms: mp.max_fir_length_ms,
                pre_ringing_threshold_db: mp.pre_ringing_threshold_db,
                min_spatial_depth: mp.min_spatial_depth,
                phase_smoothing_octaves: mp.phase_smoothing_octaves,
            };
        }

        // Processing mode → optimization mode
        self.mode = match backend.processing_mode {
            BackendProcessingMode::LowLatency => RoomEqOptimizationMode::Iir,
            BackendProcessingMode::PhaseLinear => RoomEqOptimizationMode::Fir,
            BackendProcessingMode::Hybrid => RoomEqOptimizationMode::Mixed,
            BackendProcessingMode::MixedPhase => RoomEqOptimizationMode::MixedPhase,
        };

        // Feature toggles: None in backend = feature was not configured = disabled.
        self.target_tilt.enabled = backend.target_tilt.is_some();
        if let Some(ref tilt) = backend.target_tilt {
            self.target_tilt.tilt_type = match tilt.tilt_type {
                TiltType::Harman => "harman".to_string(),
                TiltType::Custom => "custom".to_string(),
                TiltType::Flat => "flat".to_string(),
            };
            self.target_tilt.slope = tilt.slope_db_per_octave;
            self.target_tilt.reference_freq = tilt.reference_freq;
            self.target_tilt.bass_shelf_db = tilt.bass_shelf_db;
            self.target_tilt.bass_shelf_freq = tilt.bass_shelf_freq;
        }

        self.excursion_protection.enabled = backend.excursion_protection.as_ref().is_some_and(|e| e.enabled);
        if let Some(ref ep) = backend.excursion_protection {
            self.excursion_protection.auto_detect_f3 = ep.auto_detect_f3;
            self.excursion_protection.manual_f3_hz = ep.manual_f3_hz.unwrap_or(40.0);
            self.excursion_protection.filter_order = ep.filter_order;
            self.excursion_protection.filter_type = match ep.filter_type {
                HighpassType::Butterworth => "bw".to_string(),
                HighpassType::LinkwitzRiley => "lr".to_string(),
            };
            self.excursion_protection.margin_octaves = ep.margin_octaves;
        }

        self.schroeder_split.enabled = backend.schroeder_split.as_ref().is_some_and(|s| s.enabled);
        if let Some(ref ss) = backend.schroeder_split {
            self.schroeder_split.schroeder_freq = ss.schroeder_freq;
            self.schroeder_split.low_freq_max_q = ss.low_freq_config.max_q;
            self.schroeder_split.low_freq_allow_boost = ss.low_freq_config.allow_boost;
            self.schroeder_split.low_freq_max_db = ss.low_freq_config.max_db;
            self.schroeder_split.high_freq_max_q = ss.high_freq_config.max_q;
            self.schroeder_split.high_freq_shelving_only = ss.high_freq_config.shelving_only;
        }

        self.broadband_target_matching.enabled = backend.broadband_target_matching.as_ref().is_some_and(|b| b.enabled);

        self.allow_delay = backend.allow_delay.unwrap_or(false);

        self.gd_opt.enabled = backend.gd_opt.as_ref().is_some_and(|g| g.enabled);
        if let Some(ref gd) = backend.gd_opt {
            self.gd_opt.target_ms = gd.target_ms;
        }

        self.vog.enabled = backend.vog.as_ref().is_some_and(|v| v.enabled);
        if let Some(ref vog) = backend.vog {
            self.vog.reference_channel = vog.reference_channel.clone();
        }

        self.phase_alignment.enabled = backend.phase_alignment.as_ref().is_some_and(|p| p.enabled);
        if let Some(ref pa) = backend.phase_alignment {
            self.phase_alignment.min_freq = pa.min_freq;
            self.phase_alignment.max_freq = pa.max_freq;
            self.phase_alignment.optimize_polarity = pa.optimize_polarity;
            self.phase_alignment.max_delay_ms = pa.max_delay_ms;
        }

        self.multi_seat.enabled = backend.multi_seat.as_ref().is_some_and(|m| m.enabled);
        if let Some(ref ms) = backend.multi_seat {
            self.multi_seat.strategy = match ms.strategy {
                autoeq::roomeq::MultiSeatStrategy::MinimizeVariance => "variance".to_string(),
                autoeq::roomeq::MultiSeatStrategy::PrimaryWithConstraints => "primary".to_string(),
                autoeq::roomeq::MultiSeatStrategy::Average => "average".to_string(),
            };
            self.multi_seat.primary_seat = ms.primary_seat;
            self.multi_seat.max_deviation_db = ms.max_deviation_db;
        }

        if let Some(ref mm) = backend.multi_measurement {
            self.multi_measurement.enabled = true;
            self.multi_measurement.strategy = match mm.strategy {
                autoeq::roomeq::MultiMeasurementStrategy::Average => "average".to_string(),
                autoeq::roomeq::MultiMeasurementStrategy::WeightedSum => "weighted_sum".to_string(),
                autoeq::roomeq::MultiMeasurementStrategy::Minimax => "minimax".to_string(),
                autoeq::roomeq::MultiMeasurementStrategy::VariancePenalized => "variance_penalized".to_string(),
                autoeq::roomeq::MultiMeasurementStrategy::SpatialRobustness => "spatial_robustness".to_string(),
            };
            self.multi_measurement.variance_lambda = mm.variance_lambda;
            self.multi_measurement.weights = mm.weights.clone().unwrap_or_default();
        } else {
            self.multi_measurement.enabled = false;
        }

        // Sub-specific optimizer overrides
        self.sub_config.enabled = backend.sub_config.is_some();
        if let Some(ref sc) = backend.sub_config {
            self.sub_config.num_filters = sc.num_filters;
            self.sub_config.max_db = sc.max_db;
            self.sub_config.min_db = sc.min_db;
            self.sub_config.min_q = sc.min_q;
            self.sub_config.max_q = sc.max_q;
        }

        // Channel matching correction
        self.channel_matching.enabled = backend.channel_matching.as_ref().is_some_and(|c| c.enabled);
        if let Some(ref cm) = backend.channel_matching {
            self.channel_matching.threshold_db = cm.threshold_db;
            self.channel_matching.max_filters = cm.max_filters;
        }

        self.imported_from_file = true;
    }
}

/// UI state for Room EQ dropdowns
#[derive(Debug, Clone, Default)]
pub struct RoomEqDropdowns {
    pub data_source_open: bool,
    pub opt_mode_open: bool,
    pub fir_phase_open: bool,
    pub algorithm_open: bool,
    pub peq_model_open: bool,
    pub crossover_type_open: bool,
    pub export_format_open: bool,
    /// DE strategy dropdown
    pub strategy_open: bool,
    /// Local algorithm dropdown
    pub local_algo_open: bool,
    /// Loss type dropdown
    pub loss_type_open: bool,
    /// Target curve dropdown
    pub target_curve_open: bool,
    /// System type dropdown
    pub system_type_open: bool,

    // Advanced dropdowns
    pub tilt_type_open: bool,
    pub excursion_filter_type_open: bool,
    pub multi_seat_strategy_open: bool,

    // v2 dropdowns
    pub mixed_crossover_type_open: bool,
    pub mixed_fir_band_open: bool,
    pub vog_reference_channel_open: bool,
    pub multi_measurement_strategy_open: bool,

    /// Review step smoothing dropdown
    pub review_smoothing_open: bool,
    /// AutoEQ form editing state
    pub autoeq_editing_field: Option<AutoEqField>,
    /// AutoEQ form edit text
    pub autoeq_edit_text: String,
    /// Custom target curve editor modal open
    pub custom_target_modal_open: bool,
    /// Custom target presets dropdown open
    pub custom_target_presets_open: bool,
    /// Currently dragging control point index (None if not dragging)
    pub dragging_control_point: Option<usize>,
}

/// Complete Room EQ screen state
#[derive(Debug, Clone)]
pub struct RoomEqState {
    /// Current step in the workflow
    pub step: RoomEqStep,

    // === Step 1: Load Data ===
    /// Source of measurement data
    pub data_source: RoomEqDataSource,
    /// Loaded channel measurements
    pub channel_measurements: Vec<ChannelMeasurement>,

    // === Step 2: Configuration ===
    /// Per-channel speaker configurations
    pub speaker_configs: Vec<RoomEqSpeakerConfig>,
    /// Global optimizer configuration
    pub optimizer_config: RoomEqOptimizerConfig,

    // === Step 3: Optimization ===
    /// Current optimization status
    pub optimization_status: OptimizationStatus,
    /// Currently optimizing channel name
    pub current_channel: Option<String>,
    /// Per-channel optimization results
    pub channel_results: Vec<ChannelOptResult>,
    /// Overall progress (0.0 - 1.0)
    pub overall_progress: f32,
    /// Progress history for visualization: (iteration, loss, channel_name)
    pub progress_history: Vec<(usize, f64, String)>,
    /// Current iteration number
    pub current_iteration: usize,
    /// Current loss value
    pub current_loss: f64,

    // === Step 5: Export ===
    /// Generated DSP chain output
    pub dsp_output: Option<DspChainOutput>,

    // === UI State ===
    pub dropdowns: RoomEqDropdowns,
    pub status_message: String,
    pub error_message: Option<String>,
    /// Review graph smoothing level in octaves (0 = none, 1 = 1 octave, etc.)
    pub review_smoothing_octaves: f64,
    /// Selected channel index for review (0-based)
    pub review_selected_channel: usize,
    /// Interactive chart state for review graph (zoom/pan) - initialized lazily
    pub review_chart_state: Option<InteractiveChartStateWrapper>,
    /// Whether to auto-scale Y axis for review graph (if false, uses fixed range)
    pub review_y_axis_auto: bool,
    /// Interactive chart state for progress chart (zoom/pan) - initialized lazily
    pub progress_chart_state: Option<InteractiveChartStateWrapper>,
    /// Custom target curve for manual entry mode
    pub custom_target_curve: CustomTargetCurve,

    /// When false (default), the Configure step shows only basic settings
    /// (mode, algorithm, num_filters, target_curve). Toggle to show all parameters.
    pub show_advanced_config: bool,

    // === Multi-position data detection ===
    /// Whether loaded data has multi-position measurements (MeasurementSource::Multiple)
    pub has_multi_position_data: bool,
    /// Per-speaker measurement counts: (channel_name, count)
    pub multi_position_counts: Vec<(String, usize)>,
}

impl Default for RoomEqState {
    fn default() -> Self {
        Self {
            step: RoomEqStep::LoadData,
            data_source: RoomEqDataSource::FromRecording,
            channel_measurements: Vec::new(),
            speaker_configs: Vec::new(),
            optimizer_config: RoomEqOptimizerConfig::default(),
            optimization_status: OptimizationStatus::Idle,
            current_channel: None,
            channel_results: Vec::new(),
            overall_progress: 0.0,
            progress_history: Vec::new(),
            current_iteration: 0,
            current_loss: 0.0,
            dsp_output: None,
            dropdowns: RoomEqDropdowns::default(),
            status_message: String::new(),
            error_message: None,
            review_smoothing_octaves: 1.0, // Default to 1 octave smoothing
            review_selected_channel: 0,
            review_chart_state: None,
            review_y_axis_auto: true,
            progress_chart_state: None,
            custom_target_curve: CustomTargetCurve::new_flat(),
            show_advanced_config: false,
            has_multi_position_data: false,
            multi_position_counts: Vec::new(),
        }
    }
}

impl RoomEqState {
    /// Check if we have measurement data loaded
    pub fn has_measurements(&self) -> bool {
        !self.channel_measurements.is_empty()
    }

    /// Get the number of channels
    pub fn channel_count(&self) -> usize {
        self.channel_measurements.len()
    }

    /// Check if any channel is a subwoofer (LFE, Sub, SW)
    pub fn has_subwoofer(&self) -> bool {
        self.channel_names().iter().any(|name| {
            let upper = name.to_uppercase();
            upper == "LFE" || upper == "SUB" || upper == "SW" || upper.starts_with("SUB")
        })
    }

    /// Check if the setup is surround (3+ channels, excluding subs)
    pub fn is_surround(&self) -> bool {
        let non_sub_count = self
            .channel_names()
            .iter()
            .filter(|name| {
                let upper = name.to_uppercase();
                upper != "LFE" && upper != "SUB" && upper != "SW" && !upper.starts_with("SUB")
            })
            .count();
        non_sub_count >= 3
    }

    /// Check if any channel has phase data
    pub fn has_phase_data(&self) -> bool {
        self.channel_measurements
            .iter()
            .any(|m| !m.measurement.phase_deg.is_empty())
    }

    /// Check if any channel is a multi-driver group
    pub fn has_multi_driver(&self) -> bool {
        self.channel_measurements.iter().any(|m| m.is_group)
    }

    /// Check if multi-position measurement data is available
    pub fn has_multiple_measurements(&self) -> bool {
        self.has_multi_position_data
    }

    /// Height channel names used for Voice of God detection
    const HEIGHT_CHANNELS: &[&str] = &[
        "TFL", "TFR", "TSL", "TSR", "TBL", "TBR", "VOG", "TFC", "TBC", "TSC",
    ];

    /// Check if measurement has height channels (for VoG)
    pub fn has_height_channels(&self) -> bool {
        self.channel_names().iter().any(|name| {
            let upper = name.to_uppercase();
            Self::HEIGHT_CHANNELS.iter().any(|&h| upper == h)
        })
    }

    /// Check if the setup is home cinema (5+ non-sub channels)
    pub fn is_home_cinema(&self) -> bool {
        let non_sub_count = self
            .channel_names()
            .iter()
            .filter(|name| {
                let upper = name.to_uppercase();
                upper != "LFE" && upper != "SUB" && upper != "SW" && !upper.starts_with("SUB")
            })
            .count();
        non_sub_count >= 5
    }

    /// Apply smart defaults based on loaded measurement data.
    /// Called after loading measurements to set sensible initial values.
    pub fn apply_smart_defaults(&mut self) {
        // Compute detection values before mutably borrowing optimizer_config
        let is_surround = self.is_surround();
        let has_subwoofer = self.has_subwoofer();
        let has_height = self.has_height_channels();
        let is_cinema = self.is_home_cinema();

        let config = &mut self.optimizer_config;

        // Loss type is always flat for room EQ
        config.loss_type = "flat".to_string();

        // Only override algorithm/seed defaults when not imported from file
        if !config.imported_from_file {
            config.local_algo = "cobyla".to_string();
            config.refine = true;
            config.seed = None;
        }

        // System type: auto-detect from channel count
        config.system_type = if is_surround {
            "multichannel".to_string()
        } else {
            "stereo".to_string()
        };

        // Feature flags: only auto-enable when NOT imported from file.
        // When imported, the file's feature state is authoritative
        // (None = disabled, Some = enabled with those params).
        if !config.imported_from_file {
            config.target_tilt.enabled = true;
            config.target_tilt.tilt_type = "harman".to_string();
            config.excursion_protection.enabled = true;
            config.schroeder_split.enabled = true;
            config.allow_delay = true;
            config.broadband_target_matching.enabled = true;
            config.gd_opt.enabled = has_subwoofer;
            config.vog.enabled = has_height;
            config.vog.reference_channel = if is_cinema {
                "C".to_string()
            } else {
                "L".to_string()
            };
        }
    }

    /// Get channel names from speaker configs
    pub fn channel_names(&self) -> Vec<String> {
        self.speaker_configs
            .iter()
            .map(|c| c.channel_name.clone())
            .collect()
    }

    /// Check if optimization is complete
    pub fn is_optimization_complete(&self) -> bool {
        self.optimization_status == OptimizationStatus::Completed
    }

    /// Check if optimization is running
    pub fn is_optimizing(&self) -> bool {
        self.optimization_status == OptimizationStatus::Running
    }

    /// Initialize speaker configs from measurements
    pub fn init_speaker_configs(&mut self) {
        self.speaker_configs = self
            .channel_measurements
            .iter()
            .map(|m| RoomEqSpeakerConfig {
                channel_name: m.channel_name.clone(),
                config_type: if m.is_group {
                    SpeakerConfigType::MultiDriver
                } else {
                    SpeakerConfigType::Single
                },
                crossover_type: CrossoverType::LR24,
                driver_names: if m.is_group {
                    m.group_drivers
                        .iter()
                        .enumerate()
                        .map(|(i, _)| format!("driver_{}", i + 1))
                        .collect()
                } else {
                    Vec::new()
                },
                crossover_freq_hints: Vec::new(),
            })
            .collect();
    }

    /// Load measurements from recording state.
    ///
    /// Groups multi-mic recordings by speaker index so that each physical channel
    /// produces one `ChannelMeasurement` with additional mic data stored in
    /// `multi_mic_measurements` for multi-position optimization.
    pub fn load_from_recording(&mut self, recording_state: &RecordingState) {
        use std::collections::BTreeMap;

        // Group completed recordings by speaker index (channel_index)
        let mut grouped: BTreeMap<usize, Vec<&sotf_audio_player::recording_types::ChannelRecording>> =
            BTreeMap::new();
        for r in &recording_state.channel_recordings {
            if r.result.is_some() {
                grouped.entry(r.channel_index).or_default().push(r);
            }
        }

        self.channel_measurements = grouped
            .into_values()
            .map(|recordings| {
                let first = recordings[0];
                // Strip " (Mic N)" suffix for the channel name
                let base_name = first
                    .channel_name
                    .find(" (Mic ")
                    .map_or(first.channel_name.as_str(), |pos| {
                        &first.channel_name[..pos]
                    })
                    .to_string();

                let primary_result = first.result.clone().unwrap();
                let multi_mic = if recordings.len() > 1 {
                    recordings
                        .iter()
                        .skip(1)
                        .filter_map(|r| r.result.clone())
                        .collect()
                } else {
                    Vec::new()
                };

                ChannelMeasurement {
                    channel_name: base_name,
                    measurement: primary_result,
                    is_group: false,
                    group_drivers: Vec::new(),
                    multi_mic_measurements: multi_mic,
                }
            })
            .collect();

        self.data_source = RoomEqDataSource::FromRecording;
        self.init_speaker_configs();
    }

    /// Reset optimization state
    pub fn reset_optimization(&mut self) {
        self.optimization_status = OptimizationStatus::Idle;
        self.current_channel = None;
        self.channel_results.clear();
        self.overall_progress = 0.0;
        self.progress_history.clear();
        self.current_iteration = 0;
        self.current_loss = 0.0;
        self.error_message = None;
    }

    /// Get average pre-score
    pub fn average_pre_score(&self) -> f64 {
        if self.channel_results.is_empty() {
            0.0
        } else {
            self.channel_results
                .iter()
                .map(|r| r.pre_score)
                .sum::<f64>()
                / self.channel_results.len() as f64
        }
    }

    /// Get average post-score
    pub fn average_post_score(&self) -> f64 {
        if self.channel_results.is_empty() {
            0.0
        } else {
            self.channel_results
                .iter()
                .map(|r| r.post_score)
                .sum::<f64>()
                / self.channel_results.len() as f64
        }
    }

    /// Convert UI state to backend RoomConfig
    pub fn to_room_config(&self) -> RoomConfig {
        let mut speakers: HashMap<String, SpeakerConfig> = HashMap::new();
        let mut crossovers: HashMap<String, BackendCrossoverConfig> = HashMap::new();

        // Helper to convert measurement to curve (preserving phase if available)
        let to_curve = |meas: &ChannelMeasurement| -> autoeq::Curve {
            let frequencies: Vec<f64> = meas
                .measurement
                .frequencies
                .iter()
                .map(|&f| f as f64)
                .collect();
            let magnitude_db: Vec<f64> = meas
                .measurement
                .magnitude_db
                .iter()
                .map(|&db| db as f64)
                .collect();
            let phase = if !meas.measurement.phase_deg.is_empty()
                && meas.measurement.phase_deg.len() == frequencies.len()
            {
                Some(ndarray::Array1::from_vec(
                    meas.measurement
                        .phase_deg
                        .iter()
                        .map(|&p| p as f64)
                        .collect(),
                ))
            } else {
                None
            };

            autoeq::Curve {
                freq: ndarray::Array1::from_vec(frequencies),
                spl: ndarray::Array1::from_vec(magnitude_db),
                phase,
            }
        };

        // Helper to convert recording result to curve (preserving phase if available)
        let result_to_curve = |res: &RecordingResult| -> autoeq::Curve {
            let frequencies: Vec<f64> = res.frequencies.iter().map(|&f| f as f64).collect();
            let magnitude_db: Vec<f64> = res.magnitude_db.iter().map(|&db| db as f64).collect();
            let phase = if !res.phase_deg.is_empty() && res.phase_deg.len() == frequencies.len() {
                Some(ndarray::Array1::from_vec(
                    res.phase_deg.iter().map(|&p| p as f64).collect(),
                ))
            } else {
                None
            };

            autoeq::Curve {
                freq: ndarray::Array1::from_vec(frequencies),
                spl: ndarray::Array1::from_vec(magnitude_db),
                phase,
            }
        };

        // Iterate over configured speakers
        for speaker_config in &self.speaker_configs {
            let channel_name = &speaker_config.channel_name;

            // Find corresponding measurement
            if let Some(meas) = self
                .channel_measurements
                .iter()
                .find(|m| &m.channel_name == channel_name)
            {
                match speaker_config.config_type {
                    SpeakerConfigType::Single => {
                        if meas.multi_mic_measurements.is_empty() {
                            let curve = to_curve(meas);
                            speakers.insert(
                                channel_name.clone(),
                                SpeakerConfig::Single(MeasurementSource::InMemory(curve)),
                            );
                        } else {
                            // Multiple mic measurements → InMemoryMultiple for multi-position optimization
                            let mut curves = vec![to_curve(meas)];
                            for extra in &meas.multi_mic_measurements {
                                curves.push(result_to_curve(extra));
                            }
                            speakers.insert(
                                channel_name.clone(),
                                SpeakerConfig::Single(MeasurementSource::InMemoryMultiple(curves)),
                            );
                        }
                    }
                    SpeakerConfigType::MultiDriver => {
                        let mut driver_measurements = Vec::new();
                        if meas.is_group && !meas.group_drivers.is_empty() {
                            for driver_res in &meas.group_drivers {
                                driver_measurements
                                    .push(MeasurementSource::InMemory(result_to_curve(driver_res)));
                            }
                        } else {
                            driver_measurements.push(MeasurementSource::InMemory(to_curve(meas)));
                        }

                        let xover_id = format!("xover_{}", channel_name);
                        let xover_type = match speaker_config.crossover_type {
                            CrossoverType::LR12 => "LR12",
                            CrossoverType::LR24 => "LR24",
                            CrossoverType::LR48 => "LR48",
                            CrossoverType::Butterworth12 => "Butterworth12",
                            CrossoverType::Butterworth24 => "Butterworth24",
                        };

                        crossovers.insert(
                            xover_id.clone(),
                            BackendCrossoverConfig {
                                crossover_type: xover_type.to_string(),
                                frequency: None,
                                frequencies: None,
                                frequency_range: None,
                            },
                        );

                        speakers.insert(
                            channel_name.clone(),
                            SpeakerConfig::Group(SpeakerGroup {
                                name: channel_name.clone(),
                                speaker_name: None,
                                measurements: driver_measurements,
                                crossover: Some(xover_id),
                            }),
                        );
                    }
                }
            }
        }

        let algorithm = self.optimizer_config.algorithm.clone();

        let processing_mode = match self.optimizer_config.mode {
            RoomEqOptimizationMode::Iir => BackendProcessingMode::LowLatency,
            RoomEqOptimizationMode::Fir => BackendProcessingMode::PhaseLinear,
            RoomEqOptimizationMode::Mixed => BackendProcessingMode::Hybrid,
            RoomEqOptimizationMode::MixedPhase => BackendProcessingMode::MixedPhase,
        };

        let optimizer = BackendOptimizerConfig {
            loss_type: self.optimizer_config.loss_type.clone(),
            algorithm,
            strategy: self.optimizer_config.strategy.clone(),
            num_filters: self.optimizer_config.num_filters,
            min_q: self.optimizer_config.min_q,
            max_q: self.optimizer_config.max_q,
            min_db: self.optimizer_config.min_db,
            max_db: self.optimizer_config.max_db,
            min_freq: self.optimizer_config.min_freq,
            max_freq: self.optimizer_config.max_freq,
            max_iter: self.optimizer_config.max_iter,
            population: self.optimizer_config.population,
            peq_model: self.optimizer_config.peq_model.clone(),
            mode: self.optimizer_config.mode.to_code().to_string(),
            processing_mode,
            fir: Some(BackendFirConfig {
                taps: self.optimizer_config.fir.taps,
                phase: self.optimizer_config.fir.phase.clone(),
                correct_excess_phase: self.optimizer_config.fir.correct_excess_phase,
                phase_smoothing: self.optimizer_config.fir.phase_smoothing,
                pre_ringing: self.optimizer_config.fir.pre_ringing.as_ref().map(|pr| {
                    BackendPreRingingConfig {
                        threshold_db: pr.threshold_db,
                        max_time_s: pr.max_time_s,
                    }
                }),
            }),
            mixed_phase: if self.optimizer_config.mode == RoomEqOptimizationMode::MixedPhase {
                Some(BackendMixedPhaseConfig {
                    max_fir_length_ms: self.optimizer_config.mixed_phase.max_fir_length_ms,
                    pre_ringing_threshold_db: self
                        .optimizer_config
                        .mixed_phase
                        .pre_ringing_threshold_db,
                    min_spatial_depth: self.optimizer_config.mixed_phase.min_spatial_depth,
                    phase_smoothing_octaves: self
                        .optimizer_config
                        .mixed_phase
                        .phase_smoothing_octaves,
                })
            } else {
                None
            },
            seed: self.optimizer_config.seed,
            mixed_config: if self.optimizer_config.mode == RoomEqOptimizationMode::Mixed {
                Some(autoeq::roomeq::MixedModeConfig {
                    crossover_freq: self.optimizer_config.mixed_config.crossover_freq,
                    crossover_type: self.optimizer_config.mixed_config.crossover_type.clone(),
                    fir_band: self.optimizer_config.mixed_config.fir_band.clone(),
                })
            } else {
                None
            },
            refine: self.optimizer_config.refine,
            local_algo: self.optimizer_config.local_algo.clone(),
            psychoacoustic: self.optimizer_config.psychoacoustic,
            asymmetric_loss: self.optimizer_config.asymmetric_loss,
            tolerance: self.optimizer_config.tolerance,
            atolerance: self.optimizer_config.atolerance,
            allow_delay: Some(self.optimizer_config.allow_delay),
            target_tilt: if self.optimizer_config.target_tilt.enabled {
                let tilt_type = match self.optimizer_config.target_tilt.tilt_type.as_str() {
                    "harman" => TiltType::Harman,
                    "custom" => TiltType::Custom,
                    _ => TiltType::Flat,
                };
                Some(BackendTargetTiltConfig {
                    tilt_type,
                    slope_db_per_octave: self.optimizer_config.target_tilt.slope,
                    reference_freq: self.optimizer_config.target_tilt.reference_freq,
                    bass_shelf_db: self.optimizer_config.target_tilt.bass_shelf_db,
                    bass_shelf_freq: self.optimizer_config.target_tilt.bass_shelf_freq,
                })
            } else {
                None
            },
            excursion_protection: if self.optimizer_config.excursion_protection.enabled {
                let filter_type = if self.optimizer_config.excursion_protection.filter_type == "bw"
                {
                    HighpassType::Butterworth
                } else {
                    HighpassType::LinkwitzRiley
                };
                Some(BackendExcursionProtectionConfig {
                    enabled: true,
                    auto_detect_f3: self.optimizer_config.excursion_protection.auto_detect_f3,
                    manual_f3_hz: Some(self.optimizer_config.excursion_protection.manual_f3_hz),
                    filter_order: self.optimizer_config.excursion_protection.filter_order,
                    filter_type,
                    margin_octaves: self.optimizer_config.excursion_protection.margin_octaves,
                })
            } else {
                None
            },
            schroeder_split: if self.optimizer_config.schroeder_split.enabled {
                Some(BackendSchroederSplitConfig {
                    enabled: true,
                    schroeder_freq: self.optimizer_config.schroeder_split.schroeder_freq,
                    room_dimensions: None,
                    low_freq_config: LowFreqFilterConfig {
                        max_q: self.optimizer_config.schroeder_split.low_freq_max_q,
                        min_q: 0.5,
                        allow_boost: self.optimizer_config.schroeder_split.low_freq_allow_boost,
                        max_db: self.optimizer_config.schroeder_split.low_freq_max_db,
                    },
                    high_freq_config: HighFreqFilterConfig {
                        max_q: self.optimizer_config.schroeder_split.high_freq_max_q,
                        shelving_only: self
                            .optimizer_config
                            .schroeder_split
                            .high_freq_shelving_only,
                    },
                })
            } else {
                None
            },
            phase_alignment: if self.optimizer_config.phase_alignment.enabled {
                Some(BackendPhaseAlignmentConfig {
                    enabled: true,
                    min_freq: self.optimizer_config.phase_alignment.min_freq,
                    max_freq: self.optimizer_config.phase_alignment.max_freq,
                    optimize_polarity: self.optimizer_config.phase_alignment.optimize_polarity,
                    max_delay_ms: self.optimizer_config.phase_alignment.max_delay_ms,
                })
            } else {
                None
            },
            multi_seat: if self.optimizer_config.multi_seat.enabled {
                let strategy = match self.optimizer_config.multi_seat.strategy.as_str() {
                    "primary" => MultiSeatStrategy::PrimaryWithConstraints,
                    "average" => MultiSeatStrategy::Average,
                    _ => MultiSeatStrategy::MinimizeVariance,
                };
                Some(BackendMultiSeatConfig {
                    enabled: true,
                    strategy,
                    primary_seat: self.optimizer_config.multi_seat.primary_seat,
                    max_deviation_db: self.optimizer_config.multi_seat.max_deviation_db,
                })
            } else {
                None
            },
            gd_opt: if self.optimizer_config.gd_opt.enabled {
                Some(autoeq::roomeq::GroupDelayOptimizationConfig {
                    enabled: true,
                    target_ms: self.optimizer_config.gd_opt.target_ms,
                })
            } else {
                None
            },
            vog: if self.optimizer_config.vog.enabled {
                Some(autoeq::roomeq::VoiceOfGodConfig {
                    enabled: true,
                    reference_channel: self.optimizer_config.vog.reference_channel.clone(),
                })
            } else {
                None
            },
            broadband_target_matching: if self.optimizer_config.broadband_target_matching.enabled {
                Some(autoeq::roomeq::BroadbandTargetMatchingConfig { enabled: true })
            } else {
                None
            },
            multi_measurement: if self.optimizer_config.multi_measurement.enabled {
                let strategy = match self.optimizer_config.multi_measurement.strategy.as_str() {
                    "average" => MultiMeasurementStrategy::Average,
                    "weighted_sum" => MultiMeasurementStrategy::WeightedSum,
                    "minimax" => MultiMeasurementStrategy::Minimax,
                    "variance_penalized" => MultiMeasurementStrategy::VariancePenalized,
                    "spatial_robustness" => MultiMeasurementStrategy::SpatialRobustness,
                    s => panic!("Unknown multi_measurement strategy: {s}"),
                };
                let weights = if self.optimizer_config.multi_measurement.weights.is_empty() {
                    None
                } else {
                    Some(self.optimizer_config.multi_measurement.weights.clone())
                };
                Some(MultiMeasurementConfig {
                    strategy,
                    weights,
                    variance_lambda: self.optimizer_config.multi_measurement.variance_lambda,
                    spatial_robustness: None,
                })
            } else {
                None
            },
            smooth_n: self.optimizer_config.smooth_n,
            decomposed_correction: None,
            target_response: None,
            cea2034_correction: None,
            sub_config: if self.optimizer_config.sub_config.enabled {
                Some(autoeq::roomeq::SubOptimizerConfig {
                    num_filters: self.optimizer_config.sub_config.num_filters,
                    max_db: self.optimizer_config.sub_config.max_db,
                    min_db: self.optimizer_config.sub_config.min_db,
                    min_q: self.optimizer_config.sub_config.min_q,
                    max_q: self.optimizer_config.sub_config.max_q,
                })
            } else {
                None
            },
            channel_matching: if self.optimizer_config.channel_matching.enabled {
                Some(autoeq::roomeq::ChannelMatchingConfig {
                    enabled: true,
                    threshold_db: self.optimizer_config.channel_matching.threshold_db,
                    max_filters: self.optimizer_config.channel_matching.max_filters,
                })
            } else {
                None
            },
            min_filter_improvement: 0.0,
            elimination_threshold: 0.0,
            ssir_wav_path: None,
        };

        log::info!(
            "RoomConfig: filters={}, max_q={}, max_freq={}, schroeder={}, target_tilt={}, excursion={}, broadband={}, imported={}",
            optimizer.num_filters,
            optimizer.max_q,
            optimizer.max_freq,
            optimizer.schroeder_split.is_some(),
            optimizer.target_tilt.is_some(),
            optimizer.excursion_protection.is_some(),
            optimizer.broadband_target_matching.is_some(),
            self.optimizer_config.imported_from_file,
        );

        RoomConfig {
            version: autoeq::roomeq::default_config_version(),
            system: None,
            speakers,
            crossovers: Some(crossovers),
            target_curve: None,
            optimizer,
            recording_config: None,
            cea2034_cache: None,
        }
    }

    /// Validate the current configuration
    pub fn validate(&self) -> autoeq::roomeq::ValidationResult {
        let config = self.to_room_config();
        autoeq::roomeq::validate_room_config(&config)
    }

    /// Calculate the level offset needed to normalize a curve to 0dB.
    /// Uses mean SPL in the 1kHz to 2kHz range by default.
    pub fn calculate_normalization_offset(frequencies: &[f64], spl: &[f64]) -> f64 {
        let min_freq = 1000.0;
        let max_freq = 2000.0;

        let mut sum = 0.0;
        let mut count = 0;

        for (i, &f) in frequencies.iter().enumerate() {
            if f >= min_freq
                && f <= max_freq
                && let Some(&db) = spl.get(i)
            {
                sum += db;
                count += 1;
            }
        }

        if count > 0 {
            sum / count as f64
        } else {
            // Fallback: overall mean
            if spl.is_empty() {
                0.0
            } else {
                spl.iter().sum::<f64>() / spl.len() as f64
            }
        }
    }

    /// Normalize a set of points by subtracting an offset.
    pub fn normalize_points(points: &[(f64, f64)], offset: f64) -> Vec<(f64, f64)> {
        points.iter().map(|&(f, db)| (f, db - offset)).collect()
    }
}
