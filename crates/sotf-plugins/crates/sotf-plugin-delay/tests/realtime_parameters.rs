use sotf_host::{
    CountingAlloc, ParameterId, ParameterValue, ParametricInPlacePlugin, assert_no_allocs,
};
use sotf_plugin_delay::DelayPlugin;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

#[test]
fn realtime_parameter_writes_do_not_allocate() {
    let mut plugin = DelayPlugin::new(2, 100.0, 0.3, 0.5);
    let feedback = ParameterId::from("feedback");
    let mix = ParameterId::from("mix");
    let lfo_rate = ParameterId::from("lfo_rate_hz");
    let allpass = ParameterId::from("allpass_feedback");

    assert_no_allocs("Delay realtime parameter writes", || {
        plugin
            .set_parameter(feedback.clone(), ParameterValue::Float(0.4))
            .unwrap();
        plugin
            .set_parameter(mix.clone(), ParameterValue::Float(0.6))
            .unwrap();
        plugin
            .set_parameter(lfo_rate.clone(), ParameterValue::Float(2.0))
            .unwrap();
        plugin
            .set_parameter(allpass.clone(), ParameterValue::Bool(true))
            .unwrap();
    });
}
