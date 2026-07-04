use sotf_host::{CountingAlloc, ParametricInPlacePluginAdapter, run_standard_tests};
use sotf_plugin_declick::DeclickPlugin;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let plugin = DeclickPlugin::new(2);
    let mut plugin = ParametricInPlacePluginAdapter::new(plugin);
    run_standard_tests(&mut plugin, "DeclickPlugin");
}
