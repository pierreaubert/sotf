// ============================================================================
// Zero-Allocation Benchmark
// ============================================================================
//
// Proves that plugin process() / process_in_place() methods perform zero heap
// allocations on the hot path. Uses a custom GlobalAlloc wrapper that counts
// allocations, then asserts the count is zero after processing.
//
// Run with: cargo bench -p plugins --no-default-features --bench allocation-benchmark
//
// Plugins excluded:
// - ResamplerPlugin: rubato's process() API returns Vec<Vec<f32>> (external limitation)
// - PndPlugin: uses rubato internally (same limitation)
// - ConvolutionPlugin: requires IR file loading for meaningful processing
// - BinauralDecoderPlugin: requires HRTF file for initialization

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use criterion::{Criterion, criterion_group, criterion_main};

use math_audio_iir_fir::{Biquad, BiquadFilterType};
use sotf_plugins::{
    ABComparePlugin, AutoGain, AutoGainParams, BandMergePlugin, BandSplitPlugin,
    ChannelMuteSoloPlugin, CompressorPlugin, CrossoverPlugin, DelayPlugin, DenoiserPlugin,
    EqPlugin, ExpanderPlugin, FletcherMunsonPlugin, FletcherMunsonPluginParams, GainPlugin,
    GatePlugin, InPlacePlugin, InPlacePluginAdapter, LimiterPlugin, LoudnessCompensationPlugin,
    LoudnessMonitorPlugin, MatrixPlugin, MultibandCompressorPlugin, MultibandExpanderPlugin,
    Plugin, ProcessContext, SpectrumAnalyzerPlugin, SpectrumConfig, UpmixerPlugin,
    UpmixerPluginParams, XtcPlugin, XtcPluginParams,
};

// ============================================================================
// Counting Allocator
// ============================================================================

static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
static COUNTING_ENABLED: AtomicBool = AtomicBool::new(false);

struct CountingAlloc;

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING_ENABLED.load(Ordering::Relaxed) {
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

/// Run a closure and assert it performs zero heap allocations.
fn assert_no_allocs<F: FnOnce()>(label: &str, f: F) {
    // Ensure any pending allocations from setup are done
    ALLOC_COUNT.store(0, Ordering::SeqCst);
    COUNTING_ENABLED.store(true, Ordering::SeqCst);
    f();
    COUNTING_ENABLED.store(false, Ordering::SeqCst);
    let count = ALLOC_COUNT.load(Ordering::SeqCst);
    assert!(
        count == 0,
        "{label}: {count} allocations detected in hot path (expected 0)"
    );
}

// ============================================================================
// Test Helpers
// ============================================================================

const SAMPLE_RATE: u32 = 48000;
const BUFFER_SIZE: usize = 512;

fn generate_test_buffer(num_frames: usize, channels: usize) -> Vec<f32> {
    (0..num_frames * channels)
        .map(|i| {
            let t = i as f32 / (SAMPLE_RATE as f32 * channels as f32);
            (t * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.5
        })
        .collect()
}

// ============================================================================
// Plugin process() wrappers — each one creates, initializes, warms up,
// then asserts zero allocations on a subsequent process() call.
// ============================================================================

fn test_eq_zero_alloc() {
    let filters = vec![
        Biquad::new(BiquadFilterType::Peak, 1000.0, SAMPLE_RATE as f64, 1.0, 3.0),
        Biquad::new(
            BiquadFilterType::Highshelf,
            8000.0,
            SAMPLE_RATE as f64,
            0.707,
            2.0,
        ),
    ];
    let mut plugin = InPlacePluginAdapter::new(EqPlugin::new(2, filters));
    plugin.initialize(SAMPLE_RATE).unwrap();

    let input = generate_test_buffer(BUFFER_SIZE, 2);
    let mut output = vec![0.0f32; BUFFER_SIZE * 2];
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up
    plugin.process(&input, &mut output, &ctx).unwrap();

    assert_no_allocs("EqPlugin", || {
        plugin.process(&input, &mut output, &ctx).unwrap();
    });
}

fn test_gain_zero_alloc() {
    let mut plugin = GainPlugin::new(2, -3.0);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let mut buffer = generate_test_buffer(BUFFER_SIZE, 2);
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    plugin.process_in_place(&mut buffer, &ctx).unwrap();

    assert_no_allocs("GainPlugin", || {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
    });
}

fn test_compressor_zero_alloc() {
    let mut plugin = CompressorPlugin::new(2, -20.0, 4.0, 10.0, 100.0, 6.0, 0.0);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let mut buffer = generate_test_buffer(BUFFER_SIZE, 2);
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    plugin.process_in_place(&mut buffer, &ctx).unwrap();

    assert_no_allocs("CompressorPlugin", || {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
    });
}

fn test_expander_zero_alloc() {
    let mut plugin = ExpanderPlugin::new(2);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let mut buffer = generate_test_buffer(BUFFER_SIZE, 2);
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    plugin.process_in_place(&mut buffer, &ctx).unwrap();

    assert_no_allocs("ExpanderPlugin", || {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
    });
}

fn test_gate_zero_alloc() {
    let mut plugin = GatePlugin::new(2, -40.0, 10.0, 1.0, 10.0, 100.0);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let mut buffer = generate_test_buffer(BUFFER_SIZE, 2);
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    plugin.process_in_place(&mut buffer, &ctx).unwrap();

    assert_no_allocs("GatePlugin", || {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
    });
}

fn test_limiter_zero_alloc() {
    let mut plugin = LimiterPlugin::new(2, -1.0, 50.0, 5.0, false);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let mut buffer = generate_test_buffer(BUFFER_SIZE, 2);
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    plugin.process_in_place(&mut buffer, &ctx).unwrap();

    assert_no_allocs("LimiterPlugin", || {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
    });
}

fn test_delay_zero_alloc() {
    let mut plugin = DelayPlugin::new(2, 100.0, 0.3, 0.5);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let mut buffer = generate_test_buffer(BUFFER_SIZE, 2);
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    plugin.process_in_place(&mut buffer, &ctx).unwrap();

    assert_no_allocs("DelayPlugin", || {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
    });
}

fn test_crossover_zero_alloc() {
    let mut plugin = CrossoverPlugin::new(2, "LR24", 1000.0, "low").unwrap();
    plugin.initialize(SAMPLE_RATE).unwrap();

    let mut buffer = generate_test_buffer(BUFFER_SIZE, 2);
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    plugin.process_in_place(&mut buffer, &ctx).unwrap();

    assert_no_allocs("CrossoverPlugin", || {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
    });
}

fn test_matrix_zero_alloc() {
    let mut plugin = MatrixPlugin::new(2, 2);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let input = generate_test_buffer(BUFFER_SIZE, 2);
    let mut output = vec![0.0f32; BUFFER_SIZE * 2];
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    plugin.process(&input, &mut output, &ctx).unwrap();

    assert_no_allocs("MatrixPlugin", || {
        plugin.process(&input, &mut output, &ctx).unwrap();
    });
}

fn test_channel_mute_solo_zero_alloc() {
    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let mut buffer = generate_test_buffer(BUFFER_SIZE, 2);
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    plugin.process_in_place(&mut buffer, &ctx).unwrap();

    assert_no_allocs("ChannelMuteSoloPlugin", || {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
    });
}

fn test_loudness_compensation_zero_alloc() {
    let mut plugin =
        InPlacePluginAdapter::new(LoudnessCompensationPlugin::new(2, 200.0, 3.0, 6000.0, 2.0));
    plugin.initialize(SAMPLE_RATE).unwrap();

    let input = generate_test_buffer(BUFFER_SIZE, 2);
    let mut output = vec![0.0f32; BUFFER_SIZE * 2];
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    plugin.process(&input, &mut output, &ctx).unwrap();

    assert_no_allocs("LoudnessCompensationPlugin", || {
        plugin.process(&input, &mut output, &ctx).unwrap();
    });
}

fn test_fletcher_munson_zero_alloc() {
    let params = FletcherMunsonPluginParams {
        playback_volume_db: -30.0,
        reference_level_db: -14.0,
        ..Default::default()
    };
    let mut plugin = FletcherMunsonPlugin::from_params(2, params).unwrap();
    Plugin::initialize(&mut plugin, SAMPLE_RATE).unwrap();

    let input = generate_test_buffer(BUFFER_SIZE, 2);
    let mut output = vec![0.0f32; BUFFER_SIZE * 2];
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    plugin.process(&input, &mut output, &ctx).unwrap();

    assert_no_allocs("FletcherMunsonPlugin", || {
        plugin.process(&input, &mut output, &ctx).unwrap();
    });
}

fn test_multiband_compressor_zero_alloc() {
    let mut plugin = MultibandCompressorPlugin::new(2);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let mut buffer = generate_test_buffer(BUFFER_SIZE, 2);
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    plugin.process_in_place(&mut buffer, &ctx).unwrap();

    assert_no_allocs("MultibandCompressorPlugin", || {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
    });
}

fn test_multiband_expander_zero_alloc() {
    let mut plugin = MultibandExpanderPlugin::new(2);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let mut buffer = generate_test_buffer(BUFFER_SIZE, 2);
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    plugin.process_in_place(&mut buffer, &ctx).unwrap();

    assert_no_allocs("MultibandExpanderPlugin", || {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
    });
}

fn test_ab_compare_zero_alloc() {
    let mut plugin = ABComparePlugin::new(2).unwrap();
    plugin.initialize(SAMPLE_RATE).unwrap();

    let input = generate_test_buffer(BUFFER_SIZE, 2);
    let mut output = vec![0.0f32; BUFFER_SIZE * 2];
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    plugin.process(&input, &mut output, &ctx).unwrap();

    assert_no_allocs("ABComparePlugin", || {
        plugin.process(&input, &mut output, &ctx).unwrap();
    });
}

fn test_band_split_zero_alloc() {
    let mut plugin = BandSplitPlugin::new(2, 1000.0, "LR24").unwrap();
    plugin.initialize(SAMPLE_RATE).unwrap();

    let input = generate_test_buffer(BUFFER_SIZE, 2);
    let mut output = vec![0.0f32; BUFFER_SIZE * 4]; // 2 bands * 2 channels
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    plugin.process(&input, &mut output, &ctx).unwrap();

    assert_no_allocs("BandSplitPlugin", || {
        plugin.process(&input, &mut output, &ctx).unwrap();
    });
}

fn test_band_merge_zero_alloc() {
    let mut plugin = BandMergePlugin::new(2, 2).unwrap();
    plugin.initialize(SAMPLE_RATE).unwrap();

    let input = generate_test_buffer(BUFFER_SIZE, 4); // 2 bands * 2 channels
    let mut output = vec![0.0f32; BUFFER_SIZE * 2];
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    plugin.process(&input, &mut output, &ctx).unwrap();

    assert_no_allocs("BandMergePlugin", || {
        plugin.process(&input, &mut output, &ctx).unwrap();
    });
}

fn test_upmixer_zero_alloc() {
    let params: UpmixerPluginParams = serde_json::from_str("{}").unwrap();
    let mut plugin = UpmixerPlugin::from_params(params);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let input = generate_test_buffer(BUFFER_SIZE, 2);
    let out_ch = plugin.output_channels();
    let mut output = vec![0.0f32; BUFFER_SIZE * out_ch];
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    plugin.process(&input, &mut output, &ctx).unwrap();

    assert_no_allocs("UpmixerPlugin", || {
        plugin.process(&input, &mut output, &ctx).unwrap();
    });
}

fn test_xtc_zero_alloc() {
    let params = XtcPluginParams::default();
    let mut plugin = XtcPlugin::new(params, SAMPLE_RATE).unwrap();
    plugin.initialize(SAMPLE_RATE).unwrap();

    let input = generate_test_buffer(BUFFER_SIZE, 2);
    let mut output = vec![0.0f32; BUFFER_SIZE * 2];
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up: XTC needs several blocks to fill its STFT buffers
    for _ in 0..5 {
        plugin.process(&input, &mut output, &ctx).unwrap();
    }

    assert_no_allocs("XtcPlugin", || {
        plugin.process(&input, &mut output, &ctx).unwrap();
    });
}

fn test_denoiser_zero_alloc() {
    let mut plugin = DenoiserPlugin::new(2, false);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let mut buffer = generate_test_buffer(BUFFER_SIZE, 2);
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up
    for _ in 0..3 {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
    }

    assert_no_allocs("DenoiserPlugin", || {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
    });
}

fn test_spectrum_analyzer_zero_alloc() {
    let config = SpectrumConfig {
        num_bins: 30,
        min_freq: 20.0,
        max_freq: 20000.0,
        smoothing: 0.7,
    };
    let mut plugin = SpectrumAnalyzerPlugin::with_config(2, config).unwrap();
    plugin.initialize(SAMPLE_RATE).unwrap();

    let input = generate_test_buffer(BUFFER_SIZE, 2);
    let mut output = vec![0.0f32; BUFFER_SIZE * 2];
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up (fill FFT buffer)
    for _ in 0..10 {
        plugin.process(&input, &mut output, &ctx).unwrap();
    }

    assert_no_allocs("SpectrumAnalyzerPlugin", || {
        plugin.process(&input, &mut output, &ctx).unwrap();
    });
}

fn test_loudness_monitor_zero_alloc() {
    let mut plugin = LoudnessMonitorPlugin::new(2).unwrap();
    plugin.initialize(SAMPLE_RATE).unwrap();

    let input = generate_test_buffer(BUFFER_SIZE, 2);
    let mut output = vec![0.0f32; BUFFER_SIZE * 2];
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up
    plugin.process(&input, &mut output, &ctx).unwrap();

    assert_no_allocs("LoudnessMonitorPlugin", || {
        for _ in 0..10 {
            plugin.process(&input, &mut output, &ctx).unwrap();
        }
    });
}

fn test_auto_gain_zero_alloc() {
    let params = AutoGainParams::default();
    let mut plugin = AutoGain::new(2, SAMPLE_RATE, params).unwrap();

    let input = generate_test_buffer(BUFFER_SIZE, 2);
    let mut output = input.clone();

    // Warm-up
    plugin.measure_input(&input).unwrap();
    plugin.apply_compensation(&mut output, BUFFER_SIZE);
    plugin.measure_output(&output).unwrap();

    assert_no_allocs("AutoGain", || {
        plugin.measure_input(&input).unwrap();
        plugin.apply_compensation(&mut output, BUFFER_SIZE);
        plugin.measure_output(&output).unwrap();
    });
}

// ============================================================================
// Criterion benchmark that runs all zero-allocation assertions
// ============================================================================

fn benchmark_zero_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("ZeroAllocation");

    // Run each plugin's zero-allocation test as a benchmark.
    // The assertion inside assert_no_allocs will panic if any allocations occur.

    group.bench_function("eq", |b| b.iter(test_eq_zero_alloc));
    group.bench_function("gain", |b| b.iter(test_gain_zero_alloc));
    group.bench_function("compressor", |b| b.iter(test_compressor_zero_alloc));
    group.bench_function("expander", |b| b.iter(test_expander_zero_alloc));
    group.bench_function("gate", |b| b.iter(test_gate_zero_alloc));
    group.bench_function("limiter", |b| b.iter(test_limiter_zero_alloc));
    group.bench_function("delay", |b| b.iter(test_delay_zero_alloc));
    group.bench_function("crossover", |b| b.iter(test_crossover_zero_alloc));
    group.bench_function("matrix", |b| b.iter(test_matrix_zero_alloc));
    group.bench_function("channel_mute_solo", |b| {
        b.iter(test_channel_mute_solo_zero_alloc)
    });
    group.bench_function("loudness_compensation", |b| {
        b.iter(test_loudness_compensation_zero_alloc)
    });
    group.bench_function("fletcher_munson", |b| {
        b.iter(test_fletcher_munson_zero_alloc)
    });
    group.bench_function("multiband_compressor", |b| {
        b.iter(test_multiband_compressor_zero_alloc)
    });
    group.bench_function("multiband_expander", |b| {
        b.iter(test_multiband_expander_zero_alloc)
    });
    group.bench_function("ab_compare", |b| b.iter(test_ab_compare_zero_alloc));
    group.bench_function("band_split", |b| b.iter(test_band_split_zero_alloc));
    group.bench_function("band_merge", |b| b.iter(test_band_merge_zero_alloc));
    group.bench_function("upmixer", |b| b.iter(test_upmixer_zero_alloc));
    group.bench_function("xtc", |b| b.iter(test_xtc_zero_alloc));
    group.bench_function("denoiser", |b| b.iter(test_denoiser_zero_alloc));
    group.bench_function("spectrum_analyzer", |b| {
        b.iter(test_spectrum_analyzer_zero_alloc)
    });
    group.bench_function("loudness_monitor", |b| {
        b.iter(test_loudness_monitor_zero_alloc)
    });
    group.bench_function("auto_gain", |b| b.iter(test_auto_gain_zero_alloc));

    group.finish();
}

criterion_group!(benches, benchmark_zero_allocation);
criterion_main!(benches);
