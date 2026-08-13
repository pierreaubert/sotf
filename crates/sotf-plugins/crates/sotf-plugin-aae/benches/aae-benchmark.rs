// AAE plugin benchmarks
//
// Measures performance of the Active Acoustic Enhancement plugin under
// realistic workloads: various speaker configs, room presets, RT60 values,
// and block sizes. Uses broadband multi-sine input to exercise all code
// paths (FDN, early reflections, diffusion, VBAP routing).

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use sotf_host::{Plugin, ProcessContext};
use sotf_plugin_aae::{AaePlugin, params::AaePluginParams};
use std::hint::black_box;
use std::time::Duration;

/// Generate a deterministic broadband stereo signal.
///
/// Sum of log-spaced sinusoids (40Hz–16kHz) with different phase offsets
/// per channel for non-unity coherence.
fn generate_realistic_input(block_size: usize, sample_rate: u32) -> Vec<f32> {
    let sr = sample_rate as f64;
    let freqs: Vec<f64> = (0..20)
        .map(|i| 40.0 * (400.0_f64).powf(i as f64 / 19.0))
        .collect();

    let mut output = vec![0.0f32; block_size * 2];
    let two_pi = std::f64::consts::TAU;

    for (tone_idx, &freq) in freqs.iter().enumerate() {
        let phase_l = tone_idx as f64 * 0.37;
        let phase_r = tone_idx as f64 * 0.37 + 0.7 + tone_idx as f64 * 0.13;
        let amplitude = 0.3 / (1.0 + (freq / 200.0).ln().max(0.0));

        for i in 0..block_size {
            let t = i as f64 / sr;
            let l = amplitude * (two_pi * freq * t + phase_l).sin();
            let r = amplitude * (two_pi * freq * t + phase_r).sin();
            output[i * 2] += l as f32;
            output[i * 2 + 1] += r as f32;
        }
    }

    output
}

fn warmup_plugin(
    plugin: &mut AaePlugin,
    input: &[f32],
    output: &mut [f32],
    context: &ProcessContext,
    iterations: usize,
) {
    for _ in 0..iterations {
        plugin.process(input, output, context).unwrap();
    }
}

/// Benchmark 5.1 processing across block sizes and sample rates.
fn bench_aae_block_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("aae_5_1_block_sizes");
    group.warm_up_time(Duration::from_secs(2));

    let sample_rates = [44_100u32, 48_000, 96_000];

    for &sample_rate in &sample_rates {
        for &block_size in &[256usize, 512, 1024, 2048] {
            group.throughput(Throughput::Elements((block_size * 2) as u64));

            group.bench_with_input(
                BenchmarkId::new(format!("{}Hz", sample_rate), block_size),
                &block_size,
                |b, &block_size| {
                    let mut plugin = AaePlugin::from_params(AaePluginParams::default()).unwrap();
                    plugin.initialize(sample_rate).unwrap();

                    let input = generate_realistic_input(block_size, sample_rate);
                    let mut output = vec![0.0f32; block_size * plugin.output_channels()];
                    let context = ProcessContext::new(sample_rate, block_size);

                    warmup_plugin(&mut plugin, &input, &mut output, &context, 8);

                    b.iter(|| {
                        plugin
                            .process(
                                black_box(&input),
                                black_box(&mut output),
                                black_box(&context),
                            )
                            .unwrap();
                    });
                },
            );
        }
    }

    group.finish();
}

/// Benchmark scaling across speaker configurations.
fn bench_aae_configs(c: &mut Criterion) {
    let mut group = c.benchmark_group("aae_configs");
    group.warm_up_time(Duration::from_secs(2));

    let sample_rate = 48_000;
    let block_size = 512;
    let configs = ["5.0", "5.1", "7.1", "7.1.4", "9.1.6"];

    group.throughput(Throughput::Elements((block_size * 2) as u64));

    for &config in &configs {
        group.bench_with_input(BenchmarkId::from_parameter(config), &config, |b, &cfg| {
            let params = AaePluginParams {
                speaker_config: cfg.to_string(),
                ..AaePluginParams::default()
            };
            let mut plugin = AaePlugin::from_params(params).unwrap();
            plugin.initialize(sample_rate).unwrap();

            let input = generate_realistic_input(block_size, sample_rate);
            let mut output = vec![0.0f32; block_size * plugin.output_channels()];
            let context = ProcessContext::new(sample_rate, block_size);

            warmup_plugin(&mut plugin, &input, &mut output, &context, 8);

            b.iter(|| {
                plugin
                    .process(
                        black_box(&input),
                        black_box(&mut output),
                        black_box(&context),
                    )
                    .unwrap();
            });
        });
    }

    group.finish();
}

/// Benchmark room presets (different ER tap counts: 12–20).
fn bench_aae_room_presets(c: &mut Criterion) {
    let mut group = c.benchmark_group("aae_room_presets");
    group.warm_up_time(Duration::from_secs(2));

    let sample_rate = 48_000;
    let block_size = 512;
    let presets = ["small", "medium", "large", "cathedral"];

    group.throughput(Throughput::Elements((block_size * 2) as u64));

    for &preset in &presets {
        group.bench_with_input(
            BenchmarkId::from_parameter(preset),
            &preset,
            |b, &preset| {
                let params = AaePluginParams {
                    room_preset: preset.to_string(),
                    ..AaePluginParams::default()
                };
                let mut plugin = AaePlugin::from_params(params).unwrap();
                plugin.initialize(sample_rate).unwrap();

                let input = generate_realistic_input(block_size, sample_rate);
                let mut output = vec![0.0f32; block_size * plugin.output_channels()];
                let context = ProcessContext::new(sample_rate, block_size);

                warmup_plugin(&mut plugin, &input, &mut output, &context, 8);

                b.iter(|| {
                    plugin
                        .process(
                            black_box(&input),
                            black_box(&mut output),
                            black_box(&context),
                        )
                        .unwrap();
                });
            },
        );
    }

    group.finish();
}

/// Benchmark production config with large block (exercises sustained processing).
fn bench_aae_production(c: &mut Criterion) {
    let mut group = c.benchmark_group("aae_production");
    group.warm_up_time(Duration::from_secs(2));

    let sample_rate = 48_000;
    let block_size = 10240; // ~213ms of audio

    group.throughput(Throughput::Elements((block_size * 2) as u64));

    let configs = ["5.1", "7.1.4", "9.1.6"];
    for &config in &configs {
        group.bench_with_input(BenchmarkId::from_parameter(config), &config, |b, &cfg| {
            let params = AaePluginParams {
                speaker_config: cfg.to_string(),
                room_preset: "cathedral".to_string(),
                rt60: 3.0,
                mod_depth: 0.7,
                ..AaePluginParams::default()
            };
            let mut plugin = AaePlugin::from_params(params).unwrap();
            plugin.initialize(sample_rate).unwrap();

            let input = generate_realistic_input(block_size, sample_rate);
            let mut output = vec![0.0f32; block_size * plugin.output_channels()];
            let context = ProcessContext::new(sample_rate, block_size);

            warmup_plugin(&mut plugin, &input, &mut output, &context, 4);

            b.iter(|| {
                plugin
                    .process(
                        black_box(&input),
                        black_box(&mut output),
                        black_box(&context),
                    )
                    .unwrap();
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_aae_block_sizes,
    bench_aae_configs,
    bench_aae_room_presets,
    bench_aae_production,
);

criterion_main!(benches);
