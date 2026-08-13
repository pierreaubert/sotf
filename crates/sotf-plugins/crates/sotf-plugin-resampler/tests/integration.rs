// Integration tests for sotf-plugin-resampler exercising the public Plugin trait.

use sotf_host::host::DawHost;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, PluginDrainResult, PluginInfo, PluginResult, ProcessContext};
use sotf_plugin_resampler::{ResamplerPlugin, ResamplerQuality};

#[test]
fn integration_plugin_info_and_channels() {
    let resampler = ResamplerPlugin::new(2, 44100, 48000, 1024).unwrap();
    assert_eq!(resampler.input_channels(), 2);
    assert_eq!(resampler.output_channels(), 2);
    let info = resampler.info();
    assert_eq!(info.name, "Resampler");
    assert!(info.description.contains("44100Hz -> 48000Hz"));
}

#[test]
fn integration_constructors_reject_invalid_configs() {
    assert!(ResamplerPlugin::new(0, 44100, 48000, 1024).is_err());
    assert!(ResamplerPlugin::new(2, 0, 48000, 1024).is_err());
    assert!(ResamplerPlugin::new(2, 44100, 0, 1024).is_err());
    assert!(ResamplerPlugin::new(2, 44100, 48000, 0).is_err());
}

#[test]
fn integration_default_constructor() {
    let mut resampler = ResamplerPlugin::new_default(2, 44100, 48000).unwrap();
    resampler.initialize(44100).unwrap();
    assert_eq!(resampler.input_channels(), 2);
    assert!((resampler.ratio() - 48000.0 / 44100.0).abs() < 1e-6);
}

#[test]
fn integration_quality_change_via_parameter() {
    let mut resampler = ResamplerPlugin::new(2, 44100, 48000, 1024).unwrap();

    // Switch to high quality through the public Plugin trait.
    resampler
        .set_parameter(
            ParameterId::from("quality"),
            ParameterValue::String("high".to_string()),
        )
        .unwrap();
    let v = resampler
        .get_parameter(&ParameterId::from("quality"))
        .unwrap();
    assert_eq!(v, ParameterValue::Int(2));

    // Invalid quality string must be rejected.
    let res = resampler.set_parameter(
        ParameterId::from("quality"),
        ParameterValue::String("ultra".to_string()),
    );
    assert!(res.is_err());
    resampler.initialize(44100).unwrap();
    assert!(
        resampler
            .set_parameter(ParameterId::from("quality"), ParameterValue::Int(1))
            .is_err()
    );
}

#[test]
fn integration_with_quality_constructor() {
    let mut resampler =
        ResamplerPlugin::with_quality(2, 44100, 48000, 1024, ResamplerQuality::High).unwrap();
    resampler.initialize(44100).unwrap();

    let v = resampler
        .get_parameter(&ParameterId::from("quality"))
        .unwrap();
    assert_eq!(v, ParameterValue::Int(2));
}

#[test]
fn integration_dynamic_ratio_workflow() {
    let mut resampler = ResamplerPlugin::new(2, 44100, 48000, 1024).unwrap();
    resampler.initialize(44100).unwrap();
    let nominal = resampler.ratio();

    // Cannot change ratio while dynamic mode is disabled.
    let res = resampler.set_parameter(ParameterId::from("ratio"), ParameterValue::Float(0.9));
    assert!(res.is_err());

    // Enable dynamic ratio.
    resampler
        .set_parameter(
            ParameterId::from("dynamic_ratio"),
            ParameterValue::Bool(true),
        )
        .unwrap();
    assert!(resampler.is_dynamic_ratio());

    // Now a runtime ratio change should succeed.  Keep the new ratio within
    // the supported range and close enough to nominal that rubato's dynamic
    // ratio path does not need a different input frame count.
    let new_ratio = nominal * 1.05;
    resampler
        .set_parameter(
            ParameterId::from("ratio"),
            ParameterValue::Float(new_ratio as f32),
        )
        .unwrap();
    assert!((resampler.current_ratio() - new_ratio).abs() < 1e-6);

    // Process a block at the new ratio.
    let input = vec![0.5f32; 1024 * 2];
    let max_out = resampler.output_frames_for_input(1024);
    let mut output = vec![0.0f32; max_out * 2];
    let ctx = ProcessContext::new(44100, 1024);
    let produced = resampler.process(&input, &mut output, &ctx).unwrap();
    assert!(produced > 0);

    // Disabling dynamic ratio should snap back to the nominal ratio.
    resampler
        .set_parameter(
            ParameterId::from("dynamic_ratio"),
            ParameterValue::Bool(false),
        )
        .unwrap();
    let v = resampler
        .get_parameter(&ParameterId::from("ratio"))
        .unwrap()
        .as_float()
        .unwrap();
    assert!((v as f64 - nominal).abs() < 1e-4);
}

#[test]
fn integration_flush_drains_residual() {
    let mut resampler = ResamplerPlugin::new(2, 44100, 48000, 1024).unwrap();
    resampler.initialize(44100).unwrap();

    // Process less than a full chunk: no output yet.
    let partial_frames = 512;
    let input = vec![0.5f32; partial_frames * 2];
    let max_out = resampler.output_frames_for_input(partial_frames);
    let mut output = vec![0.0f32; max_out * 2];
    let ctx = ProcessContext::new(44100, partial_frames);
    let produced = resampler.process(&input, &mut output, &ctx).unwrap();
    assert_eq!(produced, 0);

    // Flush must produce the buffered residual frames.
    let flush_capacity = resampler.flush_output_frames_max();
    let mut flush_buf = vec![0.0f32; flush_capacity * 2];
    let (flush_frames, discard) = resampler.flush(&mut flush_buf).unwrap();
    assert!(flush_frames > 0);
    assert_eq!(discard, 0, "complete-stream drain trims internally");
}

#[test]
fn integration_multiple_blocks_maintain_continuity() {
    let mut resampler = ResamplerPlugin::new(2, 44100, 48000, 1024).unwrap();
    resampler.initialize(44100).unwrap();

    let num_frames = 1024;
    let max_out = resampler.output_frames_for_input(num_frames);
    let ctx = ProcessContext::new(44100, num_frames);

    let mut total_output_frames = 0usize;
    for block in 0..3 {
        let mut input = vec![0.0f32; num_frames * 2];
        for i in 0..num_frames {
            let frame_idx = block * num_frames + i;
            let t = frame_idx as f32 / 44100.0;
            let s = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
            input[i * 2] = s;
            input[i * 2 + 1] = s;
        }
        let mut output = vec![0.0f32; max_out * 2];
        let produced = resampler.process(&input, &mut output, &ctx).unwrap();
        assert!(produced > 0);
        assert!(output.iter().all(|s| s.is_finite()));
        total_output_frames += produced;
    }
    assert!(total_output_frames > 0);
    assert!(resampler.last_output_frames().is_some());
}

#[test]
fn integration_process_rejects_bad_input_size() {
    let mut resampler = ResamplerPlugin::new(2, 44100, 48000, 1024).unwrap();
    resampler.initialize(44100).unwrap();

    let ctx = ProcessContext::new(44100, 1024);
    let input = vec![0.0f32; 1024 * 2];
    let mut output = vec![0.0f32; 1024]; // too short
    let res = resampler.process(&input, &mut output, &ctx);
    assert!(res.is_err());

    let input_short = vec![0.0f32; 512];
    let mut output_ok = vec![0.0f32; resampler.output_frames_for_input(1024) * 2];
    let res = resampler.process(&input_short, &mut output_ok, &ctx);
    assert!(res.is_err());
}

#[test]
fn integration_output_frames_for_input_matches_ratio() {
    let resampler = ResamplerPlugin::new(2, 44100, 48000, 1024).unwrap();
    let ratio = resampler.ratio();
    let input_frames = 1024usize;
    let estimated = resampler.output_frames_for_input(input_frames);
    let expected = (input_frames as f64 * ratio).ceil() as usize;
    assert!(estimated >= expected);
}

#[test]
fn integration_reset_recoverable() {
    let mut resampler = ResamplerPlugin::new(2, 44100, 48000, 1024).unwrap();
    resampler.initialize(44100).unwrap();

    let input = vec![0.5f32; 1024 * 2];
    let max_out = resampler.output_frames_for_input(1024);
    let mut output = vec![0.0f32; max_out * 2];
    let ctx = ProcessContext::new(44100, 1024);
    resampler.process(&input, &mut output, &ctx).unwrap();

    resampler.reset();

    let mut output2 = vec![0.0f32; max_out * 2];
    resampler.process(&input, &mut output2, &ctx).unwrap();
    assert!(output2.iter().all(|s| s.is_finite()));
}

#[test]
fn complete_stream_counts_cover_all_qualities_ratios_and_residual_boundaries() {
    for quality in [
        ResamplerQuality::Fast,
        ResamplerQuality::Medium,
        ResamplerQuality::High,
    ] {
        for &(input_rate, output_rate) in &[(22_050, 96_000), (96_000, 22_050)] {
            for input_frames in [1usize, 1023, 1024, 1025] {
                let mut plugin =
                    ResamplerPlugin::with_quality(1, input_rate, output_rate, 1024, quality)
                        .unwrap();
                plugin.initialize(input_rate).unwrap();
                let input = vec![0.25; input_frames];
                let capacity = plugin.output_frames_for_input(input_frames);
                let mut output = vec![0.0; capacity];
                let mut total = plugin
                    .process(
                        &input,
                        &mut output,
                        &ProcessContext::new(input_rate, input_frames),
                    )
                    .unwrap();
                for _ in 0..8 {
                    let mut tail = vec![0.0; plugin.drain_output_frames_max()];
                    let step = plugin
                        .drain(&mut tail, &ProcessContext::new(input_rate, 0))
                        .unwrap();
                    total += step.frames;
                    if step.complete {
                        break;
                    }
                }
                let expected = (input_frames as f64 * output_rate as f64 / input_rate as f64).ceil()
                    as usize
                    + plugin.output_delay_frames();
                assert_eq!(
                    total, expected,
                    "quality={quality:?}, {input_rate}->{output_rate}, input={input_frames}"
                );
                let mut empty = vec![0.0; plugin.drain_output_frames_max()];
                assert!(
                    plugin
                        .drain(&mut empty, &ProcessContext::new(input_rate, 0))
                        .unwrap()
                        .complete
                );
                assert_eq!(plugin.last_output_frames(), Some(0));
            }
        }
    }
}

#[test]
fn ratio_ramps_use_cumulative_stream_duration_when_draining() {
    let mut plugin = ResamplerPlugin::new(1, 44_100, 48_000, 64).unwrap();
    plugin.initialize(44_100).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("dynamic_ratio"),
            ParameterValue::Bool(true),
        )
        .unwrap();
    let nominal = 48_000.0 / 44_100.0;
    let mut expected = 0.0;
    let mut total = 0;
    for (frames, ratio) in [(64usize, nominal * 0.99), (31, nominal * 1.01)] {
        plugin.set_ratio(ratio, true).unwrap();
        expected += frames as f64 * ratio;
        let input = vec![0.25; frames];
        let mut output = vec![0.0; plugin.output_frames_for_input(frames)];
        total += plugin
            .process(&input, &mut output, &ProcessContext::new(44_100, frames))
            .unwrap();
    }
    let frozen_drain_delay = plugin.output_delay_frames();
    loop {
        let mut output = vec![0.0; plugin.drain_output_frames_max()];
        let step = plugin
            .drain(&mut output, &ProcessContext::new(44_100, 0))
            .unwrap();
        total += step.frames;
        if step.complete {
            break;
        }
    }
    assert_eq!(total, expected.ceil() as usize + frozen_drain_delay);
}

struct RateProbe {
    expected_rate: u32,
    tail_pending: bool,
}

impl Plugin for RateProbe {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Rate probe", "test", "SOTF")
    }
    fn input_channels(&self) -> usize {
        1
    }
    fn output_channels(&self) -> usize {
        1
    }
    fn parameters(&self) -> Vec<sotf_host::parameters::Parameter> {
        Vec::new()
    }
    fn set_parameter(&mut self, _: ParameterId, _: ParameterValue) -> PluginResult<()> {
        Err("no parameters".to_string())
    }
    fn get_parameter(&self, _: &ParameterId) -> Option<ParameterValue> {
        None
    }
    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        if sample_rate == self.expected_rate {
            Ok(())
        } else {
            Err(format!(
                "expected {}, got {sample_rate}",
                self.expected_rate
            ))
        }
    }
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        assert_eq!(context.sample_rate, self.expected_rate);
        output[..input.len()].copy_from_slice(input);
        self.tail_pending = context.num_frames > 0;
        Ok(context.num_frames)
    }
    fn drain_output_frames_max(&self) -> usize {
        usize::from(self.tail_pending)
    }
    fn drain(
        &mut self,
        output: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<PluginDrainResult> {
        assert_eq!(context.sample_rate, self.expected_rate);
        if self.tail_pending {
            output[0] = 0.125;
            self.tail_pending = false;
            Ok(PluginDrainResult {
                frames: 1,
                complete: true,
            })
        } else {
            Ok(PluginDrainResult::COMPLETE)
        }
    }
}

#[test]
fn host_negotiates_resampler_output_rate_for_downstream_nodes() {
    let mut mismatched = DawHost::new(1, 48_000);
    assert!(
        mismatched
            .add_plugin(Box::new(
                ResamplerPlugin::new(1, 44_100, 48_000, 64).unwrap()
            ))
            .is_err(),
        "rate mismatch must fail before graph activation"
    );

    let mut host = DawHost::new(1, 44_100);
    host.add_plugin(Box::new(
        ResamplerPlugin::new(1, 44_100, 48_000, 64).unwrap(),
    ))
    .unwrap();
    host.add_plugin(Box::new(RateProbe {
        expected_rate: 48_000,
        tail_pending: false,
    }))
    .unwrap();
    let input = vec![0.25; 64];
    let mut output = vec![0.0; host.output_frames_for_input(64)];
    assert!(host.process(&input, &mut output).unwrap() > 0);
    let mut tail = vec![0.0; host.drain_output_frames_max()];
    let mut tail_frames = 0;
    loop {
        let step = host.drain(&mut tail).unwrap();
        tail_frames += step.frames;
        if step.complete {
            break;
        }
    }
    assert!(
        tail_frames > 1,
        "host must emit both the resampler tail and downstream stateful tail"
    );
}
