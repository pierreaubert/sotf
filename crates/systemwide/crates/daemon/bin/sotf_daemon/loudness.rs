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
        "measurement_valid": info.measurement_valid,
        "measurement_enabled": info.measurement_enabled,
        "query_error_generation": info.query_error_generation,
        "channel_layout_is_compliant": info.channel_layout_is_compliant,
        "true_peak_is_compliant": info.true_peak_is_compliant,
        "integrated_window_seconds": info.integrated_window_seconds,
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
        "measurement_valid": false,
        "measurement_enabled": true,
        "query_error_generation": 0,
        "channel_layout_is_compliant": false,
        "true_peak_is_compliant": false,
        "integrated_window_seconds": 3600,
    })
}
