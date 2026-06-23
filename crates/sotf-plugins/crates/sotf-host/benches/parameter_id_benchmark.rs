// Benchmark for ParameterId clone cost and automation-event application.

use criterion::{Criterion, criterion_group, criterion_main};
use sotf_host::{Parameter, ParameterId, ParameterValue, Plugin, PluginInfo, ProcessContext};
use std::hint::black_box;

/// Minimal gain plugin for benchmarking the automation path.
struct GainPlugin {
    channels: usize,
    gain: f32,
}

impl GainPlugin {
    fn new(channels: usize, initial_gain: f32) -> Self {
        Self {
            channels,
            gain: initial_gain,
        }
    }
}

impl Plugin for GainPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Gain", "0.1", "bench")
    }
    fn input_channels(&self) -> usize {
        self.channels
    }
    fn output_channels(&self) -> usize {
        self.channels
    }
    fn parameters(&self) -> Vec<Parameter> {
        vec![Parameter::new_float("gain", "Gain", 1.0, 0.0, 4.0)]
    }
    fn set_parameter(&mut self, id: ParameterId, val: ParameterValue) -> Result<(), String> {
        if id.as_str() == "gain"
            && let ParameterValue::Float(v) = val
        {
            self.gain = v;
            return Ok(());
        }
        Err(format!("unknown parameter: {id}"))
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
        for (o, &i) in output.iter_mut().zip(input.iter()) {
            *o = i * self.gain;
        }
        Ok(ctx.num_frames)
    }
}

fn benchmark_parameter_id_clone(c: &mut Criterion) {
    let mut group = c.benchmark_group("parameter_id");

    let id = ParameterId::from("band_0_gain_db");
    let s = "band_0_gain_db".to_string();

    group.bench_function("clone_arc_str", |b| {
        b.iter(|| black_box(id.clone()));
    });

    group.bench_function("clone_string_baseline", |b| {
        b.iter(|| black_box(s.clone()));
    });

    group.finish();
}

fn benchmark_automation_event(c: &mut Criterion) {
    let mut group = c.benchmark_group("automation_event");

    let sample_rate = 48_000;
    let block_size = 64;
    let mut plugin = GainPlugin::new(2, 0.0);
    plugin.initialize(sample_rate).unwrap();

    let id = ParameterId::from("gain");
    let input = vec![0.5f32; block_size * 2];
    let mut output = vec![0.0f32; block_size * 2];
    let context = ProcessContext::new(sample_rate, block_size);

    group.bench_function("set_parameter_and_process", |b| {
        b.iter(|| {
            plugin
                .set_parameter(black_box(id.clone()), ParameterValue::Float(-12.0))
                .unwrap();
            plugin
                .process(
                    black_box(&input),
                    black_box(&mut output),
                    black_box(&context),
                )
                .unwrap();
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_parameter_id_clone,
    benchmark_automation_event
);
criterion_main!(benches);
