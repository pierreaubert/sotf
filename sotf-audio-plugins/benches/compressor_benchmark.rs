use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sotf_plugins::{CompressorPlugin, CompressorPluginParams};
use sotf_plugins::{InPlacePlugin, ProcessContext};

fn benchmark_compressor(c: &mut Criterion) {
    let mut group = c.benchmark_group("Compressor");
    
    // Setup
    let channels = 2;
    let sample_rate = 48000;
    let buffer_size = 1024;
    
    let mut plugin = CompressorPlugin::new(
        channels,
        -20.0, // Threshold
        4.0,   // Ratio
        5.0,   // Attack
        50.0,  // Release
        6.0,   // Knee
        0.0,   // Makeup
    );
    plugin.initialize(sample_rate).unwrap();
    
    let mut buffer = vec![0.0; buffer_size * channels];
    // Fill with some data
    for i in 0..buffer.len() {
        buffer[i] = (i as f32 / buffer.len() as f32).sin();
    }
    
    let context = ProcessContext {
        sample_rate: sample_rate,
        num_frames: buffer_size,
    };

    group.bench_function("process_stereo_linked", |b| {
        b.iter(|| {
            plugin.process_in_place(black_box(&mut buffer), black_box(&context)).unwrap();
        })
    });
    
    // Unlink channels
    let params = CompressorPluginParams {
        link_channels: false,
        threshold_db: -20.0,
        ratio: 4.0,
        attack_ms: 5.0,
        release_ms: 50.0,
        knee_db: 6.0,
        makeup_gain_db: 0.0,
        mix: 1.0,
        auto_makeup: false,
        sidechain_hpf_hz: 80.0,
    };
    let mut plugin_unlinked = CompressorPlugin::from_params(channels, params);
    plugin_unlinked.initialize(sample_rate).unwrap();

    group.bench_function("process_stereo_unlinked", |b| {
        b.iter(|| {
            plugin_unlinked.process_in_place(black_box(&mut buffer), black_box(&context)).unwrap();
        })
    });

    group.finish();
}

criterion_group!(benches, benchmark_compressor);
criterion_main!(benches);
