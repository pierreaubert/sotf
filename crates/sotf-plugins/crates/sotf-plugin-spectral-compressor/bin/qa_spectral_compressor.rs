use sotf_host::parametric_in_place_plugin::{ParametricInPlacePlugin, ParametricInPlacePluginAdapter};
use sotf_host::plugin::{InPlacePlugin, ProcessContext};
use sotf_plugin_spectral_compressor::SpectralCompressorPlugin;

fn main() {
    let inner = SpectralCompressorPlugin::from_params(
        2,
        sotf_plugin_spectral_compressor::SpectralCompressorPluginParams::default(),
    );
    let mut plugin = ParametricInPlacePluginAdapter::new(inner);
    let mut buffer = vec![0.0f32; 4096 * 2];
    plugin.initialize(48000).unwrap();
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(48000, 4096))
        .unwrap();
}
