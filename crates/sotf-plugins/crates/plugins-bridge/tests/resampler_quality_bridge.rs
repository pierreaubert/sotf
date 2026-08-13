use plugins_bridge::ParamBridge;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::Plugin;
use sotf_plugin_resampler::{ResamplerPlugin, params};

#[test]
fn all_resampler_quality_choices_roundtrip_canonical_bridge_indices() {
    let bridge = ParamBridge::new(params::PARAMS);
    let quality = bridge.find_index("quality").unwrap();
    let mut plugin = ResamplerPlugin::new(1, 44_100, 48_000, 64).unwrap();

    for (raw, normalized) in [(0, 0.0), (1, 0.5), (2, 1.0)] {
        bridge
            .set_normalized(&mut plugin, quality, normalized)
            .unwrap();
        assert_eq!(
            plugin.get_parameter(&ParameterId::from("quality")),
            Some(ParameterValue::Int(raw))
        );
        assert_eq!(bridge.get_normalized(&plugin, quality), Some(normalized));
    }
}
