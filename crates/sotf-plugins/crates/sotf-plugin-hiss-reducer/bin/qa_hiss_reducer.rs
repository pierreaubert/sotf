use sotf_host::{ParametricInPlacePluginAdapter, ParametricInPlacePlugin, ParametricPluginAdapter, CountingAlloc, run_standard_tests};
use sotf_plugin_hiss_reducer::HissReducerPlugin;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let plugin = HissReducerPlugin::new(2);
    let mut plugin = ParametricInPlacePluginAdapter::new(plugin);
    run_standard_tests(&mut plugin, "HissReducerPlugin");
}
