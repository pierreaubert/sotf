use sotf_host::{
    CountingAlloc, ParametricInPlacePlugin, ParametricInPlacePluginAdapter, run_standard_tests,
};
use sotf_host::{ParameterId, ParameterValue};
use sotf_plugin_hiss_reducer::HissReducerPlugin;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let mut plugin = HissReducerPlugin::new(2);
    plugin.initialize(48_000).expect("initialize QA plugin");
    let mut plugin = ParametricInPlacePluginAdapter::new(plugin);
    run_standard_tests(&mut plugin, "HissReducerPlugin");

    let mut spectral = HissReducerPlugin::new(2);
    spectral
        .set_parameter(
            ParameterId::from("spectral_mode"),
            ParameterValue::Bool(true),
        )
        .expect("enable spectral QA mode");
    spectral
        .initialize(48_000)
        .expect("initialize spectral QA plugin");
    let mut spectral = ParametricInPlacePluginAdapter::new(spectral);
    run_standard_tests(&mut spectral, "HissReducerPlugin spectral");
}
