use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::Plugin;
use sotf_host::{CountingAlloc, assert_no_allocs};
use sotf_plugin_band_split::BandSplitPlugin;

#[global_allocator]
static ALLOCATOR: CountingAlloc = CountingAlloc;

#[test]
fn live_frequency_and_gain_updates_do_not_allocate() {
    let mut plugin = BandSplitPlugin::new_multiband(2, &[500.0, 2_000.0], "LR48").unwrap();
    plugin.initialize(48_000).unwrap();
    let frequency = ParameterId::from("frequency_2");
    let gain = ParameterId::from("band_1_gain_db");

    assert_no_allocs("Band Split live frequency update", || {
        plugin
            .set_parameter(frequency.clone(), ParameterValue::Float(3_000.0))
            .unwrap();
    });
    assert_no_allocs("Band Split live gain update", || {
        plugin
            .set_parameter(gain.clone(), ParameterValue::Float(-6.0))
            .unwrap();
    });
}

#[test]
fn structural_type_change_is_rejected_without_rebuilding_dsp() {
    let mut plugin = BandSplitPlugin::new(2, 1_000.0, "LR24").unwrap();
    plugin.initialize(48_000).unwrap();
    let type_id = ParameterId::from("crossover_type");
    let before = plugin.get_parameter(&type_id);

    let error = plugin
        .set_parameter(type_id.clone(), ParameterValue::Int(1))
        .unwrap_err();
    assert!(error.contains("structural"));
    assert_eq!(plugin.get_parameter(&type_id), before);
}
