// ============================================================================
// Real-Time Allocation Tests
// ============================================================================
//
// These tests verify that plugin process() methods perform zero heap allocations
// on the hot path. They use a custom GlobalAlloc wrapper to count allocations.

use std::alloc::{GlobalAlloc, Layout, System};
use serial_test::serial;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};


use sotf_plugins::{
    ABComparePlugin, AutoGain, AutoGainParams, BandMergePlugin, BandSplitPlugin,
    BinauralDecoderPlugin, ChannelMuteSoloPlugin, CompressorPlugin, ConvolutionPlugin,
    CrossfeedMode, CrossfeedPlugin, CrossfeedPluginParams, CrossoverPlugin, DenoiserPlugin,
    DownmixPlugin, DownmixPluginParams, EqPlugin, ExpanderPlugin, GainPlugin,
    GatePlugin, InPlacePlugin, InPlacePluginAdapter, LimiterPlugin, LoudnessCompensationPlugin,
    LoudnessMonitorPlugin, MatrixPlugin, MonoToStereoPlugin, MultibandCompressorPlugin,
    MultibandExpanderPlugin, Plugin, PndPlugin, ProcessContext, ResamplerPlugin, RoomModel,
    SpectrumAnalyzerPlugin, SpectrumConfig, UpmixerPlugin, XtcPlugin, XtcPluginParams,
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
    std::thread::sleep(std::time::Duration::from_millis(100));
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
const BUFFER_SIZE: usize = 1024;

fn generate_test_buffer(num_frames: usize, channels: usize) -> Vec<f32> {
    (0..num_frames * channels)
        .map(|i| {
            let t = i as f32 / (SAMPLE_RATE as f32 * channels as f32);
            (t * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.5
        })
        .collect()
}

// ============================================================================
// Plugin Allocation Tests
// ============================================================================

#[test]
#[serial]
fn test_eq_zero_alloc() {

    let mut plugin = InPlacePluginAdapter::new(EqPlugin::new(2, vec![]));
    plugin.initialize(SAMPLE_RATE).unwrap();

    let input = generate_test_buffer(BUFFER_SIZE, 2);
    let mut output = vec![0.0f32; BUFFER_SIZE * 2];
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up
    for _ in 0..20 {
        plugin.process(&input, &mut output, &ctx).unwrap();
        let _ = plugin.get_data();
    }

    assert_no_allocs("EqPlugin", || {
        for _ in 0..1000 {
            plugin.process(&input, &mut output, &ctx).unwrap();
            let _ = plugin.get_data();
        }
    });
}

#[test]
#[serial]
fn test_gain_zero_alloc() {

    let mut plugin = GainPlugin::new(2, -3.0);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let mut buffer = generate_test_buffer(BUFFER_SIZE, 2);
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up
    for _ in 0..20 {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
        let _ = plugin.get_data();
    }

    assert_no_allocs("GainPlugin", || {
        for _ in 0..1000 {
            plugin.process_in_place(&mut buffer, &ctx).unwrap();
            let _ = plugin.get_data();
        }
    });
}

#[test]
#[serial]
fn test_compressor_zero_alloc() {

    let mut plugin = CompressorPlugin::new(2);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let mut buffer = generate_test_buffer(BUFFER_SIZE, 2);
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up
    for _ in 0..20 {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
        let _ = plugin.get_data();
    }

    assert_no_allocs("CompressorPlugin", || {
        for _ in 0..1000 {
            plugin.process_in_place(&mut buffer, &ctx).unwrap();
            let _ = plugin.get_data();
        }
    });
}

#[test]
#[serial]
fn test_upmixer_zero_alloc() {

    let mut plugin = UpmixerPlugin::new(
        2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
    );
    plugin.initialize(SAMPLE_RATE).unwrap();

    let input = generate_test_buffer(BUFFER_SIZE, 2);
    let out_ch = plugin.output_channels();
    let mut output = vec![0.0f32; BUFFER_SIZE * out_ch];
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up STFT buffers
    for _ in 0..20 {
        plugin.process(&input, &mut output, &ctx).unwrap();
        let _ = plugin.get_data();
    }

    assert_no_allocs("UpmixerPlugin", || {
        for _ in 0..1000 {
            plugin.process(&input, &mut output, &ctx).unwrap();
            let _ = plugin.get_data();
        }
    });
}

#[test]
#[serial]
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
    for _ in 0..20 {
        plugin.process(&input, &mut output, &ctx).unwrap();
        let _ = plugin.get_data();
    }

    assert_no_allocs("XtcPlugin", || {
        for _ in 0..1000 {
            plugin.process(&input, &mut output, &ctx).unwrap();
            let _ = plugin.get_data();
        }
    });
}

#[test]
#[serial]
fn test_resampler_zero_alloc() {

    let mut plugin = ResamplerPlugin::new(2, 44100, 48000, BUFFER_SIZE).unwrap();
    plugin.initialize(44100).unwrap();

    let input = vec![0.0f32; BUFFER_SIZE * 2];
    let mut output = vec![0.0f32; BUFFER_SIZE * 4]; // Extra space
    let ctx = ProcessContext {
        sample_rate: 44100,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up
    for _ in 0..20 {
        plugin.process(&input, &mut output, &ctx).unwrap();
        let _ = plugin.get_data();
    }

    assert_no_allocs("ResamplerPlugin", || {
        for _ in 0..1000 {
            plugin.process(&input, &mut output, &ctx).unwrap();
            let _ = plugin.get_data();
        }
    });
}

#[test]
#[serial]
fn test_convolution_zero_alloc() {

    let mut plugin = ConvolutionPlugin::new(2, SAMPLE_RATE);

    let temp_dir = tempfile::tempdir().unwrap();
    let ir_path = temp_dir.path().join("ir.wav");

    {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: SAMPLE_RATE,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(&ir_path, spec).unwrap();
        for _ in 0..2048 {
            writer.write_sample(0.1f32).unwrap();
            writer.write_sample(0.1f32).unwrap();
        }
    }

    plugin.load_ir(ir_path.to_str().unwrap()).unwrap();
    plugin.initialize(SAMPLE_RATE).unwrap();

    let mut buffer = generate_test_buffer(BUFFER_SIZE, 2);
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up
    for _ in 0..20 {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
        let _ = plugin.get_data();
    }

    assert_no_allocs("ConvolutionPlugin", || {
        for _ in 0..1000 {
            plugin.process_in_place(&mut buffer, &ctx).unwrap();
            let _ = plugin.get_data();
        }
    });
}

#[test]
#[serial]
fn test_binaural_zero_alloc() {

    let mut plugin = BinauralDecoderPlugin::new(
        2,
        1024,
        None,
        true,
        0.0,
        0.0,
        false,
        120.0,
        2.0,
        0.0,
        RoomModel::default(),
    );
    plugin.initialize(SAMPLE_RATE).unwrap();

    let input = generate_test_buffer(BUFFER_SIZE, 2);
    let mut output = vec![0.0f32; BUFFER_SIZE * 2];
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up
    for _ in 0..20 {
        plugin.process(&input, &mut output, &ctx).unwrap();
        let _ = plugin.get_data();
    }

    assert_no_allocs("BinauralDecoderPlugin", || {
        for _ in 0..1000 {
            plugin.process(&input, &mut output, &ctx).unwrap();
            let _ = plugin.get_data();
        }
    });
}

#[test]
#[serial]
fn test_limiter_zero_alloc() {

    let mut plugin = LimiterPlugin::new(2, -1.0, 50.0, 5.0, false);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let mut buffer = generate_test_buffer(BUFFER_SIZE, 2);
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up
    for _ in 0..20 {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
        let _ = plugin.get_data();
    }

    assert_no_allocs("LimiterPlugin", || {
        for _ in 0..1000 {
            plugin.process_in_place(&mut buffer, &ctx).unwrap();
            let _ = plugin.get_data();
        }
    });
}

#[test]
#[serial]
fn test_gate_zero_alloc() {

    let mut plugin = GatePlugin::new(2, -40.0, 10.0, 1.0, 10.0, 100.0);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let mut buffer = generate_test_buffer(BUFFER_SIZE, 2);
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up
    for _ in 0..20 {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
        let _ = plugin.get_data();
    }

    assert_no_allocs("GatePlugin", || {
        for _ in 0..1000 {
            plugin.process_in_place(&mut buffer, &ctx).unwrap();
            let _ = plugin.get_data();
        }
    });
}

#[test]
#[serial]
fn test_ab_compare_zero_alloc() {

    let mut plugin = ABComparePlugin::new(2).unwrap();
    plugin.initialize(SAMPLE_RATE).unwrap();

    let input = generate_test_buffer(BUFFER_SIZE, 2);
    let mut output = vec![0.0f32; BUFFER_SIZE * 2];
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up
    for _ in 0..20 {
        plugin.process(&input, &mut output, &ctx).unwrap();
        let _ = plugin.get_data();
    }

    assert_no_allocs("ABComparePlugin", || {
        for _ in 0..1000 {
            plugin.process(&input, &mut output, &ctx).unwrap();
            let _ = plugin.get_data();
        }
    });
}

#[test]
#[serial]
fn test_denoiser_zero_alloc() {

    let mut plugin = DenoiserPlugin::new(2, false);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let mut buffer = generate_test_buffer(BUFFER_SIZE, 2);
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up
    for _ in 0..20 {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
        let _ = plugin.get_data();
    }

    assert_no_allocs("DenoiserPlugin", || {
        for _ in 0..1000 {
            plugin.process_in_place(&mut buffer, &ctx).unwrap();
            let _ = plugin.get_data();
        }
    });
}

#[test]
#[serial]
fn test_pnd_zero_alloc() {

    let mut plugin = PndPlugin::new(2);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let input = generate_test_buffer(BUFFER_SIZE, 2);
    let mut output = vec![0.0f32; BUFFER_SIZE * 2];
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up
    for _ in 0..20 {
        plugin.process(&input, &mut output, &ctx).unwrap();
        let _ = plugin.get_data();
    }

    assert_no_allocs("PndPlugin", || {
        for _ in 0..1000 {
            plugin.process(&input, &mut output, &ctx).unwrap();
            let _ = plugin.get_data();
        }
    });
}

#[test]
#[serial]
fn test_band_merge_zero_alloc() {

    let mut plugin = BandMergePlugin::new(2, 2).unwrap();
    plugin.initialize(SAMPLE_RATE).unwrap();

    let input = generate_test_buffer(BUFFER_SIZE, 4); // 2 bands * 2 channels
    let mut output = vec![0.0f32; BUFFER_SIZE * 2];
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up
    for _ in 0..20 {
        plugin.process(&input, &mut output, &ctx).unwrap();
        let _ = plugin.get_data();
    }

    assert_no_allocs("BandMergePlugin", || {
        for _ in 0..1000 {
            plugin.process(&input, &mut output, &ctx).unwrap();
            let _ = plugin.get_data();
        }
    });
}

#[test]
#[serial]
fn test_band_split_zero_alloc() {

    let mut plugin = BandSplitPlugin::new(2, 1000.0, "LR24").unwrap();
    plugin.initialize(SAMPLE_RATE).unwrap();

    let input = generate_test_buffer(BUFFER_SIZE, 2);
    let mut output = vec![0.0f32; BUFFER_SIZE * 4]; // 2 bands * 2 channels
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up
    for _ in 0..20 {
        plugin.process(&input, &mut output, &ctx).unwrap();
        let _ = plugin.get_data();
    }

    assert_no_allocs("BandSplitPlugin", || {
        for _ in 0..1000 {
            plugin.process(&input, &mut output, &ctx).unwrap();
            let _ = plugin.get_data();
        }
    });
}

#[test]
#[serial]
fn test_channel_mute_solo_zero_alloc() {

    let mut plugin = ChannelMuteSoloPlugin::new(2, true);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let mut buffer = generate_test_buffer(BUFFER_SIZE, 2);
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up
    for _ in 0..20 {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
        let _ = plugin.get_data();
    }

    assert_no_allocs("ChannelMuteSoloPlugin", || {
        for _ in 0..1000 {
            plugin.process_in_place(&mut buffer, &ctx).unwrap();
            let _ = plugin.get_data();
        }
    });
}

#[test]
#[serial]
fn test_crossfeed_zero_alloc() {

    let params = CrossfeedPluginParams {
        mode: CrossfeedMode::Bauer,
        enabled: true,
        mix: 1.0,
        ..Default::default()
    };
    let mut plugin = CrossfeedPlugin::new(params).unwrap();
    plugin.initialize(SAMPLE_RATE).unwrap();

    let mut buffer = generate_test_buffer(BUFFER_SIZE, 2);
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up
    for _ in 0..20 {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
        let _ = plugin.get_data();
    }

    assert_no_allocs("CrossfeedPlugin", || {
        for _ in 0..1000 {
            plugin.process_in_place(&mut buffer, &ctx).unwrap();
            let _ = plugin.get_data();
        }
    });
}

#[test]
#[serial]
fn test_crossover_zero_alloc() {

    let mut plugin = CrossoverPlugin::new(2, "LR24", 1000.0, "low").unwrap();
    plugin.initialize(SAMPLE_RATE).unwrap();

    let input = generate_test_buffer(BUFFER_SIZE, 2);
    let mut output = vec![0.0f32; input.len()];
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up
    for _ in 0..20 {
        plugin.process(&input, &mut output, &ctx).unwrap();
        let _ = plugin.get_data();
    }

    assert_no_allocs("CrossoverPlugin", || {
        for _ in 0..1000 {
            plugin.process(&input, &mut output, &ctx).unwrap();
            let _ = plugin.get_data();
        }
    });
}

#[test]
#[serial]
fn test_downmix_zero_alloc() {

    let params = DownmixPluginParams {
        input_channels: 6,
        center_gain_db: 0.0,
        surround_gain_db: 0.0,
        height_gain_db: 0.0,
        lfe_gain_db: 0.0,
        phase_coherence: true,
        phase_blend_low_hz: 200.0,
        phase_blend_high_hz: 500.0,
        itu_mode: false,
        matrix_ltrt: false,
    };
    let mut plugin = DownmixPlugin::from_params(params);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let input = generate_test_buffer(BUFFER_SIZE, 6);
    let mut output = vec![0.0f32; BUFFER_SIZE * 2];
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up
    for _ in 0..20 {
        plugin.process(&input, &mut output, &ctx).unwrap();
        let _ = plugin.get_data();
    }

    assert_no_allocs("DownmixPlugin", || {
        for _ in 0..1000 {
            plugin.process(&input, &mut output, &ctx).unwrap();
            let _ = plugin.get_data();
        }
    });
}

#[test]
#[serial]
fn test_expander_zero_alloc() {

    let mut plugin = ExpanderPlugin::new(2);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let mut buffer = generate_test_buffer(BUFFER_SIZE, 2);
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up
    for _ in 0..20 {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
        let _ = plugin.get_data();
    }

    assert_no_allocs("ExpanderPlugin", || {
        for _ in 0..1000 {
            plugin.process_in_place(&mut buffer, &ctx).unwrap();
            let _ = plugin.get_data();
        }
    });
}

#[test]
#[serial]
fn test_fletcher_munson_zero_alloc() {
    // Fletcher-Munson merged into LoudnessCompensation with mode=2 (Auto)
    let mut plugin = LoudnessCompensationPlugin::new(2, 100.0, 6.0, 10000.0, 6.0);
    plugin.set_parameter(
        sotf_plugins::ParameterId::from("mode"),
        sotf_plugins::ParameterValue::Int(2),
    ).unwrap();
    plugin.set_parameter(
        sotf_plugins::ParameterId::from("playback_volume_db"),
        sotf_plugins::ParameterValue::Float(-20.0),
    ).unwrap();
    InPlacePlugin::initialize(&mut plugin, SAMPLE_RATE).unwrap();

    let mut buffer = generate_test_buffer(BUFFER_SIZE, 2);
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up
    for _ in 0..20 {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
        let _ = InPlacePlugin::get_data(&plugin);
    }

    assert_no_allocs("FletcherMunsonPlugin (LoudnessComp Auto)", || {
        for _ in 0..1000 {
            plugin.process_in_place(&mut buffer, &ctx).unwrap();
            let _ = InPlacePlugin::get_data(&plugin);
        }
    });
}

#[test]
#[serial]
fn test_loudness_compensation_zero_alloc() {

    let mut plugin = LoudnessCompensationPlugin::new(2, 200.0, 3.0, 6000.0, 2.0);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let mut buffer = generate_test_buffer(BUFFER_SIZE, 2);
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up
    for _ in 0..20 {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
        let _ = plugin.get_data();
    }

    assert_no_allocs("LoudnessCompensationPlugin", || {
        for _ in 0..1000 {
            plugin.process_in_place(&mut buffer, &ctx).unwrap();
            let _ = plugin.get_data();
        }
    });
}

#[test]
#[serial]
fn test_matrix_zero_alloc() {

    let mut plugin = MatrixPlugin::new(2, 2);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let input = generate_test_buffer(BUFFER_SIZE, 2);
    let mut output = vec![0.0f32; BUFFER_SIZE * 2];
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up
    for _ in 0..20 {
        plugin.process(&input, &mut output, &ctx).unwrap();
        let _ = plugin.get_data();
    }

    assert_no_allocs("MatrixPlugin", || {
        for _ in 0..1000 {
            plugin.process(&input, &mut output, &ctx).unwrap();
            let _ = plugin.get_data();
        }
    });
}

#[test]
#[serial]
fn test_mono_to_stereo_zero_alloc() {

    let mut plugin = MonoToStereoPlugin::new();
    plugin.initialize(SAMPLE_RATE).unwrap();

    let input = generate_test_buffer(BUFFER_SIZE, 1);
    let mut output = vec![0.0f32; BUFFER_SIZE * 2];
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up
    for _ in 0..20 {
        plugin.process(&input, &mut output, &ctx).unwrap();
        let _ = plugin.get_data();
    }

    assert_no_allocs("MonoToStereoPlugin", || {
        for _ in 0..1000 {
            plugin.process(&input, &mut output, &ctx).unwrap();
            let _ = plugin.get_data();
        }
    });
}

#[test]
#[serial]
fn test_multiband_compressor_zero_alloc() {

    let mut plugin = MultibandCompressorPlugin::new(2);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let mut buffer = generate_test_buffer(BUFFER_SIZE, 2);
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up
    for _ in 0..20 {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
        let _ = plugin.get_data();
    }

    assert_no_allocs("MultibandCompressorPlugin", || {
        for _ in 0..1000 {
            plugin.process_in_place(&mut buffer, &ctx).unwrap();
            let _ = plugin.get_data();
        }
    });
}

#[test]
#[serial]
fn test_multiband_expander_zero_alloc() {

    let mut plugin = MultibandExpanderPlugin::new(2);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let mut buffer = generate_test_buffer(BUFFER_SIZE, 2);
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    // Warm-up
    for _ in 0..20 {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
        let _ = plugin.get_data();
    }

    assert_no_allocs("MultibandExpanderPlugin", || {
        for _ in 0..1000 {
            plugin.process_in_place(&mut buffer, &ctx).unwrap();
            let _ = plugin.get_data();
        }
    });
}

#[test]
#[serial]
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
    for _ in 0..20 {
        plugin.process(&input, &mut output, &ctx).unwrap();
        let _ = plugin.get_data();
    }

    assert_no_allocs("LoudnessMonitorPlugin", || {
        for _ in 0..1000 {
            plugin.process(&input, &mut output, &ctx).unwrap();
            let _ = plugin.get_data();
        }
    });
}

#[test]
#[serial]
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

    // Warm-up
    for _ in 0..20 {
        plugin.process(&input, &mut output, &ctx).unwrap();
        let _ = plugin.get_data();
    }

    assert_no_allocs("SpectrumAnalyzerPlugin", || {
        for _ in 0..1000 {
            plugin.process(&input, &mut output, &ctx).unwrap();
            let _ = plugin.get_data();
        }
    });
}

#[test]
#[serial]
fn test_auto_gain_zero_alloc() {

    let params = AutoGainParams::default();
    let mut plugin = AutoGain::new(2, SAMPLE_RATE, params).unwrap();

    let input = generate_test_buffer(BUFFER_SIZE, 2);
    let mut output = input.clone();

    // Warm-up
    for _ in 0..20 {
        plugin.measure_input(&input).unwrap();
        plugin.apply_compensation(&mut output, BUFFER_SIZE);
        plugin.measure_output(&output).unwrap();
    }

    assert_no_allocs("AutoGain", || {
        for _ in 0..1000 {
            plugin.measure_input(&input).unwrap();
            plugin.apply_compensation(&mut output, BUFFER_SIZE);
            plugin.measure_output(&output).unwrap();
        }
    });
}
