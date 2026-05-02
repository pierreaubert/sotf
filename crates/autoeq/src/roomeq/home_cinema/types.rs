use crate::roomeq::types::BassManagementConfig;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum HomeCinemaRole {
    FrontLeft,
    FrontRight,
    Center,
    Lfe,
    SideSurroundLeft,
    SideSurroundRight,
    RearSurroundLeft,
    RearSurroundRight,
    WideLeft,
    WideRight,
    TopFrontLeft,
    TopFrontRight,
    TopMiddleLeft,
    TopMiddleRight,
    TopRearLeft,
    TopRearRight,
    Subwoofer,
    Unknown,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum HomeCinemaRoleGroup {
    FrontLr,
    Center,
    Lfe,
    Subwoofer,
    SideSurrounds,
    RearSurrounds,
    Wides,
    TopFront,
    TopMiddle,
    TopRear,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HomeCinemaChannelReport {
    pub name: String,
    pub role: HomeCinemaRole,
    pub role_group: HomeCinemaRoleGroup,
    pub is_bass_managed: bool,
    pub matching_group: Option<String>,
    pub target_band_hz: (f64, f64),
    pub target_profile: String,
    pub target_advisory: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HomeCinemaLayoutReport {
    pub layout: String,
    pub bed_channels: usize,
    pub lfe_channels: usize,
    pub height_channels: usize,
    pub subwoofer_channels: usize,
    pub channels: Vec<HomeCinemaChannelReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MultiSeatCoverageReport {
    pub channels_with_multiple_measurements: usize,
    pub non_sub_channel_count: usize,
    pub non_sub_channels_with_multiple_measurements: usize,
    pub max_seat_count: usize,
    pub by_role_group: BTreeMap<String, usize>,
    pub all_channel_correction_ready: bool,
    pub recommended_scope: String,
    pub advisories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MultiSeatCorrectionReport {
    pub enabled: bool,
    pub applied: bool,
    pub strategy: String,
    pub seat_count: usize,
    pub primary_seat: usize,
    pub seat_weights: Vec<f64>,
    pub channels: Vec<MultiSeatChannelCorrectionReport>,
    pub role_groups: Vec<MultiSeatRoleGroupCorrectionReport>,
    pub advisories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MultiSeatChannelCorrectionReport {
    pub channel: String,
    pub role: HomeCinemaRole,
    pub role_group: HomeCinemaRoleGroup,
    pub status: String,
    pub seat_count: usize,
    pub target_band_hz: (f64, f64),
    pub rms_target_error_db: Option<f64>,
    pub max_abs_deviation_db: Option<f64>,
    pub primary_pass: Option<bool>,
    pub non_primary_pass: Option<bool>,
    pub spatial_variance_peak_db: Option<f64>,
    pub min_correction_depth: Option<f64>,
    pub seats: Vec<MultiSeatPredictionReport>,
    pub advisories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MultiSeatPredictionReport {
    pub seat_index: usize,
    pub weight: f64,
    pub is_primary: bool,
    pub rms_target_error_db: f64,
    pub max_abs_deviation_db: f64,
    pub pass: bool,
    pub null_risk: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MultiSeatRoleGroupCorrectionReport {
    pub role_group: HomeCinemaRoleGroup,
    pub channel_count: usize,
    pub applied_channel_count: usize,
    pub pass: bool,
    pub worst_rms_target_error_db: Option<f64>,
    pub worst_max_abs_deviation_db: Option<f64>,
    pub advisories: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AllChannelMultiSeatAcceptance {
    pub accepted: bool,
    pub advisories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BassManagementReport {
    pub enabled: bool,
    pub crossover_type: String,
    pub crossover_frequency_hz: Option<f64>,
    pub redirected_bass_enabled: bool,
    pub lfe_channel: String,
    pub lfe_playback_gain_db: f64,
    pub lfe_gain_applied_to_chain: bool,
    pub sub_trim_db: f64,
    pub max_sub_boost_db: f64,
    pub headroom_margin_db: f64,
    pub applied_sub_gain_db: Option<f64>,
    pub gain_limited: bool,
    pub physical_sub_output: String,
    pub redirected_bass_channel_count: usize,
    pub main_high_pass_hz: Option<f64>,
    pub sub_low_pass_hz: Option<f64>,
    pub lfe_headroom_required_db: f64,
    pub signal_flow: Vec<BassManagementSignalFlowEntry>,
    pub signal_flow_advisories: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routing_graph: Option<BassManagementRoutingGraph>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optimization: Option<BassManagementOptimizationReport>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<BassManagementGroupReport>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sub_outputs: Vec<BassManagementSubOutputReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headroom_simulation: Option<BassBusHeadroomSimulationReport>,
    pub advisory: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BassManagementOptimizationReport {
    pub applied: bool,
    pub phase_required: bool,
    pub phase_available: bool,
    pub configured_crossover_hz: Option<f64>,
    pub optimized_crossover_hz: Option<f64>,
    pub crossover_range_hz: Option<(f64, f64)>,
    pub crossover_type: String,
    pub main_delay_ms: f64,
    pub sub_delay_ms: f64,
    pub relative_sub_delay_ms: f64,
    pub sub_polarity_inverted: bool,
    pub requested_sub_gain_db: f64,
    pub applied_sub_gain_db: f64,
    pub gain_limited: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_bass_bus_peak_gain_db: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objective_before: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objective_after: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub group_results: Vec<BassManagementGroupReport>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sub_output_results: Vec<BassManagementSubOutputReport>,
    pub advisories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BassManagementSignalFlowEntry {
    pub source_channel: String,
    pub role: HomeCinemaRole,
    pub destination: String,
    pub high_pass_hz: Option<f64>,
    pub low_pass_hz: Option<f64>,
    pub lfe_gain_db: f64,
    pub redirects_bass: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BassManagementRoutingGraph {
    pub physical_sub_output: String,
    pub input_channels: Vec<String>,
    pub output_channels: Vec<String>,
    pub routes: Vec<BassManagementRoute>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matrix: Option<BassManagementMatrix>,
    pub advisories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BassManagementRoute {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    pub source_channel: String,
    pub source_index: usize,
    pub destination: String,
    pub destination_index: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_chain_channel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_chain_channel: Option<String>,
    pub route_kind: String,
    pub crossover_type: String,
    pub high_pass_hz: Option<f64>,
    pub low_pass_hz: Option<f64>,
    pub gain_db: f64,
    pub gain_linear: f64,
    #[serde(default = "default_route_matrix_gain")]
    pub matrix_gain: f64,
    pub delay_ms: f64,
    pub polarity_inverted: bool,
}

fn default_route_matrix_gain() -> f64 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BassManagementGroupReport {
    pub group_id: String,
    pub roles: Vec<String>,
    pub crossover_type: String,
    pub selected_crossover_hz: Option<f64>,
    pub configured_crossover_hz: Option<f64>,
    pub main_delay_ms: f64,
    pub bass_route_delay_ms: f64,
    pub polarity_inverted: bool,
    pub trim_db: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objective_before: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objective_after: Option<f64>,
    pub advisories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BassManagementSubOutputReport {
    pub output_role: String,
    pub gain_db: f64,
    pub delay_ms: f64,
    pub polarity_inverted: bool,
    pub strategy_source: String,
    pub headroom_contribution_db: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BassBusOutputHeadroomReport {
    pub output_role: String,
    pub rms_bus_gain_db: f64,
    pub coherent_peak_gain_db: f64,
    pub lfe_contribution_db: f64,
    pub pass: bool,
    pub margin_db: f64,
    pub worst_frequency_hz: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BassBusHeadroomSimulationReport {
    pub model: String,
    pub frequency_range_hz: (f64, f64),
    pub rms_bus_gain_db: f64,
    pub coherent_peak_gain_db: f64,
    pub lfe_contribution_db: f64,
    pub headroom_margin_db: f64,
    pub pass: bool,
    pub margin_db: f64,
    pub worst_frequency_hz: f64,
    pub per_output: Vec<BassBusOutputHeadroomReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BassManagementMatrix {
    pub input_channel_map: Vec<usize>,
    pub output_channel_map: Vec<usize>,
    pub matrix: Vec<f32>,
    pub route_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ChannelTimingReport {
    pub name: String,
    pub role: HomeCinemaRole,
    pub measured_arrival_ms: f64,
    pub acoustic_distance_m: f64,
    pub applied_delay_ms: f64,
    pub final_arrival_ms: f64,
    pub final_offset_from_reference_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TimingDiagnosticsReport {
    pub reference_channel: Option<String>,
    pub reference_arrival_ms: Option<f64>,
    pub arrival_spread_before_ms: f64,
    pub arrival_spread_after_ms: f64,
    pub channels: Vec<ChannelTimingReport>,
    pub advisories: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct EffectiveBassManagement {
    pub config: BassManagementConfig,
    pub crossover_type: String,
    pub crossover_frequency_hz: Option<f64>,
    pub advisory: String,
}
