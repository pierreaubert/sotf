// Benchmarks for binaural decoder plugin
//
// This benchmark suite measures performance of the binaural decoder plugin
// under various configurations and workloads.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use sotf_host::{ParameterId, ParameterValue, Plugin, ProcessContext};
use sotf_plugin_binaural::{BinauralDecoderPlugin, RoomModel};
use std::hint::black_box;
use std::time::Duration;

/// Create a binaural decoder for benchmarking
fn create_decoder(input_channels: usize, fft_size: usize) -> BinauralDecoderPlugin {
    BinauralDecoderPlugin::new(
        input_channels,
        fft_size,
        None,  // No SOFA file for basic benchmarks
        0.0,   // No externalization
        0.0,   // No near-field
        false, // No diffuse-field EQ (for performance benchmarking)
        120.0,
        2.0,
        0.0, // LFE defaults
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
                let mut decoder = create_decoder(channels, fft_size);
                decoder.initialize(sample_rate).unwrap();

                let input = vec![0.5f32; block_size * channels];
                let mut output = vec![0.0f32; block_size * 2];
                let context = ProcessContext::new(sample_rate, block_size);

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
                let mut decoder = create_decoder(channels, fft_size);
                decoder.initialize(sample_rate).unwrap();

                let input = vec![0.5f32; block_size * channels];
                let mut output = vec![0.0f32; block_size * 2];
                let context = ProcessContext::new(sample_rate, block_size);

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
                let mut decoder = BinauralDecoderPlugin::new(
                    channels,
                    fft_size,
                    None,
                    ext_level,
                    0.0,
                    false,
                    120.0,
                    2.0,
                    0.0,
                    RoomModel::default(),
                );
                decoder.initialize(sample_rate).unwrap();

                let input = vec![0.5f32; block_size * channels];
                let mut output = vec![0.0f32; block_size * 2];
                let context = ProcessContext::new(sample_rate, block_size);

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
                let mut decoder = create_decoder(channels, fft_size);
                decoder.initialize(sample_rate).unwrap();

                let input = vec![0.5f32; block_size * channels];
                let mut output = vec![0.0f32; block_size * 2];
                let context = ProcessContext::new(sample_rate, block_size);

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

/// Benchmark head-tracking induced HRTF recomputation.
fn bench_head_tracking(c: &mut Criterion) {
    let mut group = c.benchmark_group("binaural_head_tracking");
    group.warm_up_time(Duration::from_secs(3));

    let sample_rate = 48000;
    let block_size = 512;
    let fft_size = 2048;
    let channels = 6;

    group.throughput(Throughput::Elements((block_size * channels) as u64));

    // Use the MIT KEMAR SOFA file shipped with the project so the benchmark
    // actually exercises head-tracking HRTF recomputation.
    let manifest_dir = std::path::PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string()),
    );
    let sofa_path = manifest_dir
        .ancestors()
        .nth(4)
        .map(|p| p.join("data_cached/org.sofacoustics/mit/kemar_large.sofa"))
        .unwrap_or_default();
    if !sofa_path.exists() {
        eprintln!(
            "Skipping binaural_head_tracking benchmark: SOFA file not found at {}",
            sofa_path.display()
        );
        group.finish();
        return;
    }

    group.bench_function("process_after_yaw_change", |b| {
        let mut decoder = create_decoder(channels, fft_size);
        decoder.initialize(sample_rate).unwrap();
        decoder
            .set_parameter(
                ParameterId::from("hrtf_file"),
                ParameterValue::String(sofa_path.to_string_lossy().to_string()),
            )
            .unwrap();

        let input = vec![0.5f32; block_size * channels];
        let mut output = vec![0.0f32; block_size * 2];
        let context = ProcessContext::new(sample_rate, block_size);

        // Steady-state warm-up.
        for _ in 0..10 {
            decoder.process(&input, &mut output, &context).unwrap();
        }

        // Alternate yaw between two large angles on every measured iteration so
        // every process() call observes an angle change > 0.5° and must enqueue
        // a background HRTF recompute. set_parameter for yaw is a simple target
        // update and is included in the measurement as part of the real-time path.
        let mut toggle = false;
        b.iter(|| {
            toggle = !toggle;
            let yaw = if toggle { 45.0 } else { -45.0 };
            decoder
                .set_parameter(
                    ParameterId::from("head_yaw_deg"),
                    ParameterValue::Float(yaw),
                )
                .unwrap();
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
                let mut decoder = create_decoder(channels, fft_size);
                decoder.initialize(sample_rate).unwrap();

                let input = vec![0.5f32; block_size * channels];
                let mut output = vec![0.0f32; block_size * 2];
                let context = ProcessContext::new(sample_rate, block_size);

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

/// Benchmark realistic immersive 7.1.4 workload
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
            channels,
            fft_size,
            None,
            0.3, // Moderate externalization
            0.5, // Some near-field
            false,
            120.0,
            2.0,
            0.0,
            RoomModel::default(),
        );
        decoder.initialize(sample_rate).unwrap();

        let input = vec![0.5f32; block_size * channels];
        let mut output = vec![0.0f32; block_size * 2];
        let context = ProcessContext::new(sample_rate, block_size);

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
    bench_externalization,
    bench_large_blocks,
    bench_head_tracking,
    bench_passthrough,
    bench_atmos_7_1_4,
);

criterion_main!(benches);
