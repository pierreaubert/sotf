use super::room_eq_report_curve::RoomEqReportCurve;

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportIr {
    pub time_ms: Vec<f64>,
    pub amplitude: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportDriverCurve {
    pub driver_name: String,
    pub curve: RoomEqReportCurve,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportFilter {
    pub filter_type: String,
    pub freq: f64,
    pub q: f64,
    pub db_gain: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportEqPass {
    pub label: String,
    pub display_name: String,
    pub color: u32,
    pub filters: Vec<RoomEqReportFilter>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportEpaScore {
    pub preference: f64,
    pub evaluation: f64,
    pub potency: f64,
    pub activity: f64,
    pub sharpness_acum: f64,
    pub roughness: f64,
    pub total_loudness_sone: f64,
    pub loudness_balance: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportEpaComparison {
    pub pre: RoomEqReportEpaScore,
    pub post: RoomEqReportEpaScore,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportBassRoute {
    pub source_channel: String,
    pub destination: String,
    pub route_kind: String,
    pub group_id: Option<String>,
    pub crossover_type: String,
    pub high_pass_hz: Option<f64>,
    pub low_pass_hz: Option<f64>,
    pub gain_db: f64,
    pub matrix_gain: f64,
    pub delay_ms: f64,
    pub polarity_inverted: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportBassGroup {
    pub group_id: String,
    pub roles: Vec<String>,
    pub crossover_type: String,
    pub selected_crossover_hz: Option<f64>,
    pub main_delay_ms: f64,
    pub bass_route_delay_ms: f64,
    pub polarity_inverted: bool,
    pub trim_db: f64,
    pub advisories: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportBassSubOutput {
    pub output_role: String,
    pub strategy_source: String,
    pub gain_db: f64,
    pub delay_ms: f64,
    pub polarity_inverted: bool,
    pub headroom_contribution_db: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportBassHeadroomOutput {
    pub output_role: String,
    pub rms_bus_gain_db: f64,
    pub coherent_peak_gain_db: f64,
    pub lfe_contribution_db: f64,
    pub margin_db: f64,
    pub worst_frequency_hz: f64,
    pub pass: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportBassHeadroom {
    pub model: String,
    pub headroom_margin_db: f64,
    pub pass: bool,
    pub margin_db: f64,
    pub worst_frequency_hz: f64,
    pub per_output: Vec<RoomEqReportBassHeadroomOutput>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportBassManagement {
    pub enabled: bool,
    pub crossover_type: String,
    pub crossover_frequency_hz: Option<f64>,
    pub lfe_playback_gain_db: f64,
    pub applied_sub_gain_db: Option<f64>,
    pub input_channels: Vec<String>,
    pub output_channels: Vec<String>,
    pub physical_outputs: Vec<String>,
    pub route_count: usize,
    pub advisory: String,
    pub advisories: Vec<String>,
    pub routes: Vec<RoomEqReportBassRoute>,
    pub groups: Vec<RoomEqReportBassGroup>,
    pub sub_outputs: Vec<RoomEqReportBassSubOutput>,
    pub headroom: Option<RoomEqReportBassHeadroom>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportChannel {
    pub name: String,
    pub initial_curve: Option<RoomEqReportCurve>,
    pub final_curve: Option<RoomEqReportCurve>,
    pub eq_response: Option<RoomEqReportCurve>,
    pub target_curve: Option<RoomEqReportCurve>,
    pub pre_ir: Option<RoomEqReportIr>,
    pub post_ir: Option<RoomEqReportIr>,
    pub driver_initial_curves: Vec<RoomEqReportDriverCurve>,
    pub eq_passes: Vec<RoomEqReportEqPass>,
    pub epa: Option<RoomEqReportEpaComparison>,
}

/// Aggregate FIR temporal-masking metrics derived from
/// `PerceptualMetrics.fir_*`. Lower (more negative) audible dB values mean
/// less audible ringing; lower penalty means a perceptually safer FIR.
#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportFirMasking {
    pub pre_audible_db: Option<f64>,
    pub post_audible_db: Option<f64>,
    pub penalty: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomEqReportData {
    pub version: String,
    pub pre_score: Option<f64>,
    pub post_score: Option<f64>,
    pub algorithm: Option<String>,
    pub loss_type: Option<String>,
    pub iterations: Option<usize>,
    pub timestamp: Option<String>,
    pub epa_preference_avg: Option<(f64, f64)>,
    pub fir_masking: Option<RoomEqReportFirMasking>,
    pub bass_management: Option<RoomEqReportBassManagement>,
    pub channels: Vec<RoomEqReportChannel>,
}

#[derive(Clone)]
pub(super) struct RoomEqChartSeries {
    pub(super) channel_name: Option<String>,
    pub(super) label: String,
    pub(super) curve: RoomEqReportCurve,
    pub(super) color: u32,
    pub(super) stroke_width: f32,
    pub(super) opacity: f32,
}

#[derive(Clone)]
pub(super) struct RoomEqTrendSeries {
    pub(super) label: String,
    pub(super) freq: Vec<f64>,
    pub(super) spl: Vec<f64>,
    pub(super) color: u32,
}
