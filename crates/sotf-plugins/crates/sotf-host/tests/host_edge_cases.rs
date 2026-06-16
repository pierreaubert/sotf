// ============================================================================
// Host Edge-Case Integration Tests
// ============================================================================
//
// Exercises the public `DawHost` / `Host` API for the four edge cases called
// out in `testing.md` for Phase 1.2:
//
//   1. Channel count changes
//   2. Variable frame sizes
//   3. Oversampling preference propagation
//   4. Panic isolation in `process`
//
// Only the public sotf-host API is used here; all stub plugins are defined
// inline so the tests remain self-contained.

use sotf_host::{
    DawHost, Parameter, ParameterId, ParameterValue, Plugin, PluginInfo, ProcessContext,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

const SAMPLE_RATE: u32 = 48_000;

// ----------------------------------------------------------------------------
// Shared test fixtures
// ----------------------------------------------------------------------------

/// Simple stereo gain plugin used as a channel-preserving link in chains.
struct GainPlugin {
    channels: usize,
    gain: f32,
}

impl GainPlugin {
    fn new(channels: usize, gain: f32) -> Self {
        Self { channels, gain }
    }
}

impl Plugin for GainPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Gain", "0.1.0", "integration-test")
    }

    fn input_channels(&self) -> usize {
        self.channels
    }

    fn output_channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![Parameter::new_float("gain", "Gain", 1.0, 0.0, 10.0)]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> Result<(), String> {
        if id.as_str() == "gain" {
            if let ParameterValue::Float(v) = value {
                self.gain = v;
                return Ok(());
            }
            return Err("gain expects a float".into());
        }
        Err(format!("unknown parameter: {}", id.as_str()))
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id.as_str() == "gain" {
            Some(ParameterValue::Float(self.gain))
        } else {
            None
        }
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        ctx: &ProcessContext,
    ) -> Result<usize, String> {
        for (out, &sample) in output.iter_mut().zip(input.iter()) {
            *out = sample * self.gain;
        }
        Ok(ctx.num_frames)
    }
}

/// Records every `ctx.num_frames` value it receives.
struct FrameRecorderPlugin {
    channels: usize,
    recordings: Arc<Mutex<Vec<usize>>>,
}

impl FrameRecorderPlugin {
    fn new(channels: usize) -> (Self, Arc<Mutex<Vec<usize>>>) {
        let recordings = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                channels,
                recordings: Arc::clone(&recordings),
            },
            recordings,
        )
    }
}

impl Plugin for FrameRecorderPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("FrameRecorder", "0.1.0", "integration-test")
    }

    fn input_channels(&self) -> usize {
        self.channels
    }

    fn output_channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        Vec::new()
    }

    fn set_parameter(&mut self, _id: ParameterId, _value: ParameterValue) -> Result<(), String> {
        Err("no parameters".into())
    }

    fn get_parameter(&self, _id: &ParameterId) -> Option<ParameterValue> {
        None
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        ctx: &ProcessContext,
    ) -> Result<usize, String> {
        self.recordings.lock().unwrap().push(ctx.num_frames);
        let len = input.len().min(output.len());
        output[..len].copy_from_slice(&input[..len]);
        Ok(ctx.num_frames)
    }
}

// ----------------------------------------------------------------------------
// 1. Channel count changes
// ----------------------------------------------------------------------------

/// Plugin whose input and output channel counts differ.
struct ChannelChangingPlugin {
    in_ch: usize,
    out_ch: usize,
}

impl Plugin for ChannelChangingPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("ChannelChanger", "0.1.0", "integration-test")
    }

    fn input_channels(&self) -> usize {
        self.in_ch
    }

    fn output_channels(&self) -> usize {
        self.out_ch
    }

    fn parameters(&self) -> Vec<Parameter> {
        Vec::new()
    }

    fn set_parameter(&mut self, _id: ParameterId, _value: ParameterValue) -> Result<(), String> {
        Err("no parameters".into())
    }

    fn get_parameter(&self, _id: &ParameterId) -> Option<ParameterValue> {
        None
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        ctx: &ProcessContext,
    ) -> Result<usize, String> {
        let frames = ctx.num_frames;
        for frame in 0..frames {
            let in_base = frame * self.in_ch;
            let out_base = frame * self.out_ch;
            if self.out_ch == self.in_ch {
                output[out_base..out_base + self.out_ch]
                    .copy_from_slice(&input[in_base..in_base + self.in_ch]);
            } else if self.out_ch > self.in_ch {
                // Upmix: duplicate the first input channel to every output channel.
                let sample = input[in_base];
                for ch in 0..self.out_ch {
                    output[out_base + ch] = sample;
                }
            } else {
                // Downmix: average input channels into the single output channel.
                let sum: f32 = input[in_base..in_base + self.in_ch].iter().sum();
                output[out_base] = sum / self.in_ch as f32;
            }
        }
        Ok(frames)
    }
}

#[test]
fn host_chain_upmixes_then_processes_stereo_output() {
    let mut host = DawHost::new(1, SAMPLE_RATE);
    host.add_plugin(Box::new(ChannelChangingPlugin {
        in_ch: 1,
        out_ch: 2,
    }))
    .unwrap();
    host.add_plugin(Box::new(GainPlugin::new(2, 2.0))).unwrap();

    let frames = 16usize;
    let input: Vec<f32> = (0..frames).map(|i| i as f32 * 0.1).collect();
    let mut output = vec![0.0_f32; frames * 2];

    let processed = host.process(&input, &mut output).unwrap();

    assert_eq!(host.input_channels(), 1);
    assert_eq!(host.output_channels(), 2);
    assert_eq!(processed, frames);
    for frame in 0..frames {
        let expected = input[frame] * 2.0;
        assert_eq!(output[frame * 2], expected);
        assert_eq!(output[frame * 2 + 1], expected);
    }
}

#[test]
fn host_chain_downmixes_then_processes_mono_output() {
    let mut host = DawHost::new(2, SAMPLE_RATE);
    host.add_plugin(Box::new(ChannelChangingPlugin {
        in_ch: 2,
        out_ch: 1,
    }))
    .unwrap();
    host.add_plugin(Box::new(GainPlugin::new(1, 3.0))).unwrap();

    let frames = 16usize;
    let input: Vec<f32> = (0..frames * 2)
        .map(|i| if i % 2 == 0 { 0.4 } else { 0.6 })
        .collect();
    let mut output = vec![0.0_f32; frames];

    let processed = host.process(&input, &mut output).unwrap();

    assert_eq!(host.input_channels(), 2);
    assert_eq!(host.output_channels(), 1);
    assert_eq!(processed, frames);
    let expected_sample = ((0.4 + 0.6) / 2.0) * 3.0;
    for &sample in &output {
        assert!((sample - expected_sample).abs() < 1e-6);
    }
}

#[test]
fn host_rejects_channel_mismatched_chain_append() {
    let mut host = DawHost::new(2, SAMPLE_RATE);
    host.add_plugin(Box::new(GainPlugin::new(2, 1.0))).unwrap();

    // A plugin expecting 5 inputs cannot follow a plugin that produces 2 outputs.
    let result = host.add_plugin(Box::new(GainPlugin::new(5, 1.0)));
    assert!(
        result.is_err(),
        "expected channel mismatch error, got {result:?}"
    );
}

// ----------------------------------------------------------------------------
// 2. Variable frame sizes
// ----------------------------------------------------------------------------

#[test]
fn host_passes_expected_num_frames_to_plugins() {
    let mut host = DawHost::new(2, SAMPLE_RATE);

    let (recorder_a, recordings_a) = FrameRecorderPlugin::new(2);
    let (recorder_b, recordings_b) = FrameRecorderPlugin::new(2);

    host.add_plugin(Box::new(recorder_a)).unwrap();
    host.add_plugin(Box::new(GainPlugin::new(2, 0.5))).unwrap();
    host.add_plugin(Box::new(recorder_b)).unwrap();

    let frame_sizes = [64usize, 127, 256, 512, 1023];
    for &frames in &frame_sizes {
        let input = vec![0.25_f32; frames * 2];
        let mut output = vec![0.0_f32; frames * 2];
        let processed = host.process(&input, &mut output).unwrap();
        assert_eq!(
            processed, frames,
            "host should return the input frame count"
        );
    }

    let a = recordings_a.lock().unwrap();
    let b = recordings_b.lock().unwrap();
    assert_eq!(
        a.as_slice(),
        &frame_sizes,
        "first plugin did not receive the expected frame counts"
    );
    assert_eq!(
        b.as_slice(),
        &frame_sizes,
        "last plugin did not receive the expected frame counts"
    );
}

// ----------------------------------------------------------------------------
// 3. Oversampling preference propagation
// ----------------------------------------------------------------------------

struct PrefersOversamplingPlugin {
    channels: usize,
    factor: u32,
}

impl Plugin for PrefersOversamplingPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("PrefersOversampling", "0.1.0", "integration-test")
    }

    fn input_channels(&self) -> usize {
        self.channels
    }

    fn output_channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        Vec::new()
    }

    fn set_parameter(&mut self, _id: ParameterId, _value: ParameterValue) -> Result<(), String> {
        Err("no parameters".into())
    }

    fn get_parameter(&self, _id: &ParameterId) -> Option<ParameterValue> {
        None
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        ctx: &ProcessContext,
    ) -> Result<usize, String> {
        output[..input.len()].copy_from_slice(input);
        Ok(ctx.num_frames)
    }

    fn preferred_oversampling(&self) -> Option<u32> {
        Some(self.factor)
    }
}

#[test]
fn host_wraps_plugin_with_preferred_oversampling_factor() {
    let mut host = DawHost::new(2, SAMPLE_RATE);
    host.add_plugin(Box::new(PrefersOversamplingPlugin {
        channels: 2,
        factor: 2,
    }))
    .unwrap();

    let plugin = host.get_plugin(0).unwrap();
    assert_eq!(plugin.info().name, "PrefersOversampling(2x)");
    // The wrapper consumes the preference; callers see no further oversampling request.
    assert_eq!(plugin.preferred_oversampling(), None);
}

#[test]
fn host_honors_disabled_oversampling_preference() {
    let mut host = DawHost::new(2, SAMPLE_RATE);
    host.set_plugin_preferred_oversampling_enabled(false);
    host.add_plugin(Box::new(PrefersOversamplingPlugin {
        channels: 2,
        factor: 4,
    }))
    .unwrap();

    let plugin = host.get_plugin(0).unwrap();
    assert_eq!(plugin.info().name, "PrefersOversampling");
    assert_eq!(plugin.preferred_oversampling(), Some(4));
}

#[test]
fn host_applies_different_oversampling_factors_to_chain_plugins() {
    let mut host = DawHost::new(2, SAMPLE_RATE);
    host.add_plugin(Box::new(PrefersOversamplingPlugin {
        channels: 2,
        factor: 2,
    }))
    .unwrap();
    host.add_plugin(Box::new(PrefersOversamplingPlugin {
        channels: 2,
        factor: 4,
    }))
    .unwrap();

    let first = host.get_plugin(0).unwrap();
    let second = host.get_plugin(1).unwrap();
    assert_eq!(first.info().name, "PrefersOversampling(2x)");
    assert_eq!(second.info().name, "PrefersOversampling(4x)");

    // The chain remains processable after wrapping.
    let frames = 32usize;
    let input = vec![0.5_f32; frames * 2];
    let mut output = vec![0.0_f32; frames * 2];
    let processed = host.process(&input, &mut output).unwrap();
    assert_eq!(processed, frames);
}

// ----------------------------------------------------------------------------
// 4. Panic isolation
// ----------------------------------------------------------------------------

/// Plugin that panics in `process` when its `panic` flag is true.
///
/// The host wraps plugin calls in `catch_unwind(AssertUnwindSafe(...))`, so the
/// plugin itself does not need to be `UnwindSafe`. This stub uses an
/// `AtomicBool` so the flag can be toggled from the test thread after the
/// plugin has been handed to the host.
struct ConditionalPanicPlugin {
    channels: usize,
    gain: f32,
    panic_flag: Arc<AtomicBool>,
}

impl ConditionalPanicPlugin {
    fn new(channels: usize, gain: f32) -> (Self, Arc<AtomicBool>) {
        let panic_flag = Arc::new(AtomicBool::new(false));
        (
            Self {
                channels,
                gain,
                panic_flag: Arc::clone(&panic_flag),
            },
            panic_flag,
        )
    }
}

impl Plugin for ConditionalPanicPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("ConditionalPanic", "0.1.0", "integration-test")
    }

    fn input_channels(&self) -> usize {
        self.channels
    }

    fn output_channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_float("gain", "Gain", 1.0, 0.0, 10.0),
            Parameter::new_bool("panic", "Panic", false),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> Result<(), String> {
        match id.as_str() {
            "gain" => {
                if let ParameterValue::Float(v) = value {
                    self.gain = v;
                    Ok(())
                } else {
                    Err("gain expects a float".into())
                }
            }
            "panic" => {
                if let ParameterValue::Bool(v) = value {
                    self.panic_flag.store(v, Ordering::SeqCst);
                    Ok(())
                } else {
                    Err("panic expects a bool".into())
                }
            }
            _ => Err(format!("unknown parameter: {}", id.as_str())),
        }
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        match id.as_str() {
            "gain" => Some(ParameterValue::Float(self.gain)),
            "panic" => Some(ParameterValue::Bool(self.panic_flag.load(Ordering::SeqCst))),
            _ => None,
        }
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        ctx: &ProcessContext,
    ) -> Result<usize, String> {
        if self.panic_flag.load(Ordering::SeqCst) {
            panic!("simulated plugin process panic");
        }
        for (out, &sample) in output.iter_mut().zip(input.iter()) {
            *out = sample * self.gain;
        }
        Ok(ctx.num_frames)
    }
}

#[test]
fn host_catches_conditional_process_panic_without_aborting() {
    let mut host = DawHost::new(2, SAMPLE_RATE);
    let (plugin, flag) = ConditionalPanicPlugin::new(2, 2.0);
    host.add_plugin(Box::new(plugin)).unwrap();

    let input = vec![0.25_f32, -0.5, 1.0, -1.0];
    let mut output = vec![0.0_f32; input.len()];

    // Normal processing: plugin applies its gain.
    let frames = host.process(&input, &mut output).unwrap();
    assert_eq!(frames, 2);
    for (out, inp) in output.iter().zip(input.iter()) {
        assert!((out - inp * 2.0).abs() < 1e-6);
    }

    // Trigger the panic. The host catches it internally and falls back to a
    // passthrough for this block; `process` returns Ok rather than propagating
    // the panic out of the host.
    host.set_plugin_parameter_immediate(0, "panic", ParameterValue::Bool(true))
        .unwrap();

    let mut output_after_panic = vec![0.0_f32; input.len()];
    let frames_after_panic = host.process(&input, &mut output_after_panic).unwrap();
    assert_eq!(frames_after_panic, 2);
    assert_eq!(output_after_panic, input);

    // The plugin can recover: disable the panic flag and process normally again.
    host.set_plugin_parameter_immediate(0, "panic", ParameterValue::Bool(false))
        .unwrap();

    let mut output_recovered = vec![0.0_f32; input.len()];
    let frames_recovered = host.process(&input, &mut output_recovered).unwrap();
    assert_eq!(frames_recovered, 2);
    for (out, inp) in output_recovered.iter().zip(input.iter()) {
        assert!((out - inp * 2.0).abs() < 1e-6);
    }

    // Keep the flag alive until the end of the test to avoid a dangling Arc.
    assert!(!flag.load(Ordering::SeqCst));
}

#[test]
fn host_catches_f64_process_panic_without_aborting() {
    struct PanicF64Plugin {
        channels: usize,
    }

    impl Plugin for PanicF64Plugin {
        fn info(&self) -> PluginInfo {
            PluginInfo::new("PanicF64", "0.1.0", "integration-test")
        }
        fn input_channels(&self) -> usize {
            self.channels
        }
        fn output_channels(&self) -> usize {
            self.channels
        }
        fn parameters(&self) -> Vec<Parameter> {
            Vec::new()
        }
        fn set_parameter(
            &mut self,
            _id: ParameterId,
            _value: ParameterValue,
        ) -> Result<(), String> {
            Err("no parameters".into())
        }
        fn get_parameter(&self, _id: &ParameterId) -> Option<ParameterValue> {
            None
        }
        fn process(
            &mut self,
            _input: &[f32],
            _output: &mut [f32],
            _ctx: &ProcessContext,
        ) -> Result<usize, String> {
            Ok(2)
        }
        fn process_f64(
            &mut self,
            _input: &[f64],
            _output: &mut [f64],
            _ctx: &ProcessContext,
        ) -> Result<usize, String> {
            panic!("simulated f64 process panic");
        }
        fn supports_f64(&self) -> bool {
            true
        }
    }

    let mut host = DawHost::new(2, SAMPLE_RATE);
    host.add_plugin(Box::new(PanicF64Plugin { channels: 2 }))
        .unwrap();

    let input = vec![0.25_f64, -0.5, 1.0, -1.0];
    let mut output = vec![0.0_f64; input.len()];

    let frames = host.process_f64(&input, &mut output).unwrap();
    assert_eq!(frames, 2);
    assert_eq!(output, input);
}
