use super::diagnostic_deltas::DiagnosticDeltas;
use super::misc::csv_escape;
use super::types::ArtifactMetrics;
use super::types::ChannelMetrics;
use super::types::InputMetrics;
use super::types::IsolationRunResult;
use sotf_plugin_upmixer::UpmixerDiagnostics;
use std::io::Write;

pub(super) fn write_header(writer: &mut dyn Write, channels: usize) -> Result<(), String> {
    write!(
        writer,
        "block,start_frame,time_sec,frames_produced,input_peak,input_rms,output_peak_max,output_rms_sum,output_step_peak,dialogue_probability,dialogue_delta,dialogue_spatial_control,dialogue_spatial_delta,dialogue_centroid_hz,dialogue_envelope_variance,decorrelation_strength,decorrelation_delta,hr_direct_envelope,hr_transient_env,height_transient_env,spectral_flux_smooth,height_spectral_flux_smooth,safety_scale,safety_delta,output_accumulator_fill,height_gain_mean,height_gain_min,height_gain_max,height_gain_stddev,height_gain_mean_delta,height_gate_mean,height_gate_min,height_gate_max,height_gate_stddev,height_gate_mean_delta,coherence_mean,coherence_min,coherence_max,coherence_stddev"
    )
    .map_err(|e| e.to_string())?;
    for ch in 0..channels {
        write!(writer, ",out_peak_ch{ch},out_rms_ch{ch}").map_err(|e| e.to_string())?;
    }
    writeln!(writer).map_err(|e| e.to_string())
}

pub(super) fn write_row(
    writer: &mut dyn Write,
    block: usize,
    start_frame: usize,
    sample_rate: u32,
    frames_produced: usize,
    input: &InputMetrics,
    output: &ChannelMetrics,
    diag: &UpmixerDiagnostics,
    deltas: &DiagnosticDeltas,
) -> Result<(), String> {
    let time_sec = start_frame as f64 / sample_rate as f64;
    write!(
        writer,
        "{block},{start_frame},{time_sec:.9},{frames_produced},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.3},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9}",
        input.peak,
        input.rms,
        output.max_peak,
        output.rms_sum,
        output.step_peak,
        diag.dialogue_probability,
        deltas.dialogue_probability_abs,
        diag.dialogue_spatial_control,
        deltas.dialogue_spatial_control_abs,
        diag.dialogue_spectral_centroid_hz,
        diag.dialogue_envelope_variance,
        diag.decorrelation_strength,
        deltas.decorrelation_abs,
        diag.hr_direct_envelope,
        diag.hr_transient_env,
        diag.height_transient_env,
        diag.spectral_flux_smooth,
        diag.height_spectral_flux_smooth,
        diag.safety_scale,
        deltas.safety_scale_abs,
        diag.output_accumulator_fill,
        diag.height_gain.mean,
        diag.height_gain.min,
        diag.height_gain.max,
        diag.height_gain.stddev,
        deltas.height_gain_mean_abs,
        diag.height_flux_gate.mean,
        diag.height_flux_gate.min,
        diag.height_flux_gate.max,
        diag.height_flux_gate.stddev,
        deltas.height_gate_mean_abs,
        diag.coherence.mean,
        diag.coherence.min,
        diag.coherence.max,
        diag.coherence.stddev,
    )
    .map_err(|e| e.to_string())?;

    for ch in 0..output.peaks.len() {
        write!(writer, ",{:.9},{:.9}", output.peaks[ch], output.rms[ch])
            .map_err(|e| e.to_string())?;
    }
    writeln!(writer).map_err(|e| e.to_string())
}

pub(super) fn write_isolation_summary_header(writer: &mut dyn Write) -> Result<(), String> {
    write_csv_record(
        writer,
        &[
            "variant",
            "config",
            "frequency_resolution",
            "low_latency",
            "notes",
            "output_channels",
            "analysis_frames",
            "frames_produced",
            "block_csv",
            "output_wav",
            "output_peak",
            "output_peak_time_sec",
            "output_peak_channel",
            "output_max_step",
            "output_max_step_time_sec",
            "output_max_step_channel",
            "output_max_step_block",
            "output_boundary_step",
            "output_boundary_step_time_sec",
            "output_boundary_step_channel",
            "output_boundary_step_block",
            "output_hop_step",
            "output_hop_step_time_sec",
            "output_hop_step_channel",
            "output_hop_step_block",
            "output_second_diff",
            "output_second_diff_time_sec",
            "output_second_diff_channel",
            "output_second_diff_rms64",
            "output_second_diff_rms64_time_sec",
            "output_second_diff_rms64_channel",
            "input_peak",
            "input_max_step",
            "input_second_diff",
            "output_to_input_step_ratio",
            "enable_hr_direct",
            "bypass_decorrelation",
            "bypass_transient_detection",
            "bypass_all_processing",
            "height_gain",
            "center_spread",
            "gain_front_ambient",
            "gain_rear_ambient",
            "surround_direct_bleed",
            "max_dialogue_delta",
            "max_dialogue_spatial_delta",
            "max_height_gain_mean_delta",
            "max_height_gate_mean_delta",
            "max_decorrelation_delta",
            "max_safety_delta",
        ],
    )
}

pub(super) fn write_isolation_summary_row(
    writer: &mut dyn Write,
    result: &IsolationRunResult,
    sample_rate: u32,
    analysis_frames: usize,
    input_artifacts: &ArtifactMetrics,
) -> Result<(), String> {
    let artifacts = &result.artifacts;
    let params = &result.variant.params;
    let step_ratio = if input_artifacts.max_step.value > 1e-9 {
        artifacts.max_step.value / input_artifacts.max_step.value
    } else {
        0.0
    };
    write_csv_record(
        writer,
        &[
            result.variant.name.clone(),
            result.variant.config.clone(),
            result.variant.frequency_resolution.clone(),
            params.low_latency.to_string(),
            result.variant.notes.clone(),
            result.output_channels.to_string(),
            analysis_frames.to_string(),
            result.frames_produced.to_string(),
            result.block_csv_path.display().to_string(),
            result
                .wav_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
            format!("{:.9}", artifacts.peak.value),
            format!("{:.9}", artifacts.peak.time_sec(sample_rate)),
            artifacts.peak.channel.to_string(),
            format!("{:.9}", artifacts.max_step.value),
            format!("{:.9}", artifacts.max_step.time_sec(sample_rate)),
            artifacts.max_step.channel.to_string(),
            artifacts.max_step.block.to_string(),
            format!("{:.9}", artifacts.max_boundary_step.value),
            format!("{:.9}", artifacts.max_boundary_step.time_sec(sample_rate)),
            artifacts.max_boundary_step.channel.to_string(),
            artifacts.max_boundary_step.block.to_string(),
            format!("{:.9}", artifacts.max_hop_step.value),
            format!("{:.9}", artifacts.max_hop_step.time_sec(sample_rate)),
            artifacts.max_hop_step.channel.to_string(),
            artifacts.max_hop_step.block.to_string(),
            format!("{:.9}", artifacts.max_second_diff.value),
            format!("{:.9}", artifacts.max_second_diff.time_sec(sample_rate)),
            artifacts.max_second_diff.channel.to_string(),
            format!("{:.9}", artifacts.max_second_diff_rms.value),
            format!("{:.9}", artifacts.max_second_diff_rms.time_sec(sample_rate)),
            artifacts.max_second_diff_rms.channel.to_string(),
            format!("{:.9}", input_artifacts.peak.value),
            format!("{:.9}", input_artifacts.max_step.value),
            format!("{:.9}", input_artifacts.max_second_diff.value),
            format!("{step_ratio:.9}"),
            params.enable_hr_direct.to_string(),
            params.bypass_decorrelation.to_string(),
            params.bypass_transient_detection.to_string(),
            params.bypass_all_processing.to_string(),
            format!("{:.6}", params.height_gain),
            format!("{:.6}", params.center_spread),
            format!("{:.6}", params.gain_front_ambient),
            format!("{:.6}", params.gain_rear_ambient),
            format!("{:.6}", params.surround_direct_bleed),
            format!("{:.9}", result.max_deltas.dialogue_probability_abs),
            format!("{:.9}", result.max_deltas.dialogue_spatial_control_abs),
            format!("{:.9}", result.max_deltas.height_gain_mean_abs),
            format!("{:.9}", result.max_deltas.height_gate_mean_abs),
            format!("{:.9}", result.max_deltas.decorrelation_abs),
            format!("{:.9}", result.max_deltas.safety_scale_abs),
        ],
    )
}

pub(super) fn write_isolation_events_header(writer: &mut dyn Write) -> Result<(), String> {
    write_csv_record(
        writer,
        &[
            "variant", "event", "value", "time_sec", "frame", "channel", "block", "notes",
        ],
    )
}

pub(super) fn write_isolation_event_rows(
    writer: &mut dyn Write,
    result: &IsolationRunResult,
    sample_rate: u32,
) -> Result<(), String> {
    let metrics = &result.artifacts;
    for (event_name, event) in [
        ("peak", metrics.peak),
        ("max_step", metrics.max_step),
        ("max_boundary_step", metrics.max_boundary_step),
        ("max_hop_step", metrics.max_hop_step),
        ("max_second_diff", metrics.max_second_diff),
        ("max_second_diff_rms64", metrics.max_second_diff_rms),
    ] {
        write_csv_record(
            writer,
            &[
                result.variant.name.clone(),
                event_name.to_string(),
                format!("{:.9}", event.value),
                format!("{:.9}", event.time_sec(sample_rate)),
                event.frame.to_string(),
                event.channel.to_string(),
                event.block.to_string(),
                result.variant.notes.clone(),
            ],
        )?;
    }
    Ok(())
}

pub(super) fn write_csv_record(
    writer: &mut dyn Write,
    fields: &[impl AsRef<str>],
) -> Result<(), String> {
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            write!(writer, ",").map_err(|e| e.to_string())?;
        }
        write!(writer, "{}", csv_escape(field.as_ref())).map_err(|e| e.to_string())?;
    }
    writeln!(writer).map_err(|e| e.to_string())
}
