// Upmixer plugin benchmarks
//
// This benchmark suite measures performance of the stereo-to-surround
// UpmixerPlugin under realistic workloads (5.1, 7.1.4) and various
// block sizes and FFT sizes.
//
// Uses broadband multi-sine input to exercise all code paths (energy
// preservation, coherence computation, transient detection, HR overlay).

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use sotf_host::{Plugin, ProcessContext};
use sotf_plugin_upmixer::UpmixerPlugin;
use std::time::Duration;

/// Generate a deterministic broadband stereo signal that exercises all upmixer code paths.
///
/// Uses a sum of log-spaced sinusoids (40Hz–16kHz) with different phase offsets
/// per channel to produce non-unity coherence across the spectrum.
fn generate_realistic_input(
    block_size: usize,
    sample_rate: u32,
    include_transients: bool,
) -> Vec<f32> {
    let sr = sample_rate as f64;
    // Log-spaced frequencies from 40Hz to 16kHz (20 tones)
    let freqs: Vec<f64> = (0..20)
        .map(|i| 40.0 * (400.0_f64).powf(i as f64 / 19.0))
        .collect();

    let mut output = vec![0.0f32; block_size * 2];
    let two_pi = std::f64::consts::TAU;

    for (tone_idx, &freq) in freqs.iter().enumerate() {
        // Different phase offset per channel to create inter-channel decorrelation
        let phase_l = tone_idx as f64 * 0.37;
        let phase_r = tone_idx as f64 * 0.37 + 0.7 + tone_idx as f64 * 0.13;
        // Amplitude decreases with frequency (pink-ish spectrum)
        let amplitude = 0.3 / (1.0 + (freq / 200.0).ln().max(0.0));

        for i in 0..block_size {
            let t = i as f64 / sr;
            let l = amplitude * (two_pi * freq * t + phase_l).sin();
            let r = amplitude * (two_pi * freq * t + phase_r).sin();
            output[i * 2] += l as f32;
            output[i * 2 + 1] += r as f32;
        }
    }

    if include_transients && block_size >= 10 {
        // Add sharp transients every 1024 samples to trigger spectral flux based HR path
        for block_start in (0..block_size).step_by(1024) {
            for i in 0..5 {
                let idx = block_start + i;
                if idx < block_size {
                    output[idx * 2] += 5.0;
                    output[idx * 2 + 1] += 5.0;
                }
            }
        }
    }

    output
}

/// Run warmup iterations to prime plugin state (PCA covariance, coherence
/// history, spectral flux baseline) before measurement begins.
fn warmup_plugin(
    upmixer: &mut UpmixerPlugin,
    input: &[f32],
    output: &mut [f32],
    context: &ProcessContext,
    iterations: usize,
) {
    for _ in 0..iterations {
        upmixer.process(input, output, context).unwrap();
    }
}

/// Create an upmixer with typical parameters for benchmarking
fn create_upmixer(fft_size: usize, speaker_config: &str) -> UpmixerPlugin {
    UpmixerPlugin::new(
        fft_size,
        speaker_config,
        1.0,   // gain_front_direct
        0.5,   // gain_front_ambient
        1.0,   // gain_rear_ambient
        120.0, // lfe_cutoff_hz
        0.5,   // stereo_width
        250.0, // bandpass_hz
        1.0,   // height_gain
        1.0,   // lfe_gain
        false, // enable_subharmonic_synth
        0.5,   // subharmonic_gain
    )
}

/// Benchmark processing for 5.1 configuration over various block sizes
fn bench_upmixer_5_1_block_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("upmixer_5_1_block_sizes");
    group.warm_up_time(Duration::from_secs(3));

    let sample_rates = [44_100u32, 48_000, 96_000, 192_000];
    let fft_size = 2048;

    for &sample_rate in &sample_rates {
        for &block_size in &[256usize, 512, 1024, 2048] {
            // Throughput in input samples (stereo)
            group.throughput(Throughput::Elements((block_size * 2) as u64));

            group.bench_with_input(
                BenchmarkId::new(format!("{}Hz", sample_rate), block_size),
                &block_size,
                |b, &block_size| {
                    let mut upmixer = create_upmixer(fft_size, "5.1");
                    upmixer.initialize(sample_rate).unwrap();

                    let input = generate_realistic_input(block_size, sample_rate, true);
                    let mut output = vec![0.0f32; block_size * upmixer.output_channels()];
                    let context = ProcessContext {
                        num_frames: block_size,
                        sample_rate,
                    };

                    // Warmup to prime PCA/coherence/spectral flux state
                    warmup_plugin(&mut upmixer, &input, &mut output, &context, 8);

                    b.iter(|| {
                        upmixer
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

/// Benchmark scaling with different speaker configurations at fixed block size
fn bench_upmixer_configs(c: &mut Criterion) {
    let mut group = c.benchmark_group("upmixer_configs");
    group.warm_up_time(Duration::from_secs(3));

    let sample_rate = 48_000;
    let block_size = 512;
    let fft_size = 2048;

    // Stereo input -> various surround layouts
    let configs = ["2.0", "5.1", "7.1.4", "9.1.6"];

    group.throughput(Throughput::Elements((block_size * 2) as u64));

    for &config in &configs {
        group.bench_with_input(BenchmarkId::from_parameter(config), &config, |b, &cfg| {
            let mut upmixer = create_upmixer(fft_size, cfg);
            upmixer.initialize(sample_rate).unwrap();

            let input = generate_realistic_input(block_size, sample_rate, true);
            let mut output = vec![0.0f32; block_size * upmixer.output_channels()];
            let context = ProcessContext {
                num_frames: block_size,
                sample_rate,
            };

            warmup_plugin(&mut upmixer, &input, &mut output, &context, 8);

            b.iter(|| {
                upmixer
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

/// Benchmark impact of FFT size on 5.1 upmixing performance
fn bench_upmixer_fft_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("upmixer_fft_sizes");
    group.warm_up_time(Duration::from_secs(3));

    let sample_rate = 48_000;
    let block_size = 512;
    let fft_sizes = [1024usize, 2048, 4096];

    group.throughput(Throughput::Elements((block_size * 2) as u64));

    for &fft_size in &fft_sizes {
        group.bench_with_input(
            BenchmarkId::from_parameter(fft_size),
            &fft_size,
            |b, &fft_size| {
                let mut upmixer = create_upmixer(fft_size, "5.1");
                upmixer.initialize(sample_rate).unwrap();
                let input = generate_realistic_input(block_size, sample_rate, true);
                let mut output = vec![0.0f32; block_size * upmixer.output_channels()];
                let context = ProcessContext {
                    num_frames: block_size,
                    sample_rate,
                };

                warmup_plugin(&mut upmixer, &input, &mut output, &context, 8);

                b.iter(|| {
                    upmixer
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

/// Benchmark with production default configuration (from_params defaults)
fn bench_upmixer_production_config(c: &mut Criterion) {
    let mut group = c.benchmark_group("upmixer_production_config");
    group.warm_up_time(Duration::from_secs(3));

    let sample_rate = 48_000;
    let block_size = 10240; // Process 10240 samples to trigger ~10 FFT blocks

    group.throughput(Throughput::Elements((block_size * 2) as u64));

    let configs = ["5.1", "7.1.4", "9.1.6"];
    for &config in &configs {
        group.bench_with_input(BenchmarkId::from_parameter(config), &config, |b, &cfg| {
            use sotf_plugin_upmixer::UpmixerPluginParams;
            let params = UpmixerPluginParams {
                speaker_config: cfg.to_string(),
                ..serde_json::from_str("{}").unwrap()
            };
            let mut upmixer = UpmixerPlugin::from_params(params);
            upmixer.initialize(sample_rate).unwrap();

            let input = generate_realistic_input(block_size, sample_rate, true);
            let mut output = vec![0.0f32; block_size * upmixer.output_channels()];
            let context = ProcessContext {
                num_frames: block_size,
                sample_rate,
            };

            warmup_plugin(&mut upmixer, &input, &mut output, &context, 8);

            b.iter(|| {
                upmixer
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
    bench_upmixer_5_1_block_sizes,
    bench_upmixer_configs,
    bench_upmixer_fft_sizes,
    bench_upmixer_production_config,
);

criterion_main!(benches);
