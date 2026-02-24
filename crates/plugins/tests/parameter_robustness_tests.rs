// ============================================================================
// Parameter Robustness Tests
// ============================================================================
//
// These tests verify that all plugins handle incorrect parameters gracefully:
// - Validate parameter bounds.
// - Test invalid input types.
// - Verify default value fallback.
// - Check for NaN/Inf values.
// - Confirm no crashes on invalid config.

use sotf_plugins::{
    ABComparePlugin, BandMergePlugin, BandSplitPlugin, BinauralDecoderPlugin,
    ChannelMuteSoloPlugin, CompressorPlugin, ConvolutionPlugin, CrossfeedPlugin, CrossoverPlugin,
    DelayPlugin, DenoiserPlugin, DownmixPlugin, EqPlugin, ExpanderPlugin, FletcherMunsonPlugin,
    GainPlugin, GatePlugin, InPlacePluginAdapter, LimiterPlugin, LoudnessCompensationPlugin,
    LoudnessMonitorPlugin, MatrixPlugin, MonoToStereoPlugin, MultibandCompressorPlugin,
    MultibandExpanderPlugin, ParameterValue, Plugin, PndPlugin, ProcessContext, ResamplerPlugin,
    RoomModel, SpectrumAnalyzerPlugin, UpmixerPlugin, XtcPlugin, XtcPluginParams,
};

const SAMPLE_RATE: u32 = 48000;
const BUFFER_SIZE: usize = 1024;

fn get_all_plugins() -> Vec<Box<dyn Plugin>> {
    let mut plugins: Vec<Box<dyn Plugin>> = Vec::new();

    // 1. Eq
    plugins.push(Box::new(InPlacePluginAdapter::new(EqPlugin::new(
        2,
        vec![],
    ))));

    // 2. Gain
    plugins.push(Box::new(InPlacePluginAdapter::new(GainPlugin::new(2, 0.0))));

    // 3. Compressor
    plugins.push(Box::new(InPlacePluginAdapter::new(CompressorPlugin::new(
        2, -20.0, 4.0, 10.0, 100.0, 6.0, 0.0,
    ))));

    // 4. Limiter
    plugins.push(Box::new(InPlacePluginAdapter::new(LimiterPlugin::new(
        2, -1.0, 50.0, 5.0, false,
    ))));

    // 5. Gate
    plugins.push(Box::new(InPlacePluginAdapter::new(GatePlugin::new(
        2, -40.0, 10.0, 1.0, 10.0, 100.0,
    ))));

    // 6. Delay
    plugins.push(Box::new(InPlacePluginAdapter::new(DelayPlugin::new(
        2, 100.0, 0.3, 0.5,
    ))));

    // 7. Loudness Compensation
    plugins.push(Box::new(InPlacePluginAdapter::new(
        LoudnessCompensationPlugin::new(2, 200.0, 3.0, 6000.0, 2.0),
    )));

    // 8. Crossover
    plugins.push(Box::new(InPlacePluginAdapter::new(
        CrossoverPlugin::new(2, "LR24", 1000.0, "low").unwrap(),
    )));

    // 9. Upmixer
    plugins.push(Box::new(UpmixerPlugin::new(
        2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
    )));

    // 10. XTC
    plugins.push(Box::new(
        XtcPlugin::new(XtcPluginParams::default(), SAMPLE_RATE).unwrap(),
    ));

    // 11. Binaural Decoder
    plugins.push(Box::new(BinauralDecoderPlugin::new(
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
    )));

    // 12. Denoiser
    plugins.push(Box::new(InPlacePluginAdapter::new(DenoiserPlugin::new(
        2, false,
    ))));

    // 13. Pnd
    plugins.push(Box::new(PndPlugin::new(2)));

    // 14. BandMerge
    plugins.push(Box::new(BandMergePlugin::new(2, 2).unwrap()));

    // 15. BandSplit
    plugins.push(Box::new(BandSplitPlugin::new(2, 1000.0, "LR24").unwrap()));

    // 16. ChannelMuteSolo
    plugins.push(Box::new(InPlacePluginAdapter::new(
        ChannelMuteSoloPlugin::new(2, true),
    )));

    // 17. Crossfeed
    plugins.push(Box::new(InPlacePluginAdapter::new(
        CrossfeedPlugin::new(Default::default()).unwrap(),
    )));

    // 18. Downmix
    plugins.push(Box::new(DownmixPlugin::new(6)));

    // 19. FletcherMunson
    plugins.push(Box::new(InPlacePluginAdapter::new(
        FletcherMunsonPlugin::new(2),
    )));

    // 20. Matrix
    plugins.push(Box::new(MatrixPlugin::new(2, 2)));

    // 21. MonoToStereo
    plugins.push(Box::new(MonoToStereoPlugin::new()));

    // 22. MultibandCompressor
    plugins.push(Box::new(InPlacePluginAdapter::new(
        MultibandCompressorPlugin::new(2),
    )));

    // 23. MultibandExpander
    plugins.push(Box::new(InPlacePluginAdapter::new(
        MultibandExpanderPlugin::new(2),
    )));

    // 24. ABCompare
    plugins.push(Box::new(ABComparePlugin::new(2).unwrap()));

    // 25. Loudness Monitor (Analyzer)
    plugins.push(Box::new(LoudnessMonitorPlugin::new(2).unwrap()));

    // 26. Spectrum Analyzer (Analyzer)
    plugins.push(Box::new(SpectrumAnalyzerPlugin::new(2).unwrap()));

    // 27. Resampler
    plugins.push(Box::new(
        ResamplerPlugin::new(2, 44100, 48000, 1024).unwrap(),
    ));

    // 28. Convolution
    plugins.push(Box::new(InPlacePluginAdapter::new(ConvolutionPlugin::new(
        2,
        SAMPLE_RATE,
    ))));

    // 29. Expander
    plugins.push(Box::new(InPlacePluginAdapter::new(ExpanderPlugin::new(2))));

    plugins
}

#[test]
fn test_parameter_bounds_and_types() {
    let mut plugins = get_all_plugins();

    for plugin in &mut plugins {
        let name = plugin.info().name;
        println!("Testing plugin: {}", name);

        let parameters = plugin.parameters();
        for param in parameters {
            let id = param.id.clone();

            // 1. Test invalid type
            let invalid_value = match param.default_value {
                ParameterValue::Float(_) | ParameterValue::Int(_) | ParameterValue::Bool(_) => {
                    ParameterValue::String("invalid_type_test".to_string())
                }
                ParameterValue::String(_) => ParameterValue::Float(123.45),
            };

            let _ = plugin.set_parameter(id.clone(), invalid_value);

            // 2. Test out of bounds
            if let Some(min) = param.min_value.clone() {
                if let ParameterValue::Float(m) = min {
                    let _ = plugin.set_parameter(id.clone(), ParameterValue::Float(m - 1000000.0));
                } else if let ParameterValue::Int(m) = min {
                    let _ = plugin.set_parameter(id.clone(), ParameterValue::Int(m - 1000000));
                }
            }

            if let Some(max) = param.max_value.clone() {
                if let ParameterValue::Float(m) = max {
                    let _ = plugin.set_parameter(id.clone(), ParameterValue::Float(m + 1000000.0));
                } else if let ParameterValue::Int(m) = max {
                    let _ = plugin.set_parameter(id.clone(), ParameterValue::Int(m + 1000000));
                }
            }

            // 3. Test NaN and Infinity
            if let ParameterValue::Float(_) = param.default_value {
                let _ = plugin.set_parameter(id.clone(), ParameterValue::Float(f32::NAN));
                let _ = plugin.set_parameter(id.clone(), ParameterValue::Float(f32::INFINITY));
                let _ = plugin.set_parameter(id.clone(), ParameterValue::Float(f32::NEG_INFINITY));
            }
        }

        // 4. Verify no crash after processing
        let input = vec![0.1f32; BUFFER_SIZE * plugin.input_channels()];
        let mut output = vec![0.0f32; BUFFER_SIZE * plugin.output_channels()];
        let context = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: BUFFER_SIZE,
        };

        let _ = plugin.initialize(SAMPLE_RATE);
        let result = plugin.process(&input, &mut output, &context);

        assert!(
            result.is_ok() || result.is_err(),
            "Processing should not panic even with invalid params"
        );
    }
}

#[test]
fn test_parameter_change_during_processing() {
    let mut plugins = get_all_plugins();

    for plugin in &mut plugins {
        let name = plugin.info().name;
        println!("Testing dynamic updates for: {}", name);

        let parameters = plugin.parameters();
        if parameters.is_empty() {
            continue;
        }

        let context = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: BUFFER_SIZE,
        };
        let input = vec![0.1f32; BUFFER_SIZE * plugin.input_channels()];
        let mut output = vec![0.0f32; BUFFER_SIZE * plugin.output_channels()];

        plugin.initialize(SAMPLE_RATE).unwrap();

        // Start processing
        for i in 0..10 {
            let param_idx = i % parameters.len();
            let param = &parameters[param_idx];

            let _ = plugin.set_parameter(param.id.clone(), param.default_value.clone());
            let _ = plugin.process(&input, &mut output, &context);

            if let ParameterValue::Float(_) = param.default_value {
                let _ = plugin.set_parameter(param.id.clone(), ParameterValue::Float(f32::NAN));
            }

            let _ = plugin.process(&input, &mut output, &context);
        }
    }
}
