// ============================================================================
// Loudness Compensation Performance Benchmarks
// ============================================================================

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use sotf_host::{ParameterId, ParameterValue, ParametricInPlacePlugin, ProcessContext};
use sotf_plugin_loudness_compensation::LoudnessCompensationPlugin;
use std::hint::black_box;
use std::time::Duration;

fn benchmark_loudness_plugin(c: &mut Criterion) {
    let mut group = c.benchmark_group("LoudnessCompensation");
    group.warm_up_time(Duration::from_secs(2));

    let sample_rate = 48_000;
    let block_size = 512;

    for &channels in &[2, 8, 16, 32] {
        group.throughput(Throughput::Elements((block_size * channels) as u64));

        group.bench_with_input(
            BenchmarkId::new("manual", format!("{}ch", channels)),
            &channels,
            |b, &channels| {
                let mut plugin =
                    LoudnessCompensationPlugin::new(channels, 100.0, 6.0, 10000.0, 6.0);
                plugin.initialize(sample_rate).unwrap();

                let mut buffer = vec![0.5f32; block_size * channels];
                let context = ProcessContext::new(sample_rate, block_size);

                b.iter(|| {
                    let _ = plugin.process_in_place(black_box(&mut buffer), black_box(&context));
                });
            },
        );
        for &(name, mode) in &[("iso", 1), ("auto", 2)] {
            group.bench_with_input(
                BenchmarkId::new(name, format!("{}ch", channels)),
                &channels,
                |b, &channels| {
                    let mut plugin =
                        LoudnessCompensationPlugin::new(channels, 100.0, 6.0, 10_000.0, 6.0);
                    plugin.initialize(sample_rate).unwrap();
                    if mode == 2 {
                        plugin
                            .set_parameter(
                                ParameterId::from("auto_calibrated"),
                                ParameterValue::Bool(true),
                            )
                            .unwrap();
                    }
                    plugin
                        .set_parameter(ParameterId::from("mode"), ParameterValue::Int(mode))
                        .unwrap();
                    let mut buffer = vec![0.5; block_size * channels];
                    let context = ProcessContext::new(sample_rate, block_size);
                    b.iter(|| {
                        plugin
                            .process_in_place(black_box(&mut buffer), black_box(&context))
                            .unwrap()
                    });
                },
            );
        }
    }

    group.bench_function("auto_control_update_32ch", |b| {
        let mut plugin = LoudnessCompensationPlugin::new(32, 100.0, 6.0, 10_000.0, 6.0);
        plugin.initialize(sample_rate).unwrap();
        plugin
            .set_parameter(
                ParameterId::from("auto_calibrated"),
                ParameterValue::Bool(true),
            )
            .unwrap();
        plugin
            .set_parameter(ParameterId::from("mode"), ParameterValue::Int(2))
            .unwrap();
        let mut volume = -20.0;
        b.iter(|| {
            volume = if volume == -20.0 { -20.5 } else { -20.0 };
            plugin
                .set_parameter(
                    ParameterId::from("playback_volume_db"),
                    ParameterValue::Float(volume),
                )
                .unwrap();
        });
    });

    group.finish();
}

criterion_group!(benches, benchmark_loudness_plugin);
criterion_main!(benches);
