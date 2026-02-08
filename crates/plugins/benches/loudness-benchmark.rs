// ============================================================================
// Loudness Compensation Performance Benchmarks
// ============================================================================

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use sotf_plugins::{LoudnessCompensationPlugin, Plugin, ProcessContext};
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
                let mut plugin = LoudnessCompensationPlugin::new(channels, 100.0, 6.0, 10000.0, 6.0);
                plugin.initialize(sample_rate).unwrap();

                let input = vec![0.5f32; block_size * channels];
                let mut output = vec![0.0f32; block_size * channels];
                let context = ProcessContext {
                    sample_rate,
                    num_frames: block_size,
                };

                b.iter(|| {
                    plugin.process(black_box(&input), black_box(&mut output), black_box(&context)).unwrap();
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, benchmark_loudness_plugin);
criterion_main!(benches);