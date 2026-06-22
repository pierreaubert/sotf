use sotf_host::parametric_in_place_plugin::{
    ParametricInPlacePlugin, ParametricInPlacePluginAdapter,
};
use sotf_host::plugin::{InPlacePlugin, ProcessContext};
use sotf_plugin_saturation::SaturationPlugin;

fn main() {
    let inner = SaturationPlugin::new(2);
    let mut plugin = ParametricInPlacePluginAdapter::new(inner);
    plugin.initialize(48000).unwrap();
    let mut buffer = vec![0.0f32; 1024 * 2];
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(48000, 1024))
        .unwrap();
}
