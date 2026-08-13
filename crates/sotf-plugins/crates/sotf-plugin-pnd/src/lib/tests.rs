use super::consts::RESAMPLER_CHUNK_SIZE;
use super::pnd_plugin::PndPlugin;
use super::pnd_plugin::smooth_drift_ratio;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_host::{CountingAlloc, assert_no_allocs};

#[global_allocator]
static ALLOCATOR: CountingAlloc = CountingAlloc;

#[test]
fn drift_smoothing_parameter_is_monotonic_and_high_values_are_slow() {
    let current = 1.0;
    let target = 1.1;
    let fast = smooth_drift_ratio(current, target, 0.1);
    let slow = smooth_drift_ratio(current, target, 0.9);

    assert!(fast > slow, "lower smoothing should track faster");
    assert!(slow > current, "a finite target should still make progress");
    assert!(slow < target, "smoothing should not jump to the target");
}

#[test]
fn structural_parameters_are_rejected_after_initialization() {
    let mut p = PndPlugin::new(2);
    p.initialize(44_100).unwrap();

    for (id, value) in [
        (
            ParameterId::from("analysis_window_ms"),
            ParameterValue::Float(200.0),
        ),
        (
            ParameterId::from("multi_channel_analysis"),
            ParameterValue::Bool(false),
        ),
        (
            ParameterId::from("phase_vocoder"),
            ParameterValue::Bool(true),
        ),
    ] {
        let err = p.set_parameter(id, value).unwrap_err();
        assert!(err.contains("structural"), "unexpected error: {err}");
    }
}

#[test]
fn two_minutes_of_irregular_monitoring_is_exact_passthrough() {
    let mut p = PndPlugin::new(2);
    p.initialize(48_000).unwrap();
    let partitions = [64, 511, 1024, 1536, 257];
    let mut processed = 0_usize;
    let total = 2 * 60 * 48_000;
    let mut sequence = 0_u32;
    while processed < total {
        let frames = partitions[(processed / 64) % partitions.len()].min(total - processed);
        let input: Vec<f32> = (0..frames * 2)
            .map(|_| {
                sequence = sequence.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (sequence as i32) as f32 / i32::MAX as f32
            })
            .collect();
        let mut output = vec![f32::NAN; input.len()];
        let written = p
            .process(&input, &mut output, &ProcessContext::new(48_000, frames))
            .unwrap();
        assert_eq!(written, frames);
        assert_eq!(output, input);
        assert_eq!(p.input_ring_count, 0);
        assert_eq!(p.output_ring_count, 0);
        processed += frames;
    }
}

#[test]
fn process_rejects_context_sample_rate_mismatch() {
    let mut p = PndPlugin::new(1);
    p.initialize(48_000).unwrap();
    let input = vec![0.0_f32; RESAMPLER_CHUNK_SIZE];
    let mut output = vec![0.0_f32; RESAMPLER_CHUNK_SIZE];
    let err = p
        .process(
            &input,
            &mut output,
            &ProcessContext::new(44_100, RESAMPLER_CHUNK_SIZE),
        )
        .unwrap_err();
    assert!(err.contains("sample rate"));
}

#[test]
fn initialization_rejects_zero_sample_rate() {
    let mut p = PndPlugin::new(2);
    let err = p.initialize(0).unwrap_err();
    assert!(err.contains("sample rate"), "unexpected error: {err}");
}

#[test]
fn phase_vocoder_mode_is_rejected_until_a_validated_shifter_exists() {
    let mut p = PndPlugin::new(2);
    let err = p
        .set_parameter(
            ParameterId::from("phase_vocoder"),
            ParameterValue::Bool(true),
        )
        .unwrap_err();
    assert!(err.contains("unsupported"), "unexpected error: {err}");
}

#[test]
fn resampler_reset_is_in_place() {
    let mut p = PndPlugin::new(2);
    p.initialize(48_000).unwrap();
    assert_no_allocs("PND reset", || p.reset());
}

#[test]
fn failed_chunk_does_not_consume_input_queue() {
    let mut p = PndPlugin::new(1);
    p.initialize(48_000).unwrap();
    p.input_ring[..RESAMPLER_CHUNK_SIZE].fill(0.25);
    p.input_ring_count = RESAMPLER_CHUNK_SIZE;
    p.input_ring_write_pos = RESAMPLER_CHUNK_SIZE;
    let original_read = p.input_ring_read_pos;
    let original_count = p.input_ring_count;

    // Inject a prepared-capacity failure after SRC processing.
    p.output_ring.clear();
    let err = p.process_one_chunk().unwrap_err();
    assert!(
        err.contains("exceeds prepared ring"),
        "unexpected error: {err}"
    );
    assert_eq!(p.input_ring_read_pos, original_read);
    assert_eq!(p.input_ring_count, original_count);
}

/// Setting analysis_window_ms to different values should not cause panics
/// or errors, and the plugin should process audio correctly.
#[test]
fn test_analysis_window_parameter_values() {
    for &window_ms in &[10.0, 50.0, 100.0, 200.0] {
        let mut p = PndPlugin::new(2);
        p.analysis_window_ms = window_ms;
        p.initialize(48000).unwrap();

        let nf = RESAMPLER_CHUNK_SIZE;
        let ctx = ProcessContext::new(48000, nf);

        let input: Vec<f32> = (0..nf * 2).map(|i| 0.3 * (i as f32 * 0.01).sin()).collect();
        let mut output = vec![0.0f32; nf * 2];
        let result = p.process(&input, &mut output, &ctx);
        assert!(
            result.is_ok(),
            "PND plugin should process without error with analysis_window_ms={window_ms}"
        );
        assert!(
            output.iter().all(|s| s.is_finite()),
            "All output samples should be finite with analysis_window_ms={window_ms}"
        );
    }
}

/// Verify set_parameter / get_parameter round-trip for analysis_window_ms.
#[test]
fn test_analysis_window_param_roundtrip() {
    let mut p = PndPlugin::new(1);
    p.set_parameter(
        ParameterId::from("analysis_window_ms"),
        ParameterValue::Float(75.0),
    )
    .unwrap();
    p.initialize(44100).unwrap();

    let val = p.get_parameter(&ParameterId::from("analysis_window_ms"));
    assert_eq!(val, Some(ParameterValue::Float(75.0)));
}

#[test]
fn test_process_rejects_buffer_size_mismatch() {
    let mut p = PndPlugin::new(2);
    p.initialize(48000).unwrap();

    let ctx = ProcessContext::new(48000, 64);
    let input = vec![0.0f32; ctx.num_frames * p.input_channels()];
    let mut short_output = vec![0.0f32; ctx.num_frames * p.output_channels() - 1];
    let err = p.process(&input, &mut short_output, &ctx).unwrap_err();
    assert!(err.contains("Output size mismatch"));

    let short_input = vec![0.0f32; ctx.num_frames * p.input_channels() - 1];
    let mut output = vec![0.0f32; ctx.num_frames * p.output_channels()];
    let err = p.process(&short_input, &mut output, &ctx).unwrap_err();
    assert!(err.contains("Input size mismatch"));
}

#[test]
fn monitoring_accepts_oversized_block_by_chunking_analysis() {
    let mut p = PndPlugin::new(2);
    p.initialize(48000).unwrap();

    let frames = RESAMPLER_CHUNK_SIZE * 5;
    let ctx = ProcessContext::new(48000, frames);
    let input = vec![0.0f32; frames * p.input_channels()];
    let mut output = vec![0.0f32; frames * p.output_channels()];
    let written = p.process(&input, &mut output, &ctx).unwrap();
    assert_eq!(written, frames);
    assert_eq!(output, input);
}

/// The fixed-frame path substitutes corresponding dry input whenever the
/// variable-rate SRC has no frame ready, so it has no fixed positive latency.
#[test]
fn fixed_frame_path_reports_zero_latency() {
    let mut p = PndPlugin::new(2);
    p.initialize(44100).unwrap();
    assert_eq!(p.latency_samples(), 0);
}

/// §3.5: reset() must flush the resampler internal state.
/// After reset() + re-initialize, the plugin should not produce clicks
/// from stale resampler delay lines (we verify this structurally: reset
/// re-creates the resampler, so it is Some after reset).
#[test]
fn test_reset_reinitializes_resampler() {
    let mut p = PndPlugin::new(2);
    p.initialize(44100).unwrap();

    // Process some audio to get internal resampler state dirty
    let nf = RESAMPLER_CHUNK_SIZE;
    let ctx = ProcessContext::new(44100, nf);
    let input: Vec<f32> = (0..nf * 2)
        .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin())
        .collect();
    let mut output = vec![0.0f32; nf * 2];
    p.process(&input, &mut output, &ctx).unwrap();

    // Reset must succeed and the resampler must still be present (re-created)
    p.reset();
    assert!(
        p.resampler.is_some(),
        "Resampler should be present after reset()"
    );

    // After reset, processing should produce valid output (no NaN / inf / crash)
    let silence = vec![0.0f32; nf * 2];
    let mut out2 = vec![0.0f32; nf * 2];
    p.process(&silence, &mut out2, &ctx).unwrap();
    assert!(
        out2.iter().all(|s| s.is_finite()),
        "Post-reset output should be finite"
    );
}

/// Verify set_parameter / get_parameter round-trip for drift_smoothing.
#[test]
fn test_drift_smoothing_param_roundtrip() {
    let mut p = PndPlugin::new(1);
    p.initialize(44100).unwrap();

    p.set_parameter(
        ParameterId::from("drift_smoothing"),
        ParameterValue::Float(0.85),
    )
    .unwrap();

    let val = p.get_parameter(&ParameterId::from("drift_smoothing"));
    assert_eq!(val, Some(ParameterValue::Float(0.85)));
}
