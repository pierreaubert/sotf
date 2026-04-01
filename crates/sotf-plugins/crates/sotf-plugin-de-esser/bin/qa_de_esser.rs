use sotf_host::plugin::{InPlacePlugin, InPlacePluginAdapter, ProcessContext};
use sotf_host::{CountingAlloc, run_standard_tests};
use sotf_plugin_de_esser::{DeEsserPlugin, DeEsserPluginParams};
use std::f32::consts::PI;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let channels = 1;
    let params = DeEsserPluginParams {
        frequency: 8000.0,
        q: 1.5,
        threshold: -20.0,
        ratio: 10.0,
        attack_ms: 0.5,
        release_ms: 20.0,
        mode: "Split-Band".to_string(),
        mix: 1.0,
    };

    let mut inner = DeEsserPlugin::from_params(channels, params);
    inner.initialize(sample_rate).unwrap();

    println!("=== QA: DeEsser Plugin ===");

    // Test 1: High-frequency signal above threshold should be attenuated
    println!("\n[Test 1] HF attenuation (8kHz sine at -10dB, threshold -20dB)");
    let num_frames = 48000;
    let mut buffer = generate_sine(sample_rate, 8000.0, -10.0, num_frames);
    let input_rms = rms(&buffer);
    let ctx = ProcessContext {
        sample_rate,
        num_frames,
    };
    inner.process_in_place(&mut buffer, &ctx).unwrap();
    let output_rms = rms(&buffer[num_frames / 2..]);
    let attenuation_db = 20.0 * (output_rms / input_rms).log10();
    println!(
        "  Input RMS: {:.4}, Output RMS: {:.4}, Attenuation: {:.1}dB",
        input_rms, output_rms, attenuation_db
    );
    assert!(
        attenuation_db < -1.0,
        "8kHz above threshold should be attenuated"
    );

    // Run standard QA tests
    let mut plugin = InPlacePluginAdapter::new(inner);
    run_standard_tests(&mut plugin, "DeEsserPlugin");

    println!("\n[ALL PASS] DeEsser QA Complete.");
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
