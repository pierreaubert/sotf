use sotf_host::ParametricInPlacePlugin;
use sotf_host::{CountingAlloc, ParameterId, ParameterValue, assert_no_allocs};
use sotf_plugin_hiss_reducer::HissReducerPlugin;

#[global_allocator]
static ALLOCATOR: CountingAlloc = CountingAlloc;

#[test]
fn realtime_parameter_updates_do_not_allocate() {
    let mut plugin = HissReducerPlugin::new(2);
    plugin.initialize(48_000).unwrap();
    let updates = [
        ("enabled", ParameterValue::Bool(false)),
        ("threshold_db", ParameterValue::Float(-36.0)),
        ("frequency_hz", ParameterValue::Float(6_000.0)),
        ("strength", ParameterValue::Float(0.7)),
    ];
    let updates: Vec<_> = updates
        .into_iter()
        .map(|(name, value)| (ParameterId::from(name), value))
        .collect();

    for (id, value) in updates {
        assert_no_allocs("Hiss Reducer realtime parameter update", || {
            plugin
                .parametric_set_parameter(id.clone(), value.clone())
                .unwrap();
        });
    }
}
