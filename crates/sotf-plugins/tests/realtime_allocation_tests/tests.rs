use super::consts::BUFFER_SIZE;
use super::consts::SAMPLE_RATE;
use super::consts::generate_test_buffer;
use super::misc::assert_no_allocs;
use serial_test::serial;
#[cfg(any(
    feature = "external-plugin-clap",
    feature = "external-plugin-vst3",
    feature = "external-plugin-au"
))]
use sotf_host::external_plugin::ExternalPlugin;
use sotf_host::{
    ExternalPluginWorker, ExternalPluginWorkerStep, PluginIpcLayout, SecurePluginSharedMemory,
};
use sotf_plugin_ambisonics::{AmbisonicsDecoderConfig, AmbisonicsDecoderPlugin};
use sotf_plugins::{
    ABComparePlugin, AaePlugin, AaePluginParams, AecPlugin, AecPluginParams, AutoGain,
    AutoGainParams, BandMergePlugin, BandSplitPlugin, BeamformerPlugin, BinauralDecoderPlugin,
    ChannelMuteSoloPlugin, CompressorPlugin, ConvolutionPlugin, CrossfeedMode, CrossfeedPlugin,
    CrossfeedPluginParams, CrossoverPlugin, DeEsserPlugin, DeclickPlugin, DelayPlugin,
    DenoiserPlugin, DownmixPlugin, DownmixPluginParams, DynamicEqPlugin, EqPlugin, ExpanderPlugin,
    FirDesignerPlugin, GainPlugin, GatePlugin, HissReducerPlugin, IsolatedExternalPlugin,
    IsolatedExternalPluginConfig, LimiterPlugin, LinearPhaseEqPlugin, LoudnessCompensationPlugin,
    LoudnessMonitorPlugin, MatrixPlugin, MonoToStereoPlugin, MultibandCompressorPlugin,
    MultibandExpanderPlugin, ParametricInPlacePlugin, ParametricPluginAdapter, Plugin,
    PluginDescriptor, PluginFormat, PluginScanStatus, PndPlugin, ProcessContext, ResamplerPlugin,
    RoomModel, SPEECH_DENOISER_FRAME_SIZE, SaturationPlugin, SpectralCompressorPlugin,
    SpectralCompressorPluginParams, SpectrumAnalyzerPlugin, SpectrumConfig, SpeechDenoiserPlugin,
    StereoImagerPlugin, StereoImagerPluginParams, TransientShaperPlugin, UpmixerPlugin, XtcPlugin,
    XtcPluginParams,
};
#[cfg(any(
    feature = "external-plugin-clap",
    feature = "external-plugin-vst3",
    feature = "external-plugin-au"
))]
use std::path::PathBuf;
use std::time::Duration;

fn assert_parametric_in_place_process_zero_alloc<P>(
    name: &str,
    mut plugin: P,
    channels: usize,
    frames: usize,
) where
    P: ParametricInPlacePlugin,
{
    plugin.initialize(SAMPLE_RATE).unwrap();
    let mut buffer = generate_test_buffer(frames, channels);
    let ctx = ProcessContext::new(SAMPLE_RATE, frames);

    for _ in 0..20 {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
    }

    assert_no_allocs(name, || {
        for _ in 0..1000 {
            plugin.process_in_place(&mut buffer, &ctx).unwrap();
        }
    });
}

fn assert_plugin_process_zero_alloc(name: &str, plugin: &mut dyn Plugin, frames: usize) {
    plugin.initialize(SAMPLE_RATE).unwrap();
    let input = generate_test_buffer(frames, plugin.input_channels());
    let mut output = vec![0.0f32; frames * plugin.output_channels()];
    let ctx = ProcessContext::new(SAMPLE_RATE, frames);

    for _ in 0..20 {
        plugin.process(&input, &mut output, &ctx).unwrap();
    }

    assert_no_allocs(name, || {
        for _ in 0..1000 {
            plugin.process(&input, &mut output, &ctx).unwrap();
        }
    });
}

#[test]
#[serial]
fn test_eq_zero_alloc() {
    let mut plugin = ParametricPluginAdapter::new(EqPlugin::new(2, vec![]));
    plugin.initialize(SAMPLE_RATE).unwrap();

    let input = generate_test_buffer(BUFFER_SIZE, 2);
    let mut output = vec![0.0f32; BUFFER_SIZE * 2];
    let ctx = ProcessContext::new(SAMPLE_RATE, BUFFER_SIZE);

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
    let mut plugin = ParametricPluginAdapter::new(GainPlugin::new(2, -3.0));
    plugin.initialize(SAMPLE_RATE).unwrap();

    let input = generate_test_buffer(BUFFER_SIZE, 2);
    let mut output = vec![0.0f32; input.len()];
    let ctx = ProcessContext::new(SAMPLE_RATE, BUFFER_SIZE);

    // Warm-up
    for _ in 0..20 {
        plugin.process(&input, &mut output, &ctx).unwrap();
        let _ = plugin.get_data();
    }

    assert_no_allocs("GainPlugin", || {
        for _ in 0..1000 {
            plugin.process(&input, &mut output, &ctx).unwrap();
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
    let ctx = ProcessContext::new(SAMPLE_RATE, BUFFER_SIZE);

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
    let ctx = ProcessContext::new(SAMPLE_RATE, BUFFER_SIZE);

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
    let ctx = ProcessContext::new(SAMPLE_RATE, BUFFER_SIZE);

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
    let ctx = ProcessContext::new(44100, BUFFER_SIZE);

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
    let ctx = ProcessContext::new(SAMPLE_RATE, BUFFER_SIZE);

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
    let ctx = ProcessContext::new(SAMPLE_RATE, BUFFER_SIZE);

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
    let ctx = ProcessContext::new(SAMPLE_RATE, BUFFER_SIZE);

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
    let ctx = ProcessContext::new(SAMPLE_RATE, BUFFER_SIZE);

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
    let ctx = ProcessContext::new(SAMPLE_RATE, BUFFER_SIZE);

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
    let ctx = ProcessContext::new(SAMPLE_RATE, BUFFER_SIZE);

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
fn test_declick_zero_alloc() {
    let mut plugin = DeclickPlugin::new(2);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let mut buffer = generate_test_buffer(BUFFER_SIZE, 2);
    let ctx = ProcessContext::new(SAMPLE_RATE, BUFFER_SIZE);

    assert_no_allocs("DeclickPlugin", || {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
    });
}

#[test]
#[serial]
fn test_pnd_zero_alloc() {
    let mut plugin = PndPlugin::new(2);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let input = generate_test_buffer(BUFFER_SIZE, 2);
    let mut output = vec![0.0f32; BUFFER_SIZE * 2];
    let ctx = ProcessContext::new(SAMPLE_RATE, BUFFER_SIZE);

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
    let ctx = ProcessContext::new(SAMPLE_RATE, BUFFER_SIZE);

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
    let ctx = ProcessContext::new(SAMPLE_RATE, BUFFER_SIZE);

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
    let ctx = ProcessContext::new(SAMPLE_RATE, BUFFER_SIZE);

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
    let ctx = ProcessContext::new(SAMPLE_RATE, BUFFER_SIZE);

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
    let ctx = ProcessContext::new(SAMPLE_RATE, BUFFER_SIZE);

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
    let ctx = ProcessContext::new(SAMPLE_RATE, BUFFER_SIZE);

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
    let ctx = ProcessContext::new(SAMPLE_RATE, BUFFER_SIZE);

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
    plugin
        .set_parameter(
            sotf_plugins::ParameterId::from("mode"),
            sotf_plugins::ParameterValue::Int(2),
        )
        .unwrap();
    plugin
        .set_parameter(
            sotf_plugins::ParameterId::from("playback_volume_db"),
            sotf_plugins::ParameterValue::Float(-20.0),
        )
        .unwrap();
    ParametricInPlacePlugin::initialize(&mut plugin, SAMPLE_RATE).unwrap();

    let mut buffer = generate_test_buffer(BUFFER_SIZE, 2);
    let ctx = ProcessContext::new(SAMPLE_RATE, BUFFER_SIZE);

    // Warm-up
    for _ in 0..20 {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
        let _ = ParametricInPlacePlugin::get_data(&plugin);
    }

    assert_no_allocs("FletcherMunsonPlugin (LoudnessComp Auto)", || {
        for _ in 0..1000 {
            plugin.process_in_place(&mut buffer, &ctx).unwrap();
            let _ = ParametricInPlacePlugin::get_data(&plugin);
        }
    });
}

#[test]
#[serial]
fn test_loudness_compensation_zero_alloc() {
    let mut plugin = LoudnessCompensationPlugin::new(2, 200.0, 3.0, 6000.0, 2.0);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let mut buffer = generate_test_buffer(BUFFER_SIZE, 2);
    let ctx = ProcessContext::new(SAMPLE_RATE, BUFFER_SIZE);

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
    let ctx = ProcessContext::new(SAMPLE_RATE, BUFFER_SIZE);

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
    let ctx = ProcessContext::new(SAMPLE_RATE, BUFFER_SIZE);

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
    let ctx = ProcessContext::new(SAMPLE_RATE, BUFFER_SIZE);

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
    let ctx = ProcessContext::new(SAMPLE_RATE, BUFFER_SIZE);

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
    let ctx = ProcessContext::new(SAMPLE_RATE, BUFFER_SIZE);

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
    let ctx = ProcessContext::new(SAMPLE_RATE, BUFFER_SIZE);

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

#[test]
#[serial]
fn test_delay_zero_alloc() {
    assert_parametric_in_place_process_zero_alloc(
        "DelayPlugin::process_in_place",
        DelayPlugin::new(2, 100.0, 0.3, 0.5),
        2,
        BUFFER_SIZE,
    );
}

#[test]
#[serial]
fn test_aae_zero_alloc() {
    let mut plugin = AaePlugin::from_params(AaePluginParams::default());
    assert_plugin_process_zero_alloc("AaePlugin::process", &mut plugin, BUFFER_SIZE);
}

#[test]
#[serial]
fn test_de_esser_zero_alloc() {
    assert_parametric_in_place_process_zero_alloc(
        "DeEsserPlugin::process_in_place",
        DeEsserPlugin::new(2),
        2,
        BUFFER_SIZE,
    );
}

#[test]
#[serial]
fn test_dynamic_eq_zero_alloc() {
    assert_parametric_in_place_process_zero_alloc(
        "DynamicEqPlugin::process_in_place",
        DynamicEqPlugin::new(2),
        2,
        BUFFER_SIZE,
    );
}

#[test]
#[serial]
fn test_fir_designer_zero_alloc() {
    assert_parametric_in_place_process_zero_alloc(
        "FirDesignerPlugin::process_in_place",
        FirDesignerPlugin::new(2, SAMPLE_RATE),
        2,
        1024,
    );
}

#[test]
#[serial]
fn test_linear_phase_eq_zero_alloc() {
    assert_parametric_in_place_process_zero_alloc(
        "LinearPhaseEqPlugin::process_in_place",
        LinearPhaseEqPlugin::new(2, SAMPLE_RATE),
        2,
        1024,
    );
}

#[test]
#[serial]
fn test_spectral_compressor_zero_alloc() {
    assert_parametric_in_place_process_zero_alloc(
        "SpectralCompressorPlugin::process_in_place",
        SpectralCompressorPlugin::from_params(2, SpectralCompressorPluginParams::default()),
        2,
        4096,
    );
}

#[test]
#[serial]
fn test_stereo_imager_zero_alloc() {
    assert_parametric_in_place_process_zero_alloc(
        "StereoImagerPlugin::process_in_place",
        StereoImagerPlugin::new(2, StereoImagerPluginParams::default()),
        2,
        BUFFER_SIZE,
    );
}

#[test]
#[serial]
fn test_transient_shaper_zero_alloc() {
    assert_parametric_in_place_process_zero_alloc(
        "TransientShaperPlugin::process_in_place",
        TransientShaperPlugin::new(2),
        2,
        BUFFER_SIZE,
    );
}

#[test]
#[serial]
fn test_saturation_zero_alloc() {
    assert_parametric_in_place_process_zero_alloc(
        "SaturationPlugin::process_in_place",
        SaturationPlugin::new(2),
        2,
        BUFFER_SIZE,
    );
}

#[test]
#[serial]
fn test_speech_denoiser_zero_alloc() {
    assert_parametric_in_place_process_zero_alloc(
        "SpeechDenoiserPlugin::process_in_place",
        SpeechDenoiserPlugin::new(2),
        2,
        SPEECH_DENOISER_FRAME_SIZE,
    );
}

#[test]
#[serial]
fn test_hiss_reducer_zero_alloc() {
    assert_parametric_in_place_process_zero_alloc(
        "HissReducerPlugin::process_in_place",
        HissReducerPlugin::new(2),
        2,
        BUFFER_SIZE,
    );
}

#[test]
#[serial]
fn test_aec_zero_alloc() {
    let mut plugin = AecPlugin::from_params(SAMPLE_RATE, AecPluginParams::default());
    assert_plugin_process_zero_alloc("AecPlugin::process", &mut plugin, BUFFER_SIZE);
}

#[test]
#[serial]
fn test_beamformer_zero_alloc() {
    for mode in [0, 2] {
        let mut plugin = BeamformerPlugin::new(2, SAMPLE_RATE);
        plugin
            .set_parameter(
                sotf_plugins::ParameterId::from("beamformer_type"),
                sotf_plugins::ParameterValue::Int(mode),
            )
            .unwrap();
        let name = if mode == 0 {
            "BeamformerPlugin::MVDR::process"
        } else {
            "BeamformerPlugin::GSC::process"
        };
        assert_plugin_process_zero_alloc(name, &mut plugin, BUFFER_SIZE);
    }
}

#[test]
#[serial]
fn test_ambisonics_decoder_zero_alloc() {
    for dual_band in [false, true] {
        let config = AmbisonicsDecoderConfig {
            order: 1,
            target_layout: "5.1".to_owned(),
            max_re_weighting: true,
            dual_band,
        };
        let mut plugin = AmbisonicsDecoderPlugin::new(&config).unwrap();
        let name = if dual_band {
            "AmbisonicsDecoderPlugin::dual_band::process"
        } else {
            "AmbisonicsDecoderPlugin::single_band::process"
        };
        assert_plugin_process_zero_alloc(name, &mut plugin, 1024);
    }
}

#[test]
#[serial]
fn test_isolated_external_host_timeout_and_quarantine_zero_alloc() {
    let descriptor = PluginDescriptor {
        id: "test.external".into(),
        name: "External Test".into(),
        vendor: "SOTF".into(),
        version: "0.1".into(),
        format: PluginFormat::Clap,
        path: "/tmp/fake.clap".into(),
        audio_inputs: 2,
        audio_outputs: 2,
        is_instrument: false,
        categories: Vec::new(),
        scan_status: PluginScanStatus::Discovered,
    };
    let mut plugin = IsolatedExternalPlugin::new(
        descriptor,
        SAMPLE_RATE,
        IsolatedExternalPluginConfig {
            deadline: Duration::ZERO,
            start_worker: false,
            max_consecutive_block_failures: 2,
            ..Default::default()
        },
    )
    .unwrap();
    let input = generate_test_buffer(BUFFER_SIZE, 2);
    let mut output = vec![0.0f32; input.len()];
    let ctx = ProcessContext::new(SAMPLE_RATE, BUFFER_SIZE);

    // First miss increments the failure count. The measured second miss crosses
    // the quarantine threshold, and subsequent blocks exercise steady fallback.
    plugin.process(&input, &mut output, &ctx).unwrap();
    assert_no_allocs(
        "IsolatedExternalPlugin::timeout/quarantine/fallback",
        || {
            plugin.process(&input, &mut output, &ctx).unwrap();
            plugin.process(&input, &mut output, &ctx).unwrap();
        },
    );
    assert_eq!(output, input);
}

#[test]
#[serial]
fn test_external_plugin_worker_successful_round_trip_zero_alloc() {
    let layout = PluginIpcLayout::new(SAMPLE_RATE, BUFFER_SIZE as u32, 2, 2).unwrap();
    let mut host_shared = SecurePluginSharedMemory::create(layout).unwrap();
    let worker_shared = SecurePluginSharedMemory::open_existing(host_shared.path()).unwrap();
    let plugin = Box::new(ParametricPluginAdapter::new(GainPlugin::new(2, 0.0)));
    let mut worker = ExternalPluginWorker::new(worker_shared, plugin).unwrap();
    let input = generate_test_buffer(BUFFER_SIZE, 2);
    let mut output = vec![0.0; input.len()];

    host_shared
        .publish_host_block(1, BUFFER_SIZE, &input)
        .unwrap();
    assert!(matches!(
        worker.process_one().unwrap(),
        ExternalPluginWorkerStep::Processed {
            frames: BUFFER_SIZE,
            ..
        }
    ));
    host_shared.copy_worker_output(&mut output).unwrap();

    let mut sequence = 2;
    assert_no_allocs("ExternalPluginWorker::successful_round_trip", || {
        for _ in 0..1000 {
            host_shared
                .publish_host_block(sequence, BUFFER_SIZE, &input)
                .unwrap();
            worker.process_one().unwrap();
            host_shared.copy_worker_output(&mut output).unwrap();
            sequence += 1;
        }
    });
    assert_eq!(output, input);
}

#[cfg(feature = "external-plugin-clap")]
#[test]
#[serial]
#[ignore = "requires SOTF_TEST_CLAP_PLUGIN to point to the built plugins-nih gain CLAP library"]
fn test_native_clap_successful_process_zero_alloc() {
    let path = PathBuf::from(
        std::env::var_os("SOTF_TEST_CLAP_PLUGIN")
            .expect("SOTF_TEST_CLAP_PLUGIN must point to a .clap file or bundle"),
    );
    let descriptor = native_gain_descriptor(PluginFormat::Clap, path);
    let mut plugin = ExternalPlugin::new(&descriptor, SAMPLE_RATE).expect("load native CLAP gain");
    assert_plugin_process_zero_alloc(
        "ExternalPlugin::CLAP::successful_process",
        &mut plugin,
        BUFFER_SIZE,
    );
}

#[cfg(feature = "external-plugin-vst3")]
#[test]
#[serial]
#[ignore = "requires SOTF_TEST_VST3_PLUGIN to point to the built plugins-nih gain VST3 library"]
fn test_native_vst3_successful_process_zero_alloc() {
    let path = PathBuf::from(
        std::env::var_os("SOTF_TEST_VST3_PLUGIN")
            .expect("SOTF_TEST_VST3_PLUGIN must point to a .vst3 file or bundle"),
    );
    let descriptor = native_gain_descriptor(PluginFormat::Vst3, path);
    let mut plugin = ExternalPlugin::new(&descriptor, SAMPLE_RATE).expect("load native VST3 gain");
    assert_plugin_process_zero_alloc(
        "ExternalPlugin::VST3::successful_process",
        &mut plugin,
        BUFFER_SIZE,
    );
}

#[cfg(all(feature = "external-plugin-au", target_os = "macos"))]
#[test]
#[serial]
#[ignore = "requires the built-in Apple AUDelay Audio Unit and CoreAudio component registrar"]
fn test_native_audio_unit_successful_process_zero_alloc() {
    let descriptor = PluginDescriptor {
        id: "au.AUDelay".into(),
        name: "AUDelay".into(),
        vendor: "Apple".into(),
        version: "system".into(),
        format: PluginFormat::AudioUnit,
        path: PathBuf::from("/System/Library/Components/CoreAudio.component"),
        audio_inputs: 2,
        audio_outputs: 2,
        is_instrument: false,
        categories: vec!["audio-effect".into()],
        scan_status: PluginScanStatus::Loadable,
    };
    let mut plugin =
        ExternalPlugin::new(&descriptor, SAMPLE_RATE).expect("load built-in Apple AUDelay");
    assert_plugin_process_zero_alloc(
        "ExternalPlugin::AudioUnit::successful_process",
        &mut plugin,
        BUFFER_SIZE,
    );
}

#[cfg(any(feature = "external-plugin-clap", feature = "external-plugin-vst3"))]
fn native_gain_descriptor(format: PluginFormat, path: PathBuf) -> PluginDescriptor {
    PluginDescriptor {
        id: match format {
            PluginFormat::Clap => "org.spinorama.sotf.gain".into(),
            PluginFormat::Vst3 => "vst3.SOTF: Gain".into(),
            PluginFormat::AudioUnit => unreachable!("native allocation fixture is CLAP/VST3"),
        },
        name: "SOTF: Gain".into(),
        vendor: "SOTF".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        format,
        path,
        audio_inputs: 2,
        audio_outputs: 2,
        is_instrument: false,
        categories: vec!["audio-effect".into()],
        scan_status: PluginScanStatus::Loadable,
    }
}
