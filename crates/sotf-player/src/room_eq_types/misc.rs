use crate::EQFilter;
pub use autoeq::roomeq::DspChainOutput;
use math_audio_iir_fir::BiquadFilterType;
use std::path::{Path, PathBuf};

/// Return true when a channel name conventionally represents an LFE/sub output.
pub fn room_eq_channel_is_bass_output(name: &str) -> bool {
    let normalized: String = name
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect();
    normalized.contains("lfe")
        || normalized == "sub"
        || normalized == "subwoofer"
        || normalized == "sw"
        || normalized.starts_with("sub")
}

pub(super) fn canonical_multi_measurement_strategy(strategy: &str) -> Option<&'static str> {
    let normalized = strategy
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '-'], "_")
        .replace(['(', ')'], "");
    match normalized.as_str() {
        "average" | "average_rms" => Some("average"),
        "weighted_sum" => Some("weighted_sum"),
        "minimax" | "minmax" | "minimax_worst_case" => Some("minimax"),
        "variance_penalized" | "minimize_variance" | "variance" => Some("variance_penalized"),
        "spatial_robustness" => Some("spatial_robustness"),
        "minimax_uncertainty" | "minimax_bootstrap_uncertainty" => Some("minimax_uncertainty"),
        _ => None,
    }
}

pub(super) fn sanitize_ctc_filename(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            out.push(ch);
        } else if ch.is_whitespace() || matches!(ch, '/' | '\\' | ':' | '(' | ')') {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_').to_string();
    if trimmed.is_empty() {
        "measurement".to_string()
    } else {
        trimmed
    }
}

pub(super) fn resolve_recording_wav_path(path: &str, output_dir: &Path) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_relative() {
        output_dir.join(path)
    } else {
        path
    }
}

/// Estimate the total duration of a probe sequence in milliseconds.
///
/// `num_channels` probes + (`num_channels - 1`) gaps + a ~1 s head/tail
/// budget for device startup and stream settling. Used by the UI to
/// turn `DelayDetectionStatus::Running { started_at_ms }` into a
/// progress estimate.
pub fn estimate_probe_sequence_ms(
    num_channels: usize,
    probe_duration_ms: f32,
    silence_duration_ms: f32,
) -> u64 {
    if num_channels == 0 {
        return 0;
    }
    let per_channel = probe_duration_ms as f64 + silence_duration_ms as f64;
    let total = per_channel * num_channels as f64 + 1_000.0;
    total.round().max(0.0) as u64
}

/// Returns true when a RoomEQ result cannot be represented as a single linear rack.
pub fn requires_room_eq_graph(output: &DspChainOutput) -> bool {
    !output.global_plugins.is_empty()
        || output
            .channels
            .values()
            .any(|chain| chain.drivers.is_some())
        || output
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.bass_management.as_ref())
            .and_then(|report| report.routing_graph.as_ref())
            .is_some_and(|graph| !graph.routes.is_empty())
}

/// Parse EQ filters from JSON array.
///
/// Accepts both autoeq optimizer output format (`"freq"`, `"db_gain"`)
/// and engine format (`"frequency"`, `"gain_db"`).
pub fn parse_eq_filters_from_json(filters_json: &[serde_json::Value]) -> Vec<EQFilter> {
    use sotf_audio::plugins::eq::KautzSectionConfig;

    filters_json
        .iter()
        .map(|filter| {
            let filter_type_str = filter
                .get("filter_type")
                .and_then(|t| t.as_str())
                .unwrap_or("peak");
            let filter_type = match filter_type_str.to_lowercase().as_str() {
                "peak" | "pk" => BiquadFilterType::Peak,
                "lowshelf" | "ls" => BiquadFilterType::Lowshelf,
                "highshelf" | "hs" => BiquadFilterType::Highshelf,
                "lowpass" | "lp" => BiquadFilterType::Lowpass,
                "highpass" | "hp" => BiquadFilterType::Highpass,
                "notch" => BiquadFilterType::Notch,
                _ => BiquadFilterType::Peak,
            };
            let frequency = filter
                .get("frequency")
                .or_else(|| filter.get("freq"))
                .and_then(|f| f.as_f64())
                .unwrap_or(1000.0);
            let q = filter.get("q").and_then(|q| q.as_f64()).unwrap_or(1.0);
            let gain_db = filter
                .get("gain_db")
                .or_else(|| filter.get("db_gain"))
                .and_then(|g| g.as_f64())
                .unwrap_or(0.0);

            // Topology drives whether we read additional warped/Kautz fields
            // out of the JSON. Anything other than `biquad` carries optimizer
            // intent (modal correction, frequency warping) that the engine
            // must preserve end-to-end.
            let topology = filter
                .get("topology")
                .and_then(|t| t.as_str())
                .map(|s| s.to_ascii_lowercase());
            match topology.as_deref() {
                Some("warped_biquad") | Some("warped") => {
                    let lambda = filter.get("lambda").and_then(|v| v.as_f64());
                    EQFilter::new_warped(filter_type, frequency, q, gain_db, lambda)
                }
                Some("kautz_filter") | Some("kautz") => {
                    let sections = filter
                        .get("kautz_sections")
                        .or_else(|| filter.get("sections"))
                        .and_then(|v| {
                            serde_json::from_value::<Vec<KautzSectionConfig>>(v.clone()).ok()
                        })
                        .unwrap_or_default();
                    EQFilter::new_kautz(frequency, q, gain_db, sections)
                }
                _ => EQFilter::new(filter_type, frequency, q, gain_db),
            }
        })
        .collect()
}
