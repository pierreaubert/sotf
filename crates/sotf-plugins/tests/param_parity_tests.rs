// ============================================================================
// Parameter Parity Tests
// ============================================================================
//
// Enforces that every key in a plugin's PARAMS spec is:
//   1. Present in `parameters()` (cached_parameters)
//   2. Accepted by `set_parameter()` without error
//   3. Returned by `get_parameter()` as Some(...)
//
// This test catches the class of bug where a parameter is added to PARAMS
// but forgotten in the DSP plugin's cached_parameters / set_parameter /
// get_parameter.

use sotf_plugins::param_specs::{self, ParamSpec, ParamType};
use sotf_plugins::{ParametricInPlacePluginAdapter, ParametricPluginAdapter, 
    ABComparePlugin, BandMergePlugin, BandSplitPlugin, BinauralDecoderPlugin,
    ChannelMuteSoloPlugin, CompressorPlugin, ConvolutionPlugin, CrossfeedPlugin, DelayPlugin,
    DenoiserPlugin, DownmixPlugin, ExpanderPlugin, FirDesignerPlugin, GainPlugin, GatePlugin,
     LimiterPlugin, LoudnessCompensationPlugin, MatrixPlugin,
    MonoToStereoPlugin, MultibandCompressorPlugin, MultibandExpanderPlugin, ParameterId,
    ParameterValue, Plugin, PndPlugin, RoomModel, UpmixerPlugin,
    XtcPlugin, XtcPluginParams};

const SAMPLE_RATE: u32 = 48000;

/// A plugin instance paired with its PARAMS spec for parity checking.
struct PluginWithSpec {
    name: &'static str,
    plugin: Box<dyn Plugin>,
    params: &'static [ParamSpec],
}

fn all_plugins_with_specs() -> Vec<PluginWithSpec> {
    vec![
        PluginWithSpec {
            name: "gain",
            plugin: Box::new(ParametricPluginAdapter::new(GainPlugin::new(2, 0.0))),
            params: param_specs::gain::PARAMS,
        },
        PluginWithSpec {
            name: "compressor",
            plugin: Box::new(ParametricInPlacePluginAdapter::new(CompressorPlugin::new(2))),
            params: param_specs::compressor::GLOBAL_PARAMS,
        },
        PluginWithSpec {
            name: "limiter",
            plugin: Box::new(ParametricInPlacePluginAdapter::new(LimiterPlugin::new(
                2, -1.0, 50.0, 5.0, false,
            ))),
            params: param_specs::limiter::PARAMS,
        },
        PluginWithSpec {
            name: "gate",
            plugin: Box::new(ParametricInPlacePluginAdapter::new(GatePlugin::new(
                2, -40.0, 10.0, 1.0, 10.0, 100.0,
            ))),
            params: param_specs::gate::PARAMS,
        },
        PluginWithSpec {
            name: "expander",
            plugin: Box::new(ParametricInPlacePluginAdapter::new(ExpanderPlugin::new(2))),
            params: param_specs::expander::GLOBAL_PARAMS,
        },
        PluginWithSpec {
            name: "delay",
            plugin: Box::new(ParametricInPlacePluginAdapter::new(DelayPlugin::new(
                2, 100.0, 0.3, 0.5,
            ))),
            params: param_specs::delay::PARAMS,
        },
        PluginWithSpec {
            name: "loudness_compensation",
            plugin: Box::new(ParametricInPlacePluginAdapter::new(
                LoudnessCompensationPlugin::new(2, 200.0, 3.0, 6000.0, 2.0),
            )),
            params: param_specs::loudness_compensation::PARAMS,
        },
        PluginWithSpec {
            name: "upmixer",
            plugin: Box::new(UpmixerPlugin::new(
                2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
            )),
            params: param_specs::upmixer::PARAMS,
        },
        PluginWithSpec {
            name: "xtc",
            plugin: Box::new(XtcPlugin::new(XtcPluginParams::default(), SAMPLE_RATE).unwrap()),
            params: param_specs::xtc::PARAMS,
        },
        PluginWithSpec {
            name: "binaural",
            plugin: Box::new(BinauralDecoderPlugin::new(
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
            )),
            params: param_specs::binaural::PARAMS,
        },
        PluginWithSpec {
            name: "denoiser",
            plugin: Box::new(ParametricInPlacePluginAdapter::new(DenoiserPlugin::new(2, false))),
            params: param_specs::denoiser::PARAMS,
        },
        PluginWithSpec {
            name: "pnd",
            plugin: Box::new(PndPlugin::new(2)),
            params: param_specs::pnd::PARAMS,
        },
        PluginWithSpec {
            name: "ab_compare",
            plugin: Box::new(ABComparePlugin::new(2).unwrap()),
            params: param_specs::ab_compare::PARAMS,
        },
        PluginWithSpec {
            name: "band_split",
            plugin: Box::new(BandSplitPlugin::new(2, 1000.0, "LR24").unwrap()),
            params: param_specs::band_split::PARAMS,
        },
        PluginWithSpec {
            name: "band_merge",
            plugin: Box::new(BandMergePlugin::new(2, 2).unwrap()),
            params: param_specs::band_merge::PARAMS,
        },
        PluginWithSpec {
            name: "channel_mute_solo",
            plugin: Box::new(ParametricInPlacePluginAdapter::new(
                ChannelMuteSoloPlugin::new(2, true),
            )),
            params: param_specs::channel_mute_solo::PARAMS,
        },
        PluginWithSpec {
            name: "crossfeed",
            plugin: Box::new(ParametricInPlacePluginAdapter::new(
                CrossfeedPlugin::new(Default::default()).unwrap(),
            )),
            params: param_specs::crossfeed::PARAMS,
        },
        PluginWithSpec {
            name: "downmix",
            plugin: Box::new(DownmixPlugin::new(6)),
            params: param_specs::downmix::PARAMS,
        },
        PluginWithSpec {
            name: "matrix",
            plugin: Box::new(MatrixPlugin::new(2, 2)),
            params: param_specs::matrix::PARAMS,
        },
        PluginWithSpec {
            name: "mono_to_stereo",
            plugin: Box::new(MonoToStereoPlugin::new()),
            params: param_specs::mono_to_stereo::PARAMS,
        },
        PluginWithSpec {
            name: "multiband_compressor",
            plugin: Box::new(ParametricInPlacePluginAdapter::new(MultibandCompressorPlugin::new(2))),
            params: param_specs::multiband_compressor::GLOBAL_PARAMS,
        },
        PluginWithSpec {
            name: "multiband_expander",
            plugin: Box::new(ParametricInPlacePluginAdapter::new(MultibandExpanderPlugin::new(2))),
            params: param_specs::multiband_expander::GLOBAL_PARAMS,
        },
        PluginWithSpec {
            name: "convolution",
            plugin: Box::new(ParametricInPlacePluginAdapter::new(ConvolutionPlugin::new(
                2,
                SAMPLE_RATE,
            ))),
            params: param_specs::convolution::PARAMS,
        },
        PluginWithSpec {
            name: "fir_designer",
            plugin: Box::new(ParametricInPlacePluginAdapter::new(FirDesignerPlugin::new(
                2,
                SAMPLE_RATE,
            ))),
            params: param_specs::fir_designer::PARAMS,
        },
    ]
}

/// Create a default `ParameterValue` suitable for testing set_parameter.
fn default_test_value(spec: &ParamSpec) -> ParameterValue {
    match spec.param_type {
        ParamType::Float { default, .. } => ParameterValue::Float(default as f32),
        ParamType::Int { default, .. } => ParameterValue::Int(default as i32),
        ParamType::Bool { default, .. } => ParameterValue::Bool(default),
        ParamType::Choice { default_index, .. } => ParameterValue::Int(default_index as i32),
        ParamType::FilePath => ParameterValue::String(String::new()),
    }
}

/// Verify that every key in PARAMS is present in parameters(), accepted by
/// set_parameter(), and returned by get_parameter().
#[test]
fn all_params_spec_keys_are_registered_in_dsp_plugin() {
    let mut all_errors: Vec<String> = Vec::new();

    for mut pw in all_plugins_with_specs() {
        // Initialize with a sample rate so internal state is valid
        let _ = pw.plugin.initialize(SAMPLE_RATE);

        let cached = pw.plugin.parameters();
        let cached_keys: Vec<&str> = cached.iter().map(|p| p.id.as_str()).collect();

        for spec in pw.params {
            let key = spec.engine_key;

            // Skip FilePath params — they use structural updates, not set_parameter
            if matches!(spec.param_type, ParamType::FilePath) {
                continue;
            }

            // 1. Check parameters() contains this key
            if !cached_keys.contains(&key) {
                all_errors.push(format!(
                    "[{}] key '{}' in PARAMS but MISSING from parameters() (cached_parameters)",
                    pw.name, key,
                ));
            }

            // 2. Check set_parameter() accepts this key
            let test_val = default_test_value(spec);
            let set_result = pw
                .plugin
                .set_parameter(ParameterId::from(key), test_val.clone());
            if let Err(e) = set_result {
                all_errors.push(format!(
                    "[{}] set_parameter('{}', {:?}) failed: {}",
                    pw.name, key, test_val, e,
                ));
            }

            // 3. Check get_parameter() returns Some for this key
            let get_result = pw.plugin.get_parameter(&ParameterId::from(key));
            if get_result.is_none() {
                all_errors.push(format!(
                    "[{}] get_parameter('{}') returned None",
                    pw.name, key,
                ));
            }
        }
    }

    if !all_errors.is_empty() {
        panic!(
            "\n=== PARAMS / DSP parameter parity failures ({} total) ===\n  {}\n",
            all_errors.len(),
            all_errors.join("\n  "),
        );
    }
}
