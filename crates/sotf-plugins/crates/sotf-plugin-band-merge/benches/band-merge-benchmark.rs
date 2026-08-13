use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_plugin_band_merge::BandMergePlugin;
use std::hint::black_box;

fn benchmark_realistic_layouts(criterion: &mut Criterion) {
    const SAMPLE_RATE: u32 = 48_000;
    const FRAMES: usize = 512;
    let mut group = criterion.benchmark_group("band_merge_scalar_sum");

    for (channels, bands) in [(2, 2), (2, 4), (6, 4), (8, 8)] {
        let input_channels = channels * bands;
        let input: Vec<f32> = (0..FRAMES * input_channels)
            .map(|index| ((index % 97) as f32 - 48.0) / 97.0)
            .collect();
        let mut output = vec![0.0_f32; FRAMES * channels];
        let context = ProcessContext::new(SAMPLE_RATE, FRAMES);
        let mut plugin = BandMergePlugin::new(channels, bands).unwrap();
        plugin.initialize(SAMPLE_RATE).unwrap();
        group.throughput(Throughput::Elements((FRAMES * input_channels) as u64));
        group.bench_with_input(
            BenchmarkId::new("channels_x_bands", format!("{channels}x{bands}")),
            &(channels, bands),
            |bencher, _| {
                bencher.iter(|| {
                    plugin
                        .process(black_box(&input), black_box(&mut output), &context)
                        .unwrap()
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, benchmark_realistic_layouts);
criterion_main!(benches);
