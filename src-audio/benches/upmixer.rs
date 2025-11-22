// Upmixer plugin benchmarks
//
// This benchmark suite measures performance of the stereo-to-surround
// UpmixerPlugin under realistic workloads (5.1, 7.1.4) and various
// block sizes and FFT sizes.

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use sotf_audio::{Plugin, ProcessContext, UpmixerPlugin};
use std::time::Duration;

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

                    let input = vec![0.5f32; block_size * 2];
                    let mut output =
                        vec![0.0f32; block_size * upmixer.output_channels()];
                    let context = ProcessContext {
                        num_frames: block_size,
                        sample_rate,
                    };

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
        group.bench_with_input(
            BenchmarkId::from_parameter(config),
            &config,
            |b, &cfg| {
                let mut upmixer = create_upmixer(fft_size, cfg);
                upmixer.initialize(sample_rate).unwrap();

                let input = vec![0.5f32; block_size * 2];
                let mut output = vec![0.0f32; block_size * upmixer.output_channels()];
                let context = ProcessContext {
                    num_frames: block_size,
                    sample_rate,
                };

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

                let input = vec![0.5f32; block_size * 2];
                let mut output = vec![0.0f32; block_size * upmixer.output_channels()];
                let context = ProcessContext {
                    num_frames: block_size,
                    sample_rate,
                };

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

criterion_group!(
    benches,
    bench_upmixer_5_1_block_sizes,
    bench_upmixer_configs,
    bench_upmixer_fft_sizes,
);

criterion_main!(benches);
