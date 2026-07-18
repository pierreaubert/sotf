use super::bootstrap::bootstrap_uncertainty_from_backend;
use super::bootstrap::bootstrap_uncertainty_to_backend;
use super::channel_matching_ui_config::ChannelMatchingUiConfig;
use super::channel_metadata::ChannelMetadata;
use super::continuous::continuous_area_from_backend;
use super::continuous::continuous_area_to_backend;
use super::default::default_adaptive_weight_cr;
use super::default::default_adaptive_weight_f;
use super::default::default_all_channel_multiseat_strategy;
use super::default::default_bo_acquisition;
use super::default::default_de_cr;
use super::default::default_de_f;
use super::default::default_min_spacing_oct;
use super::default::default_primary_seat_weight;
use super::default::default_room_atolerance;
use super::default::default_room_smooth_n;
use super::default::default_room_strategy;
use super::default::default_room_tolerance;
use super::default::default_sample_rate;
use super::default::default_spacing_weight;
use super::epa_temporal_masking_config::EpaTemporalMaskingConfig;
use super::excursion_protection_config::ExcursionProtectionConfig;
use super::misc::canonical_multi_measurement_strategy;
use super::mixed_mode_ui_config::MixedModeUiConfig;
use super::mixed_phase_ui_config::MixedPhaseUiConfig;
use super::multi_measurement_ui_config::MultiMeasurementUiConfig;
use super::multi_seat_config::MultiSeatConfig;
use super::multi_speaker_mode::MultiSpeakerMode;
use super::phase_alignment_config::PhaseAlignmentConfig;
use super::pre_ringing_config::PreRingingConfig;
use super::room_eq_fir_config::RoomEqFirConfig;
use super::room_eq_optimization_mode::RoomEqOptimizationMode;
use super::schroeder_split_config::SchroederSplitConfig;
use super::sub_optimizer_ui_config::SubOptimizerUiConfig;
use super::target_response_ui_config::TargetResponseUiConfig;
use super::vo_gconfig::VoGConfig;
use serde::{Deserialize, Serialize};

/// Optimizer configuration for Room EQ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEqOptimizerConfig {
    #[serde(default)]
    pub mode: RoomEqOptimizationMode,
    #[serde(default)]
    pub fir: RoomEqFirConfig,
    pub multi_speaker_mode: MultiSpeakerMode,
    pub algorithm: String,
    #[serde(default = "default_room_strategy")]
    pub strategy: String,
    #[serde(default = "default_de_f")]
    pub de_f: f64,
    #[serde(default = "default_de_cr")]
    pub de_cr: f64,
    #[serde(default = "default_adaptive_weight_f")]
    pub adaptive_weight_f: f64,
    #[serde(default = "default_adaptive_weight_cr")]
    pub adaptive_weight_cr: f64,
    #[serde(default = "default_spacing_weight")]
    pub spacing_weight: f64,
    #[serde(default = "default_min_spacing_oct")]
    pub min_spacing_oct: f64,
    #[serde(default = "default_sample_rate")]
    pub sample_rate: usize,
    pub num_filters: usize,
    pub min_q: f64,
    pub max_q: f64,
    pub min_db: f64,
    pub max_db: f64,
    pub min_freq: f64,
    pub max_freq: f64,
    pub max_iter: usize,
    pub peq_model: String,
    pub population: usize,
    #[serde(default)]
    pub bo_initial_samples: usize,
    #[serde(default)]
    pub bo_batch_size: usize,
    #[serde(default)]
    pub bo_posterior_std_threshold: f64,
    #[serde(default = "default_bo_acquisition")]
    pub bo_acquisition: String,
    #[serde(default)]
    pub bo_ehvi: bool,
    pub refine: bool,
    pub local_algo: String,
    pub loss_type: String,
    pub psychoacoustic: bool,
    pub asymmetric_loss: bool,
    #[serde(default)]
    pub smooth: bool,
    #[serde(default = "default_room_smooth_n")]
    pub smooth_n: usize,
    #[serde(default = "default_room_tolerance")]
    pub tolerance: f64,
    #[serde(default = "default_room_atolerance")]
    pub atolerance: f64,
    pub target_curve: String,
    pub system_type: String,
    #[serde(default)]
    pub allow_delay: bool,
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default)]
    pub vog: VoGConfig,
    #[serde(default)]
    pub mixed_config: MixedModeUiConfig,
    #[serde(default)]
    pub mixed_phase: MixedPhaseUiConfig,
    /// Unified target response (shape + preference shelves + broadband pre-correction).
    #[serde(default)]
    pub target_response: TargetResponseUiConfig,
    #[serde(default)]
    pub excursion_protection: ExcursionProtectionConfig,
    #[serde(default)]
    pub schroeder_split: SchroederSplitConfig,
    #[serde(default)]
    pub phase_alignment: PhaseAlignmentConfig,
    #[serde(default)]
    pub multi_seat: MultiSeatConfig,
    #[serde(default)]
    pub multi_measurement: MultiMeasurementUiConfig,
    #[serde(default)]
    pub sub_config: SubOptimizerUiConfig,
    #[serde(default)]
    pub channel_matching: ChannelMatchingUiConfig,
    /// EPA temporal-masking knobs surfaced in the Step-3 configuration UI.
    /// Default keeps the backend's built-in defaults — see
    /// [`EpaTemporalMaskingConfig::differs_from_default`].
    #[serde(default)]
    pub epa_temporal_masking: EpaTemporalMaskingConfig,
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
            algorithm: "autoeq:cmaes".to_string(),
            strategy: "lshade".to_string(),
            de_f: default_de_f(),
            de_cr: default_de_cr(),
            adaptive_weight_f: default_adaptive_weight_f(),
            adaptive_weight_cr: default_adaptive_weight_cr(),
            spacing_weight: default_spacing_weight(),
            min_spacing_oct: default_min_spacing_oct(),
            sample_rate: default_sample_rate(),
            num_filters: 7,
            min_q: 0.5,
            max_q: 6.0,
            min_db: -12.0,
            max_db: 4.0,
            min_freq: 20.0,
            max_freq: 1600.0,
            max_iter: 50000,
            peq_model: "pk".to_string(),
            population: 300,
            bo_initial_samples: 0,
            bo_batch_size: 0,
            bo_posterior_std_threshold: 0.0,
            bo_acquisition: default_bo_acquisition(),
            bo_ehvi: false,
            refine: false,
            local_algo: "cobyla".to_string(),
            loss_type: "flat".to_string(),
            psychoacoustic: true,
            asymmetric_loss: true,
            smooth: false,
            smooth_n: default_room_smooth_n(),
            tolerance: 1e-5,
            atolerance: 1e-5,
            target_curve: "flat".to_string(),
            system_type: "stereo".to_string(),
            allow_delay: false,
            seed: None,
            vog: VoGConfig::default(),
            mixed_config: MixedModeUiConfig::default(),
            mixed_phase: MixedPhaseUiConfig::default(),
            target_response: TargetResponseUiConfig::default(),
            excursion_protection: ExcursionProtectionConfig::default(),
            schroeder_split: SchroederSplitConfig::default(),
            phase_alignment: PhaseAlignmentConfig::default(),
            multi_seat: MultiSeatConfig::default(),
            multi_measurement: MultiMeasurementUiConfig::default(),
            sub_config: SubOptimizerUiConfig::default(),
            channel_matching: ChannelMatchingUiConfig::default(),
            epa_temporal_masking: EpaTemporalMaskingConfig::default(),
            imported_from_file: false,
        }
    }
}

impl RoomEqOptimizerConfig {
    /// Import optimizer parameters and feature toggles from a backend `OptimizerConfig`.
    ///
    /// This is used when loading a RoomConfig JSON file so that the UI
    /// uses the same optimizer settings as the roomeq CLI.
    /// Sets `imported_from_file = true` so that `apply_smart_defaults()` will
    /// not override the imported feature toggle state.
    pub fn import_from_backend(&mut self, backend: &autoeq::roomeq::OptimizerConfig) {
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
        self.bo_initial_samples = backend.bo_initial_samples.unwrap_or(0);
        self.bo_batch_size = backend.bo_batch_size.unwrap_or(0);
        self.bo_posterior_std_threshold = backend.bo_posterior_std_threshold.unwrap_or(0.0);
        self.bo_acquisition = backend
            .bo_acquisition
            .clone()
            .unwrap_or_else(default_bo_acquisition);
        self.bo_ehvi = backend.bo_ehvi.unwrap_or(false);
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
            autoeq::roomeq::ProcessingMode::LowLatency => RoomEqOptimizationMode::Iir,
            autoeq::roomeq::ProcessingMode::PhaseLinear => RoomEqOptimizationMode::Fir,
            autoeq::roomeq::ProcessingMode::Hybrid => RoomEqOptimizationMode::Mixed,
            autoeq::roomeq::ProcessingMode::MixedPhase => RoomEqOptimizationMode::MixedPhase,
            // WarpedIir and KautzModal are IIR-based modes
            autoeq::roomeq::ProcessingMode::WarpedIir
            | autoeq::roomeq::ProcessingMode::KautzModal => RoomEqOptimizationMode::Iir,
        };

        // Feature toggles: only override from backend when explicitly present.
        if let Some(ref tr) = backend.target_response {
            self.target_response.enabled = true;
            self.target_response.shape = match tr.shape {
                autoeq::roomeq::TargetShape::Flat => "flat".to_string(),
                autoeq::roomeq::TargetShape::Harman => "harman".to_string(),
                autoeq::roomeq::TargetShape::Custom => "custom".to_string(),
                autoeq::roomeq::TargetShape::File => "file".to_string(),
                autoeq::roomeq::TargetShape::FromMeasurement => "from_measurement".to_string(),
            };
            self.target_response.slope_db_per_octave = tr.slope_db_per_octave;
            self.target_response.reference_freq = tr.reference_freq;
            self.target_response.curve_path = tr.curve_path.clone();
            self.target_response.bass_shelf_db = tr.preference.bass_shelf_db;
            self.target_response.bass_shelf_freq = tr.preference.bass_shelf_freq;
            self.target_response.treble_shelf_db = tr.preference.treble_shelf_db;
            self.target_response.treble_shelf_freq = tr.preference.treble_shelf_freq;
            self.target_response.broadband_precorrection = tr.broadband_precorrection;
        } else {
            self.target_response.enabled = false;
        }

        self.excursion_protection.enabled = backend
            .excursion_protection
            .as_ref()
            .is_some_and(|e| e.enabled);
        if let Some(ref ep) = backend.excursion_protection {
            self.excursion_protection.auto_detect_f3 = ep.auto_detect_f3;
            self.excursion_protection.manual_f3_hz = ep.manual_f3_hz.unwrap_or(40.0);
            self.excursion_protection.f3_reference_min_hz = ep.f3_reference_min_hz;
            self.excursion_protection.f3_reference_max_hz = ep.f3_reference_max_hz;
            self.excursion_protection.filter_order = ep.filter_order;
            self.excursion_protection.filter_type = match ep.filter_type {
                autoeq::roomeq::HighpassType::Butterworth => "bw".to_string(),
                autoeq::roomeq::HighpassType::LinkwitzRiley => "lr".to_string(),
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

        self.allow_delay = backend.allow_delay.unwrap_or(false);

        self.vog.enabled = backend
            .inter_channel_timbre_matching
            .as_ref()
            .is_some_and(|config| config.enabled);
        if let Some(ref config) = backend.inter_channel_timbre_matching {
            self.vog.reference_channel = config.reference_channel.clone();
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
                autoeq::roomeq::MultiSeatStrategy::ModalBasis => "modal_basis".to_string(),
                autoeq::roomeq::MultiSeatStrategy::ContinuousArea => "continuous_area".to_string(),
            };
            self.multi_seat.primary_seat = ms.primary_seat;
            self.multi_seat.max_deviation_db = ms.max_deviation_db;
            self.multi_seat.continuous_area = ms
                .continuous_area
                .as_ref()
                .map(continuous_area_from_backend);
        }

        if let Some(ref mm) = backend.multi_measurement {
            self.multi_measurement.enabled = true;
            self.multi_measurement.strategy = match mm.strategy {
                autoeq::roomeq::MultiMeasurementStrategy::Average => "average".to_string(),
                autoeq::roomeq::MultiMeasurementStrategy::WeightedSum => "weighted_sum".to_string(),
                autoeq::roomeq::MultiMeasurementStrategy::Minimax => "minimax".to_string(),
                autoeq::roomeq::MultiMeasurementStrategy::VariancePenalized => {
                    "variance_penalized".to_string()
                }
                autoeq::roomeq::MultiMeasurementStrategy::SpatialRobustness => {
                    "spatial_robustness".to_string()
                }
                autoeq::roomeq::MultiMeasurementStrategy::MinimaxUncertainty => {
                    "minimax_uncertainty".to_string()
                }
            };
            self.multi_measurement.variance_lambda = mm.variance_lambda;
            self.multi_measurement.weights = mm.weights.clone().unwrap_or_default();
            self.multi_measurement.bootstrap_uncertainty = mm
                .bootstrap_uncertainty
                .as_ref()
                .map(bootstrap_uncertainty_from_backend);
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
        self.channel_matching.enabled =
            backend.channel_matching.as_ref().is_some_and(|c| c.enabled);
        if let Some(ref cm) = backend.channel_matching {
            self.channel_matching.threshold_db = cm.threshold_db;
            self.channel_matching.max_filters = cm.max_filters;
        }

        self.imported_from_file = true;
    }

    /// Convert the flat UI optimizer config to a backend
    /// [`OptimizerConfig`](autoeq::roomeq::OptimizerConfig).
    ///
    /// This is the single canonical conversion used by both GPUI and TUI
    /// when building a `RoomConfig` for the optimizer.
    pub fn to_optimizer_config(&self) -> autoeq::roomeq::OptimizerConfig {
        use autoeq::roomeq::{
            ChannelMatchingConfig as BackendChannelMatchingConfig, DecomposedCorrectionSerdeConfig,
            ExcursionProtectionConfig as BackendExcursionProtectionConfig,
            FirConfig as BackendFirConfig, HighFreqFilterConfig, HighpassType,
            InterChannelTimbreMatchingConfig, LowFreqFilterConfig, MixedModeConfig,
            MixedPhaseSerdeConfig as BackendMixedPhaseConfig, MultiMeasurementConfig,
            MultiMeasurementStrategy, MultiSeatConfig as BackendMultiSeatConfig, MultiSeatStrategy,
            OptimizerConfig as BackendOptimizerConfig,
            PhaseAlignmentConfig as BackendPhaseAlignmentConfig,
            PreRingingSerdeConfig as BackendPreRingingConfig, ProcessingMode,
            SchroederSplitConfig as BackendSchroederSplitConfig, SubOptimizerConfig,
            TargetResponseConfig as BackendTargetResponseConfig, TargetShape, UserPreference,
        };

        let processing_mode = match self.mode {
            RoomEqOptimizationMode::Iir => ProcessingMode::LowLatency,
            RoomEqOptimizationMode::Fir => ProcessingMode::PhaseLinear,
            RoomEqOptimizationMode::Mixed => ProcessingMode::Hybrid,
            RoomEqOptimizationMode::MixedPhase => ProcessingMode::MixedPhase,
        };

        let fir = Some(BackendFirConfig {
            taps: self.fir.taps,
            phase: self.fir.phase.clone(),
            correct_excess_phase: self.fir.correct_excess_phase,
            phase_smoothing: self.fir.phase_smoothing,
            pre_ringing: self
                .fir
                .pre_ringing
                .as_ref()
                .map(|pr| BackendPreRingingConfig {
                    threshold_db: pr.threshold_db,
                    max_time_s: pr.max_time_s,
                }),
        });

        let mixed_phase = if self.mode == RoomEqOptimizationMode::MixedPhase {
            Some(BackendMixedPhaseConfig {
                max_fir_length_ms: self.mixed_phase.max_fir_length_ms,
                pre_ringing_threshold_db: self.mixed_phase.pre_ringing_threshold_db,
                min_spatial_depth: self.mixed_phase.min_spatial_depth,
                phase_smoothing_octaves: self.mixed_phase.phase_smoothing_octaves,
            })
        } else {
            None
        };

        let mixed_config = if self.mode == RoomEqOptimizationMode::Mixed {
            Some(MixedModeConfig {
                crossover_freq: self.mixed_config.crossover_freq,
                crossover_type: self.mixed_config.crossover_type.clone(),
                fir_band: self.mixed_config.fir_band.clone(),
            })
        } else {
            None
        };

        let target_response = if self.target_response.enabled {
            let tr = &self.target_response;
            let shape = match tr.shape.as_str() {
                "flat" => TargetShape::Flat,
                "harman" => TargetShape::Harman,
                "custom" => TargetShape::Custom,
                "file" => TargetShape::File,
                "from_measurement" => TargetShape::FromMeasurement,
                _ => TargetShape::Custom,
            };
            Some(BackendTargetResponseConfig {
                shape,
                slope_db_per_octave: tr.slope_db_per_octave,
                reference_freq: tr.reference_freq,
                curve_path: tr.curve_path.clone(),
                preference: UserPreference {
                    bass_shelf_db: tr.bass_shelf_db,
                    bass_shelf_freq: tr.bass_shelf_freq,
                    treble_shelf_db: tr.treble_shelf_db,
                    treble_shelf_freq: tr.treble_shelf_freq,
                },
                broadband_precorrection: tr.broadband_precorrection,
                role_targets: None,
            })
        } else {
            None
        };

        let excursion_protection = if self.excursion_protection.enabled {
            let filter_type = if self.excursion_protection.filter_type == "bw" {
                HighpassType::Butterworth
            } else {
                HighpassType::LinkwitzRiley
            };
            Some(BackendExcursionProtectionConfig {
                enabled: true,
                auto_detect_f3: self.excursion_protection.auto_detect_f3,
                manual_f3_hz: Some(self.excursion_protection.manual_f3_hz),
                f3_reference_min_hz: self.excursion_protection.f3_reference_min_hz,
                f3_reference_max_hz: self.excursion_protection.f3_reference_max_hz,
                filter_order: self.excursion_protection.filter_order,
                filter_type,
                margin_octaves: self.excursion_protection.margin_octaves,
            })
        } else {
            None
        };

        let schroeder_split = if self.schroeder_split.enabled {
            Some(BackendSchroederSplitConfig {
                enabled: true,
                schroeder_freq: self.schroeder_split.schroeder_freq,
                room_dimensions: None,
                low_freq_config: LowFreqFilterConfig {
                    max_q: self.schroeder_split.low_freq_max_q,
                    min_q: 0.5,
                    allow_boost: self.schroeder_split.low_freq_allow_boost,
                    max_db: self.schroeder_split.low_freq_max_db,
                },
                high_freq_config: HighFreqFilterConfig {
                    max_q: self.schroeder_split.high_freq_max_q,
                    shelving_only: self.schroeder_split.high_freq_shelving_only,
                },
            })
        } else {
            None
        };

        let phase_alignment = if self.phase_alignment.enabled {
            Some(BackendPhaseAlignmentConfig {
                enabled: true,
                min_freq: self.phase_alignment.min_freq,
                max_freq: self.phase_alignment.max_freq,
                optimize_polarity: self.phase_alignment.optimize_polarity,
                max_delay_ms: self.phase_alignment.max_delay_ms,
            })
        } else {
            None
        };

        let has_all_channel_multiseat_policy = !self.multi_seat.all_channel_enabled
            || self.multi_seat.all_channel_strategy != default_all_channel_multiseat_strategy()
            || self.multi_seat.seat_weights.is_some()
            || (self.multi_seat.primary_seat_weight - default_primary_seat_weight()).abs() > 1e-9
            || self.multi_seat.primary_seat != 0
            || (self.multi_seat.max_deviation_db - 6.0).abs() > 1e-9;
        let multi_seat = if self.multi_seat.enabled || has_all_channel_multiseat_policy {
            let strategy = match self.multi_seat.strategy.as_str() {
                "primary" => MultiSeatStrategy::PrimaryWithConstraints,
                "average" => MultiSeatStrategy::Average,
                "modal_basis" => MultiSeatStrategy::ModalBasis,
                "continuous_area" => MultiSeatStrategy::ContinuousArea,
                _ => MultiSeatStrategy::MinimizeVariance,
            };
            Some(BackendMultiSeatConfig {
                enabled: self.multi_seat.enabled,
                strategy,
                primary_seat: self.multi_seat.primary_seat,
                max_deviation_db: self.multi_seat.max_deviation_db,
                optimize_polarity: false,
                allpass_filters_per_sub: 0,
                per_sub_peq: true,
                global_eq: true,
                all_channel_enabled: self.multi_seat.all_channel_enabled,
                all_channel_strategy: match self.multi_seat.all_channel_strategy.as_str() {
                    "weighted_sum" => autoeq::roomeq::MultiMeasurementStrategy::WeightedSum,
                    "minimax" => autoeq::roomeq::MultiMeasurementStrategy::Minimax,
                    "variance_penalized" => {
                        autoeq::roomeq::MultiMeasurementStrategy::VariancePenalized
                    }
                    "average" => autoeq::roomeq::MultiMeasurementStrategy::Average,
                    "minimax_uncertainty" => {
                        autoeq::roomeq::MultiMeasurementStrategy::MinimaxUncertainty
                    }
                    _ => autoeq::roomeq::MultiMeasurementStrategy::SpatialRobustness,
                },
                seat_weights: self.multi_seat.seat_weights.clone(),
                primary_seat_weight: self.multi_seat.primary_seat_weight,
                continuous_area: self
                    .multi_seat
                    .continuous_area
                    .as_ref()
                    .map(continuous_area_to_backend),
            })
        } else {
            None
        };

        let inter_channel_timbre_matching = if self.vog.enabled {
            Some(InterChannelTimbreMatchingConfig {
                enabled: true,
                reference_channel: self.vog.reference_channel.clone(),
                ..Default::default()
            })
        } else {
            None
        };

        let multi_measurement = if self.multi_measurement.enabled {
            let strategy_key =
                canonical_multi_measurement_strategy(&self.multi_measurement.strategy)
                    .unwrap_or_else(|| {
                        log::warn!(
                            "Unknown multi_measurement strategy '{}'; falling back to average",
                            self.multi_measurement.strategy
                        );
                        "average"
                    });
            let strategy = match strategy_key {
                "average" => MultiMeasurementStrategy::Average,
                "weighted_sum" => MultiMeasurementStrategy::WeightedSum,
                "minimax" => MultiMeasurementStrategy::Minimax,
                "variance_penalized" => MultiMeasurementStrategy::VariancePenalized,
                "spatial_robustness" => MultiMeasurementStrategy::SpatialRobustness,
                "minimax_uncertainty" => MultiMeasurementStrategy::MinimaxUncertainty,
                _ => MultiMeasurementStrategy::Average,
            };
            let weights = if self.multi_measurement.weights.is_empty() {
                None
            } else {
                Some(self.multi_measurement.weights.clone())
            };
            Some(MultiMeasurementConfig {
                strategy,
                weights,
                variance_lambda: self.multi_measurement.variance_lambda,
                spatial_robustness: None,
                bootstrap_uncertainty: self
                    .multi_measurement
                    .bootstrap_uncertainty
                    .as_ref()
                    .map(bootstrap_uncertainty_to_backend),
                rir_prototype: None,
            })
        } else {
            None
        };

        let sub_config = if self.sub_config.enabled {
            Some(SubOptimizerConfig {
                num_filters: self.sub_config.num_filters,
                max_db: self.sub_config.max_db,
                min_db: self.sub_config.min_db,
                min_q: self.sub_config.min_q,
                max_q: self.sub_config.max_q,
            })
        } else {
            None
        };

        let channel_matching = if self.channel_matching.enabled {
            Some(BackendChannelMatchingConfig {
                enabled: true,
                threshold_db: self.channel_matching.threshold_db,
                max_filters: self.channel_matching.max_filters,
            })
        } else {
            None
        };
        let is_bo_algorithm = self.algorithm.eq_ignore_ascii_case("autoeq:bo")
            || self.algorithm.eq_ignore_ascii_case("bo");

        BackendOptimizerConfig {
            loss_type: self.loss_type.clone(),
            algorithm: self.algorithm.clone(),
            strategy: self.strategy.clone(),
            num_filters: self.num_filters,
            min_q: self.min_q,
            max_q: self.max_q,
            min_db: self.min_db,
            max_db: self.max_db,
            min_freq: self.min_freq,
            max_freq: self.max_freq,
            max_iter: self.max_iter,
            population: self.population,
            peq_model: self.peq_model.clone(),
            processing_mode,
            fir,
            mixed_phase,
            mixed_config,
            seed: self.seed,
            refine: self.refine,
            local_algo: self.local_algo.clone(),
            bo_initial_samples: (is_bo_algorithm && self.bo_initial_samples > 0)
                .then_some(self.bo_initial_samples),
            bo_batch_size: (is_bo_algorithm && self.bo_batch_size > 0)
                .then_some(self.bo_batch_size),
            bo_posterior_std_threshold: (is_bo_algorithm && self.bo_posterior_std_threshold > 0.0)
                .then_some(self.bo_posterior_std_threshold),
            bo_acquisition: (is_bo_algorithm && !self.bo_acquisition.is_empty())
                .then(|| self.bo_acquisition.clone()),
            bo_ehvi: (is_bo_algorithm && self.bo_ehvi).then_some(true),
            psychoacoustic: self.psychoacoustic,
            asymmetric_loss: self.asymmetric_loss,
            tolerance: self.tolerance,
            atolerance: self.atolerance,
            allow_delay: Some(self.allow_delay),
            smooth_n: self.smooth_n,
            target_response,
            excursion_protection,
            schroeder_split,
            phase_alignment,
            multi_seat,
            inter_channel_timbre_matching,
            multi_measurement,
            sub_config,
            channel_matching,
            decomposed_correction: Some(DecomposedCorrectionSerdeConfig::default()),
            // Only emit an `epa_config` override when the user actually
            // tweaked the temporal-masking knobs. Otherwise the backend's
            // `EpaConfig::default()` (which also includes
            // `flatness_band_weights`, etc.) is the right baseline.
            epa_config: if self.epa_temporal_masking.differs_from_default() {
                Some(autoeq::loss::epa::score::EpaConfig {
                    temporal_masking: self.epa_temporal_masking.to_backend(),
                    ..autoeq::loss::epa::score::EpaConfig::default()
                })
            } else {
                None
            },
            ..BackendOptimizerConfig::default()
        }
    }

    /// Apply smart defaults based on measurement channel metadata.
    ///
    /// Called after loading measurements to set sensible initial values.
    /// When `imported_from_file` is true, feature toggles are preserved.
    pub fn apply_smart_defaults(&mut self, meta: &ChannelMetadata) {
        // Seed sample rate from playback device when still at default
        if let Some(sr) = meta.playback_sample_rate
            && self.sample_rate == 48000
        {
            self.sample_rate = sr as usize;
        }

        // Loss type is always flat for room EQ
        self.loss_type = "flat".to_string();

        // Only override algorithm/seed defaults when not imported from file
        if !self.imported_from_file {
            self.local_algo = "cobyla".to_string();
            self.refine = true;
            self.seed = None;
        }

        // System type: auto-detect from channel count
        self.system_type = if meta.is_surround() {
            "multichannel".to_string()
        } else {
            "stereo".to_string()
        };

        // Feature flags: only auto-enable when NOT imported from file.
        // When imported, the file's feature state is authoritative
        // (None = disabled, Some = enabled with those params).
        if !self.imported_from_file {
            self.target_response.enabled = true;
            self.target_response.shape = "harman".to_string();
            self.excursion_protection.enabled = true;
            // Schroeder split only makes sense with a subwoofer
            self.schroeder_split.enabled = meta.has_subwoofer();
            self.allow_delay = true;
            self.target_response.broadband_precorrection = true;
            self.vog.enabled = meta.has_height_channels();
            self.vog.reference_channel = if meta.is_home_cinema() {
                "C".to_string()
            } else {
                "L".to_string()
            };
        }
    }
}
