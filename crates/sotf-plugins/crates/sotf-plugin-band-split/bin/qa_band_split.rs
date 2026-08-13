use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_host::{CountingAlloc, assert_no_allocs, run_standard_tests};
use sotf_plugin_band_split::{BandSplitPlugin, BandSplitPluginParams};
use std::f32::consts::PI;
use std::time::{Duration, Instant};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let in_ch = 1;
    let params = BandSplitPluginParams {
        frequencies: vec![],
        frequency: 1000.0,
        num_bands: 2,
        crossover_type: "LR24".to_string(),
    };

    let mut plugin = BandSplitPlugin::from_params(in_ch, &params).unwrap();
    plugin.initialize(sample_rate).unwrap();

    println!("=== QA: BandSplit Plugin ===");

    // Test 1: Low frequency goes to band 0
    // Output per frame: [band0_ch0, band1_ch0] (out_ch = 2 for 1ch * 2bands)
    println!("\n[Test 1] Band separation (crossover at 1kHz, mono)");
    let num_frames = 4096;
    let input = generate_sine(sample_rate, 100.0, -10.0, num_frames);
    let out_ch = in_ch * 2; // 2 bands
    let mut output = vec![0.0f32; num_frames * out_ch];
    let ctx = ProcessContext::new(sample_rate, num_frames);
    plugin.process(&input, &mut output, &ctx).unwrap();

    // Band 0 (low): every even sample in output
    let low_energy: f32 = (0..num_frames)
        .map(|f| {
            let s = output[f * out_ch];
            s * s
        })
        .sum::<f32>()
        / num_frames as f32;
    // Band 1 (high): every odd sample in output
    let high_energy: f32 = (0..num_frames)
        .map(|f| {
            let s = output[f * out_ch + 1];
            s * s
        })
        .sum::<f32>()
        / num_frames as f32;
    println!(
        "  100Hz: Low band energy={:.6}, High band energy={:.6}",
        low_energy, high_energy
    );
    assert!(
        low_energy > high_energy * 10.0,
        "100Hz should be mostly in low band"
    );

    // Run standard QA tests
    run_standard_tests(&mut plugin, "BandSplitPlugin");

    verify_supported_rates_and_layouts();
    benchmark_worst_case_automation();

    println!("\n[ALL PASS] BandSplit QA Complete.");
}

fn verify_supported_rates_and_layouts() {
    for sample_rate in [32_000, 44_100, 48_000, 96_000, 192_000] {
        for channels in [1, 2, 6, 8, 10, 12] {
            let mut plugin =
                BandSplitPlugin::new_multiband(channels, &[200.0, 2_000.0, 8_000.0], "LR48")
                    .unwrap();
            plugin.initialize(sample_rate).unwrap();
            let input = vec![0.1; 257 * channels];
            let mut output = vec![0.0; 257 * channels * 4];
            plugin
                .process(&input, &mut output, &ProcessContext::new(sample_rate, 257))
                .unwrap();
            assert!(output.iter().all(|sample| sample.is_finite()));
        }
    }
}

fn benchmark_worst_case_automation() {
    const CHANNELS: usize = 12;
    const FRAMES: usize = 512;
    let mut plugin =
        BandSplitPlugin::new_multiband(CHANNELS, &[200.0, 2_000.0, 8_000.0], "LR48").unwrap();
    plugin.initialize(48_000).unwrap();
    let input = vec![0.1; FRAMES * CHANNELS];
    let mut output = vec![0.0; FRAMES * CHANNELS * 4];
    let context = ProcessContext::new(48_000, FRAMES);
    plugin.process(&input, &mut output, &context).unwrap();
    assert_no_allocs("Band Split 12ch four-band LR48 automation", || {
        plugin.process(&input, &mut output, &context).unwrap();
    });

    let mut durations = Vec::with_capacity(400);
    for iteration in 0..400 {
        let targets = if iteration % 2 == 0 {
            [300.0, 3_000.0, 10_000.0]
        } else {
            [200.0, 2_000.0, 8_000.0]
        };
        for (id, target) in ["frequency", "frequency_2", "frequency_3"]
            .into_iter()
            .zip(targets)
        {
            plugin
                .set_parameter(ParameterId::from(id), ParameterValue::Float(target))
                .unwrap();
        }
        let started = Instant::now();
        plugin.process(&input, &mut output, &context).unwrap();
        durations.push(started.elapsed());
    }
    durations.sort_unstable();
    let percentile = |p: usize| durations[(durations.len() - 1) * p / 100];
    let maximum = *durations.last().unwrap_or(&Duration::ZERO);
    println!(
        "Band Split 12ch/4-band LR48 automation: p50={:?}, p95={:?}, p99={:?}, max={:?}, zero process allocations",
        percentile(50),
        percentile(95),
        percentile(99),
        maximum
    );
    assert!(maximum < Duration::from_millis(10));
}

fn generate_sine(sr: u32, freq: f32, db: f32, frames: usize) -> Vec<f32> {
    let amp = 10.0f32.powf(db / 20.0);
    (0..frames)
        .map(|i| (2.0 * PI * freq * i as f32 / sr as f32).sin() * amp)
        .collect()
}
