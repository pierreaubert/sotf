use sotf_host::{CountingAlloc, ParameterId, ParameterValue, assert_no_allocs};
use sotf_plugin_multiband_expander::MultibandExpanderPlugin;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

#[test]
fn realtime_parameter_updates_do_not_allocate() {
    let mut plugin = MultibandExpanderPlugin::new(2);
    let threshold = ParameterId::from("threshold");
    let mix = ParameterId::from("mix");
    let hpf = ParameterId::from("sidechain_hpf_hz");
    let band_threshold = ParameterId::from("band_0_threshold");
    assert_no_allocs("Expander threshold write", || {
        plugin
            .set_parameter(threshold.clone(), ParameterValue::Float(-35.0))
            .unwrap();
    });
    assert_no_allocs("Expander mix write", || {
        plugin
            .set_parameter(mix.clone(), ParameterValue::Float(0.75))
            .unwrap();
    });
    assert_no_allocs("Expander HPF write", || {
        plugin
            .set_parameter(hpf.clone(), ParameterValue::Float(120.0))
            .unwrap();
    });
    assert_no_allocs("Expander band threshold write", || {
        plugin
            .set_parameter(band_threshold.clone(), ParameterValue::Float(-30.0))
            .unwrap();
    });
}
