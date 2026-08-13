use sotf_host::{CountingAlloc, ParameterId, ParameterValue, assert_no_allocs};
use sotf_plugin_spectral_compressor::{SpectralCompressorPlugin, SpectralCompressorPluginParams};

#[global_allocator]
static ALLOCATOR: CountingAlloc = CountingAlloc;

#[test]
fn every_realtime_parameter_update_is_allocation_free() {
    let mut plugin =
        SpectralCompressorPlugin::from_params(2, SpectralCompressorPluginParams::default());
    let updates = [
        ("threshold", ParameterValue::Float(-30.0)),
        ("ratio", ParameterValue::Float(4.0)),
        ("attack", ParameterValue::Float(10.0)),
        ("release", ParameterValue::Float(100.0)),
        ("knee", ParameterValue::Float(3.0)),
        ("spectral_smoothing", ParameterValue::Float(0.5)),
        ("mix", ParameterValue::Float(0.8)),
        ("target_mode", ParameterValue::Int(1)),
        ("delta_listen", ParameterValue::Bool(true)),
        ("adaptive_threshold", ParameterValue::Bool(true)),
        ("adaptive_offset_db", ParameterValue::Float(3.0)),
        ("channel_link", ParameterValue::Float(1.0)),
    ];
    let updates: Vec<_> = updates
        .into_iter()
        .map(|(name, value)| (ParameterId::from(name), value))
        .collect();

    for (id, value) in updates {
        assert_no_allocs("Spectral Compressor realtime parameter write", || {
            plugin.set_parameter(id.clone(), value.clone()).unwrap();
        });
    }
}
