//! Home-cinema role and layout helpers for RoomEQ.
//!
//! This intentionally mirrors the channel-label vocabulary used by
//! `sotf-host` speaker configurations without making `autoeq` depend on the
//! host crate. RoomEQ needs the same semantic model for target bands, channel
//! matching, and multi-seat diagnostics.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

use super::types::{
    BassManagementConfig, CrossoverConfig, RoleTargetConfig, RoomConfig, SpeakerConfig,
    SystemConfig, TargetResponseConfig, TargetShape, UserPreference,
};
use crate::{Curve, MeasurementSource};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
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
    pub non_sub_channels_with_multiple_measurements: usize,
    pub max_seat_count: usize,
    pub by_role_group: BTreeMap<String, usize>,
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
    pub advisory: String,
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

pub fn analyze_layout(config: &RoomConfig) -> HomeCinemaLayoutReport {
    let mut channels = Vec::new();
    let channel_names = logical_channel_names(config);
    for name in channel_names {
        let role = role_for_channel(&name);
        let role_group = role.group();
        channels.push(HomeCinemaChannelReport {
            name: name.clone(),
            role,
            role_group,
            is_bass_managed: role.is_bass_managed_candidate(),
            matching_group: matching_group_key_for_role(role).map(str::to_string),
            target_band_hz: role.default_target_band_hz(),
            target_profile: target_profile_for_role(config, role),
            target_advisory: target_advisory_for_role(config, role),
        });
    }

    let lfe_channels = channels
        .iter()
        .filter(|ch| ch.role == HomeCinemaRole::Lfe)
        .count();
    let subwoofer_channels = channels
        .iter()
        .filter(|ch| ch.role == HomeCinemaRole::Subwoofer)
        .count();
    let height_channels = channels.iter().filter(|ch| ch.role.is_height()).count();
    let bed_channels = channels
        .iter()
        .filter(|ch| ch.role.is_bed_channel())
        .count();
    let layout = detect_layout_name(bed_channels, lfe_channels, height_channels);

    HomeCinemaLayoutReport {
        layout,
        bed_channels,
        lfe_channels,
        height_channels,
        subwoofer_channels,
        channels,
    }
}

pub fn effective_bass_management(config: &RoomConfig) -> Option<EffectiveBassManagement> {
    let system = config.system.as_ref()?;
    let sub_system = system.subwoofers.as_ref()?;
    let bm = system.bass_management.clone().unwrap_or_default();
    if !bm.enabled {
        return None;
    }
    let (crossover_type, crossover_frequency_hz, advisory) =
        resolve_crossover_descriptor(config, sub_system.crossover.as_deref());

    Some(EffectiveBassManagement {
        config: bm,
        crossover_type,
        crossover_frequency_hz,
        advisory,
    })
}

pub fn bass_output_role(_config: &RoomConfig, system: &SystemConfig) -> String {
    if let Some(bm) = system.bass_management.as_ref()
        && system.speakers.contains_key(&bm.lfe_channel)
    {
        return bm.lfe_channel.clone();
    }
    if system.speakers.contains_key("LFE") {
        return "LFE".to_string();
    }
    let mut candidates: Vec<_> = system
        .speakers
        .keys()
        .filter(|role| role_for_channel(role).is_sub_or_lfe())
        .cloned()
        .collect();
    candidates.sort();
    candidates
        .into_iter()
        .next()
        .unwrap_or_else(|| "LFE".to_string())
}

pub fn bass_management_report(
    config: &RoomConfig,
    applied_sub_gain_db: Option<f64>,
    gain_limited: bool,
) -> Option<BassManagementReport> {
    let effective = effective_bass_management(config)?;
    let mut advisory = effective.advisory;
    if gain_limited {
        advisory = if advisory == "ok" {
            "sub_gain_limited_for_headroom".to_string()
        } else {
            format!("{advisory};sub_gain_limited_for_headroom")
        };
    }
    if effective.config.lfe_playback_gain_db.abs() > 0.01
        && !effective.config.apply_lfe_gain_to_chain
    {
        advisory = if advisory == "ok" {
            "lfe_gain_reported_not_applied_to_physical_sub_chain".to_string()
        } else {
            format!("{advisory};lfe_gain_reported_not_applied_to_physical_sub_chain")
        };
    }

    Some(BassManagementReport {
        enabled: true,
        crossover_type: effective.crossover_type,
        crossover_frequency_hz: effective.crossover_frequency_hz,
        redirected_bass_enabled: effective.config.redirect_bass,
        lfe_channel: effective.config.lfe_channel,
        lfe_playback_gain_db: effective.config.lfe_playback_gain_db,
        lfe_gain_applied_to_chain: effective.config.apply_lfe_gain_to_chain,
        sub_trim_db: effective.config.sub_trim_db,
        max_sub_boost_db: effective.config.max_sub_boost_db,
        headroom_margin_db: effective.config.headroom_margin_db,
        applied_sub_gain_db,
        gain_limited,
        advisory,
    })
}

pub fn limited_sub_gain(
    requested_gain_db: f64,
    bass_management: Option<&EffectiveBassManagement>,
) -> (f64, bool) {
    let Some(bm) = bass_management else {
        return (requested_gain_db, false);
    };
    let with_trim = requested_gain_db + bm.config.sub_trim_db;
    let max_boost = bm.config.max_sub_boost_db.max(0.0);
    if with_trim > max_boost {
        (max_boost, true)
    } else {
        (with_trim, false)
    }
}

pub fn multi_seat_coverage(config: &RoomConfig) -> MultiSeatCoverageReport {
    let mut by_role_group: BTreeMap<String, usize> = BTreeMap::new();
    let mut channels_with_multiple_measurements = 0;
    let mut non_sub_channels_with_multiple_measurements = 0;
    let mut max_seat_count = 0;

    for (channel, speaker) in logical_speaker_configs(config) {
        let Some(seat_count) = speaker_measurement_count(&speaker) else {
            continue;
        };
        if seat_count < 2 {
            continue;
        }

        channels_with_multiple_measurements += 1;
        max_seat_count = max_seat_count.max(seat_count);
        let role = role_for_channel(&channel);
        if !role.is_sub_or_lfe() {
            non_sub_channels_with_multiple_measurements += 1;
        }
        *by_role_group
            .entry(role_group_key(role.group()).to_string())
            .or_insert(0) += 1;
    }

    MultiSeatCoverageReport {
        channels_with_multiple_measurements,
        non_sub_channels_with_multiple_measurements,
        max_seat_count,
        by_role_group,
    }
}

pub fn role_for_channel(channel_name: &str) -> HomeCinemaRole {
    let normalized = normalize_channel_name(channel_name);
    match normalized.as_str() {
        "l" | "fl" | "left" | "frontleft" => HomeCinemaRole::FrontLeft,
        "r" | "fr" | "right" | "frontright" => HomeCinemaRole::FrontRight,
        "c" | "center" | "centre" => HomeCinemaRole::Center,
        "lfe" | "lf" => HomeCinemaRole::Lfe,
        "sub" | "subwoofer" | "sw" | "sub1" | "sub2" => HomeCinemaRole::Subwoofer,
        "sl" | "ls" | "surroundleft" | "sideleft" => HomeCinemaRole::SideSurroundLeft,
        "sr" | "rs" | "surroundright" | "sideright" => HomeCinemaRole::SideSurroundRight,
        "bl" | "rl" | "sbl" | "rearleft" | "backleft" | "surroundbackleft" => {
            HomeCinemaRole::RearSurroundLeft
        }
        "br" | "rr" | "sbr" | "rearright" | "backright" | "surroundbackright" => {
            HomeCinemaRole::RearSurroundRight
        }
        "wl" | "wideleft" | "frontwideleft" => HomeCinemaRole::WideLeft,
        "wr" | "wideright" | "frontwideright" => HomeCinemaRole::WideRight,
        "tfl" | "fhl" | "topfrontleft" | "frontheightleft" => HomeCinemaRole::TopFrontLeft,
        "tfr" | "fhr" | "topfrontright" | "frontheightright" => HomeCinemaRole::TopFrontRight,
        "tml" | "topmiddleleft" => HomeCinemaRole::TopMiddleLeft,
        "tmr" | "topmiddleright" => HomeCinemaRole::TopMiddleRight,
        "tbl" | "trl" | "rhl" | "topbackleft" | "toprearleft" | "rearheightleft" => {
            HomeCinemaRole::TopRearLeft
        }
        "tbr" | "trr" | "rhr" | "topbackright" | "toprearright" | "rearheightright" => {
            HomeCinemaRole::TopRearRight
        }
        _ if normalized.contains("sub") => HomeCinemaRole::Subwoofer,
        _ if normalized.contains("lfe") => HomeCinemaRole::Lfe,
        _ => HomeCinemaRole::Unknown,
    }
}

pub fn matching_group_key(channel_name: &str) -> Option<&'static str> {
    matching_group_key_for_role(role_for_channel(channel_name))
}

pub fn role_score_band(config: &RoomConfig, channel_name: &str) -> (f64, f64) {
    let role = role_for_channel(channel_name);
    let (role_min, role_max) = role.default_target_band_hz();
    let min = config.optimizer.min_freq.max(role_min);
    let max = config.optimizer.max_freq.min(role_max).max(min);
    (min, max)
}

pub fn role_adjusted_target_response(
    channel_name: &str,
    base: &TargetResponseConfig,
) -> TargetResponseConfig {
    let Some(role_targets) = base.role_targets.as_ref().filter(|cfg| cfg.enabled) else {
        return base.clone();
    };

    let mut adjusted = base.clone();
    apply_role_target_adjustment(role_for_channel(channel_name), role_targets, &mut adjusted);
    adjusted
}

pub fn apply_role_target_curve_shape(
    channel_name: &str,
    target_curve: &mut Curve,
    target: &TargetResponseConfig,
) {
    let Some(role_targets) = target.role_targets.as_ref().filter(|cfg| cfg.enabled) else {
        return;
    };
    let role = role_for_channel(channel_name);

    if role == HomeCinemaRole::Center && role_targets.center_dialog_boost_db.abs() > 0.001 {
        apply_log_band_emphasis(
            target_curve,
            role_targets.center_dialog_low_hz,
            role_targets.center_dialog_high_hz,
            role_targets.center_dialog_boost_db,
        );
    }

    if role_targets.cinema_x_curve_enabled
        && role_targets.cinema_x_curve_db_per_octave.abs() > 0.001
    {
        apply_high_frequency_slope(
            target_curve,
            role_targets.cinema_x_curve_start_hz,
            role_targets.cinema_x_curve_db_per_octave,
        );
    }

    if let Some(distance_m) = role_targets.listening_distance_m {
        let ref_m = role_targets.cinema_reference_distance_m;
        if distance_m > ref_m
            && ref_m > 0.0
            && role_targets.distance_treble_rolloff_db_per_doubling.abs() > 0.001
        {
            let distance_doublings = (distance_m / ref_m).log2();
            apply_high_frequency_slope(
                target_curve,
                role_targets.cinema_x_curve_start_hz,
                -role_targets.distance_treble_rolloff_db_per_doubling.abs() * distance_doublings,
            );
        }
    }
}

pub fn role_target_curve_shape_active(channel_name: &str, target: &TargetResponseConfig) -> bool {
    let Some(role_targets) = target.role_targets.as_ref().filter(|cfg| cfg.enabled) else {
        return false;
    };
    let role = role_for_channel(channel_name);
    (role == HomeCinemaRole::Center && role_targets.center_dialog_boost_db.abs() > 0.001)
        || (role_targets.cinema_x_curve_enabled
            && role_targets.cinema_x_curve_db_per_octave.abs() > 0.001)
        || (role_targets.listening_distance_m.is_some()
            && role_targets.distance_treble_rolloff_db_per_doubling.abs() > 0.001)
}

impl HomeCinemaRole {
    pub fn group(self) -> HomeCinemaRoleGroup {
        match self {
            HomeCinemaRole::FrontLeft | HomeCinemaRole::FrontRight => HomeCinemaRoleGroup::FrontLr,
            HomeCinemaRole::Center => HomeCinemaRoleGroup::Center,
            HomeCinemaRole::Lfe => HomeCinemaRoleGroup::Lfe,
            HomeCinemaRole::Subwoofer => HomeCinemaRoleGroup::Subwoofer,
            HomeCinemaRole::SideSurroundLeft | HomeCinemaRole::SideSurroundRight => {
                HomeCinemaRoleGroup::SideSurrounds
            }
            HomeCinemaRole::RearSurroundLeft | HomeCinemaRole::RearSurroundRight => {
                HomeCinemaRoleGroup::RearSurrounds
            }
            HomeCinemaRole::WideLeft | HomeCinemaRole::WideRight => HomeCinemaRoleGroup::Wides,
            HomeCinemaRole::TopFrontLeft | HomeCinemaRole::TopFrontRight => {
                HomeCinemaRoleGroup::TopFront
            }
            HomeCinemaRole::TopMiddleLeft | HomeCinemaRole::TopMiddleRight => {
                HomeCinemaRoleGroup::TopMiddle
            }
            HomeCinemaRole::TopRearLeft | HomeCinemaRole::TopRearRight => {
                HomeCinemaRoleGroup::TopRear
            }
            HomeCinemaRole::Unknown => HomeCinemaRoleGroup::Unknown,
        }
    }

    pub fn is_height(self) -> bool {
        matches!(
            self,
            HomeCinemaRole::TopFrontLeft
                | HomeCinemaRole::TopFrontRight
                | HomeCinemaRole::TopMiddleLeft
                | HomeCinemaRole::TopMiddleRight
                | HomeCinemaRole::TopRearLeft
                | HomeCinemaRole::TopRearRight
        )
    }

    pub fn is_sub_or_lfe(self) -> bool {
        matches!(self, HomeCinemaRole::Subwoofer | HomeCinemaRole::Lfe)
    }

    pub fn is_bed_channel(self) -> bool {
        !self.is_height() && !self.is_sub_or_lfe() && self != HomeCinemaRole::Unknown
    }

    pub fn is_bass_managed_candidate(self) -> bool {
        self.is_bed_channel() || self.is_height()
    }

    pub fn default_target_band_hz(self) -> (f64, f64) {
        match self {
            HomeCinemaRole::Lfe | HomeCinemaRole::Subwoofer => (20.0, 160.0),
            HomeCinemaRole::Center => (80.0, 16_000.0),
            HomeCinemaRole::SideSurroundLeft
            | HomeCinemaRole::SideSurroundRight
            | HomeCinemaRole::RearSurroundLeft
            | HomeCinemaRole::RearSurroundRight
            | HomeCinemaRole::WideLeft
            | HomeCinemaRole::WideRight => (80.0, 12_000.0),
            role if role.is_height() => (120.0, 10_000.0),
            HomeCinemaRole::Unknown => (20.0, 20_000.0),
            _ => (40.0, 18_000.0),
        }
    }
}

fn apply_role_target_adjustment(
    role: HomeCinemaRole,
    role_targets: &RoleTargetConfig,
    target: &mut TargetResponseConfig,
) {
    let slope_offset = role_slope_offset(role, role_targets);
    if slope_offset.abs() > 0.001 {
        add_slope_offset(target, slope_offset);
    }

    match role {
        HomeCinemaRole::Center => {
            target.preference.treble_shelf_db += role_targets.center_treble_shelf_db;
        }
        HomeCinemaRole::SideSurroundLeft
        | HomeCinemaRole::SideSurroundRight
        | HomeCinemaRole::RearSurroundLeft
        | HomeCinemaRole::RearSurroundRight
        | HomeCinemaRole::WideLeft
        | HomeCinemaRole::WideRight => {
            target.preference.treble_shelf_db += role_targets.surround_treble_shelf_db;
        }
        role if role.is_height() => {
            target.preference.treble_shelf_db += role_targets.height_treble_shelf_db;
        }
        HomeCinemaRole::Lfe => {
            target.preference.bass_shelf_db += role_targets.lfe_bass_shelf_db;
        }
        HomeCinemaRole::Subwoofer => {
            target.preference.bass_shelf_db += role_targets.subwoofer_bass_shelf_db;
        }
        _ => {}
    }

    if target.preference.treble_shelf_freq <= 0.0 {
        target.preference.treble_shelf_freq = UserPreference::default().treble_shelf_freq;
    }
    if target.preference.bass_shelf_freq <= 0.0 {
        target.preference.bass_shelf_freq = UserPreference::default().bass_shelf_freq;
    }
}

fn role_slope_offset(role: HomeCinemaRole, role_targets: &RoleTargetConfig) -> f64 {
    match role {
        HomeCinemaRole::FrontLeft | HomeCinemaRole::FrontRight => {
            role_targets.front_slope_offset_db_per_octave
        }
        HomeCinemaRole::Center => role_targets.center_slope_offset_db_per_octave,
        HomeCinemaRole::SideSurroundLeft
        | HomeCinemaRole::SideSurroundRight
        | HomeCinemaRole::RearSurroundLeft
        | HomeCinemaRole::RearSurroundRight
        | HomeCinemaRole::WideLeft
        | HomeCinemaRole::WideRight => role_targets.surround_slope_offset_db_per_octave,
        HomeCinemaRole::TopFrontLeft
        | HomeCinemaRole::TopFrontRight
        | HomeCinemaRole::TopMiddleLeft
        | HomeCinemaRole::TopMiddleRight
        | HomeCinemaRole::TopRearLeft
        | HomeCinemaRole::TopRearRight => role_targets.height_slope_offset_db_per_octave,
        HomeCinemaRole::Subwoofer => role_targets.subwoofer_slope_offset_db_per_octave,
        HomeCinemaRole::Lfe => role_targets.lfe_slope_offset_db_per_octave,
        HomeCinemaRole::Unknown => 0.0,
    }
}

fn add_slope_offset(target: &mut TargetResponseConfig, slope_offset_db_per_octave: f64) {
    let base_slope = match target.shape {
        TargetShape::Flat => 0.0,
        TargetShape::Harman => -0.8,
        TargetShape::Custom => target.slope_db_per_octave,
        TargetShape::File | TargetShape::FromMeasurement => target.slope_db_per_octave,
    };
    target.shape = TargetShape::Custom;
    target.slope_db_per_octave = base_slope + slope_offset_db_per_octave;
}

fn apply_log_band_emphasis(target_curve: &mut Curve, low_hz: f64, high_hz: f64, gain_db: f64) {
    if !(low_hz > 0.0 && high_hz > low_hz) {
        return;
    }
    let center_hz = (low_hz * high_hz).sqrt();
    let half_width_oct = (high_hz / low_hz).log2() / 2.0;
    if half_width_oct <= 0.0 {
        return;
    }

    for (freq, spl) in target_curve.freq.iter().zip(target_curve.spl.iter_mut()) {
        let distance_oct = (*freq / center_hz).max(1e-9).log2().abs();
        if distance_oct <= half_width_oct {
            let weight = 0.5 * (1.0 + (std::f64::consts::PI * distance_oct / half_width_oct).cos());
            *spl += gain_db * weight;
        }
    }
}

fn apply_high_frequency_slope(target_curve: &mut Curve, start_hz: f64, slope_db_per_octave: f64) {
    if start_hz <= 0.0 {
        return;
    }
    for (freq, spl) in target_curve.freq.iter().zip(target_curve.spl.iter_mut()) {
        if *freq > start_hz {
            *spl += slope_db_per_octave * (*freq / start_hz).log2();
        }
    }
}

fn target_profile_for_role(config: &RoomConfig, role: HomeCinemaRole) -> String {
    let enabled = config
        .optimizer
        .target_response
        .as_ref()
        .and_then(|target| target.role_targets.as_ref())
        .is_some_and(|role_targets| role_targets.enabled);
    let suffix = if enabled { "_role_target" } else { "_default" };
    format!("{}{}", role_profile_base(role), suffix)
}

fn target_advisory_for_role(config: &RoomConfig, role: HomeCinemaRole) -> Option<String> {
    let role_targets = config
        .optimizer
        .target_response
        .as_ref()
        .and_then(|target| target.role_targets.as_ref())
        .filter(|role_targets| role_targets.enabled)?;
    let mut advisories = Vec::new();
    if role_slope_offset(role, role_targets).abs() > 0.001 {
        advisories.push("role_slope_offset");
    }
    if role == HomeCinemaRole::Center && role_targets.center_dialog_boost_db.abs() > 0.001 {
        advisories.push("center_dialog_band");
    }
    if role_targets.cinema_x_curve_enabled
        && role_targets.cinema_x_curve_db_per_octave.abs() > 0.001
    {
        advisories.push("cinema_x_curve");
    }
    if role_targets.listening_distance_m.is_some()
        && role_targets.distance_treble_rolloff_db_per_doubling.abs() > 0.001
    {
        advisories.push("distance_treble_rolloff");
    }
    if advisories.is_empty() {
        Some("role_targets_enabled_neutral".to_string())
    } else {
        Some(advisories.join(";"))
    }
}

fn role_profile_base(role: HomeCinemaRole) -> &'static str {
    match role {
        HomeCinemaRole::FrontLeft | HomeCinemaRole::FrontRight => "front_lr",
        HomeCinemaRole::Center => "center_dialog",
        HomeCinemaRole::Lfe => "lfe",
        HomeCinemaRole::Subwoofer => "subwoofer",
        HomeCinemaRole::SideSurroundLeft
        | HomeCinemaRole::SideSurroundRight
        | HomeCinemaRole::RearSurroundLeft
        | HomeCinemaRole::RearSurroundRight
        | HomeCinemaRole::WideLeft
        | HomeCinemaRole::WideRight => "surround",
        HomeCinemaRole::TopFrontLeft
        | HomeCinemaRole::TopFrontRight
        | HomeCinemaRole::TopMiddleLeft
        | HomeCinemaRole::TopMiddleRight
        | HomeCinemaRole::TopRearLeft
        | HomeCinemaRole::TopRearRight => "height",
        HomeCinemaRole::Unknown => "generic",
    }
}

fn matching_group_key_for_role(role: HomeCinemaRole) -> Option<&'static str> {
    match role.group() {
        HomeCinemaRoleGroup::FrontLr => Some("front_lr"),
        HomeCinemaRoleGroup::SideSurrounds => Some("side_surrounds"),
        HomeCinemaRoleGroup::RearSurrounds => Some("rear_surrounds"),
        HomeCinemaRoleGroup::Wides => Some("wides"),
        HomeCinemaRoleGroup::TopFront => Some("top_front"),
        HomeCinemaRoleGroup::TopMiddle => Some("top_middle"),
        HomeCinemaRoleGroup::TopRear => Some("top_rear"),
        HomeCinemaRoleGroup::Unknown => Some("generic"),
        HomeCinemaRoleGroup::Center | HomeCinemaRoleGroup::Lfe | HomeCinemaRoleGroup::Subwoofer => {
            None
        }
    }
}

fn role_group_key(group: HomeCinemaRoleGroup) -> &'static str {
    match group {
        HomeCinemaRoleGroup::FrontLr => "front_lr",
        HomeCinemaRoleGroup::Center => "center",
        HomeCinemaRoleGroup::Lfe => "lfe",
        HomeCinemaRoleGroup::Subwoofer => "subwoofer",
        HomeCinemaRoleGroup::SideSurrounds => "side_surrounds",
        HomeCinemaRoleGroup::RearSurrounds => "rear_surrounds",
        HomeCinemaRoleGroup::Wides => "wides",
        HomeCinemaRoleGroup::TopFront => "top_front",
        HomeCinemaRoleGroup::TopMiddle => "top_middle",
        HomeCinemaRoleGroup::TopRear => "top_rear",
        HomeCinemaRoleGroup::Unknown => "unknown",
    }
}

fn detect_layout_name(bed_channels: usize, lfe_channels: usize, height_channels: usize) -> String {
    if height_channels > 0 {
        format!("{bed_channels}.{lfe_channels}.{height_channels}")
    } else {
        format!("{bed_channels}.{lfe_channels}")
    }
}

fn resolve_crossover_descriptor(
    config: &RoomConfig,
    crossover_key: Option<&str>,
) -> (String, Option<f64>, String) {
    let Some(key) = crossover_key else {
        return (
            "unknown".to_string(),
            None,
            "missing_subwoofer_crossover".to_string(),
        );
    };
    let Some(crossover) = config.crossovers.as_ref().and_then(|xos| xos.get(key)) else {
        return (
            "unknown".to_string(),
            None,
            format!("missing_crossover_config:{key}"),
        );
    };
    let frequency = crossover.frequency.or_else(|| {
        crossover
            .frequency_range
            .map(|(min, max)| (min * max).sqrt())
    });
    (
        crossover.crossover_type.clone(),
        frequency,
        crossover_advisory(crossover),
    )
}

fn crossover_advisory(crossover: &CrossoverConfig) -> String {
    if crossover.frequency.is_none() && crossover.frequency_range.is_none() {
        "missing_crossover_frequency".to_string()
    } else {
        "ok".to_string()
    }
}

fn logical_channel_names(config: &RoomConfig) -> Vec<String> {
    if let Some(system) = config.system.as_ref() {
        let mut pairs: Vec<_> = system.speakers.keys().cloned().collect();
        pairs.sort();
        pairs
    } else if let Some(recording) = config.recording_config.as_ref()
        && let Some(names) = recording.channel_names.as_ref()
        && !names.is_empty()
    {
        names.clone()
    } else {
        let mut names: Vec<_> = config.speakers.keys().cloned().collect();
        names.sort();
        names
    }
}

fn logical_speaker_configs(config: &RoomConfig) -> HashMap<String, SpeakerConfig> {
    if let Some(system) = config.system.as_ref() {
        system
            .speakers
            .iter()
            .filter_map(|(role, key)| {
                config
                    .speakers
                    .get(key)
                    .cloned()
                    .map(|speaker| (role.clone(), speaker))
            })
            .collect()
    } else {
        config.speakers.clone()
    }
}

fn speaker_measurement_count(speaker: &SpeakerConfig) -> Option<usize> {
    match speaker {
        SpeakerConfig::Single(source) => measurement_source_count(source),
        SpeakerConfig::Group(group) => group
            .measurements
            .iter()
            .filter_map(measurement_source_count)
            .max(),
        SpeakerConfig::MultiSub(group) => group
            .subwoofers
            .iter()
            .filter_map(measurement_source_count)
            .max(),
        SpeakerConfig::Dba(config) => config
            .front
            .iter()
            .chain(config.rear.iter())
            .filter_map(measurement_source_count)
            .max(),
        SpeakerConfig::Cardioid(config) => [measurement_source_count(&config.front)]
            .into_iter()
            .chain([measurement_source_count(&config.rear)])
            .flatten()
            .max(),
    }
}

fn measurement_source_count(source: &MeasurementSource) -> Option<usize> {
    match source {
        MeasurementSource::Single(_) | MeasurementSource::InMemory(_) => Some(1),
        MeasurementSource::Multiple(m) => Some(m.measurements.len()),
        MeasurementSource::InMemoryMultiple(curves) => Some(curves.len()),
    }
}

fn normalize_channel_name(channel_name: &str) -> String {
    channel_name
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| !matches!(ch, ' ' | '-' | '_' | '.'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roomeq::types::{
        BassManagementConfig, CrossoverConfig, OptimizerConfig, RoomConfig, SpeakerConfig,
        SubwooferStrategy, SubwooferSystemConfig, SystemConfig, SystemModel,
    };
    use crate::{Curve, MeasurementSource};
    use ndarray::Array1;

    fn flat_curve() -> Curve {
        Curve {
            freq: Array1::from_vec(vec![20.0, 100.0, 1000.0]),
            spl: Array1::from_vec(vec![80.0, 80.0, 80.0]),
            phase: Some(Array1::from_vec(vec![0.0, 0.0, 0.0])),
            ..Default::default()
        }
    }

    #[test]
    fn detects_immersive_layout_from_standard_roles() {
        let mut speakers = HashMap::new();
        for name in ["L", "R", "C", "LFE", "SL", "SR", "TFL", "TFR"] {
            speakers.insert(
                name.to_string(),
                SpeakerConfig::Single(MeasurementSource::InMemory(flat_curve())),
            );
        }
        let config = RoomConfig {
            version: "test".to_string(),
            system: None,
            speakers,
            crossovers: None,
            target_curve: None,
            optimizer: OptimizerConfig::default(),
            recording_config: None,
            cea2034_cache: None,
        };

        let layout = analyze_layout(&config);
        assert_eq!(layout.layout, "5.1.2");
        assert_eq!(layout.height_channels, 2);
        assert!(
            layout
                .channels
                .iter()
                .any(|ch| ch.role == HomeCinemaRole::Center)
        );
    }

    #[test]
    fn system_roles_override_measurement_keys() {
        let mut speakers = HashMap::new();
        speakers.insert(
            "mic_file_1".to_string(),
            SpeakerConfig::Single(MeasurementSource::InMemory(flat_curve())),
        );
        speakers.insert(
            "mic_file_2".to_string(),
            SpeakerConfig::Single(MeasurementSource::InMemory(flat_curve())),
        );
        let system = SystemConfig {
            model: Default::default(),
            speakers: HashMap::from([
                ("L".to_string(), "mic_file_1".to_string()),
                ("R".to_string(), "mic_file_2".to_string()),
            ]),
            subwoofers: None,
            bass_management: None,
        };
        let config = RoomConfig {
            version: "test".to_string(),
            system: Some(system),
            speakers,
            crossovers: None,
            target_curve: None,
            optimizer: OptimizerConfig::default(),
            recording_config: None,
            cea2034_cache: None,
        };

        let layout = analyze_layout(&config);
        assert_eq!(layout.layout, "2.0");
        assert!(
            layout
                .channels
                .iter()
                .all(|ch| ch.matching_group.as_deref() == Some("front_lr"))
        );
    }

    #[test]
    fn role_targets_adjust_only_matching_roles() {
        let base = TargetResponseConfig {
            role_targets: Some(RoleTargetConfig {
                enabled: true,
                height_treble_shelf_db: -2.0,
                ..Default::default()
            }),
            ..Default::default()
        };

        let height = role_adjusted_target_response("TFL", &base);
        let front = role_adjusted_target_response("L", &base);

        assert_eq!(height.preference.treble_shelf_db, -2.0);
        assert_eq!(front.preference.treble_shelf_db, 0.0);
    }

    #[test]
    fn role_targets_apply_role_specific_slope_offsets() {
        let base = TargetResponseConfig {
            shape: super::super::types::TargetShape::Harman,
            role_targets: Some(RoleTargetConfig {
                enabled: true,
                center_slope_offset_db_per_octave: -0.4,
                height_slope_offset_db_per_octave: -1.0,
                ..Default::default()
            }),
            ..Default::default()
        };

        let center = role_adjusted_target_response("C", &base);
        let height = role_adjusted_target_response("TFL", &base);
        let front = role_adjusted_target_response("L", &base);

        assert_eq!(center.shape, super::super::types::TargetShape::Custom);
        assert!((center.slope_db_per_octave - (-1.2)).abs() < 1e-9);
        assert!((height.slope_db_per_octave - (-1.8)).abs() < 1e-9);
        assert_eq!(front.shape, super::super::types::TargetShape::Harman);
    }

    #[test]
    fn role_target_curve_shape_boosts_center_dialog_band() {
        let mut target_curve = Curve {
            freq: Array1::from_vec(vec![100.0, 1000.0, 8000.0]),
            spl: Array1::zeros(3),
            phase: None,
            ..Default::default()
        };
        let target = TargetResponseConfig {
            role_targets: Some(RoleTargetConfig {
                enabled: true,
                center_dialog_boost_db: 2.0,
                ..Default::default()
            }),
            ..Default::default()
        };

        apply_role_target_curve_shape("C", &mut target_curve, &target);

        assert!(target_curve.spl[1] > 1.0);
        assert!(target_curve.spl[1] > target_curve.spl[0]);
        assert!(target_curve.spl[1] > target_curve.spl[2]);
    }

    #[test]
    fn role_target_curve_shape_applies_cinema_distance_rolloff() {
        let mut target_curve = Curve {
            freq: Array1::from_vec(vec![1000.0, 4000.0, 8000.0]),
            spl: Array1::zeros(3),
            phase: None,
            ..Default::default()
        };
        let target = TargetResponseConfig {
            role_targets: Some(RoleTargetConfig {
                enabled: true,
                cinema_x_curve_enabled: true,
                cinema_x_curve_db_per_octave: -0.5,
                listening_distance_m: Some(6.0),
                distance_treble_rolloff_db_per_doubling: 1.0,
                ..Default::default()
            }),
            ..Default::default()
        };

        apply_role_target_curve_shape("SL", &mut target_curve, &target);

        assert_eq!(target_curve.spl[0], 0.0);
        assert!(target_curve.spl[2] < target_curve.spl[1]);
        assert!(target_curve.spl[2] < -2.0);
    }

    #[test]
    fn layout_metadata_reports_role_target_profile() {
        let mut speakers = HashMap::new();
        speakers.insert(
            "C".to_string(),
            SpeakerConfig::Single(MeasurementSource::InMemory(flat_curve())),
        );
        let config = RoomConfig {
            version: "test".to_string(),
            system: None,
            speakers,
            crossovers: None,
            target_curve: None,
            optimizer: OptimizerConfig {
                target_response: Some(TargetResponseConfig {
                    role_targets: Some(RoleTargetConfig {
                        enabled: true,
                        center_dialog_boost_db: 1.5,
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
            recording_config: None,
            cea2034_cache: None,
        };

        let layout = analyze_layout(&config);
        let center = layout
            .channels
            .iter()
            .find(|channel| channel.role == HomeCinemaRole::Center)
            .unwrap();
        assert_eq!(center.target_profile, "center_dialog_role_target");
        assert_eq!(
            center.target_advisory.as_deref(),
            Some("center_dialog_band")
        );
    }

    #[test]
    fn reports_non_sub_multiseat_coverage() {
        let mut speakers = HashMap::new();
        speakers.insert(
            "L".to_string(),
            SpeakerConfig::Single(MeasurementSource::InMemoryMultiple(vec![
                flat_curve(),
                flat_curve(),
            ])),
        );
        speakers.insert(
            "Sub".to_string(),
            SpeakerConfig::Single(MeasurementSource::InMemoryMultiple(vec![
                flat_curve(),
                flat_curve(),
                flat_curve(),
            ])),
        );
        let config = RoomConfig {
            version: "test".to_string(),
            system: None,
            speakers,
            crossovers: None,
            target_curve: None,
            optimizer: OptimizerConfig::default(),
            recording_config: None,
            cea2034_cache: None,
        };

        let report = multi_seat_coverage(&config);
        assert_eq!(report.channels_with_multiple_measurements, 2);
        assert_eq!(report.non_sub_channels_with_multiple_measurements, 1);
        assert_eq!(report.max_seat_count, 3);
    }

    #[test]
    fn bass_management_reports_lfe_and_limits_sub_gain() {
        let config = RoomConfig {
            version: "test".to_string(),
            system: Some(SystemConfig {
                model: SystemModel::HomeCinema,
                speakers: HashMap::from([
                    ("L".to_string(), "L".to_string()),
                    ("R".to_string(), "R".to_string()),
                    ("LFE".to_string(), "Sub".to_string()),
                ]),
                subwoofers: Some(SubwooferSystemConfig {
                    config: SubwooferStrategy::Single,
                    crossover: Some("xo".to_string()),
                    mapping: HashMap::new(),
                }),
                bass_management: Some(BassManagementConfig {
                    max_sub_boost_db: 3.0,
                    sub_trim_db: 1.0,
                    ..Default::default()
                }),
            }),
            speakers: HashMap::from([
                (
                    "L".to_string(),
                    SpeakerConfig::Single(MeasurementSource::InMemory(flat_curve())),
                ),
                (
                    "R".to_string(),
                    SpeakerConfig::Single(MeasurementSource::InMemory(flat_curve())),
                ),
                (
                    "Sub".to_string(),
                    SpeakerConfig::Single(MeasurementSource::InMemory(flat_curve())),
                ),
            ]),
            crossovers: Some(HashMap::from([(
                "xo".to_string(),
                CrossoverConfig {
                    crossover_type: "LR24".to_string(),
                    frequency: Some(80.0),
                    frequencies: None,
                    frequency_range: None,
                },
            )])),
            target_curve: None,
            optimizer: OptimizerConfig::default(),
            recording_config: None,
            cea2034_cache: None,
        };

        let effective = effective_bass_management(&config).expect("bass management");
        let (gain, limited) = limited_sub_gain(5.0, Some(&effective));
        let report = bass_management_report(&config, Some(gain), limited).expect("report");

        assert_eq!(gain, 3.0);
        assert!(limited);
        assert_eq!(report.crossover_type, "LR24");
        assert_eq!(report.crossover_frequency_hz, Some(80.0));
        assert_eq!(report.lfe_playback_gain_db, 10.0);
        assert!(report.advisory.contains("sub_gain_limited_for_headroom"));
        assert!(
            report
                .advisory
                .contains("lfe_gain_reported_not_applied_to_physical_sub_chain")
        );
    }

    #[test]
    fn bass_output_role_uses_configured_lfe_channel() {
        let system = SystemConfig {
            model: SystemModel::HomeCinema,
            speakers: HashMap::from([
                ("L".to_string(), "L".to_string()),
                ("R".to_string(), "R".to_string()),
                ("Sub".to_string(), "Sub".to_string()),
            ]),
            subwoofers: Some(SubwooferSystemConfig {
                config: SubwooferStrategy::Single,
                crossover: Some("xo".to_string()),
                mapping: HashMap::new(),
            }),
            bass_management: Some(BassManagementConfig {
                lfe_channel: "Sub".to_string(),
                ..Default::default()
            }),
        };
        let config = RoomConfig {
            version: "test".to_string(),
            system: Some(system.clone()),
            speakers: HashMap::new(),
            crossovers: None,
            target_curve: None,
            optimizer: OptimizerConfig::default(),
            recording_config: None,
            cea2034_cache: None,
        };

        assert_eq!(bass_output_role(&config, &system), "Sub");
    }
}
