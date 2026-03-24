// ============================================================================
// Comprehensive Plugin Benchmarks
// ============================================================================
//
// Benchmarks for plugins not covered by dedicated benchmark files.
// Covers: EQ, Delay, Gate, Limiter, Expander, Crossover, Matrix,
//         FletcherMunson, LoudnessCompensation, ChannelMuteSolo,
//         SpectrumAnalyzer, LoudnessMonitor.

use criterion::{Criterion, criterion_group, criterion_main};
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use sotf_plugins::{
    ChannelMuteSoloPlugin, CrossoverPlugin, DelayPlugin, EqPlugin, ExpanderPlugin,
    FletcherMunsonPlugin, FletcherMunsonPluginParams, GatePlugin, InPlacePlugin,
    InPlacePluginAdapter, LimiterPlugin, LoudnessCompensationPlugin, LoudnessMonitorPlugin,
    MatrixPlugin, MultibandCompressorPlugin, MultibandExpanderPlugin, Plugin, ProcessContext,
    SpectrumAnalyzerPlugin, SpectrumConfig,
};
use std::hint::black_box;

const SAMPLE_RATE: u32 = 48000;
const BUFFER_SIZE: usize = 512;
const CHANNELS: usize = 2;

/// Generate a test audio buffer for benchmarking
fn generate_test_buffer(num_frames: usize, channels: usize) -> Vec<f32> {
    (0..num_frames * channels)
        .map(|i| {
            let t = i as f32 / (SAMPLE_RATE as f32 * channels as f32);
            (t * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.5
        })
        .collect()
}

// ============================================================================
// EQ Plugin Benchmarks
// ============================================================================

fn benchmark_eq(c: &mut Criterion) {
    let mut group = c.benchmark_group("EqPlugin");

    // Single band EQ
    {
        let filters = vec![Biquad::new(
            BiquadFilterType::Peak,
            1000.0,
            SAMPLE_RATE as f64,
            1.0,
            3.0,
        )];
        let mut plugin = InPlacePluginAdapter::new(EqPlugin::new(CHANNELS, filters));
        plugin.initialize(SAMPLE_RATE).unwrap();

        let input = generate_test_buffer(BUFFER_SIZE, CHANNELS);
        let mut output = vec![0.0f32; BUFFER_SIZE * CHANNELS];
        let context = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: BUFFER_SIZE,
        };

        group.bench_function("1band_stereo", |b| {
            b.iter(|| {
                plugin
                    .process(
                        black_box(&input),
                        black_box(&mut output),
                        black_box(&context),
                    )
                    .unwrap();
            })
        });
    }

    // Multi-band EQ (6 bands)
    {
        let filters = vec![
            Biquad::new(
                BiquadFilterType::Highpass,
                30.0,
                SAMPLE_RATE as f64,
                0.707,
                0.0,
            ),
            Biquad::new(
                BiquadFilterType::Lowshelf,
                100.0,
                SAMPLE_RATE as f64,
                0.707,
                4.0,
            ),
            Biquad::new(BiquadFilterType::Peak, 250.0, SAMPLE_RATE as f64, 1.0, -2.0),
            Biquad::new(BiquadFilterType::Peak, 2000.0, SAMPLE_RATE as f64, 2.0, 3.0),
            Biquad::new(
                BiquadFilterType::Peak,
                4000.0,
                SAMPLE_RATE as f64,
                1.5,
                -2.0,
            ),
            Biquad::new(
                BiquadFilterType::Highshelf,
                10000.0,
                SAMPLE_RATE as f64,
                0.707,
                3.0,
            ),
        ];
        let mut plugin = InPlacePluginAdapter::new(EqPlugin::new(CHANNELS, filters));
        plugin.initialize(SAMPLE_RATE).unwrap();

        let input = generate_test_buffer(BUFFER_SIZE, CHANNELS);
        let mut output = vec![0.0f32; BUFFER_SIZE * CHANNELS];
        let context = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: BUFFER_SIZE,
        };

        group.bench_function("6band_stereo", |b| {
            b.iter(|| {
                plugin
                    .process(
                        black_box(&input),
                        black_box(&mut output),
                        black_box(&context),
                    )
                    .unwrap();
            })
        });
    }

    // EQ with 5.1 channels
    {
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
        let channels = 6;
        let mut plugin = InPlacePluginAdapter::new(EqPlugin::new(channels, filters));
        plugin.initialize(SAMPLE_RATE).unwrap();

        let input = generate_test_buffer(BUFFER_SIZE, channels);
        let mut output = vec![0.0f32; BUFFER_SIZE * channels];
        let context = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: BUFFER_SIZE,
        };

        group.bench_function("2band_5.1", |b| {
            b.iter(|| {
                plugin
                    .process(
                        black_box(&input),
                        black_box(&mut output),
                        black_box(&context),
                    )
                    .unwrap();
            })
        });
    }

    // Different buffer sizes
    for &buf_size in &[256, 512, 1024, 2048] {
        let filters = vec![Biquad::new(
            BiquadFilterType::Peak,
            1000.0,
            SAMPLE_RATE as f64,
            1.0,
            3.0,
        )];
        let mut plugin = InPlacePluginAdapter::new(EqPlugin::new(CHANNELS, filters));
        plugin.initialize(SAMPLE_RATE).unwrap();

        let input = generate_test_buffer(buf_size, CHANNELS);
        let mut output = vec![0.0f32; buf_size * CHANNELS];
        let context = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: buf_size,
        };

        group.bench_function(format!("1band_{}frames", buf_size), |b| {
            b.iter(|| {
                plugin
                    .process(
                        black_box(&input),
                        black_box(&mut output),
                        black_box(&context),
                    )
                    .unwrap();
            })
        });
    }

    group.finish();
}

// ============================================================================
// Delay Plugin Benchmarks
// ============================================================================

fn benchmark_delay(c: &mut Criterion) {
    let mut group = c.benchmark_group("DelayPlugin");

    for &buf_size in &[256, 512, 1024] {
        let mut plugin = DelayPlugin::new(CHANNELS, 100.0, 0.3, 0.5);
        plugin.initialize(SAMPLE_RATE).unwrap();

        let mut buffer = generate_test_buffer(buf_size, CHANNELS);
        let context = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: buf_size,
        };

        group.bench_function(format!("stereo_{}frames", buf_size), |b| {
            b.iter(|| {
                plugin
                    .process_in_place(black_box(&mut buffer), black_box(&context))
                    .unwrap();
            })
        });
    }

    // Different feedback values
    for &feedback in &[0.0, 0.5, 0.9] {
        let mut plugin = DelayPlugin::new(CHANNELS, 100.0, feedback, 0.5);
        plugin.initialize(SAMPLE_RATE).unwrap();

        let mut buffer = generate_test_buffer(BUFFER_SIZE, CHANNELS);
        let context = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: BUFFER_SIZE,
        };

        group.bench_function(format!("feedback_{:.0}pct", feedback * 100.0), |b| {
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
// Gate Plugin Benchmarks
// ============================================================================

fn benchmark_gate(c: &mut Criterion) {
    let mut group = c.benchmark_group("GatePlugin");

    let mut plugin = GatePlugin::new(CHANNELS, -40.0, 10.0, 1.0, 10.0, 100.0);
    plugin.initialize(SAMPLE_RATE).unwrap();

    for &buf_size in &[256, 512, 1024] {
        let mut buffer = generate_test_buffer(buf_size, CHANNELS);
        let context = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: buf_size,
        };

        group.bench_function(format!("stereo_{}frames", buf_size), |b| {
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
// Limiter Plugin Benchmarks
// ============================================================================

fn benchmark_limiter(c: &mut Criterion) {
    let mut group = c.benchmark_group("LimiterPlugin");

    // Hard limiter
    {
        let mut plugin = LimiterPlugin::new(CHANNELS, -1.0, 50.0, 5.0, false);
        plugin.initialize(SAMPLE_RATE).unwrap();

        let mut buffer = generate_test_buffer(BUFFER_SIZE, CHANNELS);
        let context = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: BUFFER_SIZE,
        };

        group.bench_function("hard_stereo_512", |b| {
            b.iter(|| {
                plugin
                    .process_in_place(black_box(&mut buffer), black_box(&context))
                    .unwrap();
            })
        });
    }

    // Soft limiter
    {
        let mut plugin = LimiterPlugin::new(CHANNELS, -1.0, 50.0, 5.0, true);
        plugin.initialize(SAMPLE_RATE).unwrap();

        let mut buffer = generate_test_buffer(BUFFER_SIZE, CHANNELS);
        let context = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: BUFFER_SIZE,
        };

        group.bench_function("soft_stereo_512", |b| {
            b.iter(|| {
                plugin
                    .process_in_place(black_box(&mut buffer), black_box(&context))
                    .unwrap();
            })
        });
    }

    // Different lookahead values
    for &lookahead in &[0.0, 5.0, 10.0] {
        let mut plugin = LimiterPlugin::new(CHANNELS, -1.0, 50.0, lookahead, false);
        plugin.initialize(SAMPLE_RATE).unwrap();

        let mut buffer = generate_test_buffer(BUFFER_SIZE, CHANNELS);
        let context = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: BUFFER_SIZE,
        };

        group.bench_function(format!("lookahead_{}ms", lookahead), |b| {
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
// Expander Plugin Benchmarks
// ============================================================================

fn benchmark_expander(c: &mut Criterion) {
    let mut group = c.benchmark_group("ExpanderPlugin");

    let mut plugin = ExpanderPlugin::new(CHANNELS);
    plugin.initialize(SAMPLE_RATE).unwrap();

    for &buf_size in &[256, 512, 1024] {
        let mut buffer = generate_test_buffer(buf_size, CHANNELS);
        let context = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: buf_size,
        };

        group.bench_function(format!("stereo_{}frames", buf_size), |b| {
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
// Crossover Plugin Benchmarks
// ============================================================================

fn benchmark_crossover(c: &mut Criterion) {
    let mut group = c.benchmark_group("CrossoverPlugin");

    // LR24 lowpass
    {
        let mut plugin = CrossoverPlugin::new(CHANNELS, "LR24", 1000.0, "low").unwrap();
        plugin.initialize(SAMPLE_RATE).unwrap();

        let input = generate_test_buffer(BUFFER_SIZE, CHANNELS);
        let mut output = vec![0.0f32; BUFFER_SIZE * plugin.output_channels()];
        let context = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: BUFFER_SIZE,
        };

        group.bench_function("lr24_lowpass", |b| {
            b.iter(|| {
                plugin
                    .process(black_box(&input), black_box(&mut output), black_box(&context))
                    .unwrap();
            })
        });
    }

    // LR48 lowpass (steeper, more computation)
    {
        let mut plugin = CrossoverPlugin::new(CHANNELS, "LR48", 1000.0, "low").unwrap();
        plugin.initialize(SAMPLE_RATE).unwrap();

        let input = generate_test_buffer(BUFFER_SIZE, CHANNELS);
        let mut output = vec![0.0f32; BUFFER_SIZE * plugin.output_channels()];
        let context = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: BUFFER_SIZE,
        };

        group.bench_function("lr48_lowpass", |b| {
            b.iter(|| {
                plugin
                    .process(black_box(&input), black_box(&mut output), black_box(&context))
                    .unwrap();
            })
        });
    }

    // Multichannel
    for &channels in &[2, 4, 8] {
        let mut plugin = CrossoverPlugin::new(channels, "LR24", 1000.0, "low").unwrap();
        plugin.initialize(SAMPLE_RATE).unwrap();

        let input = generate_test_buffer(BUFFER_SIZE, channels);
        let mut output = vec![0.0f32; BUFFER_SIZE * plugin.output_channels()];
        let context = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: BUFFER_SIZE,
        };

        group.bench_function(format!("lr24_{}ch", channels), |b| {
            b.iter(|| {
                plugin
                    .process(black_box(&input), black_box(&mut output), black_box(&context))
                    .unwrap();
            })
        });
    }

    group.finish();
}

// ============================================================================
// Matrix Plugin Benchmarks
// ============================================================================

fn benchmark_matrix(c: &mut Criterion) {
    let mut group = c.benchmark_group("MatrixPlugin");

    // Identity 2x2
    {
        let mut plugin = MatrixPlugin::new(2, 2);
        plugin.initialize(SAMPLE_RATE).unwrap();

        let input = generate_test_buffer(BUFFER_SIZE, 2);
        let mut output = vec![0.0f32; BUFFER_SIZE * 2];
        let context = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: BUFFER_SIZE,
        };

        group.bench_function("identity_2x2", |b| {
            b.iter(|| {
                plugin
                    .process(
                        black_box(&input),
                        black_box(&mut output),
                        black_box(&context),
                    )
                    .unwrap();
            })
        });
    }

    // Upmix 2 -> 6
    {
        let mut plugin = MatrixPlugin::new(2, 6);
        plugin.initialize(SAMPLE_RATE).unwrap();

        let input = generate_test_buffer(BUFFER_SIZE, 2);
        let mut output = vec![0.0f32; BUFFER_SIZE * 6];
        let context = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: BUFFER_SIZE,
        };

        group.bench_function("upmix_2to6", |b| {
            b.iter(|| {
                plugin
                    .process(
                        black_box(&input),
                        black_box(&mut output),
                        black_box(&context),
                    )
                    .unwrap();
            })
        });
    }

    // Large matrix 8x8
    {
        let mut plugin = MatrixPlugin::new(8, 8);
        plugin.initialize(SAMPLE_RATE).unwrap();

        let input = generate_test_buffer(BUFFER_SIZE, 8);
        let mut output = vec![0.0f32; BUFFER_SIZE * 8];
        let context = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: BUFFER_SIZE,
        };

        group.bench_function("routing_8x8", |b| {
            b.iter(|| {
                plugin
                    .process(
                        black_box(&input),
                        black_box(&mut output),
                        black_box(&context),
                    )
                    .unwrap();
            })
        });
    }

    group.finish();
}

// ============================================================================
// Analyzer Plugin Benchmarks
// ============================================================================

fn benchmark_analyzers(c: &mut Criterion) {
    let mut group = c.benchmark_group("Analyzers");

    // Spectrum Analyzer
    {
        let config = SpectrumConfig {
            num_bins: 30,
            min_freq: 20.0,
            max_freq: 20000.0,
            smoothing: 0.7,
        };
        let mut plugin = SpectrumAnalyzerPlugin::with_config(CHANNELS, config).unwrap();
        plugin.initialize(SAMPLE_RATE).unwrap();

        let input = generate_test_buffer(BUFFER_SIZE, CHANNELS);
        let mut output = vec![0.0f32; BUFFER_SIZE * CHANNELS];
        let context = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: BUFFER_SIZE,
        };

        group.bench_function("spectrum_30bins", |b| {
            b.iter(|| {
                plugin
                    .process(
                        black_box(&input),
                        black_box(&mut output),
                        black_box(&context),
                    )
                    .unwrap();
            })
        });
    }

    // Loudness Monitor
    {
        let mut plugin = LoudnessMonitorPlugin::new(CHANNELS).unwrap();
        plugin.initialize(SAMPLE_RATE).unwrap();

        let input = generate_test_buffer(BUFFER_SIZE, CHANNELS);
        let mut output = vec![0.0f32; BUFFER_SIZE * CHANNELS];
        let context = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: BUFFER_SIZE,
        };

        group.bench_function("loudness_monitor", |b| {
            b.iter(|| {
                plugin
                    .process(
                        black_box(&input),
                        black_box(&mut output),
                        black_box(&context),
                    )
                    .unwrap();
            })
        });
    }

    group.finish();
}

// ============================================================================
// Fletcher-Munson and Loudness Compensation Benchmarks
// ============================================================================

fn benchmark_loudness(c: &mut Criterion) {
    let mut group = c.benchmark_group("Loudness");

    // Fletcher-Munson (implements Plugin, not InPlacePlugin)
    {
        let params = FletcherMunsonPluginParams {
            playback_volume_db: -30.0,
            reference_level_db: -14.0,
            ..Default::default()
        };
        let mut plugin = FletcherMunsonPlugin::from_params(CHANNELS, params).unwrap();
        Plugin::initialize(&mut plugin, SAMPLE_RATE).unwrap();

        let input = generate_test_buffer(BUFFER_SIZE, CHANNELS);
        let mut output = vec![0.0f32; BUFFER_SIZE * CHANNELS];
        let context = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: BUFFER_SIZE,
        };

        group.bench_function("fletcher_munson", |b| {
            b.iter(|| {
                plugin
                    .process(
                        black_box(&input),
                        black_box(&mut output),
                        black_box(&context),
                    )
                    .unwrap();
            })
        });
    }

    // Loudness Compensation (implements Plugin, not InPlacePlugin)
    {
        let mut plugin = InPlacePluginAdapter::new(LoudnessCompensationPlugin::new(
            CHANNELS, 200.0, 3.0, 6000.0, 2.0,
        ));
        plugin.initialize(SAMPLE_RATE).unwrap();

        let input = generate_test_buffer(BUFFER_SIZE, CHANNELS);
        let mut output = vec![0.0f32; BUFFER_SIZE * CHANNELS];
        let context = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: BUFFER_SIZE,
        };

        group.bench_function("loudness_compensation", |b| {
            b.iter(|| {
                plugin
                    .process(
                        black_box(&input),
                        black_box(&mut output),
                        black_box(&context),
                    )
                    .unwrap();
            })
        });
    }

    group.finish();
}

// ============================================================================
// Channel Mute/Solo Benchmarks
// ============================================================================

fn benchmark_channel_mute_solo(c: &mut Criterion) {
    let mut group = c.benchmark_group("ChannelMuteSolo");

    let mut plugin = ChannelMuteSoloPlugin::new(CHANNELS, true);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let mut buffer = generate_test_buffer(BUFFER_SIZE, CHANNELS);
    let context = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: BUFFER_SIZE,
    };

    group.bench_function("stereo_512", |b| {
        b.iter(|| {
            plugin
                .process_in_place(black_box(&mut buffer), black_box(&context))
                .unwrap();
        })
    });

    // 8 channel mute/solo
    {
        let channels = 8;
        let mut plugin8 = ChannelMuteSoloPlugin::new(channels, true);
        plugin8.initialize(SAMPLE_RATE).unwrap();

        let mut buffer8 = generate_test_buffer(BUFFER_SIZE, channels);
        let context8 = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: BUFFER_SIZE,
        };

        group.bench_function("8ch_512", |b| {
            b.iter(|| {
                plugin8
                    .process_in_place(black_box(&mut buffer8), black_box(&context8))
                    .unwrap();
            })
        });
    }

    group.finish();
}

// ============================================================================
// Multiband Compressor Plugin Benchmarks
// ============================================================================

fn benchmark_multiband_compressor(c: &mut Criterion) {
    let mut group = c.benchmark_group("MultibandCompressor");

    // Default 3-band compressor
    {
        let mut plugin = MultibandCompressorPlugin::new(CHANNELS);
        plugin.initialize(SAMPLE_RATE).unwrap();

        let mut buffer = generate_test_buffer(BUFFER_SIZE, CHANNELS);
        let context = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: BUFFER_SIZE,
        };

        group.bench_function("3band_stereo_512", |b| {
            b.iter(|| {
                plugin
                    .process_in_place(black_box(&mut buffer), black_box(&context))
                    .unwrap();
            })
        });
    }

    // 5-band compressor
    {
        use sotf_plugins::MultibandCompressorPluginParams;
        let params = MultibandCompressorPluginParams {
            num_bands: 5,
            ..Default::default()
        };
        let mut plugin = MultibandCompressorPlugin::with_params(CHANNELS, params);
        plugin.initialize(SAMPLE_RATE).unwrap();

        let mut buffer = generate_test_buffer(BUFFER_SIZE, CHANNELS);
        let context = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: BUFFER_SIZE,
        };

        group.bench_function("5band_stereo_512", |b| {
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
// Multiband Expander Plugin Benchmarks
// ============================================================================

fn benchmark_multiband_expander(c: &mut Criterion) {
    let mut group = c.benchmark_group("MultibandExpander");

    // Default 3-band expander
    {
        let mut plugin = MultibandExpanderPlugin::new(CHANNELS);
        plugin.initialize(SAMPLE_RATE).unwrap();

        let mut buffer = generate_test_buffer(BUFFER_SIZE, CHANNELS);
        let context = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: BUFFER_SIZE,
        };

        group.bench_function("3band_stereo_512", |b| {
            b.iter(|| {
                plugin
                    .process_in_place(black_box(&mut buffer), black_box(&context))
                    .unwrap();
            })
        });
    }

    // 5-band expander
    {
        use sotf_plugins::MultibandExpanderPluginParams;
        let params = MultibandExpanderPluginParams {
            num_bands: 5,
            ..Default::default()
        };
        let mut plugin = MultibandExpanderPlugin::with_params(CHANNELS, params);
        plugin.initialize(SAMPLE_RATE).unwrap();

        let mut buffer = generate_test_buffer(BUFFER_SIZE, CHANNELS);
        let context = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: BUFFER_SIZE,
        };

        group.bench_function("5band_stereo_512", |b| {
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
// Benchmark Groups
// ============================================================================

criterion_group!(
    benches,
    benchmark_eq,
    benchmark_delay,
    benchmark_gate,
    benchmark_limiter,
    benchmark_expander,
    benchmark_crossover,
    benchmark_matrix,
    benchmark_analyzers,
    benchmark_loudness,
    benchmark_channel_mute_solo,
    benchmark_multiband_compressor,
    benchmark_multiband_expander,
);
criterion_main!(benches);
