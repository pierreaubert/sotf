use serde_json::Value;

pub(super) fn loudness_data_to_json(info: &sotf_audio::LoudnessData) -> Value {
    serde_json::json!({
        "momentary": info.momentary_lufs,
        "short_term": info.shortterm_lufs,
        "integrated": info.integrated_lufs,
        "peak": info.peak,
        "channel_peaks": info.channel_peaks.as_ref(),
        "true_peaks_dbtp": info.true_peaks_dbtp.as_ref(),
        "correlation_lr": info.correlation_lr,
    })
}

pub(super) fn loudness_info_to_json(info: &sotf_audio::LoudnessInfo) -> Value {
    serde_json::json!({
        "momentary": info.momentary_lufs,
        "short_term": info.shortterm_lufs,
        "integrated": info.integrated_lufs,
        "peak": info.peak,
        "channel_peaks": [],
        "true_peaks_dbtp": [],
        "correlation_lr": null,
    })
}
