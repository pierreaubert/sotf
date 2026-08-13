use super::consts::PV_FFT_SIZE;
use super::consts::PV_LATENCY_FRAMES;
use super::pnd_plugin::PndPlugin;
use super::pnd_plugin::smooth_drift_ratio;
use super::pnd_plugin::weighted_channel_consensus;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, ProcessContext};

const PROCESS_CHUNK_FRAMES: usize = 1024;

/// With high drift_smoothing, the correction ratio should change slowly
/// (no sudden jumps between frames).
#[test]
fn test_drift_smoothing_slow_correction() {
    let mut p = PndPlugin::new(2);
    p.drift_smoothing = 0.99; // very high smoothing
    p.correction_strength = 1.0;
    p.initialize(48000).unwrap();

    let nf = PROCESS_CHUNK_FRAMES;
    let ctx = ProcessContext::new(48000, nf);

    // Process several blocks and track how current_ratio evolves
    let mut ratios = Vec::new();
    for block in 0..10 {
        let input: Vec<f32> = (0..nf * 2)
            .map(|i| {
                0.3 * (2.0 * std::f32::consts::PI * 440.0 * (block * nf * 2 + i) as f32 / 48000.0)
                    .sin()
            })
            .collect();
        let mut output = vec![0.0f32; nf * 2];
        let _ = p.process(&input, &mut output, &ctx);
        ratios.push(p.current_ratio);
    }

    // With high smoothing, ratio changes should be very small between blocks
    for i in 1..ratios.len() {
        let delta = (ratios[i] - ratios[i - 1]).abs();
        assert!(
            delta < 0.01,
            "Correction ratio changed too fast at block {i}: delta={delta:.6}, \
                 prev={:.6}, curr={:.6}",
            ratios[i - 1],
            ratios[i]
        );
    }
}

#[test]
fn drift_smoothing_parameter_is_monotonic_and_high_values_are_slow() {
    let current = 1.0;
    let target = 1.1;
    let fast = smooth_drift_ratio(current, target, 0.1, 512, 48_000);
    let slow = smooth_drift_ratio(current, target, 0.9, 512, 48_000);

    assert!(fast > slow, "lower smoothing should track faster");
    assert!(slow > current, "a finite target should still make progress");
    assert!(slow < target, "smoothing should not jump to the target");
}

#[test]
fn drift_smoothing_is_elapsed_time_and_sample_rate_invariant() {
    let current = 1.0;
    let target = 1.1;
    let tau = 0.1;

    let once = smooth_drift_ratio(current, target, tau, 1024, 48_000);
    let half = smooth_drift_ratio(current, target, tau, 512, 48_000);
    let twice = smooth_drift_ratio(half, target, tau, 512, 48_000);
    assert!((once - twice).abs() < 1e-12);

    let at_96k = smooth_drift_ratio(current, target, tau, 2048, 96_000);
    assert!((once - at_96k).abs() < 1e-12);
    assert_eq!(smooth_drift_ratio(current, target, tau, 0, 48_000), current);
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
    ] {
        let err = p.set_parameter(id, value).unwrap_err();
        assert!(err.contains("structural"), "unexpected error: {err}");
    }
}

#[test]
fn sustained_correction_keeps_fixed_frame_contract_without_zeros_or_overflow() {
    for ratio in [0.95, 1.05] {
        let mut p = PndPlugin::new(1);
        p.correction_strength = 1.0;
        p.current_ratio = ratio;
        p.initialize(48_000).unwrap();
        p.current_ratio = ratio;
        p.analyzers.clear();
        let nf = PROCESS_CHUNK_FRAMES;
        let ctx = ProcessContext::new(48_000, nf);
        let input: Vec<f32> = (0..nf)
            .map(|frame| {
                (2.0 * std::f32::consts::PI * 440.0 * frame as f32 / 48_000.0).sin() * 0.25
            })
            .collect();
        let mut output = vec![0.0_f32; nf];
        let callbacks = 2 * 60 * 48_000 / nf;

        for callback in 0..callbacks {
            assert_eq!(p.process(&input, &mut output, &ctx).unwrap(), nf);
            assert!(output.iter().all(|sample| sample.is_finite()));
            if callback * nf > p.latency_samples() + PV_FFT_SIZE {
                assert!(output.iter().any(|sample| sample.abs() > 1.0e-5));
            }
        }
    }
}

#[test]
fn process_errors_are_transactional() {
    let input: Vec<f32> = (0..PROCESS_CHUNK_FRAMES)
        .map(|frame| (frame as f32 * 0.03).sin() * 0.25)
        .collect();
    let context = ProcessContext::new(48_000, PROCESS_CHUNK_FRAMES);
    let mut retried = PndPlugin::new(1);
    let mut fresh = PndPlugin::new(1);
    retried.initialize(48_000).unwrap();
    fresh.initialize(48_000).unwrap();

    assert!(retried.process(&input, &mut [0.0], &context).is_err());
    let mut retry_output = vec![0.0; PROCESS_CHUNK_FRAMES];
    let mut fresh_output = vec![0.0; PROCESS_CHUNK_FRAMES];
    retried
        .process(&input, &mut retry_output, &context)
        .unwrap();
    fresh.process(&input, &mut fresh_output, &context).unwrap();
    assert_eq!(retry_output, fresh_output);

    // Content validation is also pre-state: repairing a non-finite callback
    // and retrying is bit-identical to an uninterrupted fresh instance.
    let mut invalid_input = input.clone();
    invalid_input[PROCESS_CHUNK_FRAMES / 2] = f32::NAN;
    assert!(
        retried
            .process(&invalid_input, &mut retry_output, &context)
            .is_err()
    );
    retried
        .process(&input, &mut retry_output, &context)
        .unwrap();
    fresh.process(&input, &mut fresh_output, &context).unwrap();
    assert_eq!(retry_output, fresh_output);
}

#[test]
fn multichannel_consensus_excludes_silence_and_rejects_conflicting_sources() {
    let mut tonal_plus_silence = [(1.01, 0.9), (1.0, 0.0), (0.99, 0.1)];
    let (ratio, confidence) = weighted_channel_consensus(&mut tonal_plus_silence, 0.5);
    assert!((ratio - 1.01).abs() < 1.0e-6);
    assert!((confidence - 0.9).abs() < 1.0e-6);

    let mut conflict = [(0.99, 0.95), (1.01, 0.95)];
    assert_eq!(
        weighted_channel_consensus(&mut conflict, 0.5),
        (1.0, 0.0),
        "opposite high-confidence sources must not authorize a fictitious average"
    );
}

#[test]
fn multichannel_consensus_is_permutation_invariant_and_requires_majority_support() {
    let observations = [(1.010, 0.9), (1.011, 0.8), (1.009, 0.7), (0.98, 0.9)];
    let mut forward = observations;
    let mut reverse = observations;
    reverse.reverse();
    let expected = weighted_channel_consensus(&mut forward, 0.5);
    let permuted = weighted_channel_consensus(&mut reverse, 0.5);
    assert_eq!(expected, permuted);
    assert!((expected.0 - 1.01).abs() < 0.001);
    assert!(
        expected.1 > 0.6,
        "a coherent three-of-four majority should pass the default confidence gate: {expected:?}"
    );
}

fn render_multichannel_reference_ratio(frequencies: &[Option<f32>]) -> f64 {
    let sample_rate = 48_000;
    let frames = sample_rate as usize;
    let channels = frequencies.len();
    let mut plugin = PndPlugin::new(channels);
    plugin.reference_frequency_hz = 440.0;
    plugin.confidence_threshold = 0.5;
    plugin.drift_smoothing = 0.001;
    plugin.initialize(sample_rate).unwrap();
    let input = (0..frames)
        .flat_map(|frame| {
            frequencies.iter().map(move |frequency| {
                frequency.map_or(0.0, |frequency| {
                    0.25 * (2.0 * std::f32::consts::PI * frequency * frame as f32
                        / sample_rate as f32)
                        .sin()
                })
            })
        })
        .collect::<Vec<_>>();
    let mut output = vec![0.0; input.len()];
    plugin
        .process(
            &input,
            &mut output,
            &ProcessContext::new(sample_rate, frames),
        )
        .unwrap();
    plugin.current_ratio
}

#[test]
fn end_to_end_spatial_consensus_is_order_independent_and_fails_closed_on_a_split() {
    let forward = [Some(444.4), Some(444.4), None, Some(444.4), Some(435.6)];
    let mut reversed = forward;
    reversed.reverse();
    let forward_ratio = render_multichannel_reference_ratio(&forward);
    let reversed_ratio = render_multichannel_reference_ratio(&reversed);
    assert_eq!(forward_ratio, reversed_ratio);
    assert!(
        forward_ratio < 0.995,
        "the coherent referenced majority should authorize correction: {forward_ratio}"
    );

    let split_ratio = render_multichannel_reference_ratio(&[Some(444.4), Some(435.6)]);
    assert!(
        (split_ratio - 1.0).abs() < 1.0e-9,
        "an even spatial conflict must fail closed: {split_ratio}"
    );
}

#[test]
fn multichannel_policy_preserves_every_channel_and_uses_shared_correction() {
    for channels in [1, 2, 6] {
        let mut p = PndPlugin::new(channels);
        p.initialize(48_000).unwrap();
        assert_eq!(p.input_channels(), channels);
        assert_eq!(p.output_channels(), channels);
        let frames = PROCESS_CHUNK_FRAMES * 4;
        let input: Vec<f32> = (0..frames)
            .flat_map(|frame| {
                (0..channels).map(move |channel| {
                    let hz = 220.0 + channel as f32 * 55.0;
                    (2.0 * std::f32::consts::PI * hz * frame as f32 / 48_000.0).sin() * 0.2
                })
            })
            .collect();
        let mut output = vec![0.0; input.len()];
        assert_eq!(
            p.process(&input, &mut output, &ProcessContext::new(48_000, frames))
                .unwrap(),
            frames
        );
        for channel in 0..channels {
            assert!(
                output[channel..]
                    .iter()
                    .step_by(channels)
                    .any(|s| s.abs() > 1.0e-6)
            );
        }
    }
}

#[test]
fn process_rejects_context_sample_rate_mismatch() {
    let mut p = PndPlugin::new(1);
    p.initialize(48_000).unwrap();
    let input = vec![0.0_f32; PROCESS_CHUNK_FRAMES];
    let mut output = vec![0.0_f32; PROCESS_CHUNK_FRAMES];
    let err = p
        .process(
            &input,
            &mut output,
            &ProcessContext::new(44_100, PROCESS_CHUNK_FRAMES),
        )
        .unwrap_err();
    assert!(err.contains("sample rate"));
}

/// Setting analysis_window_ms to different values should not cause panics
/// or errors, and the plugin should process audio correctly.
#[test]
fn test_analysis_window_parameter_values() {
    for &window_ms in &[10.0, 50.0, 100.0, 200.0] {
        let mut p = PndPlugin::new(2);
        p.analysis_window_ms = window_ms;
        p.initialize(48000).unwrap();

        let nf = PROCESS_CHUNK_FRAMES;
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
fn process_accepts_large_callbacks_without_internal_chunk_queues() {
    let mut p = PndPlugin::new(2);
    p.initialize(48000).unwrap();

    let frames = PROCESS_CHUNK_FRAMES * 5;
    let ctx = ProcessContext::new(48000, frames);
    let input = vec![0.0f32; frames * p.input_channels()];
    let mut output = vec![0.0f32; frames * p.output_channels()];
    assert_eq!(p.process(&input, &mut output, &ctx).unwrap(), frames);
    assert!(output.iter().all(|sample| sample.is_finite()));
}

#[test]
fn test_latency_samples_reports_fixed_duration_preserving_latency() {
    let mut p = PndPlugin::new(2);
    p.initialize(44100).unwrap();
    assert_eq!(p.latency_samples(), PV_LATENCY_FRAMES);
}

fn render_partitioned_impulse(partitions: &[usize]) -> (Vec<f32>, usize) {
    let mut p = PndPlugin::new(1);
    p.set_parameter(
        ParameterId::from("correction_strength"),
        ParameterValue::Float(0.0),
    )
    .unwrap();
    p.initialize(48_000).unwrap();
    let latency = p.latency_samples();
    let total_frames = latency + PV_FFT_SIZE * 3;
    let mut rendered = Vec::with_capacity(total_frames);
    let mut position = 0;
    let mut partition = 0;
    while position < total_frames {
        let frames = partitions[partition % partitions.len()].min(total_frames - position);
        let mut input = vec![0.0; frames];
        if position == 0 {
            input[0] = 1.0;
        }
        let mut output = vec![0.0; frames];
        p.process(&input, &mut output, &ProcessContext::new(48_000, frames))
            .unwrap();
        rendered.extend_from_slice(&output);
        position += frames;
        partition += 1;
    }
    (rendered, latency)
}

#[test]
fn impulse_latency_is_invariant_across_callback_partitions() {
    for partitions in [
        vec![1],
        vec![64],
        vec![511],
        vec![512],
        vec![1024],
        vec![1, 64, 511, 512, 1024, 73, 997],
    ] {
        let (rendered, reported_latency) = render_partitioned_impulse(&partitions);
        let peak = rendered
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                a.abs()
                    .partial_cmp(&b.abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap();
        assert_eq!(
            peak.0, reported_latency,
            "partition {partitions:?} produced impulse at {}, metadata reported {reported_latency}",
            peak.0
        );
        assert!(
            peak.1.abs() > 0.8,
            "partition {partitions:?} lost the impulse: peak {}",
            peak.1
        );
    }
}

fn render_reference_step(partitions: &[usize]) -> (Vec<f32>, f64) {
    let sample_rate = 48_000;
    let silence_frames = 4096;
    let tone_frames = 24_000;
    let total_frames = silence_frames + tone_frames * 2;
    let input: Vec<f32> = (0..total_frames)
        .map(|frame| {
            if frame < silence_frames {
                0.0
            } else {
                let frequency = if frame < silence_frames + tone_frames {
                    444.4
                } else {
                    435.6
                };
                0.25 * (2.0 * std::f32::consts::PI * frequency * frame as f32 / sample_rate as f32)
                    .sin()
            }
        })
        .collect();

    let mut plugin = PndPlugin::new(1);
    plugin.reference_frequency_hz = 440.0;
    plugin.initialize(sample_rate).unwrap();
    let mut rendered = Vec::with_capacity(total_frames);
    let mut position = 0;
    let mut partition = 0;
    while position < total_frames {
        let frames = partitions[partition % partitions.len()].min(total_frames - position);
        let mut output = vec![0.0; frames];
        plugin
            .process(
                &input[position..position + frames],
                &mut output,
                &ProcessContext::new(sample_rate, frames),
            )
            .unwrap();
        rendered.extend_from_slice(&output);
        position += frames;
        partition += 1;
    }
    (rendered, plugin.current_ratio)
}

#[test]
fn referenced_control_and_output_are_callback_partition_invariant() {
    let (reference_output, reference_ratio) = render_reference_step(&[1]);
    for partitions in [
        vec![64],
        vec![511],
        vec![512],
        vec![1024],
        vec![1, 64, 511, 512, 1024, 73, 997],
    ] {
        let (output, ratio) = render_reference_step(&partitions);
        assert_eq!(ratio, reference_ratio, "ratio for {partitions:?}");
        assert_eq!(output, reference_output, "output for {partitions:?}");
    }
}

#[test]
fn referenced_control_releases_stale_correction_when_pilot_authority_disappears() {
    let sample_rate = 48_000;
    let mut plugin = PndPlugin::new(1);
    plugin.reference_frequency_hz = 440.0;
    plugin.initialize(sample_rate).unwrap();

    let process_tone = |plugin: &mut PndPlugin, frequency: Option<f32>, frames: usize| {
        let input: Vec<f32> = (0..frames)
            .map(|frame| {
                frequency.map_or(0.0, |hz| {
                    0.25 * (2.0 * std::f32::consts::PI * hz * frame as f32 / sample_rate as f32)
                        .sin()
                })
            })
            .collect();
        let mut output = vec![0.0; frames];
        assert_eq!(
            plugin
                .process(
                    &input,
                    &mut output,
                    &ProcessContext::new(sample_rate, frames),
                )
                .unwrap(),
            frames
        );
        output
    };

    process_tone(&mut plugin, Some(444.4), sample_rate as usize);
    assert!(
        plugin.current_ratio < 0.995,
        "pilot should establish a corrective ratio: {}",
        plugin.current_ratio
    );

    let silence = process_tone(&mut plugin, None, sample_rate as usize / 2);
    assert!(silence.iter().all(|sample| sample.is_finite()));
    let moved = process_tone(&mut plugin, Some(523.25), sample_rate as usize);
    assert!(
        (plugin.current_ratio - 1.0).abs() < 1.0e-4,
        "lost pilot authority must release correction: {}",
        plugin.current_ratio
    );
    assert!(
        moved[plugin.latency_samples()..]
            .iter()
            .any(|sample| sample.abs() > 1.0e-5),
        "unrelated programme material should continue through the insert"
    );
}

#[test]
fn reset_publishes_default_diagnostics_with_two_held_generations() {
    let mut plugin = PndPlugin::new(1);
    let held_initial = plugin.cache.load();
    plugin.cache.update(|data| {
        data.drift_ratio = 1.05;
        data.correction_ratio = 0.95;
        data.confidence = 0.9;
        data.matched_partials = 3;
        data.total_peaks = 4;
    });
    let held_active = plugin.cache.load();

    plugin.reset();

    let current = plugin.cache.load();
    assert_eq!(current.drift_ratio, 1.0);
    assert_eq!(current.correction_ratio, 1.0);
    assert_eq!(current.confidence, 0.0);
    assert_eq!(current.matched_partials, 0);
    assert_eq!(current.total_peaks, 0);
    assert_eq!(held_initial.drift_ratio, 1.0);
    assert_eq!(held_active.drift_ratio, 1.05);
    assert_eq!(plugin.cache_update_counter, 0);
}

#[test]
fn test_reset_clears_vocoder_state_without_rebuilding_it() {
    let mut p = PndPlugin::new(2);
    p.initialize(44100).unwrap();

    let nf = PROCESS_CHUNK_FRAMES;
    let ctx = ProcessContext::new(44100, nf);
    let input: Vec<f32> = (0..nf * 2)
        .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin())
        .collect();
    let mut output = vec![0.0f32; nf * 2];
    p.process(&input, &mut output, &ctx).unwrap();

    let vocoder_address = p.vocoder.as_ref().unwrap() as *const _;
    p.reset();
    assert_eq!(vocoder_address, p.vocoder.as_ref().unwrap() as *const _);

    // After reset, processing should produce valid output (no NaN / inf / crash)
    let silence = vec![0.0f32; nf * 2];
    let mut out2 = vec![0.0f32; nf * 2];
    p.process(&silence, &mut out2, &ctx).unwrap();
    assert!(
        out2.iter().all(|s| s.is_finite()),
        "Post-reset output should be finite"
    );
}

/// §4.4: correction_strength_smoother must be advanced in the phase vocoder path.
/// A rapid correction_strength change should not produce a discontinuity larger
/// than what the smoother allows in one call.
#[test]
fn test_pv_path_uses_correction_strength_smoother() {
    let mut p = PndPlugin::new(2);
    p.set_parameter(
        ParameterId::from("correction_strength"),
        ParameterValue::Float(0.0),
    )
    .unwrap();
    p.initialize(44100).unwrap();

    let nf = 512;
    let ctx = ProcessContext::new(44100, nf);
    let input: Vec<f32> = (0..nf * 2)
        .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin())
        .collect();
    let mut output = vec![0.0f32; nf * 2];
    // Process with strength=0 to prime the smoother
    p.process(&input, &mut output, &ctx).unwrap();

    // Now jump strength to 1.0
    p.set_parameter(
        ParameterId::from("correction_strength"),
        ParameterValue::Float(1.0),
    )
    .unwrap();

    // Process one block; the smoother should advance (not jump to 1.0 instantly)
    // We can verify the smoother is "moving" by checking that the cached value
    // of the smoother is between 0 and 1 after one advance.
    let mut out2 = vec![0.0f32; nf * 2];
    p.process(&input, &mut out2, &ctx).unwrap();

    // Verify: output must be finite (no NaN/inf from unsmoothed parameter jump)
    assert!(
        out2.iter().all(|s| s.is_finite()),
        "Phase vocoder output must be finite after correction_strength jump"
    );

    assert!(p.correction_strength_current > 0.0);
    assert!(p.correction_strength_current < 1.0);
}

#[test]
fn phase_vocoder_accepts_large_callbacks_directly() {
    let mut p = PndPlugin::new(2);
    p.initialize(44100).unwrap();

    let frames = PROCESS_CHUNK_FRAMES + 256;
    let ctx = ProcessContext::new(44100, frames);
    let input: Vec<f32> = (0..frames * 2)
        .map(|i| 0.25 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin())
        .collect();
    let mut output = vec![0.0f32; frames * 2];

    let processed = p.process(&input, &mut output, &ctx).unwrap();

    assert_eq!(processed, frames);
    assert!(output.iter().all(|s| s.is_finite()));
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
