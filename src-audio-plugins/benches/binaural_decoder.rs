// Benchmarks for binaural decoder plugin
//
// This benchmark suite measures performance of the binaural decoder plugin
// under various configurations and workloads.

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use sotf_plugins::{BinauralDecoderPlugin, Plugin, ProcessContext, RoomModel};
use std::time::Duration;

/// Create a binaural decoder for benchmarking
fn create_decoder(
    input_channels: usize,
    fft_size: usize,
    optimization: bool,
) -> BinauralDecoderPlugin {
    BinauralDecoderPlugin::new(
        input_channels,
        fft_size,
        None, // No SOFA file for basic benchmarks
        optimization,
        0.0, // No externalization
        0.0, // No near-field
        false, // No diffuse-field EQ (for performance benchmarking)
        120.0, 2.0, 0.0, // LFE defaults
        RoomModel::default(),
    )
}

/// Benchmark audio processing with different channel configurations
fn bench_process_channels(c: &mut Criterion) {
    let mut group = c.benchmark_group("binaural_process_channels");
    group.warm_up_time(Duration::from_secs(3)); // Warm up to stabilize SIMD and cache

    let sample_rate = 48000;
    let block_size = 512;
    let fft_size = 2048;

    for &channels in &[2, 5, 6, 8] {
        group.throughput(Throughput::Elements((block_size * channels) as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}ch", channels)),
            &channels,
            |b, &channels| {
                let mut decoder = create_decoder(channels, fft_size, true);
                decoder.initialize(sample_rate).unwrap();

                let input = vec![0.5f32; block_size * channels];
                let mut output = vec![0.0f32; block_size * 2];
                let context = ProcessContext {
                    num_frames: block_size,
                    sample_rate,
                };

                b.iter(|| {
                    decoder
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

/// Benchmark different FFT sizes
fn bench_process_fft_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("binaural_fft_sizes");
    group.warm_up_time(Duration::from_secs(3)); // Warm up to stabilize SIMD and cache

    let sample_rate = 48000;
    let block_size = 512;
    let channels = 6; // 5.1 surround

    for &fft_size in &[512, 1024, 2048, 4096] {
        group.throughput(Throughput::Elements((block_size * channels) as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(fft_size),
            &fft_size,
            |b, &fft_size| {
                let mut decoder = create_decoder(channels, fft_size, true);
                decoder.initialize(sample_rate).unwrap();

                let input = vec![0.5f32; block_size * channels];
                let mut output = vec![0.0f32; block_size * 2];
                let context = ProcessContext {
                    num_frames: block_size,
                    sample_rate,
                };

                b.iter(|| {
                    decoder
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

/// Benchmark optimization enabled vs disabled
fn bench_optimization_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("binaural_optimization");
    group.warm_up_time(Duration::from_secs(3)); // Warm up to stabilize SIMD and cache

    let sample_rate = 48000;
    let block_size = 512;
    let fft_size = 2048;
    let channels = 6; // 5.1 surround

    group.throughput(Throughput::Elements((block_size * channels) as u64));

    for &enabled in &[false, true] {
        let label = if enabled { "optimized" } else { "standard" };

        group.bench_with_input(
            BenchmarkId::from_parameter(label),
            &enabled,
            |b, &enabled| {
                let mut decoder = create_decoder(channels, fft_size, enabled);
                decoder.initialize(sample_rate).unwrap();

                let input = vec![0.5f32; block_size * channels];
                let mut output = vec![0.0f32; block_size * 2];
                let context = ProcessContext {
                    num_frames: block_size,
                    sample_rate,
                };

                b.iter(|| {
                    decoder
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

/// Benchmark externalization effect overhead
fn bench_externalization(c: &mut Criterion) {
    let mut group = c.benchmark_group("binaural_externalization");
    group.warm_up_time(Duration::from_secs(3)); // Warm up to stabilize SIMD and cache

    let sample_rate = 48000;
    let block_size = 512;
    let fft_size = 2048;
    let channels = 6;

    for &ext_level in &[0.0, 0.5, 1.0] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{:.1}", ext_level)),
            &ext_level,
            |b, &ext_level| {
                let mut decoder =
                    BinauralDecoderPlugin::new(channels, fft_size, None, true, ext_level, 0.0);
                decoder.initialize(sample_rate).unwrap();

                let input = vec![0.5f32; block_size * channels];
                let mut output = vec![0.0f32; block_size * 2];
                let context = ProcessContext {
                    num_frames: block_size,
                    sample_rate,
                };

                b.iter(|| {
                    decoder
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

/// Benchmark large block sizes (stress test)
fn bench_large_blocks(c: &mut Criterion) {
    let mut group = c.benchmark_group("binaural_large_blocks");
    group.sample_size(20); // Fewer samples for large blocks
    group.measurement_time(Duration::from_secs(10));
    group.warm_up_time(Duration::from_secs(3)); // Warm up to stabilize SIMD and cache

    let sample_rate = 48000;
    let fft_size = 2048;
    let channels = 6;

    for &block_size in &[512, 1024, 2048, 4096] {
        group.throughput(Throughput::Elements((block_size * channels) as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}frames", block_size)),
            &block_size,
            |b, &block_size| {
                let mut decoder = create_decoder(channels, fft_size, true);
                decoder.initialize(sample_rate).unwrap();

                let input = vec![0.5f32; block_size * channels];
                let mut output = vec![0.0f32; block_size * 2];
                let context = ProcessContext {
                    num_frames: block_size,
                    sample_rate,
                };

                b.iter(|| {
                    decoder
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

/// Benchmark passthrough mode (no SOFA)
fn bench_passthrough(c: &mut Criterion) {
    let mut group = c.benchmark_group("binaural_passthrough");
    group.warm_up_time(Duration::from_secs(3)); // Warm up to stabilize SIMD and cache

    let sample_rate = 48000;
    let block_size = 512;
    let fft_size = 2048;

    for &channels in &[1, 2, 6] {
        group.throughput(Throughput::Elements((block_size * channels) as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}ch", channels)),
            &channels,
            |b, &channels| {
                let mut decoder = create_decoder(channels, fft_size, true);
                decoder.initialize(sample_rate).unwrap();

                let input = vec![0.5f32; block_size * channels];
                let mut output = vec![0.0f32; block_size * 2];
                let context = ProcessContext {
                    num_frames: block_size,
                    sample_rate,
                };

                b.iter(|| {
                    decoder
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

/// Benchmark realistic Dolby Atmos 7.1.4 workload
fn bench_atmos_7_1_4(c: &mut Criterion) {
    let mut group = c.benchmark_group("binaural_atmos_7_1_4");
    group.sample_size(30);
    group.warm_up_time(Duration::from_secs(3)); // Warm up to stabilize SIMD and cache

    let sample_rate = 48000;
    let block_size = 512;
    let fft_size = 2048;
    let channels = 12; // 7.1.4 Atmos

    group.throughput(Throughput::Elements((block_size * channels) as u64));

    group.bench_function("with_externalization", |b| {
        let mut decoder = BinauralDecoderPlugin::new(
            channels, fft_size, None, true, 0.3, // Moderate externalization
            0.5, // Some near-field
        );
        decoder.initialize(sample_rate).unwrap();

        let input = vec![0.5f32; block_size * channels];
        let mut output = vec![0.0f32; block_size * 2];
        let context = ProcessContext {
            num_frames: block_size,
            sample_rate,
        };

        b.iter(|| {
            decoder
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
    bench_process_channels,
    bench_process_fft_sizes,
    bench_optimization_comparison,
    bench_externalization,
    bench_large_blocks,
    bench_passthrough,
    bench_atmos_7_1_4,
);

criterion_main!(benches);
