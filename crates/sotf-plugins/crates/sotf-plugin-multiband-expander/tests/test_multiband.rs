// Integration tests for Multiband Expander plugin

use sotf_host::InPlacePlugin;
use sotf_plugin_multiband_expander::MultibandExpanderPlugin;

#[test]
fn test_multiband_expander_instantiation() {
    let mut plugin = MultibandExpanderPlugin::new(2);
    plugin.initialize(44100).unwrap();

    assert_eq!(plugin.channels(), 2);
    assert!(plugin.info().name.contains("Expander"));
}
