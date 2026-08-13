use sotf_host::plugin::ProcessContext;
use sotf_host::{
    CountingAlloc, ParametricInPlacePlugin, ParametricInPlacePluginAdapter, measure_peak_db,
    run_standard_tests,
};
use sotf_plugin_spectral_compressor::{SpectralCompressorPlugin, SpectralCompressorPluginParams};
use std::time::{Duration, Instant};

#[global_allocator]
static ALLOCATOR: CountingAlloc = CountingAlloc;

fn main() {
    const SAMPLE_RATE: u32 = 48_000;
    println!("=== QA: Spectral Compressor ===");

    println!("\n[Test 1] Bin-centred compression calibration");
    let params = SpectralCompressorPluginParams {
        fft_size_index: 1,
        threshold_db: -30.0,
        ratio: 4.0,
        attack_ms: 0.1,
        release_ms: 10.0,
        knee_db: 0.0,
        spectral_smoothing: 0.0,
        mix: 1.0,
        ..Default::default()
    };
    let mut calibrated = SpectralCompressorPlugin::from_params(1, params);
    calibrated.initialize(SAMPLE_RATE).unwrap();
    let frames = 48_000;
    let frequency = 48.0 * SAMPLE_RATE as f32 / 2048.0;
    let amplitude = 10.0_f32.powf(-12.0 / 20.0);
    let mut tone: Vec<f32> = (0..frames)
        .map(|frame| {
            amplitude
                * (std::f32::consts::TAU * frequency * frame as f32 / SAMPLE_RATE as f32).sin()
        })
        .collect();
    for block in tone.chunks_mut(1024) {
        let block_frames = block.len();
        calibrated
            .process_in_place(block, &ProcessContext::new(SAMPLE_RATE, block_frames))
            .unwrap();
    }
    let measured = measure_peak_db(&tone[frames - 8192..]);
    println!("  Expected about -25.5 dBFS, measured {measured:.2} dBFS");
    assert!((measured + 25.5).abs() < 2.0);

    println!("\n[Test 2] Channel/FFT/target throughput matrix");
    let mut worst = Duration::ZERO;
    let mut worst_case = (0, 0, 0);
    for channels in [1, 2, 6, 8, 12, 16] {
        for fft_size_index in 0..3 {
            for target_mode in [0, 1] {
                let params = SpectralCompressorPluginParams {
                    fft_size_index,
                    target_mode,
                    ratio: 1.0,
                    ..Default::default()
                };
                let mut plugin = SpectralCompressorPlugin::from_params(channels, params);
                plugin.initialize(SAMPLE_RATE).unwrap();
                let mut block = vec![0.0; 1024 * channels];
                for _ in 0..4 {
                    plugin
                        .process_in_place(&mut block, &ProcessContext::new(SAMPLE_RATE, 1024))
                        .unwrap();
                }
                for _ in 0..24 {
                    let started = Instant::now();
                    plugin
                        .process_in_place(&mut block, &ProcessContext::new(SAMPLE_RATE, 1024))
                        .unwrap();
                    let elapsed = started.elapsed();
                    if elapsed > worst {
                        worst = elapsed;
                        worst_case = (channels, fft_size_index, target_mode);
                    }
                }
            }
        }
    }
    let callback_budget = Duration::from_secs_f64(1024.0 / SAMPLE_RATE as f64);
    println!(
        "  Worst callback: {:.2} ms ({:.1}% budget), case {:?}",
        worst.as_secs_f64() * 1000.0,
        100.0 * worst.as_secs_f64() / callback_budget.as_secs_f64(),
        worst_case
    );
    assert!(worst < callback_budget);

    println!("\n[Test 3] Standard latency/allocation/performance checks");
    let mut standard = ParametricInPlacePluginAdapter::new(SpectralCompressorPlugin::from_params(
        2,
        SpectralCompressorPluginParams::default(),
    ));
    run_standard_tests(&mut standard, "SpectralCompressorPlugin");
    println!("\n[ALL PASS] Spectral Compressor QA complete.");
}
