use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::ProcessContext;
use sotf_plugin_channel_mute_solo::{ChannelMuteSoloParams, ChannelMuteSoloPlugin, ChannelState};
use std::hint::black_box;

const SAMPLE_RATE: u32 = 48_000;
const CHANNEL_COUNTS: [usize; 5] = [2, 6, 8, 16, 32];
const BLOCK_SIZES: [usize; 3] = [64, 256, 1_024];

fn settled_plugin(channels: usize) -> ChannelMuteSoloPlugin {
    let channel_states = (0..channels)
        .map(|channel| ChannelState {
            muted: channel % 4 == 0,
            soloed: false,
            dimmed: channel % 4 == 1,
        })
        .collect();
    let mut plugin = ChannelMuteSoloPlugin::from_params(
        channels,
        ChannelMuteSoloParams {
            enabled: true,
            channel_states,
            dim_gain_db: -20.0,
            fade_ms: 5.0,
        },
    );
    plugin.initialize(SAMPLE_RATE).unwrap();
    plugin
}

fn benchmark_settled_blocks(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("channel_mute_solo_settled");
    for channels in CHANNEL_COUNTS {
        for frames in BLOCK_SIZES {
            let mut plugin = settled_plugin(channels);
            let context = ProcessContext::new(SAMPLE_RATE, frames);
            let mut buffer = vec![0.5_f32; channels * frames];
            group.throughput(Throughput::Elements(buffer.len() as u64));
            group.bench_with_input(
                BenchmarkId::new(format!("{channels}ch"), frames),
                &frames,
                |bencher, _| {
                    bencher.iter(|| {
                        buffer.fill(0.5);
                        plugin
                            .process_in_place(black_box(&mut buffer), black_box(&context))
                            .unwrap()
                    });
                },
            );
        }
    }
    group.finish();
}

criterion_group!(benches, benchmark_settled_blocks);
criterion_main!(benches);
