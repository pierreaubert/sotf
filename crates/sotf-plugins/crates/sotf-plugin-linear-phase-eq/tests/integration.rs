// Integration tests for sotf-plugin-linear-phase-eq exercising the public Plugin trait.

use sotf_host::{
    ParameterId, ParameterValue, ParametricInPlacePluginAdapter, Plugin, PluginHost, ProcessContext,
};
use sotf_plugin_linear_phase_eq::{BandConfig, LinearPhaseEqPlugin, LinearPhaseEqPluginParams};

fn sine_buffer(num_frames: usize, channels: usize, freq: f32, sample_rate: u32) -> Vec<f32> {
    let mut buf = vec![0.0_f32; num_frames * channels];
    for i in 0..num_frames {
        let t = i as f32 / sample_rate as f32;
        let s = (2.0 * std::f32::consts::PI * freq * t).sin() * 0.5;
        for ch in 0..channels {
            buf[i * channels + ch] = s;
        }
    }
    buf
}

#[test]
fn realtime_quantum_reports_tail_fft_work_budget() {
    let plugin = LinearPhaseEqPlugin::new(2, 48_000);
    assert_eq!(plugin.realtime_quantum_frames(), 128);
    let adapter = ParametricInPlacePluginAdapter::new(plugin);
    assert_eq!(Plugin::realtime_quantum_frames(&adapter), 128);

    let mut host = PluginHost::new(2, 48_000);
    host.add_plugin(Box::new(adapter)).unwrap();
    host.build().unwrap();
    assert_eq!(host.realtime_quantum_frames(), 128);
}

fn rms(samples: &[f32]) -> f32 {
    let sum = samples.iter().map(|s| s * s).sum::<f32>();
    (sum / samples.len().max(1) as f32).sqrt()
}

#[test]
fn plugin_info_and_channels() {
    let plugin = LinearPhaseEqPlugin::new(2, 48000);
    let adapter = ParametricInPlacePluginAdapter::new(plugin);

    assert!(adapter.info().name.contains("FIR EQ"));
    assert_eq!(adapter.input_channels(), 2);
    assert_eq!(adapter.output_channels(), 2);
    assert!(!adapter.parameters().is_empty());
}

#[test]
fn plugin_processes_silence_and_sine() {
    let plugin = LinearPhaseEqPlugin::new(2, 48000);
    let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
    adapter.initialize(48000).unwrap();

    let num_frames = 512;
    let input = sine_buffer(num_frames, 2, 1000.0, 48000);
    let mut output = vec![0.0_f32; input.len()];

    let frames = adapter
        .process(&input, &mut output, &ProcessContext::new(48000, num_frames))
        .unwrap();
    assert_eq!(frames, num_frames);
    assert!(
        output.iter().all(|s| s.is_finite()),
        "All output samples must be finite"
    );
}

#[test]
fn parameter_roundtrip() {
    let plugin = LinearPhaseEqPlugin::new(1, 48000);
    let mut adapter = ParametricInPlacePluginAdapter::new(plugin);

    adapter
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.75))
        .unwrap();
    assert_eq!(
        adapter.get_parameter(&ParameterId::from("mix")),
        Some(ParameterValue::Float(0.75))
    );

    for (id, value) in [
        ("auto_gain", ParameterValue::Bool(true)),
        ("num_filters", ParameterValue::Int(3)),
        ("fir_length", ParameterValue::Int(2)),
        ("band_0_gain", ParameterValue::Float(6.0)),
    ] {
        assert!(adapter.set_parameter(ParameterId::from(id), value).is_err());
    }
}

#[test]
fn dry_mix_passthrough() {
    // With mix=0 the plugin should pass the input straight through.
    let params = LinearPhaseEqPluginParams {
        num_filters: 1,
        fir_length_index: 0,
        phase_mode_index: 0,
        auto_gain: false,
        mix: 0.0,
        filters: vec![],
    };
    let plugin = LinearPhaseEqPlugin::from_params(2, 48000, params).unwrap();
    let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
    adapter.initialize(48000).unwrap();

    let num_frames = 1024;
    let input = sine_buffer(num_frames, 2, 440.0, 48000);
    let mut output = vec![0.0_f32; input.len()];

    adapter
        .process(&input, &mut output, &ProcessContext::new(48000, num_frames))
        .unwrap();

    let latency = 544usize;
    let max_error = output
        .chunks_exact(2)
        .enumerate()
        .map(|(frame, out)| {
            let expected = if frame >= latency {
                input[(frame - latency) * 2]
            } else {
                0.0
            };
            (out[0] - expected).abs()
        })
        .fold(0.0_f32, f32::max);
    assert!(
        max_error < 1e-6,
        "mix=0 should preserve the reported FIR latency: max_error={}",
        max_error
    );
}

#[test]
fn dry_wet_mix_aligns_dry_with_linear_phase_latency() {
    let params = LinearPhaseEqPluginParams {
        num_filters: 1,
        fir_length_index: 0, // 1024 taps -> 512 group delay + 32 partition latency
        phase_mode_index: 0,
        auto_gain: false,
        mix: 0.5,
        filters: vec![],
    };
    let plugin = LinearPhaseEqPlugin::from_params(1, 48_000, params).unwrap();
    let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
    adapter.initialize(48_000).unwrap();

    let block_size = 64;
    let total_frames = 1_024;
    let mut output = vec![0.0_f32; total_frames];
    for block_start in (0..total_frames).step_by(block_size) {
        let mut input = vec![0.0_f32; block_size];
        if block_start == 0 {
            input[0] = 1.0;
        }
        let mut block_output = vec![0.0_f32; block_size];
        adapter
            .process(
                &input,
                &mut block_output,
                &ProcessContext::new(48_000, block_size),
            )
            .unwrap();
        output[block_start..block_start + block_size].copy_from_slice(&block_output);
    }

    let latency = adapter.latency_samples();
    assert_eq!(latency, 544);
    assert!(
        output[..latency].iter().all(|sample| sample.abs() < 1e-6),
        "a partially dry linear-phase signal must not contain an undelayed impulse"
    );
    assert!(
        output[latency..].iter().any(|sample| sample.abs() > 0.1),
        "the aligned dry/wet impulse should appear at the reported latency"
    );
}

#[test]
fn eq_boost_changes_amplitude() {
    let params = LinearPhaseEqPluginParams {
        num_filters: 1,
        fir_length_index: 0,
        phase_mode_index: 0,
        auto_gain: false,
        mix: 1.0,
        filters: vec![BandConfig {
            filter_type: "Peak".to_string(),
            frequency: 1000.0,
            q: 1.0,
            gain_db: 9.0,
            active: true,
        }],
    };
    let plugin = LinearPhaseEqPlugin::from_params(1, 48000, params).unwrap();
    let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
    adapter.initialize(48000).unwrap();

    let num_frames = 4096;
    let input = sine_buffer(num_frames, 1, 1000.0, 48000);
    let mut output = vec![0.0_f32; input.len()];

    adapter
        .process(&input, &mut output, &ProcessContext::new(48000, num_frames))
        .unwrap();

    // Ignore the first part of the output while the linear-phase FIR latency settles.
    let steady_start = adapter.latency_samples().max(64);
    let input_rms = rms(&input[steady_start..]);
    let output_rms = rms(&output[steady_start..]);

    assert!(
        output_rms > input_rms * 1.5,
        "A +9 dB boost at 1 kHz should raise the 1 kHz sine amplitude"
    );
}

#[test]
fn latency_matches_fir_length() {
    let params = LinearPhaseEqPluginParams {
        num_filters: 1,
        fir_length_index: 2, // 4096 taps
        phase_mode_index: 0,
        auto_gain: false,
        mix: 1.0,
        filters: vec![],
    };
    let plugin = LinearPhaseEqPlugin::from_params(1, 48000, params).unwrap();
    let adapter = ParametricInPlacePluginAdapter::new(plugin);

    let fir_length = 4096;
    let expected = fir_length / 2 + 32;
    assert_eq!(adapter.latency_samples(), expected);
}

#[test]
fn unknown_parameter_is_rejected() {
    let plugin = LinearPhaseEqPlugin::new(1, 48000);
    let adapter = ParametricInPlacePluginAdapter::new(plugin);

    // The default Plugin::validate_parameter helper rejects unknown ids.
    let result = adapter.validate_parameter(
        &ParameterId::from("this_param_does_not_exist"),
        &ParameterValue::Float(1.0),
    );
    assert!(
        result.is_err(),
        "Validating an unknown parameter should fail"
    );
}

#[test]
fn invalid_parameter_value_is_rejected() {
    let plugin = LinearPhaseEqPlugin::new(1, 48000);
    let adapter = ParametricInPlacePluginAdapter::new(plugin);

    let result = adapter.validate_parameter(&ParameterId::from("mix"), &ParameterValue::Float(2.0));
    assert!(
        result.is_err(),
        "A mix value outside [0, 1] should fail validation"
    );
}

#[test]
fn reset_then_process_is_stable() {
    let plugin = LinearPhaseEqPlugin::new(2, 48000);
    let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
    adapter.initialize(48000).unwrap();

    let num_frames = 256;
    let input = sine_buffer(num_frames, 2, 500.0, 48000);
    let mut output = vec![0.0_f32; input.len()];

    adapter
        .process(&input, &mut output, &ProcessContext::new(48000, num_frames))
        .unwrap();

    adapter.reset();

    let mut output2 = vec![0.0_f32; input.len()];
    adapter
        .process(
            &input,
            &mut output2,
            &ProcessContext::new(48000, num_frames),
        )
        .unwrap();

    assert!(output2.iter().all(|s| s.is_finite()));
}

#[test]
fn minimum_phase_processes_finite_audio() {
    let params = LinearPhaseEqPluginParams {
        num_filters: 1,
        fir_length_index: 0,
        phase_mode_index: 1,
        auto_gain: false,
        mix: 1.0,
        filters: vec![BandConfig {
            filter_type: "Peak".to_string(),
            frequency: 1_000.0,
            q: 1.0,
            gain_db: 6.0,
            active: true,
        }],
    };
    let plugin = LinearPhaseEqPlugin::from_params(1, 48_000, params).unwrap();
    let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
    adapter.initialize(48_000).unwrap();

    let input = sine_buffer(2_048, 1, 1_000.0, 48_000);
    let mut output = vec![0.0; input.len()];
    adapter
        .process(&input, &mut output, &ProcessContext::new(48_000, 2_048))
        .unwrap();

    assert_eq!(adapter.latency_samples(), 32);
    assert!(output.iter().all(|sample| sample.is_finite()));
    assert!(output.iter().any(|sample| sample.abs() > 1.0e-6));
}

#[test]
fn wet_impulse_peak_matches_reported_even_tap_latency() {
    let plugin = LinearPhaseEqPlugin::from_params(
        1,
        48_000,
        LinearPhaseEqPluginParams {
            num_filters: 1,
            fir_length_index: 0,
            phase_mode_index: 0,
            auto_gain: false,
            mix: 1.0,
            filters: vec![],
        },
    )
    .unwrap();
    let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
    let latency = adapter.latency_samples();
    let mut input = vec![0.0; latency + 1_024];
    input[0] = 1.0;
    let mut output = vec![0.0; input.len()];
    adapter
        .process(
            &input,
            &mut output,
            &ProcessContext::new(48_000, input.len()),
        )
        .unwrap();
    let peak = output
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
        .unwrap()
        .0;
    assert_eq!(peak, latency);
}

#[test]
fn mix_automation_is_block_partition_invariant() {
    fn render(block: usize) -> Vec<f32> {
        let plugin = LinearPhaseEqPlugin::from_params(
            1,
            48_000,
            LinearPhaseEqPluginParams {
                num_filters: 1,
                fir_length_index: 0,
                phase_mode_index: 0,
                auto_gain: false,
                mix: 0.0,
                filters: vec![BandConfig {
                    filter_type: "Peak".into(),
                    frequency: 1_000.0,
                    q: 1.0,
                    gain_db: 12.0,
                    active: true,
                }],
            },
        )
        .unwrap();
        let mut adapter = ParametricInPlacePluginAdapter::new(plugin);
        adapter
            .set_parameter(ParameterId::from("mix"), ParameterValue::Float(1.0))
            .unwrap();
        let input = sine_buffer(4_096, 1, 1_000.0, 48_000);
        let mut rendered = vec![0.0; input.len()];
        for start in (0..input.len()).step_by(block) {
            let end = (start + block).min(input.len());
            adapter
                .process(
                    &input[start..end],
                    &mut rendered[start..end],
                    &ProcessContext::new(48_000, end - start),
                )
                .unwrap();
        }
        rendered
    }

    let reference = render(1);
    for block in [16, 63, 64, 127, 256, 1_024] {
        let candidate = render(block);
        let max_error = reference
            .iter()
            .zip(candidate)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0_f32, f32::max);
        assert!(max_error < 1.0e-5, "block {block}: {max_error}");
    }
}
