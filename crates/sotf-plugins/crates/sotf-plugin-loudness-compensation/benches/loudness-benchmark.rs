// ============================================================================
// Loudness Compensation Performance Benchmarks
// ============================================================================

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use sotf_host::{InPlacePlugin, ProcessContext};
use sotf_plugin_loudness_compensation::LoudnessCompensationPlugin;
use std::hint::black_box;
use std::time::Duration;

fn benchmark_loudness_plugin(c: &mut Criterion) {
    let mut group = c.benchmark_group("LoudnessCompensation");
    group.warm_up_time(Duration::from_secs(2));

    let sample_rate = 48_000;
    let block_size = 512;

    for &channels in &[2, 8, 16] {
        group.throughput(Throughput::Elements((block_size * channels) as u64));

        group.bench_with_input(
            BenchmarkId::new("process", format!("{}ch", channels)),
            &channels,
            |b, &channels| {
                let mut plugin =
                    LoudnessCompensationPlugin::new(channels, 100.0, 6.0, 10000.0, 6.0);
                plugin.initialize(sample_rate).unwrap();

                let mut buffer = vec![0.5f32; block_size * channels];
                let context = ProcessContext {
                    sample_rate,
                    num_frames: block_size,
                };

                b.iter(|| {
                    let _ = plugin.process_in_place(black_box(&mut buffer), black_box(&context));
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, benchmark_loudness_plugin);
criterion_main!(benches);
