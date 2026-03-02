use criterion::{Criterion, criterion_group, criterion_main};
use sotf_host::{InPlacePlugin, InPlacePluginAdapter, benchmark_plugin_full};
use sotf_plugin_compressor::{CompressorPlugin, CompressorPluginParams};

fn benchmark_compressor(c: &mut Criterion) {
    let channels = 2;
    let sample_rate = 48000;

    // 1. Linked Compressor
    let mut inner = CompressorPlugin::new(
        channels, -20.0, // Threshold
        4.0,   // Ratio
        5.0,   // Attack
        50.0,  // Release
        6.0,   // Knee
        0.0,   // Makeup
    );
    inner.initialize(sample_rate).unwrap();
    let plugin = InPlacePluginAdapter::new(inner);
    benchmark_plugin_full(c, "Compressor_Linked", Box::new(plugin), sample_rate as f64);

    // 2. Unlinked Compressor
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
    let mut inner_unlinked = CompressorPlugin::from_params(channels, params);
    inner_unlinked.initialize(sample_rate).unwrap();
    let plugin_unlinked = InPlacePluginAdapter::new(inner_unlinked);
    benchmark_plugin_full(
        c,
        "Compressor_Unlinked",
        Box::new(plugin_unlinked),
        sample_rate as f64,
    );
}

criterion_group!(benches, benchmark_compressor);
criterion_main!(benches);
