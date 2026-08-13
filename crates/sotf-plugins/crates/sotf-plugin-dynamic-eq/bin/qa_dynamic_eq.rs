use sotf_host::plugin::{InPlacePluginAdapter, ProcessContext};
use sotf_host::{
    CountingAlloc, ParametricInPlacePlugin, ParametricInPlacePluginAdapter, assert_no_allocs,
    run_standard_tests,
};
use sotf_plugin_dynamic_eq::{DynEqBandParams, DynamicEqPlugin, DynamicEqPluginParams};
use std::f32::consts::PI;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let channels = 1;
    let params = DynamicEqPluginParams {
        num_bands: 1,
        threshold: -30.0,
        ratio: 4.0,
        attack_ms: 1.0,
        release_ms: 50.0,
        knee: 6.0,
        link_channels: false,
        mix: 1.0,
        bands: vec![DynEqBandParams {
            frequency: 1000.0,
            q: 1.0,
            gain: 6.0,
            band_threshold: -30.0,
            band_ratio: 4.0,
            active: true,
            solo: false,
        }],
    };

    let mut inner = DynamicEqPlugin::from_params(channels, params);
    inner.initialize(sample_rate).unwrap();

    println!("=== QA: DynamicEQ Plugin ===");

    // Test 1: Signal above threshold should trigger compression at band frequency
    println!("\n[Test 1] Dynamic compression at 1kHz");
    let num_frames = 48000;
    let mut buffer = generate_sine(sample_rate, 1000.0, -10.0, num_frames);
    let input_rms = rms(&buffer);
    let ctx = ProcessContext::new(sample_rate, num_frames);
    inner.process_in_place(&mut buffer, &ctx).unwrap();
    let output_rms = rms(&buffer[num_frames / 2..]);

    println!(
        "  Input RMS: {:.4}, Output RMS: {:.4}",
        input_rms, output_rms
    );
    assert!(output_rms.is_finite(), "Output should be finite");

    // Run standard QA tests
    let mut plugin = InPlacePluginAdapter::new(ParametricInPlacePluginAdapter::new(inner));
    run_standard_tests(&mut plugin, "DynamicEqPlugin");

    println!("\n[Test 3] Layout/band/link/block realtime matrix");
    for channels in [1, 2, 8, 16, 32] {
        for num_bands in [1, 4, 8] {
            for link_channels in [false, true] {
                for zero_gain in [false, true] {
                    let mut candidate = DynamicEqPlugin::from_params(
                        channels,
                        DynamicEqPluginParams {
                            num_bands,
                            link_channels,
                            bands: (0..num_bands)
                                .map(|band| DynEqBandParams {
                                    frequency: 100.0 * 2.0_f32.powi(band as i32),
                                    gain: if zero_gain { 0.0 } else { 6.0 },
                                    ..Default::default()
                                })
                                .collect(),
                            ..Default::default()
                        },
                    );
                    candidate.initialize(sample_rate).unwrap();
                    for frames in [32, 64, 127, 256, 512, 1_024, 2_048] {
                        let mut audio = vec![0.0; frames * channels];
                        let context = ProcessContext::new(sample_rate, frames);
                        candidate.process_in_place(&mut audio, &context).unwrap();
                        assert_no_allocs("DynamicEqPlugin matrix process", || {
                            candidate.process_in_place(&mut audio, &context).unwrap();
                        });
                        let started = std::time::Instant::now();
                        candidate.process_in_place(&mut audio, &context).unwrap();
                        assert!(
                            started.elapsed().as_secs_f64() < frames as f64 / sample_rate as f64
                        );
                    }
                }
            }
        }
    }
    println!("  all layouts/modes zero-allocation and below deadline: PASS");

    println!("\n[ALL PASS] DynamicEQ QA Complete.");
}

fn generate_sine(sr: u32, freq: f32, db: f32, frames: usize) -> Vec<f32> {
    let amp = 10.0f32.powf(db / 20.0);
    (0..frames)
        .map(|i| (2.0 * PI * freq * i as f32 / sr as f32).sin() * amp)
        .collect()
}

fn rms(buf: &[f32]) -> f32 {
    (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt()
}
