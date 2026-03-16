// ============================================================================
// Plugin and Host Performance Benchmarks
// ============================================================================
//
// This benchmark suite measures performance of:
// - Gain plugin (basic building block)
// - Plugin host (chain processing)
// - Different buffer sizes and sample rates

use criterion::{Criterion, criterion_group, criterion_main};
use sotf_plugins::{GainPlugin, InPlacePlugin, InPlacePluginAdapter, PluginHost, ProcessContext};
use std::hint::black_box;

// ============================================================================
// Gain Plugin Benchmarks
// ============================================================================

fn benchmark_gain_plugin(c: &mut Criterion) {
    let mut group = c.benchmark_group("GainPlugin");

    // Single plugin, various buffer sizes
    for &buffer_size in &[256, 512, 1024, 2048] {
        let mut plugin = GainPlugin::new(2, 3.0);
        plugin.initialize(48000).unwrap();

        let mut buffer = vec![0.5f32; buffer_size * 2];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: buffer_size,
        };

        group.bench_function(format!("process_{}frames", buffer_size), |b| {
            b.iter(|| {
                plugin
                    .process_in_place(black_box(&mut buffer), black_box(&context))
                    .unwrap();
            })
        });
    }

    // Different sample rates
    for &sample_rate in &[44100, 48000, 96000, 192000] {
        let mut plugin = GainPlugin::new(2, 0.0);
        plugin.initialize(sample_rate).unwrap();

        let buffer_size = 512;
        let mut buffer = vec![0.5f32; buffer_size * 2];
        let context = ProcessContext {
            sample_rate,
            num_frames: buffer_size,
        };

        group.bench_function(format!("process_{}hz", sample_rate), |b| {
            b.iter(|| {
                plugin
                    .process_in_place(black_box(&mut buffer), black_box(&context))
                    .unwrap();
            })
        });
    }

    // Different gain values
    for &gain_db in &[-60.0, -12.0, 0.0, 6.0, 12.0, 24.0] {
        let mut plugin = GainPlugin::new(2, gain_db);
        plugin.initialize(48000).unwrap();

        let buffer_size = 512;
        let mut buffer = vec![0.5f32; buffer_size * 2];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: buffer_size,
        };

        group.bench_function(format!("process_{}db", gain_db), |b| {
            b.iter(|| {
                plugin
                    .process_in_place(black_box(&mut buffer), black_box(&context))
                    .unwrap();
            })
        });
    }

    // Different channel configurations
    for &channels in &[1, 2, 4, 8] {
        let mut plugin = GainPlugin::new(channels, 0.0);
        plugin.initialize(48000).unwrap();

        let buffer_size = 512;
        let mut buffer = vec![0.5f32; buffer_size * channels];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: buffer_size,
        };

        group.bench_function(format!("process_{}ch", channels), |b| {
            b.iter(|| {
                plugin
                    .process_in_place(black_box(&mut buffer), black_box(&context))
                    .unwrap();
            })
        });
    }

    group.finish();
}

// ============================================================================
// Host Processing Benchmarks
// ============================================================================

fn benchmark_host_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("HostChain");

    // Different chain lengths
    for &chain_length in &[1, 3, 5, 10] {
        let mut host = PluginHost::new(2, 48000);

        for _ in 0..chain_length {
            let gain = GainPlugin::new(2, 0.0);
            host.add_plugin(Box::new(InPlacePluginAdapter::new(gain)))
                .unwrap();
        }

        let buffer_size = 512;
        let input = vec![0.5f32; buffer_size * 2];
        let mut output = vec![0.0f32; buffer_size * 2];

        group.bench_function(format!("{}plugins", chain_length), |b| {
            b.iter(|| {
                host.process(black_box(&input), black_box(&mut output))
                    .unwrap();
            })
        });
    }

    // Different buffer sizes
    for &buffer_size in &[256, 512, 1024] {
        let mut host = PluginHost::new(2, 48000);

        for _ in 0..3 {
            let gain = GainPlugin::new(2, 0.0);
            host.add_plugin(Box::new(InPlacePluginAdapter::new(gain)))
                .unwrap();
        }

        let input = vec![0.5f32; buffer_size * 2];
        let mut output = vec![0.0f32; buffer_size * 2];

        group.bench_function(format!("{}frames", buffer_size), |b| {
            b.iter(|| {
                host.process(black_box(&input), black_box(&mut output))
                    .unwrap();
            })
        });
    }

    // Different sample rates
    for &sample_rate in &[44100, 48000, 96000] {
        let mut host = PluginHost::new(2, sample_rate);

        for _ in 0..3 {
            let gain = GainPlugin::new(2, 0.0);
            host.add_plugin(Box::new(InPlacePluginAdapter::new(gain)))
                .unwrap();
        }

        let buffer_size = 512;
        let input = vec![0.5f32; buffer_size * 2];
        let mut output = vec![0.0f32; buffer_size * 2];

        group.bench_function(format!("{}hz", sample_rate), |b| {
            b.iter(|| {
                host.process(black_box(&input), black_box(&mut output))
                    .unwrap();
            })
        });
    }

    group.finish();
}

// ============================================================================
// In-Place Processing Benchmarks
// ============================================================================

fn benchmark_inplace_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("InPlace");

    let buffer_size = 1024;

    for &channels in &[1, 2, 4] {
        let mut plugin = GainPlugin::new(channels, 3.0);
        plugin.initialize(48000).unwrap();

        let mut buffer = vec![0.5f32; buffer_size * channels];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: buffer_size,
        };

        group.bench_function(format!("gain_{}ch", channels), |b| {
            b.iter(|| {
                plugin
                    .process_in_place(black_box(&mut buffer), black_box(&context))
                    .unwrap();
            })
        });
    }

    group.finish();
}

// ============================================================================
// Per-Sample Cost Analysis
// ============================================================================

fn benchmark_per_sample_cost(c: &mut Criterion) {
    let mut group = c.benchmark_group("PerSampleCost");

    // Measure cost per sample across different configurations
    let configurations = [
        ("gain_stereo_512", 2, 512),
        ("gain_stereo_1024", 2, 1024),
        ("gain_quad_512", 4, 512),
        ("host_3gain_stereo_512", 2, 512),
        ("host_5gain_stereo_512", 2, 512),
    ];

    for &(name, channels, buffer_size) in &configurations {
        if name.starts_with("gain") {
            let mut plugin = GainPlugin::new(channels, 0.0);
            plugin.initialize(48000).unwrap();

            let mut buffer = vec![0.5f32; buffer_size * channels];
            let context = ProcessContext {
                sample_rate: 48000,
                num_frames: buffer_size,
            };

            group.bench_function(name, |b| {
                b.iter(|| {
                    plugin
                        .process_in_place(black_box(&mut buffer), black_box(&context))
                        .unwrap();
                })
            });
        } else if name.starts_with("host") {
            let mut host = PluginHost::new(channels, 48000);
            let plugin_count = name
                .trim_start_matches("host_")
                .trim_start_matches("gain_stereo_")
                .trim_end_matches("_512")
                .parse::<usize>()
                .unwrap_or(3);

            for _ in 0..plugin_count {
                let gain = GainPlugin::new(channels, 0.0);
                host.add_plugin(Box::new(InPlacePluginAdapter::new(gain)))
                    .unwrap();
            }

            let input = vec![0.5f32; buffer_size * channels];
            let mut output = vec![0.0f32; buffer_size * channels];

            group.bench_function(name, |b| {
                b.iter(|| {
                    host.process(black_box(&input), black_box(&mut output))
                        .unwrap();
                })
            });
        }
    }

    group.finish();
}

// ============================================================================
// Benchmark Groups
// ============================================================================

criterion_group!(
    benches,
    benchmark_gain_plugin,
    benchmark_host_chain,
    benchmark_inplace_processing,
    benchmark_per_sample_cost,
);
criterion_main!(benches);
