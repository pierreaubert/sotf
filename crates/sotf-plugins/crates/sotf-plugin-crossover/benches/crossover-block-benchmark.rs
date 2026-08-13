use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_plugin_crossover::{CrossoverPlugin, CrossoverPluginParams, PerChannelOpMode};
use std::hint::black_box;

const SAMPLE_RATE: u32 = 48_000;

fn bench_plugin(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    id: &str,
    frames: usize,
    mut plugin: CrossoverPlugin,
) {
    plugin.initialize(SAMPLE_RATE).unwrap();
    let input: Vec<f32> = (0..frames * plugin.input_channels())
        .map(|index| ((index % 101) as f32 - 50.0) / 101.0)
        .collect();
    let mut output = vec![0.0; frames * plugin.output_channels()];
    let context = ProcessContext::new(SAMPLE_RATE, frames);
    group.throughput(Throughput::Elements(input.len() as u64));
    group.bench_with_input(BenchmarkId::new(id, frames), &frames, |bencher, _| {
        bencher.iter(|| {
            plugin
                .process(black_box(&input), black_box(&mut output), &context)
                .unwrap()
        });
    });
}

fn benchmark_lr_blocks(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("crossover_lr_interleaved_blocks");
    for frames in [32, 512, 2_048] {
        bench_plugin(
            &mut group,
            "2ch_2band_both",
            frames,
            CrossoverPlugin::new(2, "LR24", 1_000.0, "both").unwrap(),
        );
        bench_plugin(
            &mut group,
            "2ch_4band_both",
            frames,
            CrossoverPlugin::new_multiway(2, "LR24", 200.0, "both", &[1_200.0, 6_000.0]).unwrap(),
        );
        bench_plugin(
            &mut group,
            "8ch_4band_both",
            frames,
            CrossoverPlugin::new_multiway(8, "LR24", 200.0, "both", &[1_200.0, 6_000.0]).unwrap(),
        );
        bench_plugin(
            &mut group,
            "8ch_per_channel_mixed",
            frames,
            CrossoverPlugin::new_per_channel(
                "LR24",
                vec![
                    120.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 6_000.0, 8_000.0,
                ],
                vec![
                    PerChannelOpMode::Lowpass,
                    PerChannelOpMode::Highpass,
                    PerChannelOpMode::Mute,
                    PerChannelOpMode::Passthrough,
                    PerChannelOpMode::Lowpass,
                    PerChannelOpMode::Highpass,
                    PerChannelOpMode::Mute,
                    PerChannelOpMode::Passthrough,
                ],
            )
            .unwrap(),
        );
    }
    group.finish();
}

fn fir_plugin(channels: usize, bands: usize, taps: usize) -> CrossoverPlugin {
    let extra_frequencies = match bands {
        2 => vec![],
        4 => vec![1_200.0, 6_000.0],
        _ => unreachable!(),
    };
    CrossoverPlugin::from_params(
        channels,
        &CrossoverPluginParams {
            crossover_type: "FIR".into(),
            frequency: if bands == 2 { 1_000.0 } else { 200.0 },
            output: "both".into(),
            extra_frequencies,
            fir_taps: Some(taps),
            channel_frequencies_hz: vec![],
            channel_modes: vec![],
        },
    )
    .unwrap()
}

fn benchmark_fir_blocks(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("crossover_fir_interleaved_blocks");
    for frames in [32, 512, 2_048] {
        for taps in [63, 511] {
            bench_plugin(
                &mut group,
                &format!("2ch_2band_{taps}taps"),
                frames,
                fir_plugin(2, 2, taps),
            );
            bench_plugin(
                &mut group,
                &format!("2ch_4band_{taps}taps"),
                frames,
                fir_plugin(2, 4, taps),
            );
        }
    }
    group.finish();
}

criterion_group!(benches, benchmark_lr_blocks, benchmark_fir_blocks);
criterion_main!(benches);
